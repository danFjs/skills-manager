//! Collections you add yourself (fork addition): add, edit, refresh, remove,
//! and share the list. See core::user_collections.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use serde::Serialize;
use tauri::State;

use crate::core::error::AppError;
use crate::core::skill_store::SkillStore;
use crate::core::skillssh_api::build_http_client;
use crate::core::user_collections::{self as uc, UserCollection};

/// Serializes read-modify-write of collections.json across commands.
static STORE_LOCK: Mutex<()> = Mutex::new(());

fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

fn client(store: &SkillStore) -> reqwest::blocking::Client {
    build_http_client(store.proxy_url().as_deref(), 30)
}

/// Apply `f` to the stored list and save it.
fn update<R>(f: impl FnOnce(&mut Vec<UserCollection>) -> Result<R, AppError>) -> Result<R, AppError> {
    let _guard = STORE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let path = uc::store_path();
    let mut list = uc::load(&path);
    let out = f(&mut list)?;
    uc::save(&path, &list).map_err(AppError::io)?;
    Ok(out)
}

#[tauri::command]
pub async fn list_user_collections() -> Result<Vec<UserCollection>, AppError> {
    tauri::async_runtime::spawn_blocking(|| Ok(uc::load(&uc::store_path()))).await?
}

/// Add a GitHub repository as a collection and read its skills.
#[tauri::command]
pub async fn add_user_collection(
    repo: String,
    title: Option<String>,
    store: State<'_, Arc<SkillStore>>,
) -> Result<UserCollection, AppError> {
    let store = store.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let repo = uc::parse_repo(&repo).map_err(|e| AppError::invalid_input(e.to_string()))?;
        if uc::load(&uc::store_path()).iter().any(|c| c.repo.eq_ignore_ascii_case(&repo)) {
            return Err(AppError::invalid_input(format!("{repo} is already one of your collections")));
        }
        let (branch, skills) = uc::fetch(&client(&store), &repo).map_err(|e| AppError::network(e.to_string()))?;
        let collection = UserCollection {
            title: title
                .map(|t| t.trim().to_string())
                .filter(|t| !t.is_empty())
                .unwrap_or_else(|| repo.clone()),
            repo,
            branch,
            fetched_at: now_ms(),
            skills,
        };
        update(|list| {
            list.retain(|c| !c.repo.eq_ignore_ascii_case(&collection.repo));
            list.push(collection.clone());
            Ok(())
        })?;
        Ok(collection)
    })
    .await?
}

/// Re-read a collection's skills from GitHub.
#[tauri::command]
pub async fn refresh_user_collection(
    repo: String,
    store: State<'_, Arc<SkillStore>>,
) -> Result<UserCollection, AppError> {
    let store = store.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let (branch, skills) = uc::fetch(&client(&store), &repo).map_err(|e| AppError::network(e.to_string()))?;
        update(|list| {
            let c = list
                .iter_mut()
                .find(|c| c.repo == repo)
                .ok_or_else(|| AppError::not_found("Collection not found"))?;
            c.branch = branch;
            c.skills = skills;
            c.fetched_at = now_ms();
            Ok(c.clone())
        })
    })
    .await?
}

#[tauri::command]
pub async fn rename_user_collection(repo: String, title: String) -> Result<(), AppError> {
    tauri::async_runtime::spawn_blocking(move || {
        let title = title.trim().to_string();
        if title.is_empty() {
            return Err(AppError::invalid_input("Give the collection a name"));
        }
        update(|list| {
            let c = list
                .iter_mut()
                .find(|c| c.repo == repo)
                .ok_or_else(|| AppError::not_found("Collection not found"))?;
            c.title = title;
            Ok(())
        })
    })
    .await?
}

#[tauri::command]
pub async fn remove_user_collection(repo: String) -> Result<(), AppError> {
    tauri::async_runtime::spawn_blocking(move || {
        update(|list| {
            list.retain(|c| c.repo != repo);
            Ok(())
        })
    })
    .await?
}

/// Save your collections (repos and names — not their skills) to a file.
#[tauri::command]
pub async fn export_user_collections(dest_path: String) -> Result<usize, AppError> {
    tauri::async_runtime::spawn_blocking(move || {
        let list = uc::load(&uc::store_path());
        if list.is_empty() {
            return Err(AppError::invalid_input("You haven't added any collections yet"));
        }
        std::fs::write(PathBuf::from(dest_path), uc::to_share(&list))?;
        Ok(list.len())
    })
    .await?
}

#[derive(Debug, Serialize)]
pub struct ImportCollectionsResult {
    pub added: usize,
    pub skipped: usize,
    pub errors: Vec<String>,
}

/// Add every collection in a shared file (or plain repo list) that you
/// don't have yet, fetching each one's skills.
#[tauri::command]
pub async fn import_user_collections(
    path: String,
    store: State<'_, Arc<SkillStore>>,
) -> Result<ImportCollectionsResult, AppError> {
    let store = store.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let raw = std::fs::read_to_string(&path)?;
        if raw.len() > 1024 * 1024 {
            return Err(AppError::invalid_input("That file is too large to be a collections list"));
        }
        let entries = uc::from_share(&raw).map_err(|e| AppError::invalid_input(e.to_string()))?;
        let have = uc::load(&uc::store_path());
        let client = client(&store);
        let mut result = ImportCollectionsResult { added: 0, skipped: 0, errors: vec![] };
        for entry in entries {
            if have.iter().any(|c| c.repo.eq_ignore_ascii_case(&entry.repo)) {
                result.skipped += 1;
                continue;
            }
            match uc::fetch(&client, &entry.repo) {
                Ok((branch, skills)) => {
                    let collection = UserCollection {
                        repo: entry.repo.clone(),
                        title: entry.title.clone(),
                        branch,
                        fetched_at: now_ms(),
                        skills,
                    };
                    update(|list| {
                        list.retain(|c| !c.repo.eq_ignore_ascii_case(&collection.repo));
                        list.push(collection);
                        Ok(())
                    })?;
                    result.added += 1;
                }
                Err(e) => result.errors.push(format!("{}: {e}", entry.repo)),
            }
        }
        Ok(result)
    })
    .await?
}
