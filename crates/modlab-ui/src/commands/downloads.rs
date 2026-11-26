#[allow(unused_imports)]
use crate::*;
#[allow(unused_imports)]
use modlab_ai::{diagnose, llm_phrase, rules_answer, LoadoutFacts, ModFact};
#[allow(unused_imports)]
use modlab_core::game_adapter::{fuse_path, GameAdapter, PathKind};
#[allow(unused_imports)]
use modlab_core::{conflicts, download, loadout, transaction, Core};
#[allow(unused_imports)]
use modlab_game_minecraft::MinecraftAdapter;
#[allow(unused_imports)]
use modlab_game_skyrim::fomod;
#[allow(unused_imports)]
use modlab_game_skyrim::installer::ArchiveShape;
#[allow(unused_imports)]
use modlab_game_skyrim::plugins;
#[allow(unused_imports)]
use modlab_game_skyrim::vanilla;
#[allow(unused_imports)]
use modlab_game_skyrim::SkyrimAdapter;
#[allow(unused_imports)]
use serde::Serialize;
#[allow(unused_imports)]
use std::path::{Component, Path, PathBuf};
#[allow(unused_imports)]
use std::sync::atomic::{AtomicBool, Ordering};
#[allow(unused_imports)]
use std::sync::Mutex;
#[allow(unused_imports)]
#[cfg(windows)]
use std::time::{Duration, SystemTime};
#[allow(unused_imports)]
use tauri::{Emitter, Manager, State};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadDto {
    pub id: i64,
    pub url: String,
    pub file_name: String,
    pub path: String,
    pub size_bytes: Option<i64>,
    pub captured_at: String,
    pub loadout_id: Option<i64>,
    pub mod_id: Option<i64>,
}

/// Everything the browser has captured, newest first — the "dossier
/// téléchargements" as a list rather than as a folder to go dig through.
#[tauri::command]
pub(crate) fn list_downloads(state: State<AppState>) -> Result<Vec<DownloadDto>, String> {
    let core = state.core.lock().map_err(|e| e.to_string())?;
    Ok(download::list(&core)
        .map_err(|e| e.to_string())?
        .into_iter()
        .map(|d| DownloadDto {
            id: d.id,
            url: d.url,
            file_name: d.file_name,
            path: d.path.display().to_string(),
            size_bytes: d.size_bytes,
            captured_at: d.captured_at,
            loadout_id: d.loadout_id,
            mod_id: d.mod_id,
        })
        .collect())
}

/// Promote a download that reached 100 % without a Finished/captured event.
/// `giveUp` abandons the row when the file is still missing.
#[tauri::command]
pub(crate) fn finalize_download(
    app: tauri::AppHandle,
    download_id: i64,
    give_up: bool,
) -> Result<crate::downloads::FinalizeOutcome, String> {
    Ok(crate::downloads::finalize_by_id(&app, download_id, give_up))
}

/// File a capture into a loadout: it appears in the mod table right away,
/// archived and disabled.
///
/// An archive the Skyrim adapter recognises — a silverlock file name, a
/// Nexus title that *is* SKSE (`Skyrim Script Extender (SKSE64)…`), or the
/// `skse.silverlock.org` host — is filed under that tool's display name and
/// parsed version. Address Library and Engine Fixes stay ordinary mods.
/// Anything else keeps the file stem. The install that follows is the
/// ordinary archive install; a folder that actually contains
/// `skse64_loader.exe` is labelled SKSE there even when the file name was not.
#[tauri::command]
pub(crate) fn attach_download(
    app: tauri::AppHandle,
    state: State<AppState>,
    download_id: i64,
    loadout_id: i64,
    category: Option<String>,
) -> Result<i64, String> {
    let core = state.core.lock().map_err(|e| e.to_string())?;
    let captured = download::captured_file(&core, download_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("téléchargement {download_id} introuvable"))?;
    let recognized =
        modlab_game_skyrim::tool_archive::recognize(&captured.file_name, &captured.url);
    let mod_id = match recognized {
        Some(tool) => download::attach_recognized(
            &core,
            download_id,
            loadout_id,
            category.as_deref(),
            tool.display_name,
            tool.version.as_deref(),
        ),
        None => download::attach_to_loadout(&core, download_id, loadout_id, category.as_deref()),
    }
    .map_err(|e| e.to_string())?;
    drop(core);
    // Enrich off the command path — archive walks must not hold the Core lock
    // while the shell waits on attach.
    std::thread::spawn(move || {
        let Some(state) = app.try_state::<AppState>() else {
            return;
        };
        let Ok(core) = state.core.lock() else {
            return;
        };
        if let Err(err) = crate::enrich_mod_file(&core, loadout_id, mod_id) {
            eprintln!("[dl] métadonnées non lues après classement : {err}");
        }
    });
    Ok(mod_id)
}
