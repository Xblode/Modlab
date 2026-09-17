//! Tauri host for the React shell. This crate is a *client* of
//! `modlab-core` — same rule as the CLI: no private state, no game rules.

use modlab_core::game_adapter::GameAdapter;
use modlab_core::{conflicts, loadout, Core};
use modlab_game_minecraft::MinecraftAdapter;
use modlab_game_skyrim::SkyrimAdapter;
use serde::Serialize;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Duration;
use tauri::State;

pub struct AppState {
    core: Mutex<Core>,
    data_root: PathBuf,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DetectedGameDto {
    pub id: String,
    pub name: String,
    pub root: String,
    pub short: String,
    pub capabilities: modlab_core::game_adapter::GameCapabilities,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LoadoutDto {
    pub id: i64,
    pub game_id: String,
    pub name: String,
    /// Number of mods the Core holds for this loadout. Real count, not an
    /// estimate — the dashboard instance cards show it. Everything else the
    /// maquette puts on those cards (revision, last played) has no Core
    /// backing yet and is rendered as an explicit placeholder by the UI.
    pub mod_count: i64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModRowDto {
    pub mod_id: i64,
    pub name: String,
    pub source_path: String,
    pub enabled: bool,
    pub position: i64,
}

/// One mod's share of a loadout's file conflicts. `wins`/`loses` count the
/// distinct paths this mod is involved in — the shape the Mods table needs to
/// badge a row, without shipping every conflicting path across IPC.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModConflictsDto {
    pub mod_id: i64,
    pub name: String,
    pub wins: i64,
    pub loses: i64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LoadoutConflictsDto {
    pub mods: Vec<ModConflictsDto>,
    /// Enabled mods whose folder could not be read, as `id: reason`.
    ///
    /// An empty `mods` list plus a non-empty `unreadable` list means "not
    /// scanned", never "no conflict" — the UI must not conflate the two.
    pub unreadable: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShellSnapshot {
    pub data_root: String,
    pub games: Vec<DetectedGameDto>,
    pub loadouts: Vec<LoadoutDto>,
    pub entries: Vec<ModRowDto>,
    pub selected_loadout_id: Option<i64>,
    pub usvfs_dll: Option<String>,
}

fn detect_games() -> Vec<DetectedGameDto> {
    let adapters: [&dyn GameAdapter; 2] = [&SkyrimAdapter, &MinecraftAdapter];
    adapters
        .into_iter()
        .filter_map(|adapter| {
            let install = adapter.detect()?;
            let capabilities = adapter.capabilities();
            Some(DetectedGameDto {
                id: install.id,
                name: install.name,
                root: install.root.display().to_string(),
                short: capabilities.short_name.to_string(),
                capabilities,
            })
        })
        .collect()
}

fn resolve_data_root() -> PathBuf {
    if let Some(from_env) = std::env::var_os("MODLAB_DATA_ROOT") {
        return PathBuf::from(from_env);
    }
    let real = PathBuf::from(r"F:\ModLab");
    if real.join("db").join("modlab.db").is_file() || real.join("modlab.db").is_file() {
        return real;
    }
    let sandbox = PathBuf::from(".modlab-dev");
    if sandbox.join("modlab.db").is_file() || sandbox.join("db").join("modlab.db").is_file() {
        return sandbox;
    }
    if real.is_dir() {
        return real;
    }
    sandbox
}

fn open_core(data_root: &std::path::Path) -> Result<Core, String> {
    std::fs::create_dir_all(data_root).map_err(|e| e.to_string())?;
    let nested = data_root.join("db").join("modlab.db");
    let flat = data_root.join("modlab.db");
    let db_path = if nested.is_file() {
        nested
    } else if flat.is_file() {
        flat
    } else {
        std::fs::create_dir_all(data_root.join("db")).map_err(|e| e.to_string())?;
        nested
    };
    Core::open(db_path).map_err(|e| e.to_string())
}

fn entries_dto(core: &Core, loadout_id: i64) -> Result<Vec<ModRowDto>, String> {
    loadout::entries(core, loadout_id)
        .map_err(|e| e.to_string())
        .map(|rows| {
            rows.into_iter()
                .map(|r| ModRowDto {
                    mod_id: r.mod_id,
                    name: r.name,
                    source_path: r.source_path.display().to_string(),
                    enabled: r.enabled,
                    position: r.position,
                })
                .collect()
        })
}

#[tauri::command]
fn get_shell_snapshot(
    state: State<AppState>,
    loadout_id: Option<i64>,
) -> Result<ShellSnapshot, String> {
    let core = state.core.lock().map_err(|e| e.to_string())?;
    let loadouts: Vec<LoadoutDto> = loadout::list(&core)
        .map_err(|e| e.to_string())?
        .into_iter()
        .map(|l| {
            let mod_count = loadout::entries(&core, l.id)
                .map(|rows| rows.len() as i64)
                .unwrap_or(0);
            LoadoutDto {
                id: l.id,
                game_id: l.game_id,
                name: l.name,
                mod_count,
            }
        })
        .collect();
    let selected = loadout_id.or_else(|| loadouts.first().map(|l| l.id));
    let entries = match selected {
        Some(id) => entries_dto(&core, id)?,
        None => Vec::new(),
    };
    Ok(ShellSnapshot {
        data_root: state.data_root.display().to_string(),
        games: detect_games(),
        loadouts,
        entries,
        selected_loadout_id: selected,
        usvfs_dll: modlab_game_skyrim::find_usvfs_dll().map(|p| p.display().to_string()),
    })
}

#[tauri::command]
fn set_mod_enabled(
    state: State<AppState>,
    loadout_id: i64,
    mod_id: i64,
    enabled: bool,
) -> Result<Vec<ModRowDto>, String> {
    let core = state.core.lock().map_err(|e| e.to_string())?;
    loadout::set_enabled(&core, loadout_id, mod_id, enabled).map_err(|e| e.to_string())?;
    entries_dto(&core, loadout_id)
}

/// File conflicts for a loadout — the same scan `modlab loadout-conflicts`
/// prints (`modlab_core::conflicts::report_for_entries`), so what the UI
/// badges and what the CLI reports can never drift apart.
///
/// Read-only, so it sits in the "Lecture" permission tier (§7.2): nothing is
/// written, no confirmation is involved. Deliberately recomputed on every
/// call instead of cached in `AppState`: a cache would have to be
/// invalidated on every enable/disable, and a stale conflict badge is a wrong
/// statement about the user's install. The UI debounces its calls instead.
#[tauri::command]
fn get_loadout_conflicts(
    state: State<AppState>,
    loadout_id: i64,
) -> Result<LoadoutConflictsDto, String> {
    let core = state.core.lock().map_err(|e| e.to_string())?;
    let rows = loadout::entries(&core, loadout_id).map_err(|e| e.to_string())?;
    let report = conflicts::report_for_entries(&rows);
    let count = |n: usize| i64::try_from(n).unwrap_or(i64::MAX);
    Ok(LoadoutConflictsDto {
        mods: report
            .mods
            .into_iter()
            .map(|m| ModConflictsDto {
                mod_id: m.mod_id,
                name: m.name,
                wins: count(m.wins),
                loses: count(m.loses),
            })
            .collect(),
        unreadable: report
            .unreadable
            .into_iter()
            .map(|(mod_id, why)| format!("{mod_id}: {why}"))
            .collect(),
    })
}

/// Create an empty loadout through the Core. The UI never writes SQLite
/// directly; it remains a client of the engine just like the CLI.
#[tauri::command]
fn create_loadout(
    state: State<AppState>,
    game_id: String,
    name: String,
) -> Result<LoadoutDto, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("Donne un nom à l’instance.".into());
    }
    if name.chars().count() > 100 {
        return Err("Le nom de l’instance ne peut pas dépasser 100 caractères.".into());
    }
    let core = state.core.lock().map_err(|e| e.to_string())?;
    let loadout = loadout::create(&core, &game_id, name).map_err(|e| e.to_string())?;
    Ok(LoadoutDto {
        id: loadout.id,
        game_id,
        name: loadout.name,
        mod_count: 0,
    })
}

#[tauri::command]
fn launch_skse(state: State<AppState>, loadout_id: i64) -> Result<(), String> {
    #[cfg(not(windows))]
    {
        let _ = (state, loadout_id);
        return Err("le lancement SKSE n'est disponible que sous Windows".into());
    }
    #[cfg(windows)]
    {
        let install = SkyrimAdapter
            .detect()
            .ok_or_else(|| "Skyrim SE introuvable sur cette machine".to_string())?;
        let usvfs_dll = modlab_game_skyrim::find_usvfs_dll().ok_or_else(|| {
            "usvfs_x64.dll introuvable (installe MO2, ou MODLAB_USVFS_DLL)".to_string()
        })?;
        let loader = install.root.join("skse64_loader.exe");
        if !loader.is_file() {
            return Err(format!(
                "skse64_loader.exe introuvable dans {}",
                install.root.display()
            ));
        }
        let rows = {
            let core = state.core.lock().map_err(|e| e.to_string())?;
            loadout::entries(&core, loadout_id).map_err(|e| e.to_string())?
        };
        let data_dir = install.root.join("Data");
        let vfs = modlab_game_skyrim::usvfs::Vfs::create(&usvfs_dll, "modlab")
            .map_err(|e| e.to_string())?;
        modlab_game_skyrim::usvfs::deploy_enabled_mods(&vfs, &rows, &data_dir)
            .map_err(|e| e.to_string())?;
        let cmd = format!("\"{}\"", loader.display());
        vfs.launch_hooked_visible(&cmd, &install.root)
            .map_err(|e| e.to_string())?;
        vfs.wait_for_session_end(Duration::from_secs(2), Duration::from_secs(60))
            .map_err(|e| e.to_string())?;
        Ok(())
    }
}

pub fn run() {
    let data_root = resolve_data_root();
    let core = open_core(&data_root).expect("failed to open the ModLab engine");
    let _ = core.log_event("ui.started", "{}");

    tauri::Builder::default()
        .manage(AppState {
            core: Mutex::new(core),
            data_root,
        })
        .invoke_handler(tauri::generate_handler![
            get_shell_snapshot,
            set_mod_enabled,
            get_loadout_conflicts,
            create_loadout,
            launch_skse
        ])
        .run(tauri::generate_context!())
        .expect("error while running ModLab");
}
