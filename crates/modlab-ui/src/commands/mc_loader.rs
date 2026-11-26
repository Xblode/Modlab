#[allow(unused_imports)]
use crate::*;
#[allow(unused_imports)]
use modlab_core::{loadout, Core};
#[allow(unused_imports)]
use modlab_game_minecraft::loader_install::{self, LoaderProgress, LoaderStatus};
#[allow(unused_imports)]
use serde::Serialize;
#[allow(unused_imports)]
use std::path::PathBuf;
#[allow(unused_imports)]
use tauri::{AppHandle, Emitter, State};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct McLoaderStatusDto {
    pub state: String,
    pub message: String,
}

impl From<LoaderStatus> for McLoaderStatusDto {
    fn from(s: LoaderStatus) -> Self {
        Self {
            state: s.state.to_string(),
            message: s.message,
        }
    }
}

fn branch_mc_fields(core: &Core, loadout_id: i64) -> Result<loadout::LoadoutSummary, String> {
    loadout::list(core)
        .map_err(|e| e.to_string())?
        .into_iter()
        .find(|b| b.id == loadout_id)
        .ok_or_else(|| format!("branche {loadout_id} introuvable"))
}

fn instance_root_for_branch(
    core: &Core,
    data_root: &std::path::Path,
    loadout_id: i64,
) -> Result<PathBuf, String> {
    let target = crate::paths::install_target_for_loadout(core, data_root, loadout_id)?
        .ok_or_else(|| "Cette instance n'a pas de dossier dédié.".to_string())?;
    Ok(target.instance_root)
}

#[tauri::command]
pub(crate) fn set_branch_mc_loader(
    state: State<AppState>,
    loadout_id: i64,
    loader: String,
) -> Result<(), String> {
    let core = state.core.lock().map_err(|e| e.to_string())?;
    loadout::set_mc_loader(&core, loadout_id, &loader).map_err(|e| e.to_string())
}

#[tauri::command]
pub(crate) fn set_branch_mc_game_version(
    state: State<AppState>,
    loadout_id: i64,
    version: String,
) -> Result<(), String> {
    let core = state.core.lock().map_err(|e| e.to_string())?;
    loadout::set_mc_game_version(&core, loadout_id, &version).map_err(|e| e.to_string())
}

#[tauri::command]
pub(crate) fn set_branch_mc_loader_version(
    state: State<AppState>,
    loadout_id: i64,
    version: String,
) -> Result<(), String> {
    let core = state.core.lock().map_err(|e| e.to_string())?;
    loadout::set_mc_loader_version(&core, loadout_id, &version).map_err(|e| e.to_string())
}

#[tauri::command]
pub(crate) async fn list_mc_game_versions() -> Result<Vec<String>, String> {
    crate::off_ui(loader_install::list_game_versions).await
}

#[tauri::command]
pub(crate) async fn list_mc_loader_versions(
    loader: String,
    mc_version: String,
) -> Result<Vec<String>, String> {
    crate::off_ui(move || loader_install::list_loader_versions(&loader, &mc_version)).await
}

#[tauri::command]
pub(crate) fn list_installed_mc_loader_versions(
    state: State<AppState>,
    loadout_id: i64,
    loader: String,
    mc_version: String,
) -> Result<Vec<String>, String> {
    let core = state.core.lock().map_err(|e| e.to_string())?;
    let root = instance_root_for_branch(&core, &state.data_root, loadout_id)?;
    Ok(loader_install::list_installed_loader_versions(
        &root,
        &loader,
        &mc_version,
    ))
}

#[tauri::command]
pub(crate) fn list_installed_mc_game_versions(
    state: State<AppState>,
    loadout_id: i64,
) -> Result<Vec<String>, String> {
    let core = state.core.lock().map_err(|e| e.to_string())?;
    let root = instance_root_for_branch(&core, &state.data_root, loadout_id)?;
    Ok(loader_install::list_installed_game_versions(&root))
}

#[tauri::command]
pub(crate) fn mc_loader_status(
    state: State<AppState>,
    loadout_id: i64,
) -> Result<McLoaderStatusDto, String> {
    let core = state.core.lock().map_err(|e| e.to_string())?;
    let branch = branch_mc_fields(&core, loadout_id)?;
    let root = instance_root_for_branch(&core, &state.data_root, loadout_id)?;
    Ok(loader_install::status(
        &root,
        branch.mc_loader.as_deref(),
        branch.mc_game_version.as_deref(),
        branch.mc_loader_version.as_deref(),
    )
    .into())
}

#[tauri::command]
pub(crate) async fn install_branch_mc_loader(
    app: AppHandle,
    state: State<'_, AppState>,
    loadout_id: i64,
    force: Option<bool>,
) -> Result<(), String> {
    let force = force.unwrap_or(false);
    let (data_root, loader, mc, loader_ver, root) = {
        let core = state.core.lock().map_err(|e| e.to_string())?;
        let branch = branch_mc_fields(&core, loadout_id)?;
        let loader = branch
            .mc_loader
            .clone()
            .ok_or_else(|| "Choisissez un loader.".to_string())?;
        let mc = branch
            .mc_game_version
            .clone()
            .ok_or_else(|| "Choisissez une version Minecraft.".to_string())?;
        let loader_ver = branch.mc_loader_version.clone();
        if loader != "vanilla" && loader_ver.is_none() {
            return Err("Choisissez une version du loader.".to_string());
        }
        let root = instance_root_for_branch(&core, &state.data_root, loadout_id)?;
        (state.data_root.clone(), loader, mc, loader_ver, root)
    };
    off_ui(move || {
        let emit = |p: LoaderProgress| {
            let _ = app.emit("loader://progress", p);
        };
        loader_install::install(
            &root,
            &data_root,
            loadout_id,
            &loader,
            &mc,
            loader_ver.as_deref(),
            force,
            &emit,
        )
    })
    .await?;
    let core = state.core.lock().map_err(|e| e.to_string())?;
    crate::sync_mc_branch_loader(&core, &state.data_root, loadout_id)
}
