//! Create a skill in the library and edit its SKILL.md (fork addition).

use std::path::Path;
use std::sync::Arc;

use tauri::{AppHandle, State};

use super::skills::{managed_skill_by_id, store_installed_skill_unlocked, InstallSourceMetadata, ManagedSkillDto};
use crate::core::{
    central_repo, content_hash, error::AppError, installer, repo_lock::RepoLock, scenario_service,
    semantic::worker, skill_authoring, skill_metadata, skill_store::SkillStore, sync_metadata,
};

/// Create `<library>/<name>/SKILL.md` from the template and register it as a
/// local skill (no source to update from). Deploy it to agents afterwards
/// with the usual per-agent toggles.
#[tauri::command]
pub async fn create_skill(
    app: AppHandle,
    name: String,
    description: String,
    store: State<'_, Arc<SkillStore>>,
) -> Result<ManagedSkillDto, AppError> {
    let store = store.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        skill_authoring::validate_name(&name).map_err(AppError::invalid_input)?;
        if description.trim().is_empty() {
            return Err(AppError::invalid_input(
                "Add a description — agents use it to decide when to load the skill",
            ));
        }
        // The installer would quietly pick `name-2`; a new skill must not.
        if central_repo::skills_dir().join(&name).exists() {
            return Err(AppError::invalid_input(format!(
                "Your library already has a skill folder named '{name}'"
            )));
        }

        let staging = tempfile::tempdir()?;
        let dir = staging.path().join(&name);
        std::fs::create_dir_all(&dir)?;
        std::fs::write(dir.join("SKILL.md"), skill_authoring::new_skill_md(&name, &description))?;

        let id = {
            let _lock = RepoLock::acquire_foreground("create skill").map_err(AppError::db)?;
            let result = installer::install_from_local(&dir, Some(&name)).map_err(AppError::io)?;
            let metadata = InstallSourceMetadata {
                source_type: "local".to_string(),
                source_ref: None,
                source_ref_resolved: None,
                source_subpath: None,
                source_branch: None,
                source_revision: None,
                remote_revision: None,
                update_status: "local_only".to_string(),
            };
            store_installed_skill_unlocked(&store, &result, &metadata, None)?
        };
        worker::kick(&app);
        managed_skill_by_id(&store, &id)
    })
    .await?
}

/// Overwrite a library skill's SKILL.md (the top-level one), refresh its
/// stored hash and description, and re-copy it to agents that hold copies —
/// linked agents see the change immediately.
#[tauri::command]
pub async fn save_skill_document(
    app: AppHandle,
    skill_id: String,
    content: String,
    store: State<'_, Arc<SkillStore>>,
) -> Result<ManagedSkillDto, AppError> {
    let store = store.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        if content.len() > skill_authoring::MAX_DOCUMENT_BYTES {
            return Err(AppError::invalid_input("That SKILL.md is too large to save"));
        }
        let skill = store
            .get_skill_by_id(&skill_id)
            .map_err(AppError::db)?
            .ok_or_else(|| AppError::not_found("Skill not found"))?;
        let dir = Path::new(&skill.central_path);
        // Only the top-level manifest is editable; a skill whose document
        // lives deeper is left to an external editor.
        let file = ["SKILL.md", "skill.md"]
            .iter()
            .map(|n| dir.join(n))
            .find(|p| p.is_file())
            .ok_or_else(|| AppError::invalid_input("This skill has no top-level SKILL.md to edit"))?;

        {
            let _lock = RepoLock::acquire_foreground("edit skill").map_err(AppError::db)?;
            let tmp = file.with_extension("md.part");
            std::fs::write(&tmp, &content)?;
            std::fs::rename(&tmp, &file)?;

            let meta = skill_metadata::parse_skill_md(dir);
            let hash = content_hash::hash_directory(dir).map_err(AppError::io)?;
            store
                .update_skill_after_reinstall(
                    &skill.id,
                    &skill.name,
                    meta.description.as_deref().or(skill.description.as_deref()),
                    &skill.source_type,
                    skill.source_ref.as_deref(),
                    skill.source_ref_resolved.as_deref(),
                    skill.source_subpath.as_deref(),
                    skill.source_branch.as_deref(),
                    skill.source_revision.as_deref(),
                    skill.remote_revision.as_deref(),
                    Some(&hash),
                    &skill.update_status,
                )
                .map_err(AppError::db)?;

            for target in store.get_targets_for_skill(&skill.id).map_err(AppError::db)? {
                if target.mode == "copy" {
                    if let Err(e) = scenario_service::sync_single_skill_to_tool(
                        &store,
                        &skill.id,
                        &target.tool,
                        scenario_service::DeployIntent::Managed,
                    ) {
                        log::warn!("edit: re-copying {} to {} failed: {e}", skill.name, target.tool);
                    }
                }
            }
            sync_metadata::write_all_from_db_unlocked(&store).map_err(AppError::io)?;
        }
        worker::kick(&app);
        managed_skill_by_id(&store, &skill.id)
    })
    .await?
}
