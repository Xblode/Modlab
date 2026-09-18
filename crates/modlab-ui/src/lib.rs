//! Tauri host for the React shell. This crate is a *client* of
//! `modlab-core` — same rule as the CLI: no private state, no game rules.

use modlab_core::game_adapter::GameAdapter;
use modlab_core::{conflicts, loadout, Core};
use modlab_game_minecraft::MinecraftAdapter;
use modlab_game_skyrim::SkyrimAdapter;
use serde::Serialize;
use std::path::{Path, PathBuf};
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
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Mo2ImportResultDto {
    pub loadout: LoadoutDto,
    /// How many mods actually landed in the loadout — the dialog's success
    /// message. Informational profile entries (DLC, Creation Club, unmanaged)
    /// are deliberately not part of it.
    pub imported: usize,
}

/// The scan itself, split out from the command so it can be tested against a
/// real (in-memory) Core plus real folders — the browser preview only ever
/// exercises the UI's handling of the payload, never this path.
fn loadout_conflicts(core: &Core, loadout_id: i64) -> Result<LoadoutConflictsDto, String> {
    let rows = loadout::entries(core, loadout_id).map_err(|e| e.to_string())?;
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

#[tauri::command]
fn get_loadout_conflicts(
    state: State<AppState>,
    loadout_id: i64,
) -> Result<LoadoutConflictsDto, String> {
    let core = state.core.lock().map_err(|e| e.to_string())?;
    loadout_conflicts(&core, loadout_id)
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

/// Import an MO2 profile into a new Skyrim loadout. A mutation like any
/// other: journaled by the Core, and the UI stays a client of the engine.
/// The heavy lifting (profile parsing, folder verification, import order)
/// lives in `modlab_game_skyrim::mo2::import_into`, the same code the CLI
/// runs — this host only validates what the user typed and wraps the answer.
#[tauri::command]
fn mo2_import(
    state: State<AppState>,
    profile: String,
    mods_dir: String,
    name: String,
) -> Result<Mo2ImportResultDto, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("Donne un nom à l'instance.".into());
    }
    if name.chars().count() > 100 {
        return Err("Le nom de l'instance ne peut pas dépasser 100 caractères.".into());
    }
    let core = state.core.lock().map_err(|e| e.to_string())?;
    let (created, imported) = modlab_game_skyrim::mo2::import_into(
        &core,
        Path::new(&profile),
        Path::new(&mods_dir),
        name,
    )?;
    Ok(Mo2ImportResultDto {
        loadout: LoadoutDto {
            id: created.id,
            game_id: SkyrimAdapter.id().to_string(),
            name: created.name,
            mod_count: i64::try_from(imported).unwrap_or(i64::MAX),
        },
        imported,
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
            mo2_import,
            launch_skse
        ])
        .run(tauri::generate_context!())
        .expect("error while running ModLab");
}

#[cfg(test)]
mod tests {
    use super::*;
    use modlab_core::loadout::ImportEntry;
    use std::fs;

    /// This crate has no `tempfile` dependency, and adding one just for a test
    /// would be a `Cargo.toml` change nobody asked for — so the scratch folder
    /// is made by hand and removed by the test that made it.
    fn scratch(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("modlab-ui-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// Two mods sharing `interface/config.txt`, plus a third that is disabled
    /// and also provides it. The scan must badge the enabled pair and never
    /// look at the disabled one.
    #[test]
    fn conflicts_dto_tallies_enabled_mods_and_leaves_out_disabled_ones() {
        let core = Core::open_in_memory().unwrap();
        let loadout = loadout::create(&core, "skyrimse", "Test").unwrap();

        let mut folders = Vec::new();
        for (tag, body) in [("losing", "a"), ("winning", "b"), ("disabled", "c")] {
            let dir = scratch(tag);
            fs::create_dir_all(dir.join("interface")).unwrap();
            fs::write(dir.join("interface/config.txt"), body).unwrap();
            folders.push(dir);
        }
        let [losing, winning, disabled] = folders.as_slice() else {
            unreachable!("the loop above always pushes three folders")
        };

        let imported = loadout::import_mods(
            &core,
            loadout.id,
            &[
                ImportEntry {
                    name: "Lux".into(),
                    source_path: losing.clone(),
                    enabled: true,
                },
                ImportEntry {
                    name: "Disabled Overhaul".into(),
                    source_path: disabled.clone(),
                    enabled: false,
                },
                ImportEntry {
                    name: "SkyUI".into(),
                    source_path: winning.clone(),
                    enabled: true,
                },
            ],
        )
        .unwrap();
        assert_eq!(imported, 3);

        let dto = loadout_conflicts(&core, loadout.id).unwrap();

        assert_eq!(dto.mods.len(), 2, "only enabled mods take part in a scan");
        let lux = dto.mods.iter().find(|m| m.name == "Lux").unwrap();
        let skyui = dto.mods.iter().find(|m| m.name == "SkyUI").unwrap();
        assert_eq!((lux.wins, lux.loses), (0, 1));
        assert_eq!((skyui.wins, skyui.loses), (1, 0));
        assert!(dto.unreadable.is_empty());

        for dir in folders {
            let _ = fs::remove_dir_all(dir);
        }
    }

    /// A mod folder that no longer exists must not swallow the whole answer:
    /// the readable mods are still reported, and the failure is named.
    #[test]
    fn conflicts_dto_reports_a_missing_folder_as_unreadable() {
        let core = Core::open_in_memory().unwrap();
        let loadout = loadout::create(&core, "skyrimse", "Test").unwrap();

        let present = scratch("present");
        fs::write(present.join("readme.txt"), b"x").unwrap();
        let gone = present.join("this-folder-was-renamed");

        loadout::import_mods(
            &core,
            loadout.id,
            &[
                ImportEntry {
                    name: "Present".into(),
                    source_path: present.clone(),
                    enabled: true,
                },
                ImportEntry {
                    name: "Renamed Away".into(),
                    source_path: gone,
                    enabled: true,
                },
            ],
        )
        .unwrap();

        let dto = loadout_conflicts(&core, loadout.id).unwrap();

        assert_eq!(dto.mods.len(), 1, "the readable mod is still reported");
        assert_eq!(dto.mods[0].name, "Present");
        assert_eq!(dto.unreadable.len(), 1);
        assert!(
            dto.unreadable[0].contains("this-folder-was-renamed"),
            "the payload names the folder that failed: {}",
            dto.unreadable[0]
        );

        let _ = fs::remove_dir_all(present);
    }
}
