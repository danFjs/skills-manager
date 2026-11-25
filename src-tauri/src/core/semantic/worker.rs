//! Background thread that keeps the search index and category vectors in
//! line with the library. Commands `kick` it after anything that may have
//! changed skills or categories; each pass re-indexes only what changed and
//! embeds only text whose hash moved, so a quiet launch costs a few queries
//! and never loads the model.

use super::embed::{to_blob, Embedder};
use super::{categories, index, SemanticDb};
use crate::core::skill_store::SkillStore;
use serde::Serialize;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager};

const BATCH: usize = 32;
const EMIT_EVERY: Duration = Duration::from_millis(1500);
pub const EVENT: &str = "categories-updated";

/// Set by `kick`, cleared by whoever re-syncs the index first. Lets a
/// search see a just-installed skill without waiting for the worker.
static DIRTY: AtomicBool = AtomicBool::new(true);

#[derive(Debug, Clone, Serialize)]
pub struct Progress {
    pub remaining: usize,
    pub unavailable: Option<String>,
}

/// Where the bundled model lives; set once at startup.
static MODEL_DIR: OnceLock<PathBuf> = OnceLock::new();
/// The loaded model, shared by the worker and on-demand categorizing.
/// Loaded on first use only.
static MODEL: Mutex<Option<Result<Embedder, String>>> = Mutex::new(None);

/// Run `f` with the model, loading it the first time. A failed load is
/// remembered so it isn't retried on every call.
pub fn with_model<R>(f: impl FnOnce(&Embedder) -> R) -> Result<R, String> {
    let dir = MODEL_DIR.get().ok_or("model location not set")?;
    let mut guard = MODEL.lock().unwrap_or_else(|e| e.into_inner());
    let model = guard.get_or_insert_with(|| Embedder::load(dir));
    match model {
        Ok(embedder) => Ok(f(embedder)),
        Err(e) => Err(format!("categorizer unavailable: {e}")),
    }
}

pub struct Worker(Sender<()>);

impl Worker {
    pub fn start(app: AppHandle, model_dir: PathBuf) -> Self {
        let _ = MODEL_DIR.set(model_dir);
        let (tx, rx) = mpsc::channel();
        std::thread::Builder::new()
            .name("categorizer".into())
            .spawn(move || run(app, rx))
            .expect("spawn categorizer thread");
        let worker = Self(tx);
        worker.kick();
        worker
    }

    pub fn kick(&self) {
        DIRTY.store(true, Ordering::SeqCst);
        let _ = self.0.send(());
    }
}

pub fn kick(app: &AppHandle) {
    if let Some(worker) = app.try_state::<Worker>() {
        worker.kick();
    }
}

/// Re-sync the index from the library if anything changed since the last
/// sync. Cheap when nothing did.
pub fn ensure_fresh(app: &AppHandle) -> Result<(), String> {
    if !DIRTY.swap(false, Ordering::SeqCst) {
        return Ok(());
    }
    sync_index(app).inspect_err(|_| DIRTY.store(true, Ordering::SeqCst))
}

fn sync_index(app: &AppHandle) -> Result<(), String> {
    let store = app.state::<Arc<SkillStore>>();
    let docs = index::docs_from_store(&store).map_err(|e| e.to_string())?;
    let db = app.state::<SemanticDb>();
    index::sync(&mut db.conn(), &docs).map_err(|e| e.to_string())?;
    Ok(())
}

fn run(app: AppHandle, rx: Receiver<()>) {
    while rx.recv().is_ok() {
        while rx.try_recv().is_ok() {}
        if let Err(e) = ensure_fresh(&app) {
            log::warn!("semantic index sync failed: {e}");
            continue;
        }
        let db = app.state::<SemanticDb>();
        let (stale, todo) = {
            let conn = db.conn();
            match (categories::unembedded(&conn), categories::pending(&conn)) {
                (Ok(s), Ok(t)) => (s, t),
                (Err(e), _) | (_, Err(e)) => {
                    log::warn!("categorizer: {e}");
                    continue;
                }
            }
        };
        if stale.is_empty() && todo.is_empty() {
            continue;
        }
        match with_model(|embedder| pass(&app, embedder, stale, todo)) {
            Ok(Ok(())) => {}
            Ok(Err(e)) => log::warn!("categorizer: {e}"),
            Err(unavailable) => {
                let _ = app.emit(
                    EVENT,
                    Progress {
                        remaining: 0,
                        unavailable: Some(unavailable),
                    },
                );
            }
        }
    }
}

fn pass(
    app: &AppHandle,
    embedder: &Embedder,
    stale: Vec<(String, String)>,
    todo: Vec<(String, String, String)>,
) -> Result<(), String> {
    let db = app.state::<SemanticDb>();
    let e = |e: rusqlite::Error| e.to_string();
    if !stale.is_empty() {
        let texts: Vec<String> = stale.iter().map(|(_, t)| t.clone()).collect();
        let vectors = embedder.embed(&texts)?;
        let conn = db.conn();
        for ((id, _), v) in stale.iter().zip(&vectors) {
            categories::set_vector(&conn, id, &to_blob(v)).map_err(e)?;
        }
        categories::assign_all(&conn).map_err(e)?;
    }
    let cats = categories::categories_with_vectors(&db.conn()).map_err(e)?;
    let total = todo.len();
    let mut last = Instant::now();
    if total > 0 {
        let _ = app.emit(EVENT, Progress { remaining: total, unavailable: None });
    }
    for (i, chunk) in todo.chunks(BATCH).enumerate() {
        let texts: Vec<String> = chunk.iter().map(|(_, t, _)| t.clone()).collect();
        let vectors = embedder.embed(&texts)?;
        {
            let conn = db.conn();
            let tx = conn.unchecked_transaction().map_err(e)?;
            for ((id, _, hash), v) in chunk.iter().zip(&vectors) {
                categories::put_embedding(&tx, id, hash, v, &cats).map_err(e)?;
            }
            tx.commit().map_err(e)?;
        }
        let remaining = total.saturating_sub((i + 1) * BATCH);
        if remaining > 0 && last.elapsed() >= EMIT_EVERY {
            last = Instant::now();
            let _ = app.emit(EVENT, Progress { remaining, unavailable: None });
        }
    }
    let _ = app.emit(EVENT, Progress { remaining: 0, unavailable: None });
    Ok(())
}
