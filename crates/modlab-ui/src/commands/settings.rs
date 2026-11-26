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
pub struct DownloadsDirDto {
    /// `<data root>/downloads` — where captures land absent any override.
    pub default_path: String,
    /// A human-set override, if any (Réglages → Général).
    pub override_path: Option<String>,
    /// `override_path` if set, `default_path` otherwise — the one value a
    /// client actually displays.
    pub effective: String,
}

fn downloads_dir_dto(state: &State<AppState>) -> Result<DownloadsDirDto, String> {
    let core = state.core.lock().map_err(|e| e.to_string())?;
    let default_path = crate::downloads::default_downloads_root(&state.data_root)
        .display()
        .to_string();
    let override_path = core
        .game_path_override(
            crate::downloads::SETTINGS_SCOPE,
            crate::downloads::DOWNLOADS_DIR_SLOT,
        )
        .map_err(|e| e.to_string())?;
    let effective = override_path
        .clone()
        .unwrap_or_else(|| default_path.clone());
    Ok(DownloadsDirDto {
        default_path,
        override_path,
        effective,
    })
}

/// Read-only (Lecture tier, §7.2): what Réglages → Général shows.
#[tauri::command]
pub(crate) fn downloads_dir_setting(state: State<AppState>) -> Result<DownloadsDirDto, String> {
    downloads_dir_dto(&state)
}

/// Point the downloads folder at a location the human chose — Mutation
/// réversible tier (§7.2): nothing already on disk moves, this only changes
/// where the *next* capture lands, and clearing it undoes it instantly.
#[tauri::command]
pub(crate) fn set_downloads_dir_override(
    state: State<AppState>,
    path: String,
) -> Result<DownloadsDirDto, String> {
    let path = path.trim();
    if path.is_empty() {
        return Err("Le chemin ne peut pas être vide.".into());
    }
    {
        let core = state.core.lock().map_err(|e| e.to_string())?;
        core.set_game_path_override(
            crate::downloads::SETTINGS_SCOPE,
            crate::downloads::DOWNLOADS_DIR_SLOT,
            path,
        )
        .map_err(|e| e.to_string())?;
    }
    downloads_dir_dto(&state)
}

#[tauri::command]
pub(crate) fn clear_downloads_dir_override(
    state: State<AppState>,
) -> Result<DownloadsDirDto, String> {
    {
        let core = state.core.lock().map_err(|e| e.to_string())?;
        core.clear_game_path_override(
            crate::downloads::SETTINGS_SCOPE,
            crate::downloads::DOWNLOADS_DIR_SLOT,
        )
        .map_err(|e| e.to_string())?;
    }
    downloads_dir_dto(&state)
}
