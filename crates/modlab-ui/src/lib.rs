//! Tauri host for the React shell. This crate is a *client* of
//! `modlab-core` — same rule as the CLI: no private state, no game rules.

pub mod browser;
mod commands;
mod download_progress;
pub mod downloads;
mod ipc_paths;
mod msa_store;
mod paths;
mod source_page;

use commands::launch::{skyrim_install_paths, SessionReportDto};
use modlab_core::game_adapter::{fuse_path, GameAdapter, ToolClassifyQuery};
use modlab_core::{download, loadout, transaction, Core};
use modlab_game_minecraft::MinecraftAdapter;
use modlab_game_skyrim::SkyrimAdapter;
use serde::Serialize;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

pub struct AppState {
    core: Mutex<Core>,
    data_root: PathBuf,
    /// §6 lifecycle guard: the local LLM is never allowed to infer while a
    /// game session runs. `launch_skse` sets this for the whole session.
    game_running: AtomicBool,
    /// Raised by `stop_game_session`; the session thread polls this.
    pub(crate) session_stop: AtomicBool,
    /// JVM PID while a Minecraft session is active (`stop_game_session` uses it).
    pub(crate) minecraft_pid: Mutex<Option<u32>>,
    /// Loadout tied to the in-flight session (for forced stop / reports).
    pub(crate) active_session_loadout: Mutex<Option<i64>>,
    /// Whichever game's page the embedded browser was last opened on — set
    /// by `browser_show`. Downloads captured while browsing land under this
    /// game's own subfolder (`downloads::on_requested`); `None` before the
    /// browser has ever been opened, or for a page opened with no active
    /// game selected.
    active_game: Mutex<Option<String>>,
    /// The instance on screen. A browser download is filed into it the
    /// moment it starts. `None` until the shell has selected one — the
    /// capture then stays unfiled until the UI attaches it.
    active_loadout: Mutex<Option<i64>>,
    /// Transfers filed by `downloads::on_requested` and not finished yet.
    in_flight: Mutex<Vec<downloads::InFlight>>,
    /// Last main-frame page the embedded browser finished loading, or the
    /// address `navigate` was just given. A download reads this instead of
    /// asking the webview for its URL: that call waits on the UI thread,
    /// which is already inside the download callback.
    browser_page: Mutex<Option<String>>,
    /// Page and host snapshotted when a transfer starts, so a capture that
    /// is filed only at the end still has them.
    pending_origins: Mutex<Vec<downloads::PendingOrigin>>,
    /// What SKSE's journals said about the last finished session — kept so a
    /// reloaded window can still show it (`game_session_state`).
    last_session: Mutex<Option<SessionReportDto>>,
    /// Mod ids whose install, archive inspection, or uninstall is running
    /// on a background thread. A second call for the same id is refused.
    /// Different ids may overlap; `core` still serializes database writes.
    busy_mods: Mutex<HashSet<i64>>,
    /// In-flight Microsoft device-code login (Compte).
    msa_pending: Mutex<Option<commands::msa::PendingMsaLogin>>,
}

/// Released on drop, including when the background job returns an error.
pub(crate) struct ModBusyGuard<'a> {
    state: &'a AppState,
    mod_id: i64,
}

impl Drop for ModBusyGuard<'_> {
    fn drop(&mut self) {
        if let Ok(mut busy) = self.state.busy_mods.lock() {
            busy.remove(&self.mod_id);
        }
    }
}

impl AppState {
    /// Claim `mod_id` for one install, inspection, or uninstall.
    pub(crate) fn claim_mod(&self, mod_id: i64) -> Result<ModBusyGuard<'_>, String> {
        let mut busy = self.busy_mods.lock().map_err(|e| e.to_string())?;
        if !busy.insert(mod_id) {
            return Err("Ce mod a déjà une opération en cours.".into());
        }
        drop(busy);
        Ok(ModBusyGuard {
            state: self,
            mod_id,
        })
    }
}

/// Run `job` off the UI thread. A synchronous Tauri command runs on the
/// Windows event loop; a long one makes the window report "Ne répond pas".
pub(crate) async fn off_ui<T, F>(job: F) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, String> + Send + 'static,
{
    match tauri::async_runtime::spawn_blocking(job).await {
        Ok(result) => result,
        Err(err) => Err(format!("tâche interrompue : {err}")),
    }
}

/// Resets `game_running` on drop — including on every early-return path.
pub(crate) struct GameSessionGuard<'a>(pub(crate) &'a AtomicBool);

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
    /// Branch name. The sidebar shows `instance_name` instead.
    pub name: String,
    /// Parent instance. Branches that share this id belong together; a
    /// branch of another instance never does.
    pub instance_id: i64,
    pub instance_name: String,
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
    pub mc_loader: Option<String>,
    pub mc_game_version: Option<String>,
    pub mc_loader_version: Option<String>,
}

/// Build a `LoadoutDto` from a summary row — keeps shell and commands aligned.
pub(crate) fn loadout_dto(
    core: &modlab_core::Core,
    summary: &modlab_core::loadout::LoadoutSummary,
) -> Result<LoadoutDto, String> {
    let mod_count = modlab_core::loadout::entries(core, summary.id)
        .map(|rows| rows.len() as i64)
        .unwrap_or(0);
    let revision = modlab_core::transaction::revision_count(core, summary.id).unwrap_or(0);
    Ok(LoadoutDto {
        id: summary.id,
        game_id: summary.game_id.clone(),
        name: summary.name.clone(),
        instance_id: summary.instance_id,
        instance_name: summary.instance_name.clone(),
        mod_count,
        channel: summary.channel.clone(),
        revision,
        mc_loader: summary.mc_loader.clone(),
        mc_game_version: summary.mc_game_version.clone(),
        mc_loader_version: summary.mc_loader_version.clone(),
    })
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
    /// `"downloading"` (the transfer has started, the file is not here yet),
    /// `"archived"` (captured, nothing extracted yet — the table shows it
    /// but it cannot be enabled) or `"installed"`.
    pub status: String,
    /// The human's own free tag, `None` until they type one.
    pub category: Option<String>,
    pub archive_path: Option<String>,
    /// Free loader id from the loadout's game adapter (`"skse"`,
    /// `"skse-vr"`, `"forge"`, `"fabric"`, `"neoforge"`, …), or absent.
    /// `GameAdapter::classify_tool` decides — the shell does not branch on
    /// the game and does not look at the file name itself. This flag is
    /// not a metadata cell.
    pub tool: Option<String>,
    /// Version that goes with `tool`. Absent when the adapter has none.
    /// The table's own `version` column is a separate fact.
    pub tool_version: Option<String>,
    /// Opaque host token stored on the mod (`nexus`, `curseforge`,
    /// `modrinth`, `silverlock`, `other`, `unknown`). `None` on rows that
    /// predate the column.
    pub source_host: Option<String>,
    /// Page to reopen in the embedded browser. Not the file URL.
    pub page_url: Option<String>,
    /// Game versions the file declared. Empty when it declared none.
    /// A row still downloading has none yet.
    pub game_versions: Vec<String>,
    /// Loader ids (`fabric`, `forge`, `neoforge`, `quilt`). Empty when the
    /// file did not say, and on a row that is still downloading.
    pub loaders: Vec<String>,
    /// Dependency names or ids the file declared.
    pub dependencies: Vec<String>,
    /// True when `field_sources.category` is `user`. An automatic fill
    /// will not replace that choice.
    pub category_user_set: bool,
    /// Category text as the file wrote it, before the game list translates it.
    pub category_raw: Option<String>,
    /// Nexus category number when the file had one.
    pub category_nexus_id: Option<String>,
    pub nexus_mod_id: Option<String>,
    pub author: Option<String>,
    pub website: Option<String>,
    /// Byte length of the mod file. `None` until it is known. Kept after
    /// the file is removed.
    pub size_bytes: Option<i64>,
    /// When the mod became installed, ISO 8601 UTC. `None` before that,
    /// and while the row is still a download or an archive.
    pub installed_at: Option<String>,
    /// When the mod was downloaded or otherwise obtained, ISO 8601 UTC.
    /// Set as soon as a download starts. `None` when it could not be
    /// recovered. Kept after the file is removed.
    pub downloaded_at: Option<String>,
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
pub(crate) fn adapters() -> [&'static dyn GameAdapter; 2] {
    [&SkyrimAdapter, &MinecraftAdapter]
}

/// One `PathSlot`'s effective value: a human override if the Core has one,
/// else the adapter's own live detection. Every command that actually *uses*
/// a path (launching the game, deploying mods) must go through this instead
/// of calling adapter detection directly — otherwise an override set from
/// Réglages → Jeux is silently ignored, which is exactly the bug that made
/// `launch_skse` keep reporting the DLL as missing after it was overridden.
pub(crate) fn resolve_slot(
    core: &Core,
    adapter: &dyn GameAdapter,
    slot_id: &str,
) -> Option<PathBuf> {
    let detected = adapter
        .path_slots()
        .into_iter()
        .find(|slot| slot.id == slot_id)
        .and_then(|slot| slot.detected);
    // A database error falls through to detection, same as before: a Réglages
    // read must not hide a path the adapter can still see.
    let override_path = core
        .game_path_override(adapter.id(), slot_id)
        .ok()
        .flatten()
        .map(PathBuf::from);
    fuse_path(detected, override_path).effective
}

pub(crate) fn detect_games() -> Vec<DetectedGameDto> {
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

pub(crate) fn resolve_data_root() -> PathBuf {
    modlab_core::resolve_data_root()
}

pub(crate) fn open_core(data_root: &std::path::Path) -> Result<Core, String> {
    modlab_core::open_at(data_root).map_err(|e| e.to_string())
}

/// Existing instances that own a folder but were created before that folder
/// was stored get one now. Jars already sitting in a global install are not
/// moved. Shared-install games are left alone.
fn backfill_instance_roots(core: &Core, data_root: &Path) -> Result<(), String> {
    let mut seen = std::collections::HashSet::new();
    for branch in loadout::list(core).map_err(|e| e.to_string())? {
        if !seen.insert(branch.instance_id) {
            continue;
        }
        let Some(adapter) = adapters()
            .into_iter()
            .find(|adapter| adapter.id() == branch.game_id)
        else {
            continue;
        };
        modlab_core::instance_root::ensure_instance_root(
            core,
            data_root,
            branch.instance_id,
            adapter.instance_root_policy(),
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn mc_loader_marker(data_root: &Path, loadout_id: i64) -> PathBuf {
    data_root
        .join("placed")
        .join(format!("branch-mc-loader-{loadout_id}.txt"))
}

fn find_mc_loader_row(
    core: &Core,
    loadout_id: i64,
    data_root: &Path,
) -> Result<Option<i64>, String> {
    let rows = loadout::entries(core, loadout_id).map_err(|e| e.to_string())?;
    if let Ok(text) = std::fs::read_to_string(mc_loader_marker(data_root, loadout_id)) {
        if let Ok(id) = text.trim().parse::<i64>() {
            if rows.iter().any(|row| row.mod_id == id) {
                return Ok(Some(id));
            }
        }
    }
    Ok(rows
        .into_iter()
        .find(|row| {
            row.source_path.to_str().is_some_and(|path| {
                path.starts_with(modlab_game_minecraft::branch_loader::SOURCE_PREFIX)
            })
        })
        .map(|row| row.mod_id))
}

/// Keep the branch-loader row aligned with `versions/` on disk (Minecraft only).
pub(crate) fn sync_mc_branch_loader(
    core: &Core,
    data_root: &Path,
    loadout_id: i64,
) -> Result<(), String> {
    use modlab_core::mod_meta::{apply_layer, Layer, Patch};
    use modlab_game_minecraft::branch_loader::{display_name, source_token, SOURCE_PREFIX};

    let summary = loadout::list(core)
        .map_err(|e| e.to_string())?
        .into_iter()
        .find(|row| row.id == loadout_id)
        .ok_or_else(|| format!("branche {loadout_id} introuvable"))?;
    if summary.game_id != MinecraftAdapter.id() {
        return Ok(());
    }
    let Some(loader) = summary
        .mc_loader
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    else {
        return Ok(());
    };
    let Some(mc) = summary
        .mc_game_version
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    else {
        return Ok(());
    };
    if loader != "vanilla"
        && summary
            .mc_loader_version
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .is_none()
    {
        return Ok(());
    }
    let target = paths::install_target_for_loadout(core, data_root, loadout_id)?
        .ok_or_else(|| "Cette instance n'a pas de dossier dédié.".to_string())?;
    let installed = modlab_game_minecraft::loader_install::versions_installed(
        &target.instance_root,
        loader,
        mc,
        summary.mc_loader_version.as_deref(),
    );
    let token = source_token(loader, mc, summary.mc_loader_version.as_deref());
    let name = display_name(loader);
    let version = if loader == "vanilla" {
        Some(mc.to_string())
    } else {
        summary
            .mc_loader_version
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
    };

    let existing = find_mc_loader_row(core, loadout_id, data_root)?;

    if !installed {
        if let Some(mod_id) = existing {
            let _ = loadout::set_enabled(core, loadout_id, mod_id, false);
        }
        return Ok(());
    }

    let token_path = PathBuf::from(&token);
    if let Some(mod_id) = existing {
        download::rename(core, mod_id, name).map_err(|e| e.to_string())?;
        if let Some(ver) = version.as_deref() {
            apply_layer(
                core,
                mod_id,
                Layer::Archive,
                &Patch {
                    version: Some(ver.to_string()),
                    ..Patch::default()
                },
                &[],
                true,
            )
            .map_err(|e| e.to_string())?;
        }
        download::mark_installed(core, mod_id, &token_path).map_err(|e| e.to_string())?;
        loadout::set_enabled(core, loadout_id, mod_id, true).map_err(|e| e.to_string())?;
        return Ok(());
    }

    let imported = loadout::import_mods(
        core,
        loadout_id,
        &[loadout::ImportEntry {
            name: name.to_string(),
            source_path: token_path.clone(),
            enabled: true,
            version: version.clone(),
        }],
    )
    .map_err(|e| e.to_string())?;
    if imported == 0 {
        return Err("import du loader impossible".into());
    }
    let rows = loadout::entries(core, loadout_id).map_err(|e| e.to_string())?;
    let mod_id = rows
        .iter()
        .find(|row| {
            row.source_path
                .to_str()
                .is_some_and(|path| path.starts_with(SOURCE_PREFIX))
        })
        .map(|row| row.mod_id)
        .ok_or_else(|| "ligne loader introuvable après import".to_string())?;
    if let Some(parent) = mc_loader_marker(data_root, loadout_id).parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    std::fs::write(mc_loader_marker(data_root, loadout_id), mod_id.to_string())
        .map_err(|e| e.to_string())?;
    Ok(())
}

pub(crate) fn entries_dto(core: &Core, loadout_id: i64) -> Result<Vec<ModRowDto>, String> {
    let downloads = download::list(core).map_err(|e| e.to_string())?;
    let rows = loadout::entries(core, loadout_id).map_err(|e| e.to_string())?;
    let adapter = adapter_for_loadout(core, loadout_id)?;
    let ids: Vec<i64> = rows.iter().map(|row| row.mod_id).collect();
    let user_set = modlab_core::mod_meta::category_user_flags(core, &ids)
        .map_err(|e| e.to_string())?
        .into_iter()
        .collect::<std::collections::HashMap<_, _>>();
    Ok(rows
        .into_iter()
        .map(|row| {
            let installed = row.status == modlab_core::loadout::STATUS_INSTALLED;
            let capture = downloads
                .iter()
                .find(|item| item.mod_id == Some(row.mod_id));
            let mark = adapter.and_then(|adapter| {
                adapter.classify_tool(&ToolClassifyQuery {
                    name: &row.name,
                    source_path: &row.source_path,
                    archive_path: row.archive_path.as_deref(),
                    stored_version: row.version.as_deref(),
                    source_url: capture.map(|item| item.url.as_str()).unwrap_or(""),
                    download_file_name: capture.map(|item| item.file_name.as_str()).unwrap_or(""),
                })
            });
            let downloading = row.status == modlab_core::loadout::STATUS_DOWNLOADING;
            ModRowDto {
                mod_id: row.mod_id,
                name: row.name,
                source_path: row.source_path.display().to_string(),
                enabled: row.enabled,
                position: row.position,
                version: row.version,
                status: row.status,
                category: row.category,
                archive_path: row.archive_path.map(|path| path.display().to_string()),
                tool: mark.as_ref().map(|found| found.tool.clone()),
                tool_version: mark.and_then(|found| found.version),
                source_host: row.source_host,
                page_url: row.page_url,
                game_versions: if downloading {
                    Vec::new()
                } else {
                    row.game_versions
                },
                loaders: if downloading { Vec::new() } else { row.loaders },
                dependencies: row.dependencies.into_iter().map(|dep| dep.id).collect(),
                category_user_set: user_set.get(&row.mod_id).copied().unwrap_or(false),
                category_raw: row.category_raw,
                category_nexus_id: row.category_nexus_id,
                nexus_mod_id: row.nexus_mod_id,
                author: row.author,
                website: row.website,
                size_bytes: row.size_bytes,
                installed_at: if installed { row.installed_at } else { None },
                downloaded_at: row.downloaded_at,
            }
        })
        .collect())
}

/// The loadout's own adapter, so a Minecraft row is not classified with
/// Skyrim's rules. `None` when the branch is gone or the game has no
/// adapter — `classify_tool` then contributes nothing.
pub(crate) fn adapter_for_loadout(
    core: &Core,
    loadout_id: i64,
) -> Result<Option<&'static dyn GameAdapter>, String> {
    let game_id = loadout::list(core)
        .map_err(|e| e.to_string())?
        .into_iter()
        .find(|branch| branch.id == loadout_id)
        .map(|branch| branch.game_id);
    let Some(game_id) = game_id else {
        return Ok(None);
    };
    Ok(adapters()
        .into_iter()
        .find(|adapter| adapter.id() == game_id))
}

/// After a file has landed, or when Info opens: filename, then the local
/// archive or installed folder, then the game's category list. A missing
/// file leaves whatever is already stored.
pub(crate) fn enrich_mod_file(core: &Core, loadout_id: i64, mod_id: i64) -> Result<(), String> {
    refresh_mod_metadata(core, loadout_id, mod_id)
}

/// After a successful install: sizes and dates only. Archive walks and
/// category resolution wait for branch hydration (`get_shell_snapshot` with
/// `hydrate_metadata: true`, or an explicit `refresh_mod_metadata` call).
pub(crate) fn refresh_mod_metadata_after_install(core: &Core, mod_id: i64) -> Result<(), String> {
    modlab_core::mod_measure::refresh_one(core, mod_id).map_err(|e| e.to_string())
}

/// Replay the local metadata layers on one mod. Safe to call more than once.
pub(crate) fn refresh_mod_metadata(
    core: &Core,
    loadout_id: i64,
    mod_id: i64,
) -> Result<(), String> {
    let rows = loadout::entries(core, loadout_id).map_err(|e| e.to_string())?;
    let Some(row) = rows.into_iter().find(|row| row.mod_id == mod_id) else {
        return Ok(());
    };
    if row.status == modlab_core::loadout::STATUS_DOWNLOADING {
        return Ok(());
    }
    let downloads = download::list(core).map_err(|e| e.to_string())?;
    let mut file_names = Vec::new();
    if let Some(captured) = downloads.iter().find(|item| item.mod_id == Some(mod_id)) {
        file_names.push(captured.file_name.clone());
    }
    if let Some(name) = row.archive_path.as_ref().and_then(|path| path.file_name()) {
        file_names.push(name.to_string_lossy().into_owned());
    }
    if let Some(name) = row.source_path.file_name() {
        file_names.push(name.to_string_lossy().into_owned());
    }

    let adapter = adapter_for_loadout(core, loadout_id)?;
    let facts = file_names
        .iter()
        .map(|name| modlab_core::filename_meta::parse_file_name(name))
        .find(|facts| facts.nexus_mod_id.is_some());
    let remote_id = facts
        .as_ref()
        .and_then(|facts| facts.nexus_mod_id.clone())
        .or_else(|| row.nexus_mod_id.clone());
    let host = row.source_host.as_deref().unwrap_or("").trim();
    let host_allows_nexus = host.is_empty() || host.eq_ignore_ascii_case("nexus");
    let (derived_host, derived_page) = match (adapter, remote_id) {
        (Some(adapter), Some(id)) if host_allows_nexus => {
            let page = adapter.remote_page("nexus", &id);
            let host = if host.is_empty() && page.is_some() {
                Some("nexus".to_string())
            } else {
                None
            };
            (host, page)
        }
        _ => (None, None),
    };

    let from_dir = row
        .source_path
        .is_dir()
        .then(|| adapter.and_then(|adapter| adapter.read_archive_metadata(&row.source_path)))
        .flatten();
    let from_archive = row
        .archive_path
        .as_ref()
        .filter(|path| path.is_file())
        .and_then(|path| adapter.and_then(|adapter| adapter.read_archive_metadata(path)));
    let archive = match (from_dir, from_archive) {
        (Some(dir), Some(file)) => Some(dir.overlay_missing(file)),
        (Some(dir), None) => Some(dir),
        (None, Some(file)) => Some(file),
        (None, None) => None,
    };

    let filename_extra = adapter.and_then(|adapter| best_filename_extra(adapter, &file_names));

    let keep_tool = adapter
        .and_then(|adapter| {
            adapter.classify_tool(&ToolClassifyQuery {
                name: &row.name,
                source_path: &row.source_path,
                archive_path: row.archive_path.as_deref(),
                stored_version: row.version.as_deref(),
                source_url: downloads
                    .iter()
                    .find(|item| item.mod_id == Some(mod_id))
                    .map(|item| item.url.as_str())
                    .unwrap_or(""),
                download_file_name: file_names.first().map(String::as_str).unwrap_or(""),
            })
        })
        .is_some();

    modlab_core::mod_meta::refresh_stored(
        core,
        mod_id,
        &file_names,
        derived_host.as_deref(),
        derived_page.as_deref(),
        archive.as_ref(),
        filename_extra.as_ref(),
        keep_tool,
    )
    .map_err(|e| e.to_string())?;
    modlab_core::mod_measure::refresh_one(core, mod_id).map_err(|e| e.to_string())?;
    if let Some(adapter) = adapter {
        apply_resolved_category(core, loadout_id, mod_id, adapter)?;
    }
    Ok(())
}

/// Map whatever the file stored (`category_raw`, a Nexus id, a FOMOD group,
/// a clear name token) onto this game's category id. A `user` choice is
/// left alone — `apply_layer` refuses it, and we skip the call entirely.
fn apply_resolved_category(
    core: &Core,
    loadout_id: i64,
    mod_id: i64,
    adapter: &dyn GameAdapter,
) -> Result<(), String> {
    if modlab_core::mod_meta::recorded_layer(core, mod_id, "category")
        .map_err(|e| e.to_string())?
        .is_some_and(|layer| layer == modlab_core::mod_meta::Layer::User)
    {
        return Ok(());
    }
    let rows = loadout::entries(core, loadout_id).map_err(|e| e.to_string())?;
    let Some(row) = rows.into_iter().find(|row| row.mod_id == mod_id) else {
        return Ok(());
    };
    let nexus_id = row
        .category_nexus_id
        .as_deref()
        .and_then(|raw| raw.trim().parse::<i64>().ok());
    let raw = row
        .category_raw
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| {
            row.category_nexus_id
                .as_deref()
                .filter(|_| nexus_id.is_none())
        });
    let folder = row
        .source_path
        .is_dir()
        .then_some(row.source_path.as_path());
    let archive = row
        .archive_path
        .as_deref()
        .filter(|path| path.is_file())
        .or_else(|| {
            row.source_path
                .is_file()
                .then_some(row.source_path.as_path())
        });
    let hint = modlab_core::game_adapter::CategoryHint {
        nexus_id,
        raw,
        groups: &[],
        name: Some(row.name.as_str()),
        folder,
        archive,
    };
    let Some(id) = adapter.resolve_category(&hint) else {
        return apply_category_fallback(core, mod_id, adapter, row.category.as_deref());
    };
    if row.category.as_deref() == Some(id) {
        return Ok(());
    }
    // An unsourced value outside the game's list (an old free-text tag) is
    // not a human choice worth protecting: `seed` lets the resolved id replace it.
    let stale = row.category.as_deref().is_some_and(|value| {
        !value.trim().is_empty() && !adapter.categories().iter().any(|c| c.id == value.trim())
    });
    let from_stored = adapter.resolve_category(&modlab_core::game_adapter::CategoryHint {
        folder: None,
        archive: None,
        ..hint
    });
    let layer = if from_stored == Some(id) {
        if nexus_id.is_some_and(|value| value != 0 && value != -1) {
            modlab_core::mod_meta::Layer::Api
        } else if raw_counts_as_label(raw) {
            modlab_core::mod_meta::Layer::Archive
        } else {
            modlab_core::mod_meta::Layer::Filename
        }
    } else {
        modlab_core::mod_meta::Layer::Archive
    };
    modlab_core::mod_meta::apply_layer(
        core,
        mod_id,
        layer,
        &modlab_core::mod_meta::Patch {
            category: Some(id.to_string()),
            ..modlab_core::mod_meta::Patch::default()
        },
        &[],
        stale,
    )
    .map_err(|e| e.to_string())
}

/// Nothing matched. `other` (« Autre ») is stored on the weakest layer so a
/// later archive or API read can replace it. A category already in the
/// game's list, including one the human cleared, is left alone.
fn apply_category_fallback(
    core: &Core,
    mod_id: i64,
    adapter: &dyn GameAdapter,
    current: Option<&str>,
) -> Result<(), String> {
    let known = adapter.categories();
    let Some(other) = known.iter().find(|category| category.id == "other") else {
        return Ok(());
    };
    let current = current.unwrap_or("").trim();
    if known.iter().any(|category| category.id == current) {
        return Ok(());
    }
    let stale = !current.is_empty();
    modlab_core::mod_meta::apply_layer(
        core,
        mod_id,
        modlab_core::mod_meta::Layer::Filename,
        &modlab_core::mod_meta::Patch {
            category: Some(other.id.to_string()),
            ..modlab_core::mod_meta::Patch::default()
        },
        &[],
        stale,
    )
    .map_err(|e| e.to_string())
}

fn best_filename_extra(
    adapter: &dyn GameAdapter,
    file_names: &[String],
) -> Option<modlab_core::game_adapter::ArchiveMetadata> {
    let mut fallback = None;
    for name in file_names {
        let Some(meta) = adapter.filename_metadata(name) else {
            continue;
        };
        if !meta.loaders.is_empty() || !meta.game_versions.is_empty() {
            return Some(meta);
        }
        if fallback.is_none() {
            fallback = Some(meta);
        }
    }
    fallback
}

/// `"-1,"` and `"0"` are MO2's "no category". They must not be recorded as
/// an archive label when the category actually came from the file name.
fn raw_counts_as_label(raw: Option<&str>) -> bool {
    let Some(raw) = raw.map(str::trim).filter(|value| !value.is_empty()) else {
        return false;
    };
    match modlab_core::game_adapter::parse_category_ids(raw) {
        Some(ids) => ids.iter().any(|id| *id != -1 && *id != 0),
        None => true,
    }
}

/// Replay the file-name and archive layers on every mod of a branch, then
/// the category list. A download that has not landed yet is skipped: it has
/// no jar to read. Cheap on purpose — a jar is a zip, and a 7z above 64 Mo
/// is not opened.
pub(crate) fn refresh_branch_metadata(core: &Core, loadout_id: i64) -> Result<(), String> {
    let rows = loadout::entries(core, loadout_id).map_err(|e| e.to_string())?;
    for row in rows {
        if row.status == modlab_core::loadout::STATUS_DOWNLOADING {
            continue;
        }
        refresh_mod_metadata(core, loadout_id, row.mod_id)?;
    }
    Ok(())
}

/// Fill empty categories on a branch from what is already stored and from
/// the folder or archive when one is on disk. A `user` choice, including
/// one that cleared the cell, is left alone. A cell that already holds one
/// of the game's categories is left alone too; a stale free-text value that
/// is not in the game's list counts as empty.
pub(crate) fn fill_branch_categories(core: &Core, loadout_id: i64) -> Result<(), String> {
    let Some(adapter) = adapter_for_loadout(core, loadout_id)? else {
        return Ok(());
    };
    let known = adapter.categories();
    let rows = loadout::entries(core, loadout_id).map_err(|e| e.to_string())?;
    let ids: Vec<i64> = rows.iter().map(|row| row.mod_id).collect();
    let user_set =
        modlab_core::mod_meta::category_user_flags(core, &ids).map_err(|e| e.to_string())?;
    for row in rows {
        if user_set
            .iter()
            .any(|(id, chosen)| *id == row.mod_id && *chosen)
        {
            continue;
        }
        if row
            .category
            .as_deref()
            .is_some_and(|value| known.iter().any(|c| c.id == value.trim()))
        {
            continue;
        }
        apply_resolved_category(core, loadout_id, row.mod_id, adapter)?;
    }
    Ok(())
}

/// ModLab's own folder for what it did to the game's install root: the
/// manifest of copied engine files, the originals set aside, the files taken
/// back out (`modlab_game_skyrim::engine`).
pub(crate) fn engine_store_dir(data_root: &Path) -> PathBuf {
    data_root.join("engine").join(SkyrimAdapter.id())
}

/// If `mod_id` is Skyrim's engine mod (SKSE — recognised by its content, a
/// `skse64_loader.exe` at the top of its folder, never by its name), mirror
/// the enabled state it is about to take onto the real install root: only its
/// root files (loader, `skse64_*.dll`) are concerned, see
/// `modlab_game_skyrim::engine`. A no-op for every other mod, every other
/// game, and an archived (not yet extracted) row. The launch also copies
/// them when needed (`skse::prepare_launch`), so this is not the only way in.
///
/// Returns a guard the caller must [`commit`](modlab_game_skyrim::engine::EngineInstallGuard::commit)
/// only after the Plan DB mutation succeeds. Drop without commit restores the
/// previous install-root state. `Ok(None)` means nothing was done on disk.
pub(crate) fn sync_engine_mod_direct_install(
    core: &Core,
    state: &AppState,
    loadout_id: i64,
    mod_id: i64,
    enabled: bool,
) -> Result<Option<modlab_game_skyrim::engine::EngineInstallGuard>, String> {
    let is_skyrim = loadout::list(core)
        .map_err(|e| e.to_string())?
        .into_iter()
        .any(|l| l.id == loadout_id && l.game_id == SkyrimAdapter.id());
    if !is_skyrim {
        return Ok(None);
    }
    let rows = loadout::entries(core, loadout_id).map_err(|e| e.to_string())?;
    let Some(row) = rows.into_iter().find(|r| r.mod_id == mod_id) else {
        return Ok(None);
    };
    if row.status != loadout::STATUS_INSTALLED
        || !modlab_game_skyrim::engine::is_engine_content(&row.source_path)
    {
        return Ok(None);
    }
    if state.game_running.load(Ordering::SeqCst) {
        return Err(
            "Une partie est en cours : SKSE ne peut pas être modifié avant sa fin.".to_string(),
        );
    }
    let Some(install_root) = resolve_slot(core, &SkyrimAdapter, "root") else {
        return Ok(None);
    };
    let store = engine_store_dir(&state.data_root);
    if enabled {
        // Same rule as at launch: a SKSE built for another game version is
        // never copied over the one the game folder already has.
        let game_version = skyrim_install_paths(core)
            .and_then(|(_, exe)| modlab_game_skyrim::skse::read_exe_version(&exe));
        modlab_game_skyrim::skse::check_engine_matches_game(
            &row.name,
            &row.source_path,
            game_version,
        )?;
    }
    let (summary, guard) = if enabled {
        let report =
            modlab_game_skyrim::engine::install_root_files(&row.source_path, &install_root, &store)
                .map_err(|e| e.to_string())?;
        let summary = format!(
            "SKSE : {} fichier(s) copié(s), {} mis de côté, {} déjà identique(s)",
            report.copied.len(),
            report.backed_up.len(),
            report.identical.len()
        );
        let guard = modlab_game_skyrim::engine::EngineInstallGuard::after_install(
            install_root,
            store,
            report.copied,
        );
        (summary, guard)
    } else {
        let names = modlab_game_skyrim::engine::engine_root_files(&row.source_path)
            .map_err(|e| e.to_string())?;
        let report =
            modlab_game_skyrim::engine::uninstall_root_files(&install_root, &store, Some(&names))
                .map_err(|e| e.to_string())?;
        let summary = format!(
            "SKSE : {} fichier(s) retiré(s), {} original(aux) remis, {} laissé(s) car modifié(s)",
            report.removed.len(),
            report.restored.len(),
            report.left_alone.len()
        );
        let guard = modlab_game_skyrim::engine::EngineInstallGuard::after_uninstall(
            install_root,
            store,
            row.source_path,
            report.removed,
        );
        (summary, guard)
    };
    let mut tx = transaction::Transaction::new(core, "skyrim.engine");
    tx.step(summary);
    tx.commit().map_err(|e| e.to_string())?;
    Ok(Some(guard))
}

/// What USVFS actually layers at launch: every mod, except that the engine mod
/// (SKSE, recognised by its content) is reduced to its `Data\` part — its
/// root files are copied for real (`engine::virtual_rows`). Mods merely named
/// after SKSE stay in, so their plugins reach the Plugins tab and
/// `plugins.txt`. Those two and the launch all use this set, so the tab shows
/// exactly what the game gets.
pub(crate) fn virtually_deployed_rows(
    rows: &[loadout::LoadoutEntryRow],
) -> Vec<loadout::LoadoutEntryRow> {
    modlab_game_skyrim::engine::virtual_rows(rows)
}

/// Where an instance's own `plugins.txt` lives: the data root, keyed by
/// loadout id like `mods/<id>` (a rename must not orphan it).
pub(crate) fn instance_plugins_txt(data_root: &Path, loadout_id: i64) -> PathBuf {
    data_root
        .join("loadouts")
        .join(loadout_id.to_string())
        .join("plugins.txt")
}

pub fn run() {
    let data_root = resolve_data_root();
    let core = open_core(&data_root).expect("failed to open the ModLab engine");
    let _ = core.log_event("ui.started", "{}");
    if let Err(err) = backfill_instance_roots(&core, &data_root) {
        eprintln!("[instance] {err}");
    }

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState {
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
            busy_mods: Mutex::new(HashSet::new()),
            msa_pending: Mutex::new(None),
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
            commands::shell::get_shell_snapshot,
            commands::shell::run_powershell,
            commands::paths::game_path_slots,
            paths::game_paths,
            commands::paths::game_install_entries,
            commands::paths::game_vanilla_scan,
            commands::paths::set_game_path_override,
            commands::paths::clear_game_path_override,
            commands::paths::set_instance_path_override,
            commands::paths::clear_instance_path_override,
            commands::mods::set_mod_enabled,
            commands::mods::move_mods,
            commands::mods::get_loadout_conflicts,
            commands::mods::get_loadout_dependencies,
            commands::mods::mod_dependencies,
            commands::mods::mod_config_files,
            commands::mods::loadout_directory,
            commands::mods::loadout_plugins,
            commands::mods::loadout_archives,
            commands::mods::skyrim_saves,
            commands::loadouts::create_loadout,
            commands::loadouts::create_branch,
            commands::loadouts::rename_loadout,
            commands::loadouts::set_loadout_channel,
            commands::loadouts::delete_loadout,
            commands::loadouts::list_deleted_loadouts,
            commands::loadouts::restore_loadout,
            commands::loadouts::get_loadout_version,
            commands::loadouts::mo2_import,
            commands::loadouts::minecraft_import_instance,
            commands::loadouts::minecraft_install_jar,
            commands::loadouts::minecraft_sync_mods,
            commands::loadouts::import_manual_mod,
            commands::loadouts::minecraft_content,
            commands::browser::browser_show,
            commands::browser::browser_hide,
            commands::browser::browser_navigate,
            commands::browser::browser_go,
            commands::browser::note_active_loadout,
            commands::settings::downloads_dir_setting,
            commands::settings::set_downloads_dir_override,
            commands::settings::clear_downloads_dir_override,
            commands::downloads::list_downloads,
            commands::downloads::finalize_download,
            commands::downloads::attach_download,
            commands::browser::open_external_url,
            commands::browser::official_tool_pages,
            commands::mods::set_mod_category,
            commands::mods::refresh_mod_metadata,
            commands::mods::game_categories,
            commands::mods::rename_mod,
            commands::mods::delete_mod,
            commands::mods::uninstall_mod,
            commands::browser::reveal_path,
            commands::browser::reveal_instance_folder,
            commands::mods::mod_categories,
            commands::mods::reassign_mod,
            commands::mods::inspect_archived_mod,
            commands::mods::install_archived_mod,
            commands::mods::get_fomod_module,
            commands::mods::preview_fomod,
            commands::mods::fomod_image,
            commands::mods::install_fomod,
            commands::launch::launch_skse,
            commands::launch::launch_minecraft,
            commands::launch::test_launch_minecraft,
            commands::msa::msa_account_status,
            commands::msa::msa_set_client_id,
            commands::msa::msa_begin_login,
            commands::msa::msa_poll_login,
            commands::msa::msa_cancel_login,
            commands::msa::msa_disconnect,
            commands::launch::skse_status,
            commands::launch::game_session_state,
            commands::launch::stop_game_session,
            commands::launch::skyrim_graphics_settings,
            commands::launch::set_skyrim_graphics_settings,
            commands::assistant::ai_ask,
            commands::loadouts::get_transactions,
            commands::loadouts::snapshot_loadout,
            commands::loadouts::rollback_loadout,
            commands::mc_loader::set_branch_mc_loader,
            commands::mc_loader::set_branch_mc_game_version,
            commands::mc_loader::set_branch_mc_loader_version,
            commands::mc_loader::list_mc_game_versions,
            commands::mc_loader::list_mc_loader_versions,
            commands::mc_loader::list_installed_mc_loader_versions,
            commands::mc_loader::list_installed_mc_game_versions,
            commands::mc_loader::mc_loader_status,
            commands::mc_loader::install_branch_mc_loader
        ])
        .run(tauri::generate_context!())
        .expect("error while running ModLab");
}
