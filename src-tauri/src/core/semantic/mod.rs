//! Search and semantic categories for library skills (fork addition).
//!
//! Everything lives in its own SQLite file, `<library>/semantic.db`, apart
//! from the app's main database: the main schema and migrations stay
//! untouched, and the startup reindex that rebuilds the main database from
//! metadata cannot drop it. The index and vectors are a cache — rebuilt from
//! the library at any time; only category edits are the user's data.
//!
//! - [`index`]: full-text index of library skills + ranked search
//! - [`query`]: the query language (free text + `key:value` filters,
//!   prefix/infix/typo expansion)
//! - [`categories`]: built-in and user categories, nearest-category sorting
//! - [`embed`]: all-MiniLM-L6-v2 on candle
//! - [`worker`]: background thread that keeps the index and vectors current

pub mod categories;
pub mod embed;
pub mod index;
pub mod query;
pub mod worker;

use rusqlite::Connection;
use std::path::Path;
use std::sync::{Mutex, MutexGuard};

pub const DB_FILE: &str = "semantic.db";

pub struct SemanticDb(Mutex<Connection>);

impl SemanticDb {
    pub fn open(path: &Path) -> Result<Self, String> {
        let conn = Connection::open(path).map_err(|e| e.to_string())?;
        Self::init(conn)
    }

    /// Fallback when the file can't be opened: search and categories work
    /// for this session, nothing persists.
    pub fn open_in_memory() -> Result<Self, String> {
        Self::init(Connection::open_in_memory().map_err(|e| e.to_string())?)
    }

    fn init(conn: Connection) -> Result<Self, String> {
        conn.execute_batch("PRAGMA journal_mode = WAL; PRAGMA synchronous = NORMAL;")
            .map_err(|e| e.to_string())?;
        migrate(&conn).map_err(|e| e.to_string())?;
        categories::seed(&conn).map_err(|e| e.to_string())?;
        Ok(Self(Mutex::new(conn)))
    }

    pub fn conn(&self) -> MutexGuard<'_, Connection> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }
}

/// Append-only; `PRAGMA user_version` counts what has run.
const MIGRATIONS: &[&str] = &[
    // 1: library index, categories, vectors.
    "CREATE TABLE docs (
        rid       INTEGER PRIMARY KEY,
        id        TEXT NOT NULL UNIQUE,
        name      TEXT NOT NULL,
        tags      TEXT NOT NULL,
        source    TEXT NOT NULL,
        text_hash TEXT NOT NULL
    );
    CREATE VIRTUAL TABLE docs_fts USING fts5(
        name, description, body, tags, source,
        tokenize = 'unicode61 remove_diacritics 2',
        prefix = '2 3'
    );
    CREATE VIRTUAL TABLE docs_vocab USING fts5vocab(docs_fts, 'row');

    CREATE TABLE categories (
        id          TEXT PRIMARY KEY,
        name        TEXT NOT NULL,
        emoji       TEXT NOT NULL DEFAULT '',
        description TEXT NOT NULL,
        builtin     INTEGER NOT NULL,
        position    INTEGER NOT NULL,
        vector      BLOB
    );
    CREATE TABLE embeddings (
        id          TEXT PRIMARY KEY,
        text_hash   TEXT NOT NULL,
        vector      BLOB NOT NULL,
        category_id TEXT
    );",
    // 2: vectors for skills not in the library (marketplace, git preview),
    // keyed by the embedded text so a skill seen twice is embedded once.
    "CREATE TABLE remote_vectors (
        text_hash TEXT PRIMARY KEY,
        vector    BLOB NOT NULL
    );",
];

pub fn migrate(conn: &Connection) -> rusqlite::Result<()> {
    let current: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    for (i, sql) in MIGRATIONS.iter().enumerate().skip(current as usize) {
        conn.execute_batch(&format!("BEGIN; {sql}; PRAGMA user_version = {}; COMMIT;", i + 1))?;
    }
    Ok(())
}

/// Stable across Rust releases (unlike `DefaultHasher`), so stored hashes
/// stay comparable after an app update.
pub fn text_hash(text: &str) -> String {
    let h = text.bytes().fold(0xcbf2_9ce4_8422_2325u64, |h, b| {
        (h ^ u64::from(b)).wrapping_mul(0x0000_0100_0000_01b3)
    });
    format!("{h:016x}")
}

#[cfg(test)]
pub(crate) fn test_conn() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    migrate(&conn).unwrap();
    categories::seed(&conn).unwrap();
    conn
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrations_are_idempotent() {
        let conn = test_conn();
        migrate(&conn).unwrap();
        let v: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0)).unwrap();
        assert_eq!(v as usize, MIGRATIONS.len());
    }
}
