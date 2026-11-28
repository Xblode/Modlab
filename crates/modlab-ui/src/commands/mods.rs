#[allow(unused_imports)]
use crate::*;
#[allow(unused_imports)]
use modlab_ai::{diagnose, llm_phrase, rules_answer, LoadoutFacts, ModFact};
#[allow(unused_imports)]
use modlab_core::game_adapter::{
    fuse_path, DependencyMod, GameAdapter, ModDependencyReport, PathKind, ResolvedDependency,
    SlotPath, ToolClassifyQuery,
};
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

#[tauri::command]
pub(crate) fn set_mod_enabled(
    state: State<AppState>,
    loadout_id: i64,
    mod_id: i64,
    enabled: bool,
) -> Result<Vec<ModRowDto>, String> {
    let core = state.core.lock().map_err(|e| e.to_string())?;
    let game_id = loadout::list(&core)
        .map_err(|e| e.to_string())?
        .into_iter()
        .find(|l| l.id == loadout_id)
        .map(|l| l.game_id);
    if game_id.as_deref() == Some(MinecraftAdapter.id()) {
        // Renaming a jar under a running JVM fails on Windows; refuse early.
        if state.game_running.load(Ordering::SeqCst) {
            return Err(
                "Une partie est en cours : les mods ne peuvent pas être activés ou désactivés avant sa fin."
                    .into(),
            );
        }
        // Minecraft: `foo.jar` ↔ `foo.jar.disabled` in the instance mods/, then DB.
        modlab_game_minecraft::instance::set_mod_enabled(&core, loadout_id, mod_id, enabled)?;
    } else {
        // Disk under RAII guard; commit only after the Plan DB succeeds so a
        // failed set_enabled never leaves the install root out of sync.
        let mut guard = sync_engine_mod_direct_install(&core, &state, loadout_id, mod_id, enabled)?;
        loadout::set_enabled(&core, loadout_id, mod_id, enabled).map_err(|e| e.to_string())?;
        if let Some(g) = guard.as_mut() {
            g.commit();
        }
    }
    entries_dto(&core, loadout_id)
}

/// Change the priority order: move `mod_ids` so the block lands at index `to`
/// of the list (the last mod wins conflicts). Transactional in the Core.
#[tauri::command]
pub(crate) fn move_mods(
    state: State<AppState>,
    loadout_id: i64,
    mod_ids: Vec<i64>,
    to: usize,
) -> Result<Vec<ModRowDto>, String> {
    let core = state.core.lock().map_err(|e| e.to_string())?;
    loadout::move_mods(&core, loadout_id, &mod_ids, to).map_err(|e| e.to_string())?;
    entries_dto(&core, loadout_id)
}

/// The scan itself, split out from the command so it can be tested against a
/// real (in-memory) Core plus real folders — the browser preview only ever
/// exercises the UI's handling of the payload, never this path.
fn conflicts_dto(rows: &[loadout::LoadoutEntryRow]) -> LoadoutConflictsDto {
    let report = conflicts::report_for_entries(&virtually_deployed_rows(rows));
    let count = |n: usize| i64::try_from(n).unwrap_or(i64::MAX);
    LoadoutConflictsDto {
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
    }
}

#[cfg(test)]
fn loadout_conflicts(core: &Core, loadout_id: i64) -> Result<LoadoutConflictsDto, String> {
    let rows = loadout::entries(core, loadout_id).map_err(|e| e.to_string())?;
    Ok(conflicts_dto(&rows))
}

/// File walk of every enabled mod. The database lock is dropped before the
/// walk, and the walk itself runs off the UI thread.
#[tauri::command]
pub(crate) async fn get_loadout_conflicts(
    app: tauri::AppHandle,
    loadout_id: i64,
) -> Result<LoadoutConflictsDto, String> {
    off_ui(move || {
        let state = app.state::<AppState>();
        let rows = {
            let core = state.core.lock().map_err(|e| e.to_string())?;
            loadout::entries(&core, loadout_id).map_err(|e| e.to_string())?
        };
        Ok(conflicts_dto(&rows))
    })
    .await
}

struct DependencyScan {
    game_id: Option<String>,
    rows: Vec<loadout::LoadoutEntryRow>,
    /// `(mod id, url, file name)` for `classify_tool`. The database lock is
    /// not held while those files are read.
    captures: Vec<(Option<i64>, String, String)>,
    overrides: Vec<(String, String)>,
}

fn dependency_scan(core: &Core, loadout_id: i64) -> Result<DependencyScan, String> {
    let game_id = loadout::list(core)
        .map_err(|e| e.to_string())?
        .into_iter()
        .find(|branch| branch.id == loadout_id)
        .map(|branch| branch.game_id);
    let rows = loadout::entries(core, loadout_id).map_err(|e| e.to_string())?;
    let captures = download::list(core)
        .map_err(|e| e.to_string())?
        .into_iter()
        .map(|item| (item.mod_id, item.url, item.file_name))
        .collect();
    let overrides = match &game_id {
        Some(id) => core.game_path_overrides(id).unwrap_or_default(),
        None => Vec::new(),
    };
    Ok(DependencyScan {
        game_id,
        rows,
        captures,
        overrides,
    })
}

fn reports_from_scan(scan: DependencyScan) -> Vec<ModDependencyReport> {
    let Some(game_id) = scan.game_id else {
        return Vec::new();
    };
    let Some(adapter) = adapters()
        .into_iter()
        .find(|adapter| adapter.id() == game_id)
    else {
        return Vec::new();
    };
    let mods = scan
        .rows
        .iter()
        .map(|row| {
            let capture = scan
                .captures
                .iter()
                .find(|(mod_id, _, _)| *mod_id == Some(row.mod_id));
            let mark = adapter.classify_tool(&ToolClassifyQuery {
                name: &row.name,
                source_path: &row.source_path,
                archive_path: row.archive_path.as_deref(),
                stored_version: row.version.as_deref(),
                source_url: capture.map(|(_, url, _)| url.as_str()).unwrap_or(""),
                download_file_name: capture.map(|(_, _, name)| name.as_str()).unwrap_or(""),
            });
            DependencyMod {
                mod_id: row.mod_id,
                name: row.name.clone(),
                source_path: row.source_path.clone(),
                archive_path: row.archive_path.clone(),
                enabled: row.enabled,
                position: row.position,
                tool: mark.map(|found| found.tool),
                dependencies: row.dependencies.clone(),
            }
        })
        .collect::<Vec<_>>();
    let paths = adapter
        .path_slots()
        .into_iter()
        .filter_map(|slot| {
            let override_path = scan
                .overrides
                .iter()
                .find(|(id, _)| id == slot.id)
                .map(|(_, path)| PathBuf::from(path));
            let path = fuse_path(slot.detected, override_path).effective?;
            Some(SlotPath {
                id: slot.id.to_string(),
                path,
            })
        })
        .collect::<Vec<_>>();
    adapter.resolve_dependencies(&mods, &paths)
}

async fn dependency_reports(
    app: tauri::AppHandle,
    loadout_id: i64,
) -> Result<Vec<ModDependencyReport>, String> {
    off_ui(move || {
        let state = app.state::<AppState>();
        let scan = {
            let core = state.core.lock().map_err(|e| e.to_string())?;
            dependency_scan(&core, loadout_id)?
        };
        Ok(reports_from_scan(scan))
    })
    .await
}

/// Dependencies of the active branch, from that game's adapter.
/// Read-only. Recalculated when a mod is enabled or disabled, same as conflicts.
/// An empty list means the adapter found nothing to report — not that the
/// scan was skipped.
#[tauri::command]
pub(crate) async fn get_loadout_dependencies(
    app: tauri::AppHandle,
    loadout_id: i64,
) -> Result<Vec<ModDependencyReport>, String> {
    dependency_reports(app, loadout_id).await
}

/// Dependencies of one mod. Same computation as [`get_loadout_dependencies`].
#[tauri::command]
pub(crate) async fn mod_dependencies(
    app: tauri::AppHandle,
    loadout_id: i64,
    mod_id: i64,
) -> Result<Vec<ResolvedDependency>, String> {
    let reports = dependency_reports(app, loadout_id).await?;
    Ok(reports
        .into_iter()
        .find(|report| report.mod_id == mod_id)
        .map(|report| report.dependencies)
        .unwrap_or_default())
}

/// One config file that belongs to the selected mod. `text` is the file
/// body when it is small UTF-8; otherwise the file is listed without a body.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModConfigFileDto {
    pub name: String,
    pub relative_path: String,
    pub text: Option<String>,
}

/// Directory whose files this mod actually placed.
///
/// `source_path` wins when it is a directory: that is the tree the install
/// recorded (an archive extracted into `mods/<id>`, or an MO2 folder left
/// where it was). A path that is not a directory — typically the archive
/// itself, before extraction — falls back to `extracted_tree` (`mods/<id>`)
/// when that folder exists. There is no per-file manifest in the database,
/// and deployment does not copy these files into the game directory, so
/// neither the game `Data/` folder nor a deployed view is scanned.
fn resolve_mod_tree(source_path: &Path, extracted_tree: &Path) -> Option<PathBuf> {
    if source_path.is_dir() {
        Some(source_path.to_path_buf())
    } else if extracted_tree.is_dir() {
        Some(extracted_tree.to_path_buf())
    } else {
        None
    }
}

/// Config files of one mod, read from its own folder. Read-only.
/// An unknown mod, an archive not yet extracted, or a folder with no
/// config file returns an empty list — not an error.
/// The folder walk runs off the UI thread, after the database lock is dropped.
#[tauri::command]
pub(crate) async fn mod_config_files(
    app: tauri::AppHandle,
    loadout_id: i64,
    mod_id: i64,
) -> Result<Vec<ModConfigFileDto>, String> {
    off_ui(move || {
        let state = app.state::<AppState>();
        let (source, extracted) = {
            let core = state.core.lock().map_err(|e| e.to_string())?;
            let rows = loadout::entries(&core, loadout_id).map_err(|e| e.to_string())?;
            let Some(row) = rows.iter().find(|r| r.mod_id == mod_id) else {
                return Ok(Vec::new());
            };
            (
                row.source_path.clone(),
                state.data_root.join("mods").join(mod_id.to_string()),
            )
        };
        let Some(root) = resolve_mod_tree(&source, &extracted) else {
            return Ok(Vec::new());
        };
        let files = modlab_core::mod_config::list_mod_config_files(&root)
            .map_err(|e| format!("lecture des fichiers de config : {e}"))?;
        Ok(files
            .into_iter()
            .map(|file| ModConfigFileDto {
                name: file.name,
                relative_path: file.relative_path,
                text: file.text,
            })
            .collect())
    })
    .await
}

/// One resolved path in the merged view (Répertoire tab), or one plugin/
/// archive file within it (Plugins/Archives tabs, filtered by extension).
/// The Core's own "what the game would see in Data/" answer — computed the
/// same way the conflict scan already is, just reporting every path
/// instead of only the contested ones (`modlab_core::conflicts::merged_view`).
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DirectoryEntryDto {
    pub relative_path: String,
    pub mod_id: i64,
    pub mod_name: String,
    pub contributor_count: i64,
}

fn merged_view_dto(core: &Core, loadout_id: i64) -> Result<Vec<DirectoryEntryDto>, String> {
    let rows = loadout::entries(core, loadout_id).map_err(|e| e.to_string())?;
    let mods: Vec<conflicts::ModFiles> = virtually_deployed_rows(&rows)
        .iter()
        .filter(|r| r.enabled)
        .filter_map(|r| {
            conflicts::list_files_relative(&r.source_path)
                .ok()
                .map(|files| conflicts::ModFiles {
                    mod_id: r.mod_id,
                    name: r.name.clone(),
                    files,
                })
        })
        .collect();
    Ok(conflicts::merged_view(&mods)
        .into_iter()
        .map(|f| DirectoryEntryDto {
            relative_path: f.relative_path.display().to_string(),
            mod_id: f.winner_mod_id,
            mod_name: f.winner_mod_name,
            contributor_count: i64::try_from(f.contributor_count).unwrap_or(i64::MAX),
        })
        .collect())
}

fn has_extension(path: &str, extensions: &[&str]) -> bool {
    Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| extensions.iter().any(|want| e.eq_ignore_ascii_case(want)))
}

/// The Répertoire tab: every path the enabled mods provide, resolved to
/// whichever one wins — logically "what Data/ looks like", without needing
/// a live USVFS session to ask the real one. Read-only (Lecture tier, §7.2).
#[tauri::command]
pub(crate) fn loadout_directory(
    state: State<AppState>,
    loadout_id: i64,
) -> Result<Vec<DirectoryEntryDto>, String> {
    let core = state.core.lock().map_err(|e| e.to_string())?;
    merged_view_dto(&core, loadout_id)
}

/// One row of the Plugins tab: a plugin at its real place in the load order.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginRowDto {
    pub file_name: String,
    pub official: bool,
    /// `None` for a plugin that comes from the game's own `Data/` folder.
    pub mod_id: Option<i64>,
    pub mod_name: Option<String>,
    pub contributor_count: usize,
    pub master: bool,
    pub light: bool,
    pub active: bool,
    pub missing_masters: Vec<String>,
    pub readable: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginIssueDto {
    /// Stable code (`IssueKind::code`); the UI words it in French.
    pub kind: &'static str,
    pub plugin: String,
    pub detail: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginsViewDto {
    pub plugins: Vec<PluginRowDto>,
    pub issues: Vec<PluginIssueDto>,
    pub active_full: usize,
    pub active_light: usize,
    pub full_limit: usize,
    pub light_limit: usize,
}

/// What the real game install contributes to the load order, honouring the
/// Réglages → Jeux overrides like every other path lookup.
fn skyrim_game_context(core: &Core) -> plugins::GameContext {
    plugins::GameContext {
        install_root: resolve_slot(core, &SkyrimAdapter, "root"),
        data_dir: resolve_slot(core, &SkyrimAdapter, "data_dir"),
    }
}

fn plugins_view_dto(order: &plugins::LoadOrder) -> PluginsViewDto {
    let (active_full, active_light) = order.active_counts();
    PluginsViewDto {
        plugins: order
            .entries
            .iter()
            .map(|e| {
                let (mod_id, mod_name) = match &e.source {
                    plugins::PluginSource::Mod { mod_id, mod_name } => {
                        (Some(*mod_id), Some(mod_name.clone()))
                    }
                    plugins::PluginSource::GameData => (None, None),
                };
                PluginRowDto {
                    file_name: e.file_name.clone(),
                    official: e.official,
                    mod_id,
                    mod_name,
                    contributor_count: e.contributors,
                    master: e.master,
                    light: e.light,
                    active: e.active,
                    missing_masters: e.missing_masters.clone(),
                    readable: e.readable,
                }
            })
            .collect(),
        issues: order
            .issues
            .iter()
            .map(|i| PluginIssueDto {
                kind: i.kind.code(),
                plugin: i.plugin.clone(),
                detail: i.detail.clone(),
            })
            .collect(),
        active_full,
        active_light,
        full_limit: plugins::MAX_FULL_PLUGINS,
        light_limit: plugins::MAX_LIGHT_PLUGINS,
    }
}

/// The Plugins tab: the load order ModLab will hand the game at launch —
/// computed from the enabled mods' plugins (winner of each conflict), with
/// masters first and missing masters reported. Read-only (Lecture tier,
/// §7.2): nothing is written here, `launch_skse` writes `plugins.txt`.
#[tauri::command]
pub(crate) fn loadout_plugins(
    state: State<AppState>,
    loadout_id: i64,
) -> Result<PluginsViewDto, String> {
    let core = state.core.lock().map_err(|e| e.to_string())?;
    let rows = loadout::entries(&core, loadout_id).map_err(|e| e.to_string())?;
    let order =
        plugins::compute_load_order(&virtually_deployed_rows(&rows), &skyrim_game_context(&core));
    Ok(plugins_view_dto(&order))
}

/// The Archives tab: the same merged view, filtered to `.bsa`/`.ba2`. Which
/// files exist and which mod provides each — not an inventory of what's
/// packed inside them, and not an integrity check (BSA/BA2 parsing doesn't
/// exist here yet, see `StubTab`'s former copy for this tab).
#[tauri::command]
pub(crate) fn loadout_archives(
    state: State<AppState>,
    loadout_id: i64,
) -> Result<Vec<DirectoryEntryDto>, String> {
    let core = state.core.lock().map_err(|e| e.to_string())?;
    Ok(merged_view_dto(&core, loadout_id)?
        .into_iter()
        .filter(|e| has_extension(&e.relative_path, &["bsa", "ba2"]))
        .collect())
}

/// One save file, as found on disk — the Sauvegardes tab. `revision` is
/// always `None` today: the Core has no way yet to know which loadout
/// revision was active when a given save was written (see `StubTab`'s
/// former copy for this tab) — this only ever lists what's on disk.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveFileDto {
    pub file_name: String,
    pub size_bytes: u64,
    /// Unix seconds — no `chrono` dependency just for this, the client
    /// formats it for display.
    pub modified_at: Option<i64>,
}

/// Read-only (Lecture tier, §7.2). An empty list — missing saves folder or
/// none written yet — is not an error, same rule `minecraft_content` follows.
#[tauri::command]
pub(crate) fn skyrim_saves(state: State<AppState>) -> Result<Vec<SaveFileDto>, String> {
    let core = state.core.lock().map_err(|e| e.to_string())?;
    let Some(dir) = resolve_slot(&core, &SkyrimAdapter, "saves_dir") else {
        return Ok(Vec::new());
    };
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return Ok(Vec::new());
    };
    let mut saves: Vec<SaveFileDto> = entries
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .extension()
                .and_then(|x| x.to_str())
                .is_some_and(|x| x.eq_ignore_ascii_case("ess"))
        })
        .filter_map(|e| {
            let meta = e.metadata().ok()?;
            let modified_at = meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX));
            Some(SaveFileDto {
                file_name: e.file_name().to_string_lossy().into_owned(),
                size_bytes: meta.len(),
                modified_at,
            })
        })
        .collect();
    saves.sort_by(|a, b| b.file_name.cmp(&a.file_name));
    Ok(saves)
}

#[tauri::command]
pub(crate) fn set_mod_category(
    state: State<AppState>,
    mod_id: i64,
    category: Option<String>,
) -> Result<(), String> {
    let core = state.core.lock().map_err(|e| e.to_string())?;
    let cleaned = category.as_deref().map(str::trim).filter(|s| !s.is_empty());
    let patch = match cleaned {
        Some(value) => modlab_core::mod_meta::Patch {
            category: Some(value.to_string()),
            ..modlab_core::mod_meta::Patch::default()
        },
        None => modlab_core::mod_meta::Patch {
            clear_category: true,
            ..modlab_core::mod_meta::Patch::default()
        },
    };
    // `user` is the top provenance layer: a later automatic fill cannot replace it.
    modlab_core::mod_meta::apply_layer(
        &core,
        mod_id,
        modlab_core::mod_meta::Layer::User,
        &patch,
        &[],
        false,
    )
    .map_err(|e| e.to_string())
}

/// The active game's fixed category list. The shell does not keep a copy.
#[tauri::command]
pub(crate) fn game_categories(
    game_id: String,
) -> Result<Vec<modlab_core::game_adapter::GameCategory>, String> {
    Ok(crate::adapters()
        .into_iter()
        .find(|adapter| adapter.id() == game_id)
        .map(|adapter| adapter.categories().to_vec())
        .unwrap_or_default())
}

#[tauri::command]
pub(crate) fn rename_mod(
    state: State<AppState>,
    mod_id: i64,
    loadout_id: i64,
    name: String,
) -> Result<Vec<ModRowDto>, String> {
    let core = state.core.lock().map_err(|e| e.to_string())?;
    download::rename(&core, mod_id, &name).map_err(|e| e.to_string())?;
    entries_dto(&core, loadout_id)
}

/// Replay filename and local archive metadata on one mod. Info calls this
/// when it opens, so a mod filed under its download name picks up the
/// parsed title without a separate button.
#[tauri::command]
pub(crate) fn refresh_mod_metadata(
    state: State<AppState>,
    mod_id: i64,
    loadout_id: i64,
) -> Result<Vec<ModRowDto>, String> {
    let core = state.core.lock().map_err(|e| e.to_string())?;
    crate::refresh_mod_metadata(&core, loadout_id, mod_id)?;
    entries_dto(&core, loadout_id)
}

/// Soft-delete the mod from every loadout table. Minecraft also moves the
/// jar out of the instance `mods/` (`.jar` or `.jar.disabled`) into the
/// data-root trash. Skyrim stays soft-delete-only — managed folders are not
/// deleted here (USVFS / internal trash on uninstall is a separate path).
#[tauri::command]
pub(crate) fn delete_mod(
    state: State<AppState>,
    mod_id: i64,
    loadout_id: i64,
) -> Result<Vec<ModRowDto>, String> {
    let core = state.core.lock().map_err(|e| e.to_string())?;
    let game_id = loadout::list(&core)
        .map_err(|e| e.to_string())?
        .into_iter()
        .find(|l| l.id == loadout_id)
        .map(|l| l.game_id);
    if game_id.as_deref() == Some(MinecraftAdapter.id()) {
        if state.game_running.load(Ordering::SeqCst) {
            return Err(
                "Une partie est en cours : les mods ne peuvent pas être supprimés avant sa fin."
                    .into(),
            );
        }
        modlab_game_minecraft::instance::remove_mod(&core, &state.data_root, loadout_id, mod_id)?;
    } else {
        // Disk under guard; commit only after the Plan DB soft-delete succeeds.
        let mut guard = sync_engine_mod_direct_install(&core, &state, loadout_id, mod_id, false)?;
        download::remove(&core, mod_id).map_err(|e| e.to_string())?;
        if let Some(g) = guard.as_mut() {
            g.commit();
        }
    }
    entries_dto(&core, loadout_id)
}

/// Revert an installed mod to its archive. The extracted copy is moved to
/// the data root's trash — it is not deleted. The archive itself is never
/// touched. If the database update fails, the folder is put back.
/// Runs off the UI thread. The database lock is held for the whole call:
/// the trash rename and the transaction roll back together, and a directory
/// rename on the same volume is short.
#[tauri::command]
pub(crate) async fn uninstall_mod(
    app: tauri::AppHandle,
    mod_id: i64,
    loadout_id: i64,
) -> Result<Vec<ModRowDto>, String> {
    off_ui(move || {
        let state = app.state::<AppState>();
        let _busy = state.claim_mod(mod_id)?;
        let core = state.core.lock().map_err(|e| e.to_string())?;
        // Before the mod's folder goes: the engine's root files are taken out of
        // the game's install root while their source still exists. Commit the
        // engine guard only after the Core uninstall Plan succeeds.
        let mut engine_guard =
            sync_engine_mod_direct_install(&core, &state, loadout_id, mod_id, false)?;
        let source = loadout::entries(&core, loadout_id)
            .map_err(|e| e.to_string())?
            .into_iter()
            .find(|row| row.mod_id == mod_id)
            .map(|row| row.source_path);
        let placed = source.as_ref().and_then(|source| {
            crate::adapter_for_loadout(&core, loadout_id)
                .ok()
                .flatten()
                .and_then(|adapter| adapter.retract_files(&state.data_root, mod_id, source))
        });
        let mut guards = Vec::new();
        if let Some(paths) = placed {
            for path in paths {
                if let Some(guard) = modlab_core::files::trash_for_rollback(&state.data_root, &path)
                    .map_err(|e| e.to_string())?
                {
                    guards.push(guard);
                }
            }
        }
        let uninstalled = modlab_core::install::uninstall_mod(
            &core,
            &state.data_root,
            loadout_id,
            mod_id,
            &mod_dest_dir(&state, mod_id),
        );
        if let Err(err) = uninstalled {
            drop(guards);
            // engine_guard drops without commit → restores SKSE root files
            return Err(err.to_string());
        }
        if let Some(g) = engine_guard.as_mut() {
            g.commit();
        }
        for guard in guards {
            guard.commit();
        }
        entries_dto(&core, loadout_id)
    })
    .await
}

/// Move an archived mod (not-yet-extracted only) to a different instance.
#[tauri::command]
pub(crate) fn reassign_mod(
    state: State<AppState>,
    mod_id: i64,
    from_loadout_id: i64,
    to_loadout_id: i64,
) -> Result<Vec<ModRowDto>, String> {
    let core = state.core.lock().map_err(|e| e.to_string())?;
    download::reassign(&core, mod_id, from_loadout_id, to_loadout_id).map_err(|e| e.to_string())?;
    entries_dto(&core, from_loadout_id)
}

#[tauri::command]
pub(crate) fn mod_categories(state: State<AppState>) -> Result<Vec<String>, String> {
    let core = state.core.lock().map_err(|e| e.to_string())?;
    download::categories(&core).map_err(|e| e.to_string())
}

/// What an archive turns out to contain, once extracted to staging — the
/// shell needs this to know whether it can install silently or must run the
/// FOMOD wizard first.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArchivePlanDto {
    pub mod_id: i64,
    pub name: String,
    /// `"fomod"`, `"bare-data"`, `"wrapped-data"`, `"engine-root"` or `"unknown"`.
    pub shape: String,
    /// True only for a FOMOD: nothing lands in the loadout until the human
    /// has answered the installer's own questions.
    pub needs_wizard: bool,
    /// Where the archive was unpacked, and where inside that the real
    /// content starts (an archive often wraps everything in one folder).
    pub staged_path: String,
    pub content_root: String,
    /// French sentence when this game cannot install the file. The shell
    /// shows it and does not continue.
    pub refusal: Option<String>,
}

/// Unpack an archived mod into the data root's staging area and classify it.
/// Stops there on purpose: extracting is reversible and touches nothing the
/// game can see, while *installing* is the step that needs a decision.
/// The archive lookup takes the database lock; extraction does not.
#[tauri::command]
pub(crate) async fn inspect_archived_mod(
    app: tauri::AppHandle,
    mod_id: i64,
) -> Result<ArchivePlanDto, String> {
    off_ui(move || {
        let state = app.state::<AppState>();
        let _busy = state.claim_mod(mod_id)?;
        let (pending, adapter) = {
            let core = state.core.lock().map_err(|e| e.to_string())?;
            let pending =
                modlab_core::download::archived_mod(&core, mod_id).map_err(|e| e.to_string())?;
            let adapter = adapter_for_mod(&core, mod_id)?;
            (pending, adapter)
        };
        let staged = adapter.stage_archive(
            &state.data_root,
            mod_id,
            &modlab_core::game_adapter::ArchiveSource {
                name: &pending.name,
                archive_path: &pending.archive_path,
            },
        )?;
        Ok(ArchivePlanDto {
            mod_id,
            name: staged.name,
            shape: staged.shape,
            needs_wizard: staged.needs_wizard,
            staged_path: staged.staged_path.display().to_string(),
            content_root: staged.content_root.display().to_string(),
            refusal: staged.refusal,
        })
    })
    .await
}

fn mod_dest_dir(state: &AppState, mod_id: i64) -> PathBuf {
    state.data_root.join("mods").join(mod_id.to_string())
}

fn adapter_for_mod(
    core: &Core,
    mod_id: i64,
) -> Result<&'static dyn modlab_core::game_adapter::GameAdapter, String> {
    let loadout_id = core
        .loadout_id_for_mod(mod_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "Ce mod n'est dans aucune instance.".to_string())?;
    crate::adapter_for_loadout(core, loadout_id)?
        .ok_or_else(|| "Ce jeu ne sait pas installer une archive.".to_string())
}

fn commit_prepared(
    core: &Core,
    data_root: &Path,
    loadout_id: i64,
    mod_id: i64,
    prepared: modlab_core::game_adapter::PreparedMod,
) -> Result<(), String> {
    use modlab_core::game_adapter::PreparedMod;
    match prepared {
        PreparedMod::Directory {
            dest,
            swap,
            adjustments,
        } => modlab_core::install::commit_prepared_install(
            core,
            loadout_id,
            mod_id,
            &dest,
            adjustments,
            swap,
        )
        .map_err(|e| e.to_string()),
        PreparedMod::Files {
            primary,
            placed,
            adjustments,
        } => {
            let committed = modlab_core::install::commit_placed_path(
                core,
                loadout_id,
                mod_id,
                &primary,
                adjustments,
            );
            if let Err(err) = committed {
                for path in &placed {
                    if path.exists() {
                        let _ = modlab_core::files::move_to_trash(data_root, path);
                    }
                }
                return Err(err.to_string());
            }
            Ok(())
        }
    }
}

/// The mod must still be an archived row of this loadout. Called again
/// after the file copy, which runs without the database lock.
fn require_pending_archive(core: &Core, loadout_id: i64, mod_id: i64) -> Result<(), String> {
    let rows = loadout::entries(core, loadout_id).map_err(|e| e.to_string())?;
    if rows
        .iter()
        .any(|row| row.mod_id == mod_id && row.status == loadout::STATUS_ARCHIVED)
    {
        Ok(())
    } else {
        Err(
            "Ce mod n'est plus une archive en attente dans cette instance — installation annulée."
                .into(),
        )
    }
}

/// Finish the install of an archive that does not need a wizard.
///
/// The copy runs without the database lock. The lock is taken again for
/// plan → apply → commit. A directory install rolls back through `DirSwap`;
/// a file install moves the new files to the trash. Two installs of
/// different mods may copy at the same time; a second install of this mod
/// is refused.
#[tauri::command]
pub(crate) async fn install_archived_mod(
    app: tauri::AppHandle,
    mod_id: i64,
    loadout_id: i64,
    content_root: String,
) -> Result<Vec<ModRowDto>, String> {
    off_ui(move || {
        let state = app.state::<AppState>();
        let _busy = state.claim_mod(mod_id)?;
        let prepared = prepare_for(&state, mod_id, loadout_id, &content_root, &[], false)?;
        let rows = finish_install(&state, mod_id, loadout_id, prepared)?;
        Ok(rows)
    })
    .await
}

fn prepare_for(
    state: &AppState,
    mod_id: i64,
    loadout_id: i64,
    content_root: &str,
    picked: &[String],
    guided: bool,
) -> Result<modlab_core::game_adapter::PreparedMod, String> {
    let content_root = PathBuf::from(content_root);
    let dest = mod_dest_dir(state, mod_id);
    let (adapter, pending, target, snapshot, version) = {
        let core = state.core.lock().map_err(|e| e.to_string())?;
        let pending =
            modlab_core::download::archived_mod(&core, mod_id).map_err(|e| e.to_string())?;
        let adapter = crate::adapter_for_loadout(&core, loadout_id)?
            .ok_or_else(|| "Ce jeu ne sait pas installer une archive.".to_string())?;
        let target = crate::paths::install_target_for_loadout(&core, &state.data_root, loadout_id)?;
        let (snapshot, version) = if guided {
            let pair = fomod_branch(&core, loadout_id)?;
            (Some(pair.0), pair.1)
        } else {
            (None, None)
        };
        (adapter, pending, target, snapshot, version)
    };
    let branch_file = |file: &str| match &snapshot {
        Some(snapshot) => branch_file_state(snapshot, file),
        None => modlab_core::game_adapter::BranchFileState::Missing,
    };
    let job = modlab_core::game_adapter::PrepareInstall {
        data_root: &state.data_root,
        mod_id,
        name: &pending.name,
        archive_path: &pending.archive_path,
        content_root: &content_root,
        dest: &dest,
        instance_root: target.as_ref().map(|target| target.instance_root.as_path()),
        mods_dir: target.as_ref().map(|target| target.mods_dir.as_path()),
        resource_packs: target
            .as_ref()
            .map(|target| target.resource_packs.as_path()),
        shader_packs: target.as_ref().map(|target| target.shader_packs.as_path()),
        picked,
        game_version: version.as_deref(),
        branch_file: &branch_file,
    };
    if guided {
        adapter.prepare_guided(&job)
    } else {
        adapter.prepare_install(&job)
    }
}

fn abandon_prepared(data_root: &Path, prepared: modlab_core::game_adapter::PreparedMod) {
    use modlab_core::game_adapter::PreparedMod;
    match prepared {
        PreparedMod::Directory { swap, .. } => drop(swap),
        PreparedMod::Files { placed, .. } => {
            for path in placed {
                if path.exists() {
                    let _ = modlab_core::files::move_to_trash(data_root, &path);
                }
            }
        }
    }
}

fn finish_install(
    state: &AppState,
    mod_id: i64,
    loadout_id: i64,
    prepared: modlab_core::game_adapter::PreparedMod,
) -> Result<Vec<ModRowDto>, String> {
    let rows = {
        let core = state.core.lock().map_err(|e| e.to_string())?;
        if let Err(err) = require_pending_archive(&core, loadout_id, mod_id) {
            abandon_prepared(&state.data_root, prepared);
            return Err(err);
        }
        commit_prepared(&core, &state.data_root, loadout_id, mod_id, prepared)?;
        crate::refresh_mod_metadata_after_install(&core, mod_id)?;
        entries_dto(&core, loadout_id)?
    };
    if let Some(adapter) = {
        let core = state.core.lock().map_err(|e| e.to_string())?;
        crate::adapter_for_loadout(&core, loadout_id)?
    } {
        adapter.discard_stage(&state.data_root, mod_id);
    }
    Ok(rows)
}

/// One screen of the FOMOD wizard, already settled against the branch:
/// plugin types, which steps are visible, and which options are locked.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FomodPluginDto {
    pub id: String,
    pub name: String,
    pub description: String,
    pub image: Option<String>,
    /// `"required"`, `"optional"`, `"recommended"`, `"not-usable"` or `"could-be-usable"`.
    pub type_name: String,
    pub selected: bool,
    pub locked: bool,
    pub reason: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FomodGroupDto {
    pub id: String,
    pub name: String,
    /// `"select-exactly-one"`, `"select-at-most-one"`, `"select-at-least-one"`,
    /// `"select-any"` or `"select-all"`.
    pub group_type: String,
    /// French sentence when the group is visible and its rule is not met.
    pub hint: Option<String>,
    pub plugins: Vec<FomodPluginDto>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FomodStepDto {
    pub id: String,
    pub name: String,
    pub visible: bool,
    pub groups: Vec<FomodGroupDto>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FomodImageDto {
    pub path: String,
    pub show: bool,
    pub height: Option<u32>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FomodModuleDto {
    pub name: String,
    pub image: Option<FomodImageDto>,
    pub steps: Vec<FomodStepDto>,
}

fn group_type_id(t: fomod::GroupType) -> &'static str {
    match t {
        fomod::GroupType::SelectExactlyOne => "select-exactly-one",
        fomod::GroupType::SelectAtMostOne => "select-at-most-one",
        fomod::GroupType::SelectAtLeastOne => "select-at-least-one",
        fomod::GroupType::SelectAny => "select-any",
        fomod::GroupType::SelectAll => "select-all",
    }
}

fn read_fomod_module(content_root: &str) -> Result<fomod::Module, String> {
    let root = Path::new(content_root);
    let config_path = fomod::find_module_config(root)
        .ok_or_else(|| "fomod/ModuleConfig.xml introuvable dans l'archive".to_string())?;
    fomod::read_module_config(&config_path)
}

/// Enabled mods and the game `Data/` folder, plus the executable version
/// when it can be read. The database lock is held by the caller.
fn fomod_branch(
    core: &Core,
    loadout_id: i64,
) -> Result<(fomod::BranchSnapshot, Option<String>), String> {
    let rows = loadout::entries(core, loadout_id).map_err(|e| e.to_string())?;
    let mut enabled = Vec::new();
    let mut disabled = Vec::new();
    for row in rows {
        if row.status != loadout::STATUS_INSTALLED {
            continue;
        }
        if row.enabled {
            enabled.push(row.source_path);
        } else {
            disabled.push(row.source_path);
        }
    }
    let data_dir = resolve_slot(core, &SkyrimAdapter, "data_dir");
    let version = super::launch::skyrim_install_paths(core)
        .and_then(|(_, exe)| modlab_game_skyrim::skse::read_exe_version(&exe))
        .map(|version| {
            format!(
                "{}.{}.{}.{}",
                version.major, version.minor, version.build, version.revision
            )
        });
    Ok((
        fomod::BranchSnapshot::new(data_dir, enabled, disabled),
        version,
    ))
}

fn branch_file_state(
    snapshot: &fomod::BranchSnapshot,
    file: &str,
) -> modlab_core::game_adapter::BranchFileState {
    match snapshot.state(file) {
        fomod::FileState::Active => modlab_core::game_adapter::BranchFileState::Active,
        fomod::FileState::Inactive => modlab_core::game_adapter::BranchFileState::Inactive,
        fomod::FileState::Missing => modlab_core::game_adapter::BranchFileState::Missing,
    }
}

fn module_dto(module: &fomod::Module, view: &fomod::View) -> FomodModuleDto {
    FomodModuleDto {
        name: module.name.clone(),
        image: module.image.as_ref().map(|image| FomodImageDto {
            path: image.path.clone(),
            show: image.show,
            height: image.height,
        }),
        steps: module
            .steps
            .iter()
            .map(|step| FomodStepDto {
                id: step.id.clone(),
                name: step.name.clone(),
                visible: view
                    .steps
                    .iter()
                    .find(|item| item.id == step.id)
                    .is_some_and(|item| item.visible),
                groups: step
                    .groups
                    .iter()
                    .map(|group| FomodGroupDto {
                        id: group.id.clone(),
                        name: group.name.clone(),
                        group_type: group_type_id(group.group_type).to_string(),
                        hint: view
                            .groups
                            .iter()
                            .find(|item| item.id == group.id)
                            .and_then(|item| item.hint.clone()),
                        plugins: group
                            .plugins
                            .iter()
                            .map(|plugin| {
                                let state = view.plugins.iter().find(|item| item.id == plugin.id);
                                FomodPluginDto {
                                    id: plugin.id.clone(),
                                    name: plugin.name.clone(),
                                    description: plugin.description.clone(),
                                    image: plugin.image.clone(),
                                    type_name: state
                                        .map(|item| item.type_name.as_str())
                                        .unwrap_or("optional")
                                        .to_string(),
                                    selected: state.is_some_and(|item| item.selected),
                                    locked: state.is_some_and(|item| item.locked),
                                    reason: state.and_then(|item| item.reason.clone()),
                                }
                            })
                            .collect(),
                    })
                    .collect(),
            })
            .collect(),
    }
}

fn present_fomod(
    content_root: &str,
    picked: &[String],
    snapshot: &fomod::BranchSnapshot,
    version: &Option<String>,
    mode: fomod::SelectionMode,
) -> Result<FomodModuleDto, String> {
    let module = read_fomod_module(content_root)?;
    let env = fomod::FomodEnv {
        file_state: &|file| snapshot.state(file),
        game_version: version.as_deref(),
    };
    let view = fomod::evaluate(&module, picked, &env, mode);
    Ok(module_dto(&module, &view))
}

fn fomod_context(
    state: &AppState,
    loadout_id: i64,
) -> Result<(fomod::BranchSnapshot, Option<String>), String> {
    let core = state.core.lock().map_err(|e| e.to_string())?;
    fomod_branch(&core, loadout_id)
}

/// What the wizard asks, for a `content_root` `inspect_archived_mod` already
/// reported as `"fomod"`. Recommended options start checked.
#[tauri::command]
pub(crate) async fn get_fomod_module(
    app: tauri::AppHandle,
    content_root: String,
    loadout_id: i64,
) -> Result<FomodModuleDto, String> {
    off_ui(move || {
        let state = app.state::<AppState>();
        let (snapshot, version) = fomod_context(&state, loadout_id)?;
        present_fomod(
            &content_root,
            &[],
            &snapshot,
            &version,
            fomod::SelectionMode::Defaults,
        )
    })
    .await
}

/// Same screen after the human changes a tick. `Recommended` is not put back.
#[tauri::command]
pub(crate) async fn preview_fomod(
    app: tauri::AppHandle,
    content_root: String,
    loadout_id: i64,
    picked: Vec<String>,
) -> Result<FomodModuleDto, String> {
    off_ui(move || {
        let state = app.state::<AppState>();
        let (snapshot, version) = fomod_context(&state, loadout_id)?;
        present_fomod(
            &content_root,
            &picked,
            &snapshot,
            &version,
            fomod::SelectionMode::Strict,
        )
    })
    .await
}

/// One image from the extracted archive, as a data URL. The path has to
/// resolve inside that archive, and the archive has to sit under staging.
#[tauri::command]
pub(crate) async fn fomod_image(
    app: tauri::AppHandle,
    content_root: String,
    path: String,
) -> Result<Option<String>, String> {
    off_ui(move || {
        let state = app.state::<AppState>();
        let root = PathBuf::from(&content_root);
        ensure_under_staging(&state.data_root, &root)?;
        match fomod::read_fomod_image(&root, &path) {
            Ok(bytes) => Ok(Some(fomod::image_data_url(&bytes, &path))),
            Err(err) if image_refusal(&err) => Err(err),
            Err(_) => Ok(None),
        }
    })
    .await
}

fn ensure_under_staging(data_root: &Path, content_root: &Path) -> Result<(), String> {
    let staging = fs_canonicalize(&data_root.join("staging"))
        .map_err(|_| "dossier de staging introuvable".to_string())?;
    let content =
        fs_canonicalize(content_root).map_err(|_| "archive extraite introuvable".to_string())?;
    if content.starts_with(&staging) {
        Ok(())
    } else {
        Err("chemin d'image refusé".to_string())
    }
}

fn fs_canonicalize(path: &Path) -> std::io::Result<PathBuf> {
    std::fs::canonicalize(path)
}

fn image_refusal(err: &str) -> bool {
    err.contains("refusé") || err.contains("non pris en charge") || err.contains("volumineuse")
}

/// Finish a FOMOD install: `picked` is every plugin id the wizard's answers
/// checked. Group rules are re-checked here — the wizard UI enforces them
/// too, but a real mutation never trusts the frontend alone for that.
/// Same lock split as `install_archived_mod`: the copy is outside the
/// database lock, the transaction is inside it.
#[tauri::command]
pub(crate) async fn install_fomod(
    app: tauri::AppHandle,
    mod_id: i64,
    loadout_id: i64,
    content_root: String,
    picked: Vec<String>,
) -> Result<Vec<ModRowDto>, String> {
    off_ui(move || {
        let state = app.state::<AppState>();
        let _busy = state.claim_mod(mod_id)?;
        let prepared = prepare_for(&state, mod_id, loadout_id, &content_root, &picked, true)?;
        finish_install(&state, mod_id, loadout_id, prepared)
    })
    .await
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

    #[test]
    fn resolve_mod_tree_uses_the_installed_folder_then_the_extracted_tree() {
        let root = scratch("mod-tree");
        let installed = root.join("installed");
        let extracted = root.join("mods").join("4");
        fs::create_dir_all(&installed).unwrap();
        fs::create_dir_all(&extracted).unwrap();
        assert_eq!(
            resolve_mod_tree(&installed, &extracted).as_deref(),
            Some(installed.as_path())
        );

        let archive = root.join("SkyUI.7z");
        fs::write(&archive, b"7z").unwrap();
        assert_eq!(
            resolve_mod_tree(&archive, &extracted).as_deref(),
            Some(extracted.as_path())
        );

        let missing = root.join("nope");
        let also_missing = root.join("mods").join("9");
        assert!(resolve_mod_tree(&missing, &also_missing).is_none());
        let _ = fs::remove_dir_all(&root);
    }

    /// A fake SKSE mod folder (loader beside `Data/`) and a fake "Address
    /// Library for SKSE Plugins" (plain Data mod), both imported enabled.
    fn skse_loadout(core: &Core, tag: &str) -> (i64, PathBuf, PathBuf) {
        let loadout = loadout::create(core, "skyrimse", "Test").unwrap();
        let skse = scratch(&format!("skse-engine-{tag}"));
        fs::write(skse.join("skse64_loader.exe"), b"fake loader").unwrap();
        fs::write(skse.join("skse64_1_7_104.dll"), b"fake runtime").unwrap();
        fs::create_dir_all(skse.join("Data/Scripts")).unwrap();
        fs::write(skse.join("Data/Scripts/Actor.pex"), b"fake pex").unwrap();
        let address_library = scratch(&format!("skse-address-library-{tag}"));
        fs::create_dir_all(address_library.join("SKSE/Plugins")).unwrap();
        fs::write(address_library.join("SKSE/Plugins/x.dll"), b"fake plugin").unwrap();
        loadout::import_mods(
            core,
            loadout.id,
            &[
                ImportEntry {
                    name: "SKSE64".into(),
                    source_path: skse.clone(),
                    enabled: false,
                    version: None,
                },
                ImportEntry {
                    name: "Address Library for SKSE Plugins".into(),
                    source_path: address_library.clone(),
                    enabled: true,
                    version: None,
                },
            ],
        )
        .unwrap();
        (loadout.id, skse, address_library)
    }

    fn test_state(core: Core, data_root: PathBuf) -> AppState {
        AppState {
            core: Mutex::new(core),
            data_root,
            game_running: AtomicBool::new(false),
            session_stop: AtomicBool::new(false),
            minecraft_pid: Mutex::new(None),
            active_session_loadout: Mutex::new(None),
            active_game: Mutex::new(None),
            active_loadout: Mutex::new(None),
            in_flight: Mutex::new(Vec::new()),
            browser_page: Mutex::new(None),
            pending_origins: Mutex::new(Vec::new()),
            last_session: Mutex::new(None),
            busy_mods: Mutex::new(std::collections::HashSet::new()),
            msa_pending: Mutex::new(None),
        }
    }

    #[test]
    fn the_same_mod_cannot_start_a_second_install() {
        let root = scratch("busy-mod");
        let core = Core::open_in_memory().unwrap();
        let state = test_state(core, root.clone());
        let first = state.claim_mod(4).unwrap();
        let err = match state.claim_mod(4) {
            Err(err) => err,
            Ok(_guard) => panic!("a second claim on the same mod must be refused"),
        };
        assert!(err.contains("en cours"), "{err}");
        let _other = state.claim_mod(5).unwrap();
        drop(first);
        state.claim_mod(4).unwrap();
        let _ = fs::remove_dir_all(root);
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
                    version: None,
                },
                ImportEntry {
                    name: "Disabled Overhaul".into(),
                    source_path: disabled.clone(),
                    enabled: false,
                    version: None,
                },
                ImportEntry {
                    name: "SkyUI".into(),
                    source_path: winning.clone(),
                    enabled: true,
                    version: None,
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
                    version: None,
                },
                ImportEntry {
                    name: "Renamed Away".into(),
                    source_path: gone,
                    enabled: true,
                    version: None,
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

    #[test]
    fn skse_named_mods_go_through_usvfs_and_only_the_engine_is_cut_to_its_data() {
        let core = Core::open_in_memory().unwrap();
        let (loadout_id, skse, address_library) = skse_loadout(&core, "virtual");
        let rows = loadout::entries(&core, loadout_id).unwrap();

        let deployed = virtually_deployed_rows(&rows);

        let by_name = |n: &str| deployed.iter().find(|r| r.name == n).unwrap();
        assert_eq!(deployed.len(), 2);
        assert_eq!(by_name("SKSE64").source_path, skse.join("Data"));
        assert_eq!(
            by_name("Address Library for SKSE Plugins").source_path,
            address_library
        );

        for dir in [skse, address_library] {
            let _ = fs::remove_dir_all(dir);
        }
    }

    #[test]
    fn ticking_the_engine_copies_only_its_root_files_and_unticking_takes_them_out() {
        let core = Core::open_in_memory().unwrap();
        let (loadout_id, skse, address_library) = skse_loadout(&core, "toggle");
        let game = scratch("skse-fake-game");
        let data_root = scratch("skse-data-root");
        core.set_game_path_override("skyrimse", "root", &game.display().to_string())
            .unwrap();
        let rows = loadout::entries(&core, loadout_id).unwrap();
        let engine_id = rows.iter().find(|r| r.name == "SKSE64").unwrap().mod_id;
        let plugin_id = rows
            .iter()
            .find(|r| r.name == "Address Library for SKSE Plugins")
            .unwrap()
            .mod_id;
        let state = test_state(core, data_root.clone());
        let core = state.core.lock().unwrap();

        // A mod merely named after SKSE never reaches the game folder.
        // Copying files and the enabled flag are two steps: the flag is what
        // the Mods list shows, and it is not implied by the copy.
        let plugin_guard =
            sync_engine_mod_direct_install(&core, &state, loadout_id, plugin_id, true).unwrap();
        assert!(plugin_guard.is_none());
        assert_eq!(fs::read_dir(&game).unwrap().count(), 0);
        assert!(loadout::set_enabled(&core, loadout_id, plugin_id, true).unwrap());
        assert!(
            loadout::entries(&core, loadout_id)
                .unwrap()
                .iter()
                .find(|r| r.mod_id == plugin_id)
                .unwrap()
                .enabled
        );

        let mut engine_guard =
            sync_engine_mod_direct_install(&core, &state, loadout_id, engine_id, true).unwrap();
        assert!(game.join("skse64_loader.exe").is_file());
        assert!(game.join("skse64_1_7_104.dll").is_file());
        assert!(!game.join("Data").exists());
        assert!(data_root.join("engine/skyrimse/engine-files.tsv").is_file());
        assert!(loadout::set_enabled(&core, loadout_id, engine_id, true).unwrap());
        if let Some(g) = engine_guard.as_mut() {
            g.commit();
        }
        let rows = loadout::entries(&core, loadout_id).unwrap();
        assert!(rows.iter().find(|r| r.mod_id == engine_id).unwrap().enabled);
        assert!(rows.iter().find(|r| r.mod_id == plugin_id).unwrap().enabled);

        let mut off_guard =
            sync_engine_mod_direct_install(&core, &state, loadout_id, engine_id, false).unwrap();
        assert!(!game.join("skse64_loader.exe").exists());
        assert!(skse.join("skse64_loader.exe").is_file());
        if let Some(g) = off_guard.as_mut() {
            g.commit();
        }

        drop(core);
        for dir in [skse, address_library, game, data_root] {
            let _ = fs::remove_dir_all(dir);
        }
    }

    #[test]
    fn engine_disk_rolls_back_when_plan_db_is_not_committed() {
        // Mirrors enable/uninstall order: disk under guard, commit only after
        // Plan OK. Dropping the guard without commit must restore the folder.
        let core = Core::open_in_memory().unwrap();
        let (loadout_id, skse, address_library) = skse_loadout(&core, "guard-rollback");
        let game = scratch("skse-fake-game-guard-rollback");
        let data_root = scratch("skse-data-root-guard-rollback");
        core.set_game_path_override("skyrimse", "root", &game.display().to_string())
            .unwrap();
        let rows = loadout::entries(&core, loadout_id).unwrap();
        let engine_id = rows.iter().find(|r| r.name == "SKSE64").unwrap().mod_id;
        let state = test_state(core, data_root.clone());
        let core = state.core.lock().unwrap();

        {
            let _guard =
                sync_engine_mod_direct_install(&core, &state, loadout_id, engine_id, true).unwrap();
            assert!(game.join("skse64_loader.exe").is_file());
            // Simulate Plan DB failure: leave without commit.
        }
        assert!(
            !game.join("skse64_loader.exe").exists(),
            "SKSE files must not linger when the Plan never committed"
        );

        drop(core);
        for dir in [skse, address_library, game, data_root] {
            let _ = fs::remove_dir_all(dir);
        }
    }

    #[cfg(windows)]
    #[test]
    fn ticking_an_engine_built_for_another_game_version_copies_nothing() {
        // A system DLL stands in for SkyrimSE.exe: it has a file version that
        // is certainly not 1.7.104 (read only, never written).
        let kernel32 = PathBuf::from(std::env::var_os("SystemRoot").expect("SystemRoot"))
            .join(r"System32\kernel32.dll");
        let core = Core::open_in_memory().unwrap();
        let (loadout_id, skse, address_library) = skse_loadout(&core, "wrong-version");
        let game = scratch("skse-fake-game-wrong-version");
        let data_root = scratch("skse-data-root-wrong-version");
        core.set_game_path_override("skyrimse", "root", &game.display().to_string())
            .unwrap();
        core.set_game_path_override("skyrimse", "exe", &kernel32.display().to_string())
            .unwrap();
        let rows = loadout::entries(&core, loadout_id).unwrap();
        let engine_id = rows.iter().find(|r| r.name == "SKSE64").unwrap().mod_id;
        let state = test_state(core, data_root.clone());
        let core = state.core.lock().unwrap();

        let err = match sync_engine_mod_direct_install(&core, &state, loadout_id, engine_id, true) {
            Err(e) => e,
            Ok(_) => panic!("expected an error"),
        };

        assert!(err.contains("1.7.104"), "{err}");
        assert!(err.contains("Rien n'a été copié"), "{err}");
        assert_eq!(fs::read_dir(&game).unwrap().count(), 0);
        assert!(!data_root.join("engine").exists());

        drop(core);
        for dir in [skse, address_library, game, data_root] {
            let _ = fs::remove_dir_all(dir);
        }
    }

    #[test]
    fn the_engine_is_not_touched_while_a_game_runs() {
        let core = Core::open_in_memory().unwrap();
        let (loadout_id, skse, address_library) = skse_loadout(&core, "running");
        let game = scratch("skse-fake-game-running");
        let data_root = scratch("skse-data-root-running");
        core.set_game_path_override("skyrimse", "root", &game.display().to_string())
            .unwrap();
        let rows = loadout::entries(&core, loadout_id).unwrap();
        let engine_id = rows.iter().find(|r| r.name == "SKSE64").unwrap().mod_id;
        let state = test_state(core, data_root.clone());
        state.game_running.store(true, Ordering::SeqCst);
        let core = state.core.lock().unwrap();

        let err = match sync_engine_mod_direct_install(&core, &state, loadout_id, engine_id, true) {
            Err(e) => e,
            Ok(_) => panic!("expected an error"),
        };

        assert!(err.contains("Une partie est en cours"), "{err}");
        assert_eq!(fs::read_dir(&game).unwrap().count(), 0);

        drop(core);
        for dir in [skse, address_library, game, data_root] {
            let _ = fs::remove_dir_all(dir);
        }
    }

    #[test]
    fn opening_a_branch_fills_empty_categories_and_leaves_a_user_choice() {
        let core = Core::open_in_memory().unwrap();
        let skyrim = loadout::create(&core, "skyrimse", "Skyrim").unwrap();
        let minecraft = loadout::create(&core, "minecraft", "Minecraft").unwrap();
        let skyui = scratch("cat-skyui");
        fs::write(skyui.join("SkyUI_SE.esp"), b"esp").unwrap();
        let kept = scratch("cat-kept");
        fs::write(kept.join("readme.txt"), b"x").unwrap();
        let cleared = scratch("cat-cleared");
        fs::write(cleared.join("skse64_loader.exe"), b"x").unwrap();
        let vague = scratch("cat-vague");
        fs::write(vague.join("readme.txt"), b"x").unwrap();
        let textures = scratch("cat-textures");
        fs::create_dir_all(textures.join("textures")).unwrap();
        fs::write(textures.join("textures/fur.dds"), b"dds").unwrap();
        let patch = scratch("cat-patch");
        fs::write(patch.join("readme.txt"), b"x").unwrap();
        let mine_dir = scratch("cat-mine");
        fs::write(mine_dir.join("readme.txt"), b"x").unwrap();

        loadout::import_mods(
            &core,
            skyrim.id,
            &[
                ImportEntry {
                    name: "SkyUI".into(),
                    source_path: skyui.clone(),
                    enabled: true,
                    version: None,
                },
                ImportEntry {
                    name: "Address Library".into(),
                    source_path: kept.clone(),
                    enabled: true,
                    version: None,
                },
                ImportEntry {
                    name: "SKSE".into(),
                    source_path: cleared.clone(),
                    enabled: true,
                    version: None,
                },
                ImportEntry {
                    name: "Cool Dragons".into(),
                    source_path: vague.clone(),
                    enabled: true,
                    version: None,
                },
                ImportEntry {
                    name: "Pretty Fur".into(),
                    source_path: textures.clone(),
                    enabled: true,
                    version: None,
                },
                ImportEntry {
                    name: "Unofficial Skyrim Special Edition Patch".into(),
                    source_path: patch.clone(),
                    enabled: false,
                    version: None,
                },
            ],
        )
        .unwrap();
        loadout::import_mods(
            &core,
            minecraft.id,
            &[
                ImportEntry {
                    name: "Sodium".into(),
                    source_path: mine_dir.clone(),
                    enabled: true,
                    version: None,
                },
                ImportEntry {
                    name: "jei-26.3-neoforge-31.14.0.72".into(),
                    source_path: mine_dir.clone(),
                    enabled: true,
                    version: None,
                },
                ImportEntry {
                    name: "Fabric API".into(),
                    source_path: mine_dir.clone(),
                    enabled: true,
                    version: None,
                },
                ImportEntry {
                    name: "Potato Mod".into(),
                    source_path: mine_dir.clone(),
                    enabled: true,
                    version: None,
                },
            ],
        )
        .unwrap();

        let rows = loadout::entries(&core, skyrim.id).unwrap();
        let address = rows
            .iter()
            .find(|row| row.name == "Address Library")
            .unwrap()
            .mod_id;
        let skse_id = rows.iter().find(|row| row.name == "SKSE").unwrap().mod_id;
        modlab_core::mod_meta::apply_layer(
            &core,
            address,
            modlab_core::mod_meta::Layer::User,
            &modlab_core::mod_meta::Patch {
                category: Some("magic".into()),
                ..modlab_core::mod_meta::Patch::default()
            },
            &[],
            false,
        )
        .unwrap();
        modlab_core::mod_meta::apply_layer(
            &core,
            skse_id,
            modlab_core::mod_meta::Layer::User,
            &modlab_core::mod_meta::Patch {
                clear_category: true,
                ..modlab_core::mod_meta::Patch::default()
            },
            &[],
            false,
        )
        .unwrap();

        crate::fill_branch_categories(&core, skyrim.id).unwrap();
        crate::fill_branch_categories(&core, minecraft.id).unwrap();

        let sky = loadout::entries(&core, skyrim.id).unwrap();
        let cat = |name: &str| {
            sky.iter()
                .find(|row| row.name == name)
                .unwrap()
                .category
                .clone()
        };
        assert_eq!(cat("SkyUI").as_deref(), Some("interface"));
        assert_eq!(cat("Address Library").as_deref(), Some("magic"));
        assert_eq!(cat("SKSE").as_deref(), None);
        assert_eq!(cat("Cool Dragons").as_deref(), Some("other"));
        assert_eq!(cat("Pretty Fur").as_deref(), Some("visuals"));
        assert_eq!(
            cat("Unofficial Skyrim Special Edition Patch").as_deref(),
            Some("patches")
        );

        let mine = loadout::entries(&core, minecraft.id).unwrap();
        let mcat = |name: &str| {
            mine.iter()
                .find(|row| row.name == name)
                .unwrap()
                .category
                .clone()
        };
        assert_eq!(mcat("Sodium").as_deref(), Some("performance"));
        assert_eq!(
            mcat("jei-26.3-neoforge-31.14.0.72").as_deref(),
            Some("interface")
        );
        assert_eq!(mcat("Fabric API").as_deref(), Some("library"));
        assert_eq!(mcat("Potato Mod").as_deref(), Some("other"));
        let potato = mine
            .iter()
            .find(|row| row.name == "Potato Mod")
            .unwrap()
            .mod_id;
        assert_eq!(
            modlab_core::mod_meta::recorded_layer(&core, potato, "category").unwrap(),
            Some(modlab_core::mod_meta::Layer::Filename)
        );

        let skyui_id = sky.iter().find(|row| row.name == "SkyUI").unwrap().mod_id;
        assert_eq!(
            modlab_core::mod_meta::recorded_layer(&core, skyui_id, "category").unwrap(),
            Some(modlab_core::mod_meta::Layer::Filename)
        );
        let fur_id = sky
            .iter()
            .find(|row| row.name == "Pretty Fur")
            .unwrap()
            .mod_id;
        assert_eq!(
            modlab_core::mod_meta::recorded_layer(&core, fur_id, "category").unwrap(),
            Some(modlab_core::mod_meta::Layer::Archive)
        );

        for dir in [skyui, kept, cleared, vague, textures, patch, mine_dir] {
            let _ = fs::remove_dir_all(dir);
        }
    }
}
