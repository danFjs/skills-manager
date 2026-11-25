//! Search and categories for library skills (fork addition).

use crate::core::error::AppError;
use crate::core::semantic::categories::{self, CategoryInfo};
use crate::core::semantic::index::{self, Hit};
use crate::core::semantic::{worker, SemanticDb};
use std::collections::HashMap;
use tauri::{AppHandle, Manager};

/// Ranked search over library skills: names, descriptions, SKILL.md text,
/// tags and source, with typo/infix matching and `tag:` / `source:` filters.
#[tauri::command]
pub async fn search_library(app: AppHandle, query: String) -> Result<Vec<Hit>, AppError> {
    tauri::async_runtime::spawn_blocking(move || {
        worker::ensure_fresh(&app).map_err(AppError::db)?;
        let db = app.state::<SemanticDb>();
        let hits = index::search(&db.conn(), &query).map_err(AppError::db)?;
        Ok(hits)
    })
    .await?
}

#[tauri::command]
pub async fn list_categories(app: AppHandle) -> Result<Vec<CategoryInfo>, AppError> {
    tauri::async_runtime::spawn_blocking(move || {
        categories::list(&app.state::<SemanticDb>().conn()).map_err(AppError::db)
    })
    .await?
}

/// Library skill id → category id, for every skill sorted so far.
#[tauri::command]
pub async fn category_assignments(app: AppHandle) -> Result<HashMap<String, String>, AppError> {
    tauri::async_runtime::spawn_blocking(move || {
        categories::assignments(&app.state::<SemanticDb>().conn()).map_err(AppError::db)
    })
    .await?
}

/// Create (`id` null) or edit a category. The description is required — it
/// is what skills are matched against; skills are re-sorted in the background.
#[tauri::command]
pub async fn save_category(
    app: AppHandle,
    id: Option<String>,
    name: String,
    emoji: String,
    description: String,
) -> Result<CategoryInfo, AppError> {
    tauri::async_runtime::spawn_blocking(move || {
        let saved = categories::save(
            &app.state::<SemanticDb>().conn(),
            id.as_deref(),
            &name,
            &emoji,
            &description,
        )
        .map_err(AppError::invalid_input)?;
        worker::kick(&app);
        Ok(saved)
    })
    .await?
}

#[tauri::command]
pub async fn delete_category(app: AppHandle, id: String) -> Result<(), AppError> {
    tauri::async_runtime::spawn_blocking(move || {
        categories::delete(&app.state::<SemanticDb>().conn(), &id).map_err(AppError::invalid_input)
    })
    .await?
}

#[derive(serde::Deserialize)]
pub struct CategorizeItem {
    pub key: String,
    pub text: String,
}

/// Category per item for skills outside the library (marketplace, git
/// preview). New texts are embedded once and cached.
#[tauri::command]
pub async fn categorize_items(
    app: AppHandle,
    items: Vec<CategorizeItem>,
) -> Result<HashMap<String, String>, AppError> {
    tauri::async_runtime::spawn_blocking(move || {
        let pairs: Vec<(String, String)> = items.into_iter().map(|i| (i.key, i.text)).collect();
        let db = app.state::<SemanticDb>();
        let conn = db.conn();
        categories::categorize(&conn, &pairs, |texts| {
            worker::with_model(|m| m.embed(texts)).and_then(|r| r)
        })
        .map_err(AppError::io)
    })
    .await?
}
