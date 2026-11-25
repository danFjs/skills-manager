//! Local sharing commands: export library skills into one `.zip`, and
//! import such an archive. Replaces git backup/sync in this fork — no
//! account, no network; the file can be passed on any way you like.

use std::path::PathBuf;
use std::sync::Arc;

use tauri::State;

use super::skills::{import_skill_dirs, BatchImportResult};
use crate::core::{
    error::AppError,
    repo_lock::RepoLock,
    skill_share::{self, ShareSummary},
    skill_store::SkillStore,
};

/// Pack the given library skills (all of them when `skill_ids` is empty)
/// into a zip at `dest_path`. Holds the repo lock so an install or sync
/// can't change a skill folder halfway through the export.
#[tauri::command]
pub async fn export_skills_archive(
    skill_ids: Vec<String>,
    dest_path: String,
    store: State<'_, Arc<SkillStore>>,
) -> Result<ShareSummary, AppError> {
    let store = store.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let _lock = RepoLock::acquire_foreground("share skills").map_err(AppError::db)?;
        let all = store.get_all_skills().map_err(AppError::db)?;
        let chosen: Vec<(String, PathBuf)> = all
            .into_iter()
            .filter(|s| skill_ids.is_empty() || skill_ids.contains(&s.id))
            .map(|s| {
                let dir = PathBuf::from(&s.central_path);
                let folder = dir
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| s.name.clone());
                (folder, dir)
            })
            .collect();
        skill_share::write_archive(&chosen, &PathBuf::from(dest_path)).map_err(AppError::io)
    })
    .await?
}

/// Import every skill in a shared archive. Names already in the library
/// are skipped (counted in `skipped`), exactly like folder import.
#[tauri::command]
pub async fn import_skills_archive(
    archive_path: String,
    store: State<'_, Arc<SkillStore>>,
    app_handle: tauri::AppHandle,
) -> Result<BatchImportResult, AppError> {
    let store = store.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let unpacked =
            skill_share::extract_archive(&PathBuf::from(&archive_path)).map_err(AppError::io)?;
        Ok(import_skill_dirs(
            &store,
            &app_handle,
            &unpacked.skill_dirs,
            Some(archive_path),
        ))
    })
    .await?
}
