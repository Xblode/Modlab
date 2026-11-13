//! Tauri host for the React shell. This crate is a *client* of
//! `modlab-core` — same rule as the CLI: no private state, no game rules.

pub mod browser;
pub mod downloads;

use modlab_ai::{diagnose, llm_phrase, rules_answer, LoadoutFacts, ModFact};
use modlab_core::game_adapter::{GameAdapter, PathKind};
use modlab_core::{conflicts, download, loadout, transaction, Core};
use modlab_game_minecraft::MinecraftAdapter;
use modlab_game_skyrim::fomod;
use modlab_game_skyrim::installer::ArchiveShape;
use modlab_game_skyrim::SkyrimAdapter;
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::Duration;
use tauri::State;

pub struct AppState {
    core: Mutex<Core>,
    data_root: PathBuf,
    /// §6 lifecycle guard: the local LLM is never allowed to infer while a
    /// game session runs. `launch_skse` sets this for the whole session.
    game_running: AtomicBool,
    /// Whichever game's page the embedded browser was last opened on — set
    /// by `browser_show`. Downloads captured while browsing land under this
    /// game's own subfolder (`downloads::on_requested`); `None` before the
    /// browser has ever been opened, or for a page opened with no active
    /// game selected.
    active_game: Mutex<Option<String>>,
}

/// Resets `game_running` on drop — including on every early-return path.
struct GameSessionGuard<'a>(&'a AtomicBool);

impl Drop for GameSessionGuard<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
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
    /// maquette puts on those cards (last played) has no Core backing yet
    /// and is rendered as an explicit placeholder by the UI.
    pub mod_count: i64,
    /// `"stable"`, `"beta"` or `"alpha"` — see `modlab_core::loadout::CHANNELS`.
    pub channel: String,
    /// How many snapshots this loadout has — `transaction::revision_count`.
    /// Carried on every listed loadout (not just the active one) so a card
    /// in the dashboard grid or the sidebar tree can show `r.N` without a
    /// second round-trip per row.
    pub revision: i64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModRowDto {
    pub mod_id: i64,
    pub name: String,
    pub source_path: String,
    pub enabled: bool,
    pub position: i64,
    /// The version the mod's own source declared, when it declared one —
    /// a Minecraft jar's manifest does, an MO2 import doesn't. `None`
    /// renders as a dash; the table never invents one.
    pub version: Option<String>,
    /// `"archived"` (captured by the browser, nothing extracted yet — the
    /// table shows it but it cannot be enabled) or `"installed"`.
    pub status: String,
    /// The human's own free tag, `None` until they type one.
    pub category: Option<String>,
    pub archive_path: Option<String>,
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

/// Every Game Adapter this shell knows about — the one place that lists
/// them, so `detect_games` and `game_path_slots` can never drift apart on
/// which games exist.
fn adapters() -> [&'static dyn GameAdapter; 2] {
    [&SkyrimAdapter, &MinecraftAdapter]
}

/// One `PathSlot`'s effective value: a human override if the Core has one,
/// else the adapter's own live detection. Every command that actually *uses*
/// a path (launching the game, deploying mods) must go through this instead
/// of calling adapter detection directly — otherwise an override set from
/// Réglages → Jeux is silently ignored, which is exactly the bug that made
/// `launch_skse` keep reporting the DLL as missing after it was overridden.
fn resolve_slot(core: &Core, adapter: &dyn GameAdapter, slot_id: &str) -> Option<PathBuf> {
    if let Ok(Some(over)) = core.game_path_override(adapter.id(), slot_id) {
        return Some(PathBuf::from(over));
    }
    adapter
        .path_slots()
        .into_iter()
        .find(|s| s.id == slot_id)
        .and_then(|s| s.detected)
}

fn detect_games() -> Vec<DetectedGameDto> {
    adapters()
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
                    version: r.version,
                    status: r.status,
                    category: r.category,
                    archive_path: r.archive_path.map(|p| p.display().to_string()),
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
            let revision = transaction::revision_count(&core, l.id).unwrap_or(0);
            LoadoutDto {
                id: l.id,
                game_id: l.game_id,
                name: l.name,
                mod_count,
                channel: l.channel,
                revision,
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
        usvfs_dll: resolve_slot(&core, &SkyrimAdapter, "usvfs_dll")
            .map(|p| p.display().to_string()),
    })
}

/// One `PathSlot`, merged with any human override the Core holds for it —
/// the shape Réglages → Jeux needs to show "what ModLab found" vs "what you
/// pointed it at" without doing that merge itself.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GamePathSlotDto {
    pub id: String,
    pub label: String,
    pub kind: PathKind,
    pub detected: Option<String>,
    pub override_path: Option<String>,
    /// `detected` unless `override_path` is set, in which case the override
    /// wins — the single value a client actually uses (to launch, to deploy
    /// into, …), so it never has to re-implement this precedence itself.
    pub effective: Option<String>,
}

/// The path slots this game's adapter declares, each with whatever the Core
/// has on file as a human override. Read-only (Lecture tier, §7.2): nothing
/// is written here, so it needs no confirmation.
#[tauri::command]
fn game_path_slots(
    state: State<AppState>,
    game_id: String,
) -> Result<Vec<GamePathSlotDto>, String> {
    let adapter = adapters()
        .into_iter()
        .find(|a| a.id() == game_id)
        .ok_or_else(|| format!("jeu inconnu : {game_id}"))?;
    let overrides = {
        let core = state.core.lock().map_err(|e| e.to_string())?;
        core.game_path_overrides(&game_id)
            .map_err(|e| e.to_string())?
    };
    Ok(adapter
        .path_slots()
        .into_iter()
        .map(|slot| {
            let detected = slot.detected.map(|p| p.display().to_string());
            let override_path = overrides
                .iter()
                .find(|(id, _)| id == slot.id)
                .map(|(_, path)| path.clone());
            let effective = override_path.clone().or_else(|| detected.clone());
            GamePathSlotDto {
                id: slot.id.to_string(),
                label: slot.label.to_string(),
                kind: slot.kind,
                detected,
                override_path,
                effective,
            }
        })
        .collect())
}

/// Point one of a game's path slots at a folder/file the human chose by
/// hand — Mutation réversible tier (§7.2): it only ever changes what ModLab
/// *reads from* next time, nothing on disk moves, and clearing it (see
/// below) undoes it instantly. Returns the merged slot list so the panel can
/// refresh from the Core's own answer rather than guessing what changed.
#[tauri::command]
fn set_game_path_override(
    state: State<AppState>,
    game_id: String,
    slot_id: String,
    path: String,
) -> Result<Vec<GamePathSlotDto>, String> {
    let path = path.trim();
    if path.is_empty() {
        return Err("Le chemin ne peut pas être vide.".into());
    }
    {
        let core = state.core.lock().map_err(|e| e.to_string())?;
        core.set_game_path_override(&game_id, &slot_id, path)
            .map_err(|e| e.to_string())?;
    }
    game_path_slots(state, game_id)
}

/// Drop a human override for one slot, falling back to the adapter's own
/// detection again.
#[tauri::command]
fn clear_game_path_override(
    state: State<AppState>,
    game_id: String,
    slot_id: String,
) -> Result<Vec<GamePathSlotDto>, String> {
    {
        let core = state.core.lock().map_err(|e| e.to_string())?;
        core.clear_game_path_override(&game_id, &slot_id)
            .map_err(|e| e.to_string())?;
    }
    game_path_slots(state, game_id)
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
    sync_engine_mod_direct_install(&core, loadout_id, mod_id, enabled)?;
    entries_dto(&core, loadout_id)
}

/// If `mod_id` is Skyrim's engine mod (SKSE), mirror its freshly toggled
/// enabled state onto the real install root — see
/// `modlab_game_skyrim::engine`'s doc for why this one mod is handled
/// outside USVFS entirely. A no-op for every other mod, every other game,
/// and an archived (not yet extracted) row: there is nothing to copy from
/// yet in that last case.
fn sync_engine_mod_direct_install(
    core: &Core,
    loadout_id: i64,
    mod_id: i64,
    enabled: bool,
) -> Result<(), String> {
    let is_skyrim = loadout::list(core)
        .map_err(|e| e.to_string())?
        .into_iter()
        .any(|l| l.id == loadout_id && l.game_id == SkyrimAdapter.id());
    if !is_skyrim {
        return Ok(());
    }
    let rows = loadout::entries(core, loadout_id).map_err(|e| e.to_string())?;
    let Some(row) = rows.into_iter().find(|r| r.mod_id == mod_id) else {
        return Ok(());
    };
    if row.status != loadout::STATUS_INSTALLED
        || !modlab_game_skyrim::engine::is_engine_mod(&row.name)
    {
        return Ok(());
    }
    let Some(install_root) = resolve_slot(core, &SkyrimAdapter, "root") else {
        return Ok(());
    };
    let result = if enabled {
        modlab_game_skyrim::engine::install_direct(&row.source_path, &install_root)
    } else {
        modlab_game_skyrim::engine::uninstall_direct(&row.source_path, &install_root)
    };
    result.map_err(|e| e.to_string())
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

/// One dependency the shell knows to look for on a mod — today just "SKSE,
/// if this mod isn't SKSE itself" for Skyrim. Deliberately structured as a
/// list rather than a single flag: the real "known mods" registry (SKSE,
/// Forge, Fabric… each with real metadata) is tracked as separate, bigger
/// Core work — this is the stopgap that's easy to grow into it, one more
/// `ModDependencyDto` at a time, without changing the shape callers see.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModDependencyDto {
    pub name: String,
    /// Whether the dependency is currently enabled in this same loadout.
    pub satisfied: bool,
}

/// The Inspector's "Dépendances" section. Read-only (Lecture tier, §7.2).
/// Every non-engine mod in a Skyrim loadout is reported as depending on
/// SKSE — true for the near-totality of real Skyrim mods, and cheap enough
/// to just always say rather than trying to detect it per mod from data
/// nobody parses yet (FOMOD conditions, Nexus requirement lists).
#[tauri::command]
fn mod_dependencies(
    state: State<AppState>,
    loadout_id: i64,
    mod_id: i64,
) -> Result<Vec<ModDependencyDto>, String> {
    let core = state.core.lock().map_err(|e| e.to_string())?;
    let is_skyrim = loadout::list(&core)
        .map_err(|e| e.to_string())?
        .into_iter()
        .any(|l| l.id == loadout_id && l.game_id == SkyrimAdapter.id());
    if !is_skyrim {
        return Ok(Vec::new());
    }
    let rows = loadout::entries(&core, loadout_id).map_err(|e| e.to_string())?;
    let Some(row) = rows.iter().find(|r| r.mod_id == mod_id) else {
        return Ok(Vec::new());
    };
    if modlab_game_skyrim::engine::is_engine_mod(&row.name) {
        return Ok(Vec::new());
    }
    let skse_enabled = rows
        .iter()
        .any(|r| r.enabled && modlab_game_skyrim::engine::is_engine_mod(&r.name));
    Ok(vec![ModDependencyDto {
        name: "SKSE".to_string(),
        satisfied: skse_enabled,
    }])
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
    let mods: Vec<conflicts::ModFiles> = rows
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
fn loadout_directory(
    state: State<AppState>,
    loadout_id: i64,
) -> Result<Vec<DirectoryEntryDto>, String> {
    let core = state.core.lock().map_err(|e| e.to_string())?;
    merged_view_dto(&core, loadout_id)
}

/// The Plugins tab: the same merged view, filtered to `.esp`/`.esm`/`.esl`.
/// This is the mod-position order the loadout itself stores, not a real
/// LOOT-sorted load order and not master-dependency validation — neither
/// exists yet (see `StubTab`'s former copy for this tab).
#[tauri::command]
fn loadout_plugins(
    state: State<AppState>,
    loadout_id: i64,
) -> Result<Vec<DirectoryEntryDto>, String> {
    let core = state.core.lock().map_err(|e| e.to_string())?;
    Ok(merged_view_dto(&core, loadout_id)?
        .into_iter()
        .filter(|e| has_extension(&e.relative_path, &["esp", "esm", "esl"]))
        .collect())
}

/// The Archives tab: the same merged view, filtered to `.bsa`/`.ba2`. Which
/// files exist and which mod provides each — not an inventory of what's
/// packed inside them, and not an integrity check (BSA/BA2 parsing doesn't
/// exist here yet, see `StubTab`'s former copy for this tab).
#[tauri::command]
fn loadout_archives(
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
fn skyrim_saves(state: State<AppState>) -> Result<Vec<SaveFileDto>, String> {
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
        // A brand-new loadout is always on the column's own default —
        // there is no path that creates one on another channel yet.
        channel: "stable".to_string(),
        revision: 0,
    })
}

/// Rename a loadout. Mutation réversible tier (§7.2) — a metadata edit, not
/// a configuration change, so it doesn't go through the snapshot cycle.
#[tauri::command]
fn rename_loadout(state: State<AppState>, loadout_id: i64, name: String) -> Result<(), String> {
    let core = state.core.lock().map_err(|e| e.to_string())?;
    loadout::rename(&core, loadout_id, &name).map_err(|e| e.to_string())
}

/// Move a loadout to a different channel ("stable" / "beta" / "alpha").
#[tauri::command]
fn set_loadout_channel(
    state: State<AppState>,
    loadout_id: i64,
    channel: String,
) -> Result<(), String> {
    let core = state.core.lock().map_err(|e| e.to_string())?;
    loadout::set_channel(&core, loadout_id, &channel).map_err(|e| e.to_string())
}

/// Soft-delete a loadout — see `loadout::delete`. Reversible in the Core
/// (the row and its entries survive) — `list_deleted_loadouts`/
/// `restore_loadout` are the trash screen that undoes it.
#[tauri::command]
fn delete_loadout(state: State<AppState>, loadout_id: i64) -> Result<(), String> {
    let core = state.core.lock().map_err(|e| e.to_string())?;
    loadout::delete(&core, loadout_id).map_err(|e| e.to_string())
}

/// One soft-deleted loadout, as the trash screen lists it — deliberately
/// thinner than `LoadoutDto`: mod count and revision don't change while a
/// loadout sits in the trash, so there is nothing to compute for them.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeletedLoadoutDto {
    pub id: i64,
    pub game_id: String,
    pub name: String,
}

#[tauri::command]
fn list_deleted_loadouts(state: State<AppState>) -> Result<Vec<DeletedLoadoutDto>, String> {
    let core = state.core.lock().map_err(|e| e.to_string())?;
    Ok(loadout::list_trash(&core)
        .map_err(|e| e.to_string())?
        .into_iter()
        .map(|l| DeletedLoadoutDto {
            id: l.id,
            game_id: l.game_id,
            name: l.name,
        })
        .collect())
}

#[tauri::command]
fn restore_loadout(state: State<AppState>, loadout_id: i64) -> Result<(), String> {
    let core = state.core.lock().map_err(|e| e.to_string())?;
    loadout::restore(&core, loadout_id).map_err(|e| e.to_string())
}

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
fn list_downloads(state: State<AppState>) -> Result<Vec<DownloadDto>, String> {
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

/// File a capture into a loadout: it appears in the mod table right away,
/// archived and disabled.
#[tauri::command]
fn attach_download(
    state: State<AppState>,
    download_id: i64,
    loadout_id: i64,
    category: Option<String>,
) -> Result<i64, String> {
    let core = state.core.lock().map_err(|e| e.to_string())?;
    download::attach_to_loadout(&core, download_id, loadout_id, category.as_deref())
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn set_mod_category(
    state: State<AppState>,
    mod_id: i64,
    category: Option<String>,
) -> Result<(), String> {
    let core = state.core.lock().map_err(|e| e.to_string())?;
    download::set_category(&core, mod_id, category.as_deref()).map_err(|e| e.to_string())
}

#[tauri::command]
fn rename_mod(
    state: State<AppState>,
    mod_id: i64,
    loadout_id: i64,
    name: String,
) -> Result<Vec<ModRowDto>, String> {
    let core = state.core.lock().map_err(|e| e.to_string())?;
    download::rename(&core, mod_id, &name).map_err(|e| e.to_string())?;
    entries_dto(&core, loadout_id)
}

/// Soft-delete only — see `download::remove`'s doc. Nothing on disk is
/// touched, so there is no folder to clean up here.
#[tauri::command]
fn delete_mod(
    state: State<AppState>,
    mod_id: i64,
    loadout_id: i64,
) -> Result<Vec<ModRowDto>, String> {
    let core = state.core.lock().map_err(|e| e.to_string())?;
    download::remove(&core, mod_id).map_err(|e| e.to_string())?;
    entries_dto(&core, loadout_id)
}

/// Revert an installed mod to its archive. The extracted copy this removes
/// is disposable derived data — `install_archived_mod` already treats
/// `mod_dest_dir` as something it can freely `remove_dir_all` and recreate,
/// so doing the same here on the way back out is the same guarantee, not a
/// new one. The archive itself is never touched.
#[tauri::command]
fn uninstall_mod(
    state: State<AppState>,
    mod_id: i64,
    loadout_id: i64,
) -> Result<Vec<ModRowDto>, String> {
    let core = state.core.lock().map_err(|e| e.to_string())?;
    download::uninstall(&core, mod_id).map_err(|e| e.to_string())?;
    let _ = std::fs::remove_dir_all(mod_dest_dir(&state, mod_id));
    entries_dto(&core, loadout_id)
}

/// Open Explorer at `path` — either the folder itself, or (`select: true`)
/// the folder containing `path` with that file highlighted. Fire-and-forget
/// by design: `explorer.exe` routinely exits non-zero even on success, so
/// waiting on its status would misreport a perfectly normal open as failed.
#[tauri::command]
fn reveal_path(path: String, select: bool) -> Result<(), String> {
    #[cfg(windows)]
    {
        let arg = if select {
            format!("/select,{path}")
        } else {
            path
        };
        std::process::Command::new("explorer")
            .arg(arg)
            .spawn()
            .map_err(|e| e.to_string())?;
        Ok(())
    }
    #[cfg(not(windows))]
    {
        let _ = (path, select);
        Err("ouverture du dossier disponible uniquement sous Windows".to_string())
    }
}

/// Move an archived mod (not-yet-extracted only) to a different instance.
#[tauri::command]
fn reassign_mod(
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
fn mod_categories(state: State<AppState>) -> Result<Vec<String>, String> {
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
    /// `"fomod"`, `"bare-data"`, `"wrapped-data"` or `"unknown"`.
    pub shape: String,
    /// True only for a FOMOD: nothing lands in the loadout until the human
    /// has answered the installer's own questions.
    pub needs_wizard: bool,
    /// Where the archive was unpacked, and where inside that the real
    /// content starts (an archive often wraps everything in one folder).
    pub staged_path: String,
    pub content_root: String,
}

fn shape_id(shape: &ArchiveShape) -> &'static str {
    match shape {
        ArchiveShape::Fomod => "fomod",
        ArchiveShape::BareData => "bare-data",
        ArchiveShape::WrappedData { .. } => "wrapped-data",
        ArchiveShape::Unknown => "unknown",
    }
}

/// Unpack an archived mod into the data root's staging area and classify it.
/// Stops there on purpose: extracting is reversible and touches nothing the
/// game can see, while *installing* is the step that needs a decision.
#[tauri::command]
fn inspect_archived_mod(state: State<AppState>, mod_id: i64) -> Result<ArchivePlanDto, String> {
    let pending = {
        let core = state.core.lock().map_err(|e| e.to_string())?;
        download::archived_mod(&core, mod_id).map_err(|e| e.to_string())?
    };

    let staged = state.data_root.join("staging").join(mod_id.to_string());
    if staged.exists() {
        std::fs::remove_dir_all(&staged).map_err(|e| e.to_string())?;
    }
    std::fs::create_dir_all(&staged).map_err(|e| e.to_string())?;
    modlab_core::archive::extract(&pending.archive_path, &staged).map_err(|e| e.to_string())?;

    let detected = modlab_game_skyrim::installer::detect_shape(&staged)
        .map_err(|e| format!("lecture de l'archive extraite : {e}"))?;

    Ok(ArchivePlanDto {
        mod_id,
        name: pending.name,
        shape: shape_id(&detected.shape).to_string(),
        needs_wizard: detected.shape == ArchiveShape::Fomod,
        staged_path: staged.display().to_string(),
        content_root: detected.content_root.display().to_string(),
    })
}

fn mod_dest_dir(state: &AppState, mod_id: i64) -> PathBuf {
    state.data_root.join("mods").join(mod_id.to_string())
}

/// Common tail of every install path: record the extraction, drop the now
/// useless staging copy, and hand back the loadout's fresh entry list so the
/// table can show the flipped checkbox immediately.
fn finish_install(
    core: &Core,
    state: &AppState,
    mod_id: i64,
    loadout_id: i64,
    dest: &Path,
) -> Result<Vec<ModRowDto>, String> {
    download::mark_installed(core, mod_id, dest).map_err(|e| e.to_string())?;
    let _ = std::fs::remove_dir_all(state.data_root.join("staging").join(mod_id.to_string()));
    entries_dto(core, loadout_id)
}

/// Finish the install of a non-FOMOD archive: move the staged content into
/// the loadout's own mod folder and flip the row to `installed`, which is
/// what finally makes the checkbox real.
#[tauri::command]
fn install_archived_mod(
    state: State<AppState>,
    mod_id: i64,
    loadout_id: i64,
    content_root: String,
) -> Result<Vec<ModRowDto>, String> {
    let core = state.core.lock().map_err(|e| e.to_string())?;
    // Refuses anything already installed — the guard lives in the Core so
    // the CLI gets it too.
    download::archived_mod(&core, mod_id).map_err(|e| e.to_string())?;

    let dest = mod_dest_dir(&state, mod_id);
    if dest.exists() {
        std::fs::remove_dir_all(&dest).map_err(|e| e.to_string())?;
    }
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    // A rename inside the same data root is atomic and instant; the copy
    // fallback covers a staging area that landed on another volume.
    if std::fs::rename(&content_root, &dest).is_err() {
        copy_dir(Path::new(&content_root), &dest).map_err(|e| e.to_string())?;
    }

    finish_install(&core, &state, mod_id, loadout_id, &dest)
}

/// The FOMOD wizard's questions, translated to plain data — see
/// `modlab_game_skyrim::fomod` for what is and isn't parsed.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FomodPluginDto {
    pub id: String,
    pub name: String,
    pub description: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FomodGroupDto {
    pub id: String,
    pub name: String,
    /// `"select-exactly-one"`, `"select-at-most-one"`, `"select-at-least-one"`,
    /// `"select-any"` or `"select-all"`.
    pub group_type: String,
    pub plugins: Vec<FomodPluginDto>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FomodStepDto {
    pub id: String,
    pub name: String,
    pub groups: Vec<FomodGroupDto>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FomodModuleDto {
    pub name: String,
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

/// What the wizard asks, for a `content_root` `inspect_archived_mod` already
/// reported as `"fomod"`.
#[tauri::command]
fn get_fomod_module(content_root: String) -> Result<FomodModuleDto, String> {
    let module = read_fomod_module(&content_root)?;
    Ok(FomodModuleDto {
        name: module.name,
        steps: module
            .steps
            .into_iter()
            .map(|s| FomodStepDto {
                id: s.id,
                name: s.name,
                groups: s
                    .groups
                    .into_iter()
                    .map(|g| FomodGroupDto {
                        id: g.id,
                        name: g.name,
                        group_type: group_type_id(g.group_type).to_string(),
                        plugins: g
                            .plugins
                            .into_iter()
                            .map(|p| FomodPluginDto {
                                id: p.id,
                                name: p.name,
                                description: p.description,
                            })
                            .collect(),
                    })
                    .collect(),
            })
            .collect(),
    })
}

/// Finish a FOMOD install: `picked` is every plugin id the wizard's answers
/// checked. Group rules are re-checked here — the wizard UI enforces them
/// too, but a real mutation never trusts the frontend alone for that.
#[tauri::command]
fn install_fomod(
    state: State<AppState>,
    mod_id: i64,
    loadout_id: i64,
    content_root: String,
    picked: Vec<String>,
) -> Result<Vec<ModRowDto>, String> {
    let core = state.core.lock().map_err(|e| e.to_string())?;
    download::archived_mod(&core, mod_id).map_err(|e| e.to_string())?;

    let module = read_fomod_module(&content_root)?;
    for step in &module.steps {
        for group in &step.groups {
            let count = group
                .plugins
                .iter()
                .filter(|p| picked.contains(&p.id))
                .count();
            if !group.group_type.accepts(count) {
                return Err(format!(
                    "« {} » : sélection invalide pour le groupe « {} »",
                    module.name, group.name
                ));
            }
        }
    }

    let root = Path::new(&content_root);
    let files = fomod::resolve(&module, &picked);
    let dest = mod_dest_dir(&state, mod_id);
    if dest.exists() {
        std::fs::remove_dir_all(&dest).map_err(|e| e.to_string())?;
    }
    for mapping in &files {
        let from = root.join(&mapping.source);
        let to = dest.join(&mapping.destination);
        if let Some(parent) = to.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        if from.is_dir() {
            copy_dir(&from, &to).map_err(|e| e.to_string())?;
        } else {
            std::fs::copy(&from, &to)
                .map_err(|e| format!("copie de {} : {e}", mapping.source.display()))?;
        }
    }

    finish_install(&core, &state, mod_id, loadout_id, &dest)
}

fn copy_dir(from: &Path, to: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir(&entry.path(), &target)?;
        } else {
            std::fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}

/// Show the embedded browser at `rect` (CSS pixels, measured by the React
/// side), creating it on the first call. `url` is only used for that first
/// call — afterwards the browser keeps whatever page it is on, which is the
/// point of a single persistent tab. `game_id` records which game this open
/// belongs to, so a download captured while this page is up files under
/// that game's own downloads subfolder (`downloads::on_requested`).
#[tauri::command]
fn browser_show(
    state: State<AppState>,
    app: tauri::AppHandle,
    url: String,
    rect: browser::Rect,
    game_id: Option<String>,
) -> Result<(), String> {
    if let Ok(mut active) = state.active_game.lock() {
        *active = game_id;
    }
    browser::show(&app, &url, rect)
}

#[tauri::command]
fn browser_hide(app: tauri::AppHandle) -> Result<(), String> {
    browser::hide(&app)
}

#[tauri::command]
fn browser_navigate(app: tauri::AppHandle, url: String) -> Result<(), String> {
    browser::navigate(&app, &url)
}

/// `-1` back, `1` forward — the address bar's arrows.
#[tauri::command]
fn browser_go(app: tauri::AppHandle, delta: i32) -> Result<(), String> {
    browser::go(&app, delta)
}

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
    let default_path = downloads::default_downloads_root(&state.data_root)
        .display()
        .to_string();
    let override_path = core
        .game_path_override(downloads::SETTINGS_SCOPE, downloads::DOWNLOADS_DIR_SLOT)
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
fn downloads_dir_setting(state: State<AppState>) -> Result<DownloadsDirDto, String> {
    downloads_dir_dto(&state)
}

/// Point the downloads folder at a location the human chose — Mutation
/// réversible tier (§7.2): nothing already on disk moves, this only changes
/// where the *next* capture lands, and clearing it undoes it instantly.
#[tauri::command]
fn set_downloads_dir_override(
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
            downloads::SETTINGS_SCOPE,
            downloads::DOWNLOADS_DIR_SLOT,
            path,
        )
        .map_err(|e| e.to_string())?;
    }
    downloads_dir_dto(&state)
}

#[tauri::command]
fn clear_downloads_dir_override(state: State<AppState>) -> Result<DownloadsDirDto, String> {
    {
        let core = state.core.lock().map_err(|e| e.to_string())?;
        core.clear_game_path_override(downloads::SETTINGS_SCOPE, downloads::DOWNLOADS_DIR_SLOT)
            .map_err(|e| e.to_string())?;
    }
    downloads_dir_dto(&state)
}

/// What `ActiveInstanceCard`'s version badge shows: how many revisions this
/// loadout has (`r.{revision}`, `b.{revision}`… — the channel-letter prefix
/// is the UI's own presentation, this DTO just carries the number) and
/// whether the live state has drifted from the latest one. Read-only
/// (Lecture tier, §7.2) and recomputed per call, same rule as
/// `get_loadout_conflicts`: a cached dirty count would go stale the moment
/// a mod is toggled elsewhere.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LoadoutVersionDto {
    pub revision: i64,
    pub dirty_plus: i64,
    pub dirty_minus: i64,
}

#[tauri::command]
fn get_loadout_version(
    state: State<AppState>,
    loadout_id: i64,
) -> Result<LoadoutVersionDto, String> {
    let core = state.core.lock().map_err(|e| e.to_string())?;
    let revision = transaction::revision_count(&core, loadout_id).map_err(|e| e.to_string())?;
    let dirty = transaction::dirty_summary(&core, loadout_id).map_err(|e| e.to_string())?;
    Ok(LoadoutVersionDto {
        revision,
        dirty_plus: dirty.plus,
        dirty_minus: dirty.minus,
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
            channel: "stable".to_string(),
            revision: 0,
        },
        imported,
    })
}

/// Where a Minecraft command should act: the human's override from
/// Réglages → Jeux if there is one, else the adapter's own detection.
/// Refuses rather than falling back to a guess — writing jars into the
/// wrong folder is not a mistake worth being clever about.
fn minecraft_root(core: &Core) -> Result<PathBuf, String> {
    resolve_slot(core, &MinecraftAdapter, "root").ok_or_else(|| {
        "Minecraft introuvable — renseigne son dossier dans Réglages → Jeux".to_string()
    })
}

/// Import an existing modded instance's `mods/` folder into a new loadout.
/// Read-only on the game's side: not a jar is moved. Same shape as
/// `mo2_import`, and like it the real work lives in the adapter so the CLI
/// and the shell cannot drift apart.
#[tauri::command]
fn minecraft_import_instance(
    state: State<AppState>,
    name: String,
) -> Result<Mo2ImportResultDto, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("Donne un nom à l'instance.".into());
    }
    let core = state.core.lock().map_err(|e| e.to_string())?;
    let root = minecraft_root(&core)?;
    let (created, imported) = modlab_game_minecraft::instance::import_instance(&core, &root, name)?;
    Ok(Mo2ImportResultDto {
        loadout: LoadoutDto {
            id: created.id,
            game_id: MinecraftAdapter.id().to_string(),
            name: created.name,
            mod_count: i64::try_from(imported).unwrap_or(i64::MAX),
            channel: "stable".to_string(),
            revision: 0,
        },
        imported,
    })
}

/// Install one downloaded `.jar` into the instance's `mods/` folder and
/// register it in the loadout. A real write into the game's own folder —
/// see `instance::install_jar` for what it does to stay safe (staged copy,
/// refuses to overwrite, removes the file again if the database write
/// fails). Mutation réversible tier (§7.2): the client confirms first.
#[tauri::command]
fn minecraft_install_jar(
    state: State<AppState>,
    loadout_id: i64,
    jar_path: String,
) -> Result<Vec<ModRowDto>, String> {
    let core = state.core.lock().map_err(|e| e.to_string())?;
    let root = minecraft_root(&core)?;
    modlab_game_minecraft::instance::install_jar(
        &core,
        loadout_id,
        Path::new(jar_path.trim()),
        &root,
    )?;
    entries_dto(&core, loadout_id)
}

/// What a Minecraft instance holds besides its mods — what the Packs and
/// Worlds views list. Plain directory listings, nothing parsed.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MinecraftContentDto {
    pub resource_packs: Vec<String>,
    pub shader_packs: Vec<String>,
    pub worlds: Vec<String>,
    /// Whether `servers.dat` exists. Its contents are **not** read: that
    /// file is binary NBT, and ModLab has no NBT parser — the Servers view
    /// says so out loud rather than showing an empty list that would read
    /// as "you have no servers".
    pub servers_dat_present: bool,
}

/// List an instance's resource packs, shader packs and worlds. Read-only
/// (Lecture tier, §7.2). A missing folder is an empty list, not an error —
/// a fresh instance simply has none yet.
#[tauri::command]
fn minecraft_content(state: State<AppState>) -> Result<MinecraftContentDto, String> {
    let core = state.core.lock().map_err(|e| e.to_string())?;
    let root = minecraft_root(&core)?;
    Ok(MinecraftContentDto {
        resource_packs: list_dir_names(&root.join("resourcepacks")),
        shader_packs: list_dir_names(&root.join("shaderpacks")),
        worlds: list_dir_names(&root.join("saves")),
        servers_dat_present: root.join("servers.dat").is_file(),
    })
}

/// Entry names directly inside `dir`, sorted, hidden entries skipped.
/// Unreadable or missing directory → empty list: these feed views that
/// should show "nothing here" rather than an error nobody can act on.
fn list_dir_names(dir: &Path) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut names: Vec<String> = entries
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .filter(|n| !n.starts_with('.'))
        .collect();
    names.sort_by_key(|n| n.to_lowercase());
    names
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
        // A human override from Réglages → Jeux must win here — this is the
        // one place that actually launches the game, so it's the one place
        // an ignored override would be most visible (and most confusing).
        let (install_root, usvfs_dll, data_dir, rows) = {
            let core = state.core.lock().map_err(|e| e.to_string())?;
            let install_root = resolve_slot(&core, &SkyrimAdapter, "root")
                .ok_or_else(|| "Skyrim SE introuvable sur cette machine".to_string())?;
            let usvfs_dll = resolve_slot(&core, &SkyrimAdapter, "usvfs_dll").ok_or_else(|| {
                "usvfs_x64.dll introuvable — installe MO2, ou renseigne son chemin dans \
                 Réglages → Jeux"
                    .to_string()
            })?;
            let data_dir = resolve_slot(&core, &SkyrimAdapter, "data_dir")
                .unwrap_or_else(|| install_root.join("Data"));
            let rows = loadout::entries(&core, loadout_id).map_err(|e| e.to_string())?;
            (install_root, usvfs_dll, data_dir, rows)
        };
        // SKSE is deployed for real, directly into `install_root`, by
        // `sync_engine_mod_direct_install` whenever its row is enabled —
        // never through USVFS (see `modlab_game_skyrim::engine`'s doc for
        // why). Excluding it from the virtual deployment here is what keeps
        // the two paths from fighting over the same files.
        let vfs_rows: Vec<_> = rows
            .iter()
            .filter(|r| !modlab_game_skyrim::engine::is_engine_mod(&r.name))
            .cloned()
            .collect();

        // SKSE not installed (no loader sitting in the install root) is not
        // an error — it means a vanilla launch. Every enabled mod's files
        // still need USVFS to be visible to whichever executable runs, so
        // the virtual session below is built the same way either way.
        let loader = install_root.join("skse64_loader.exe");
        let vanilla_exe = install_root.join("SkyrimSE.exe");
        let exe_to_launch = if loader.is_file() {
            loader
        } else if vanilla_exe.is_file() {
            vanilla_exe
        } else {
            return Err(format!(
                "Ni skse64_loader.exe ni SkyrimSE.exe trouvés dans {}",
                install_root.display()
            ));
        };
        // The whole session counts as "game running" (§6): while it lasts,
        // `ai_ask` refuses to run the local LLM.
        state.game_running.store(true, Ordering::SeqCst);
        let _session = GameSessionGuard(&state.game_running);

        // Deterministic per-loadout name, not a shared literal: two
        // loadouts launching independently must never reconnect to the
        // same VFS session (see modlab_game_skyrim::usvfs::instance_name's
        // doc — this is what keeps a fresh loadout from appearing to
        // already have another loadout's mods installed).
        let vfs_instance_name = modlab_game_skyrim::usvfs::instance_name("skyrim", loadout_id);
        let mut vfs = modlab_game_skyrim::usvfs::Vfs::create(&usvfs_dll, &vfs_instance_name)
            .map_err(|e| e.to_string())?;
        {
            let core = state.core.lock().map_err(|e| e.to_string())?;
            let mut tx = transaction::Transaction::new(&core, "skyrim.usvfs");
            tx.step(format!("usvfs instance '{vfs_instance_name}' connected"));
            tx.commit().map_err(|e| e.to_string())?;
        }
        modlab_game_skyrim::usvfs::deploy_enabled_mods(&vfs, &vfs_rows, &data_dir)
            .map_err(|e| e.to_string())?;
        let cmd = format!("\"{}\"", exe_to_launch.display());
        vfs.launch_hooked_visible(&cmd, &install_root)
            .map_err(|e| e.to_string())?;
        vfs.wait_for_session_end(Duration::from_secs(2), Duration::from_secs(60))
            .map_err(|e| e.to_string())?;

        // Explicit, verified disconnect rather than leaving it to `Drop`:
        // `Drop` can't propagate an error, so a teardown that silently
        // fails there could leave a stale session for the next launch to
        // collide with.
        let disconnect_result = vfs.disconnect();
        {
            let core = state.core.lock().map_err(|e| e.to_string())?;
            let step = match &disconnect_result {
                Ok(()) => format!("usvfs instance '{vfs_instance_name}' disconnected (confirmed)"),
                Err(e) => format!("usvfs instance '{vfs_instance_name}' disconnect FAILED: {e}"),
            };
            let mut tx = transaction::Transaction::new(&core, "skyrim.usvfs");
            tx.step(step);
            tx.commit().map_err(|e| e.to_string())?;
        }
        disconnect_result.map_err(|e| e.to_string())?;
        Ok(())
    }
}

#[derive(Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphicsSettingsDto {
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub fullscreen: Option<bool>,
    pub vsync: Option<bool>,
    pub antialiasing: Option<u32>,
    pub fxaa: Option<bool>,
}

impl From<modlab_game_skyrim::graphics::GraphicsSettings> for GraphicsSettingsDto {
    fn from(s: modlab_game_skyrim::graphics::GraphicsSettings) -> Self {
        Self {
            width: s.width,
            height: s.height,
            fullscreen: s.fullscreen,
            vsync: s.vsync,
            antialiasing: s.antialiasing,
            fxaa: s.fxaa,
        }
    }
}

impl From<GraphicsSettingsDto> for modlab_game_skyrim::graphics::GraphicsSettings {
    fn from(s: GraphicsSettingsDto) -> Self {
        Self {
            width: s.width,
            height: s.height,
            fullscreen: s.fullscreen,
            vsync: s.vsync,
            antialiasing: s.antialiasing,
            fxaa: s.fxaa,
        }
    }
}

fn prefs_ini_path(core: &Core) -> Result<PathBuf, String> {
    resolve_slot(core, &SkyrimAdapter, "prefs_ini").ok_or_else(|| {
        "SkyrimPrefs.ini introuvable — lance le jeu au moins une fois pour qu'il le crée, \
         ou renseigne son chemin dans Réglages → Jeux"
            .to_string()
    })
}

/// The dashboard's "Graphismes" quick action — read-only (Lecture tier,
/// §7.2). An empty settings object (every field `None`) means the file
/// exists but doesn't set that key yet — the dialog shows the game's own
/// hardcoded default in that case, not a blank field.
#[tauri::command]
fn skyrim_graphics_settings(state: State<AppState>) -> Result<GraphicsSettingsDto, String> {
    let core = state.core.lock().map_err(|e| e.to_string())?;
    let path = prefs_ini_path(&core)?;
    modlab_game_skyrim::graphics::read(&path)
        .map(GraphicsSettingsDto::from)
        .map_err(|e| e.to_string())
}

/// Mutation réversible tier (§7.2): writes straight to `SkyrimPrefs.ini`,
/// but only the handful of keys this dialog exposes, and every other line
/// in the file is preserved untouched (see `modlab_game_skyrim::graphics`'s
/// doc) — nothing here can corrupt an install, and the previous values are
/// always still readable/restorable by hand if a human wants to undo it.
#[tauri::command]
fn set_skyrim_graphics_settings(
    state: State<AppState>,
    settings: GraphicsSettingsDto,
) -> Result<GraphicsSettingsDto, String> {
    let core = state.core.lock().map_err(|e| e.to_string())?;
    let path = prefs_ini_path(&core)?;
    modlab_game_skyrim::graphics::write(&path, &settings.into()).map_err(|e| e.to_string())?;
    modlab_game_skyrim::graphics::read(&path)
        .map(GraphicsSettingsDto::from)
        .map_err(|e| e.to_string())
}

/// What the AI layer is allowed to know about a loadout — the same facts the
/// conflict scan and the entry table already carry, reshaped for
/// `modlab_ai`. Read-only; recomputed per call like the conflict scan.
fn loadout_facts(core: &Core, loadout_id: i64) -> Result<LoadoutFacts, String> {
    let (game_id, name) = loadout::list(core)
        .map_err(|e| e.to_string())?
        .into_iter()
        .find(|l| l.id == loadout_id)
        .map(|l| (l.game_id, l.name))
        .unwrap_or_else(|| ("?".to_string(), "?".to_string()));
    let rows = loadout::entries(core, loadout_id).map_err(|e| e.to_string())?;
    let report = conflicts::report_for_entries(&rows);
    let mods = rows
        .iter()
        .map(|r| {
            let share = report.mods.iter().find(|m| m.mod_id == r.mod_id);
            ModFact {
                name: r.name.clone(),
                enabled: r.enabled,
                wins: share.map(|m| m.wins).unwrap_or(0),
                loses: share.map(|m| m.loses).unwrap_or(0),
            }
        })
        .collect();
    Ok(LoadoutFacts {
        game_id,
        loadout_name: name,
        mods,
        unreadable: report
            .unreadable
            .iter()
            .map(|(id, why)| format!("{id}: {why}"))
            .collect(),
    })
}

/// Where a local GGUF lives, if one does: `MODLAB_LLM_MODEL` wins, else the
/// first `.gguf` under `<data root>/models`. No discovery beyond that —
/// model management is a later, explicit feature.
fn find_llm_model(data_root: &Path) -> Option<PathBuf> {
    if let Some(from_env) = std::env::var_os("MODLAB_LLM_MODEL") {
        let p = PathBuf::from(from_env);
        if p.is_file() {
            return Some(p);
        }
    }
    let mut entries: Vec<PathBuf> = std::fs::read_dir(models_dir(data_root))
        .ok()?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|x| x.to_str()) == Some("gguf"))
        .collect();
    entries.sort();
    entries.into_iter().next()
}

fn models_dir(data_root: &Path) -> PathBuf {
    data_root.join("models")
}

/// What the AI panel shows: the text plus *who* produced it — the rules
/// engine alone, or the local LLM phrasing the rules engine's facts (§6).
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiAnswerDto {
    pub text: String,
    /// `"rules"` or `"local-llm"` — the chat renders it as a source badge.
    pub source: String,
    pub model: Option<String>,
}

/// Answer a question about the active loadout. §6 in practice: the rules
/// engine diagnoses from Core facts; the local LLM (feature `local-llm`,
/// model found via `MODLAB_LLM_MODEL` or `<data root>/models/*.gguf`) only
/// rephrases those facts and is refused outright while a game runs.
#[tauri::command]
fn ai_ask(
    state: State<AppState>,
    loadout_id: i64,
    question: String,
) -> Result<AiAnswerDto, String> {
    let question = question.trim();
    if question.is_empty() {
        return Err("Pose ta question d'abord.".into());
    }
    if question.chars().count() > 2000 {
        return Err("La question dépasse 2000 caractères.".into());
    }
    if state.game_running.load(Ordering::SeqCst) {
        return Err(
            "Un jeu tourne : l'inférence locale est suspendue pendant la \
             session (§6). Relance ta demande une fois la partie terminée."
                .into(),
        );
    }
    let facts = {
        let core = state.core.lock().map_err(|e| e.to_string())?;
        loadout_facts(&core, loadout_id)?
    };
    let diags = diagnose(&facts);
    let rules = rules_answer(question, &facts, &diags);

    let answer = match find_llm_model(&state.data_root) {
        Some(path) => match llm_phrase(&path, question, &facts, &diags, 256) {
            Ok(text) => AiAnswerDto {
                text,
                source: "local-llm".into(),
                model: Some(path.display().to_string()),
            },
            Err(reason) => AiAnswerDto {
                text: format!("{rules}\n\n(Note : l'inférence locale n'a pas tourné — {reason}.)"),
                source: "rules".into(),
                model: None,
            },
        },
        None => AiAnswerDto {
            text: format!(
                "{rules}\n\n(Note : aucun modèle local — dépose un .gguf dans \
                 {} ou définis MODLAB_LLM_MODEL. Le moteur de règles a répondu \
                 seul.)",
                models_dir(&state.data_root).display()
            ),
            source: "rules".into(),
            model: None,
        },
    };
    Ok(answer)
}

/// The journal the transactions panel lists — what happened, when, and the
/// steps it took. Newest first, as users read it.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransactionDto {
    pub id: i64,
    pub occurred_at: String,
    pub kind: String,
    pub steps: Vec<String>,
    /// Which loadout this is about — only populated for snapshot/rollback
    /// entries, whose payload structurally carries it. `null` for everything
    /// else; a client filtering "this loadout's revisions" must expect gaps.
    pub loadout_id: Option<i64>,
}

#[tauri::command]
fn get_transactions(
    state: State<AppState>,
    limit: Option<usize>,
) -> Result<Vec<TransactionDto>, String> {
    let core = state.core.lock().map_err(|e| e.to_string())?;
    let records =
        transaction::list(&core, limit.unwrap_or(50).clamp(1, 200)).map_err(|e| e.to_string())?;
    Ok(records
        .into_iter()
        .map(|r| TransactionDto {
            id: r.id,
            occurred_at: r.occurred_at,
            kind: r.kind,
            steps: r.steps,
            loadout_id: r.loadout_id,
        })
        .collect())
}

/// Capture the active loadout's configuration (§5 snapshot step). Returns
/// the event id the panel can point at.
#[tauri::command]
fn snapshot_loadout(state: State<AppState>, loadout_id: i64) -> Result<i64, String> {
    let core = state.core.lock().map_err(|e| e.to_string())?;
    transaction::snapshot_loadout(&core, loadout_id).map_err(|e| e.to_string())
}

/// Restore the loadout to its most recent snapshot and hand back the fresh
/// entry list — the panel refreshes the Mods table from this directly.
#[tauri::command]
fn rollback_loadout(state: State<AppState>, loadout_id: i64) -> Result<Vec<ModRowDto>, String> {
    let core = state.core.lock().map_err(|e| e.to_string())?;
    transaction::rollback_loadout(&core, loadout_id).map_err(|e| e.to_string())?;
    entries_dto(&core, loadout_id)
}

pub fn run() {
    let data_root = resolve_data_root();
    let core = open_core(&data_root).expect("failed to open the ModLab engine");
    let _ = core.log_event("ui.started", "{}");

    tauri::Builder::default()
        .manage(AppState {
            core: Mutex::new(core),
            data_root,
            game_running: AtomicBool::new(false),
            active_game: Mutex::new(None),
        })
        .setup(|app| {
            // Must happen here, before the event loop runs — see
            // `browser::create_parked` for why lazy creation deadlocks.
            if let Err(e) = browser::create_parked(app.handle()) {
                eprintln!("[browser] indisponible : {e}");
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_shell_snapshot,
            game_path_slots,
            set_game_path_override,
            clear_game_path_override,
            set_mod_enabled,
            get_loadout_conflicts,
            mod_dependencies,
            loadout_directory,
            loadout_plugins,
            loadout_archives,
            skyrim_saves,
            create_loadout,
            rename_loadout,
            set_loadout_channel,
            delete_loadout,
            list_deleted_loadouts,
            restore_loadout,
            get_loadout_version,
            mo2_import,
            minecraft_import_instance,
            minecraft_install_jar,
            minecraft_content,
            browser_show,
            browser_hide,
            browser_navigate,
            browser_go,
            downloads_dir_setting,
            set_downloads_dir_override,
            clear_downloads_dir_override,
            list_downloads,
            attach_download,
            set_mod_category,
            rename_mod,
            delete_mod,
            uninstall_mod,
            reveal_path,
            mod_categories,
            reassign_mod,
            inspect_archived_mod,
            install_archived_mod,
            get_fomod_module,
            install_fomod,
            launch_skse,
            skyrim_graphics_settings,
            set_skyrim_graphics_settings,
            ai_ask,
            get_transactions,
            snapshot_loadout,
            rollback_loadout
        ])
        .run(tauri::generate_context!())
        .expect("error while running ModLab");
}

#[cfg(test)]
mod tests {
    use super::*;
    use modlab_core::loadout::ImportEntry;
    use std::fs;

    /// `ai_ask` is built on these facts; they must say about the install
    /// exactly what the conflict scan says, or the AI panel and the Mods
    /// table tell two different stories.
    #[test]
    fn ai_facts_match_the_conflict_scan() {
        let core = Core::open_in_memory().unwrap();
        let lo = loadout::create(&core, "skyrimse", "Test").unwrap();

        let dir_a = scratch("ai-a");
        let dir_b = scratch("ai-b");
        fs::write(dir_a.join("config.txt"), b"a").unwrap();
        fs::write(dir_b.join("config.txt"), b"b").unwrap();

        loadout::import_mods(
            &core,
            lo.id,
            &[
                ImportEntry {
                    name: "Mod A".into(),
                    source_path: dir_a.clone(),
                    enabled: true,
                    version: None,
                },
                ImportEntry {
                    name: "Mod B".into(),
                    source_path: dir_b.clone(),
                    enabled: true,
                    version: None,
                },
                ImportEntry {
                    name: "Mod Off".into(),
                    source_path: dir_a.join("nope").join("deeper"),
                    enabled: false,
                    version: None,
                },
            ],
        )
        .unwrap();

        let facts = loadout_facts(&core, lo.id).unwrap();

        assert_eq!(facts.loadout_name, "Test");
        assert_eq!(facts.game_id, "skyrimse");
        assert_eq!(facts.total(), 3);
        assert_eq!(facts.enabled_count(), 2);

        let a = facts.mods.iter().find(|m| m.name == "Mod A").unwrap();
        let b = facts.mods.iter().find(|m| m.name == "Mod B").unwrap();
        // B comes after A in the order, so B is the last link: it wins, A loses.
        assert_eq!(
            (a.wins, a.loses),
            (0, 1),
            "A is overridden: {:?}",
            (a.wins, a.loses)
        );
        assert_eq!((b.wins, b.loses), (1, 0), "B wins: {:?}", (b.wins, b.loses));

        // The disabled mod's missing folder must not leak into `unreadable`:
        // the scan never walks disabled mods.
        assert!(facts.unreadable.is_empty());

        let diags = modlab_ai::diagnose(&facts);
        assert!(
            diags.iter().any(|d| d.title.contains("Mod A")),
            "the loser shows up in the diagnosis: {diags:?}"
        );

        for dir in [dir_a, dir_b] {
            let _ = fs::remove_dir_all(dir);
        }
    }

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
}
