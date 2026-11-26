//! The four locations the game page shows: install folder, installed mods,
//! downloads, and the INI config folder.
//!
//! Read-only. Updating a path that already has a command stays on that
//! command (`set_game_path_override` for the install folder and
//! `prefs_ini`, `set_downloads_dir_override` for the shared downloads root).
//! This module does not invent a new place for files to land.

use modlab_core::game_adapter::{
    fuse_path, ConfigLocation, GameAdapter, InstanceRootPolicy, LocationQuery, ModsLocation,
    PathKind, PathSlot, INSTANCE_ROOT_SLOT,
};
use modlab_core::{loadout, Core};
use serde::Serialize;
use std::path::{Path, PathBuf};

use super::downloads;

/// `prefs_ini` — the only INI path slot the shell already stores an override for.
const PREFS_SLOT: &str = "prefs_ini";

/// How the path was obtained. The UI badges from this, not from whether
/// `detected` is filled: a ModLab-owned folder is `default` even though the
/// path is known.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum PathOrigin {
    Detected,
    Override,
    Default,
    Missing,
}

/// Which existing command changes this path. `null` when ModLab does not
/// offer an update (the folder is derived and installs do not read an override).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "via", rename_all = "camelCase")]
pub enum PathUpdate {
    /// `set_game_path_override` / `clear_game_path_override` with this slot id.
    #[serde(rename = "gamePathOverride", rename_all = "camelCase")]
    GamePath { slot_id: String },
    /// `set_downloads_dir_override` / `clear_downloads_dir_override`.
    /// The value those commands take is `edit_path` (the shared root), not
    /// `effective` (the per-game subfolder).
    #[serde(rename = "downloadsDirOverride")]
    DownloadsDir,
    /// `set_instance_path_override` / `clear_instance_path_override`.
    /// One instance, not the whole game.
    #[serde(rename = "instancePathOverride", rename_all = "camelCase")]
    InstancePath { instance_id: i64, slot_id: String },
}

/// One of the four locations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ManagedPathDto {
    pub id: String,
    pub label: String,
    pub kind: PathKind,
    /// Path before a human override. For a ModLab-owned folder this is the
    /// built-in location, whether or not the directory exists yet.
    pub detected: Option<String>,
    pub override_path: Option<String>,
    /// The path to show and to open. Null only when ModLab cannot name one.
    pub effective: Option<String>,
    /// `effective` is an existing directory.
    pub exists: bool,
    pub origin: PathOrigin,
    /// Value a path editor should load. Null when `update` is null.
    /// For downloads this is the shared root, while `effective` is
    /// `<edit_path>/<game id>`.
    pub edit_path: Option<String>,
    pub update: Option<PathUpdate>,
}

/// One `.ini` in the config folder. `path` is null when the folder itself
/// is unknown. `slot_id` is set only for the file `set_game_path_override`
/// already accepts (`prefs_ini`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IniFileDto {
    pub name: String,
    pub path: Option<String>,
    pub present: bool,
    pub slot_id: Option<String>,
}

/// `game_paths` payload. `game_dir.effective` is the install folder,
/// `mods_dir` is where this game's installs land, `downloads_dir` is this
/// game's downloads subfolder, `config_dir` is the folder that holds `.ini`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GamePathsDto {
    pub game_id: String,
    /// Install folder. Slot `root`. Update with `set_game_path_override`.
    pub game_dir: ManagedPathDto,
    /// Skyrim: `<data root>/mods`, where an extracted archive is placed.
    /// An MO2 import keeps each mod's own `sourcePath`; this folder is not
    /// a copy of those. Minecraft: `<instance root>/mods` (`install_jar`).
    /// A stored `mods_dir` override is ignored — install reads the instance
    /// root, not that override.
    pub mods_dir: ManagedPathDto,
    /// `<downloads root>/<game id>`. `edit_path` is the shared downloads
    /// root the Réglages command stores.
    pub downloads_dir: ManagedPathDto,
    /// Skyrim: the folder of `Skyrim.ini` / `SkyrimPrefs.ini` /
    /// `SkyrimCustom.ini` (My Games, or the parent of a `prefs_ini`
    /// override). Minecraft: `<instance root>/config`.
    pub config_dir: ManagedPathDto,
    pub ini_files: Vec<IniFileDto>,
    /// Launcher or game install kept for information when installs go to a
    /// per-instance folder. `None` when `game_dir` is already that install.
    pub reference_dir: Option<ManagedPathDto>,
}

struct Ctx<'a> {
    core: &'a Core,
    data_root: &'a Path,
    adapter: &'a dyn GameAdapter,
    slots: Vec<PathSlot>,
    overrides: Vec<(String, String)>,
}

impl Ctx<'_> {
    fn slot(&self, id: &str) -> Option<&PathSlot> {
        self.slots.iter().find(|slot| slot.id == id)
    }

    fn override_path(&self, id: &str) -> Option<&str> {
        self.overrides
            .iter()
            .find(|(slot, _)| slot == id)
            .map(|(_, path)| path.as_str())
    }

    fn label(&self, id: &str, fallback: &str) -> String {
        self.slot(id)
            .map(|slot| slot.label.to_string())
            .unwrap_or_else(|| fallback.to_string())
    }
}

/// Read the four locations for `game_id`. Unknown game is an error.
pub(crate) fn game_paths_for(
    core: &Core,
    data_root: &Path,
    game_id: &str,
    loadout_id: Option<i64>,
) -> Result<GamePathsDto, String> {
    let adapter = super::adapters()
        .into_iter()
        .find(|adapter| adapter.id() == game_id)
        .ok_or_else(|| format!("jeu inconnu : {game_id}"))?;
    let overrides = core
        .game_path_overrides(adapter.id())
        .map_err(|e| e.to_string())?;
    let ctx = Ctx {
        core,
        data_root,
        adapter,
        slots: adapter.path_slots(),
        overrides,
    };
    let instance_id = instance_id_for(core, adapter.id(), loadout_id)?;
    if let (Some(instance_id), InstanceRootPolicy::PerInstanceRoot { .. }) =
        (instance_id, adapter.instance_root_policy())
    {
        return per_instance_paths(&ctx, instance_id);
    }
    let root = effective_root(&ctx);
    let query = LocationQuery {
        install_root: root.as_deref(),
        data_root,
        instance_root: None,
    };
    let (config_dir, ini_files) = match adapter.config_location(&query) {
        ConfigLocation::PrefsParent { detected } => prefs_config(&ctx, detected),
        ConfigLocation::InInstall(path) => in_install_config(path.as_deref()),
    };
    let mods_label = ctx.label("mods_dir", "Mods installés");
    let mods_dir = match adapter.mods_location(&query) {
        ModsLocation::Managed(path) => managed_dir(&path, "mods_dir", &mods_label),
        ModsLocation::InInstall(Some(path)) => install_dir(&path, "mods_dir", &mods_label),
        ModsLocation::InInstall(None) => missing("mods_dir", "Mods installés"),
    };
    Ok(GamePathsDto {
        game_id: adapter.id().to_string(),
        game_dir: game_dir(&ctx),
        mods_dir,
        downloads_dir: downloads_path(&ctx)?,
        config_dir,
        ini_files,
        reference_dir: None,
    })
}

fn instance_id_for(
    core: &Core,
    game_id: &str,
    loadout_id: Option<i64>,
) -> Result<Option<i64>, String> {
    let branches = loadout::list(core).map_err(|e| e.to_string())?;
    if let Some(loadout_id) = loadout_id {
        if let Some(branch) = branches
            .iter()
            .find(|branch| branch.id == loadout_id && branch.game_id == game_id)
        {
            return Ok(Some(branch.instance_id));
        }
    }
    Ok(branches
        .iter()
        .find(|branch| branch.game_id == game_id)
        .map(|branch| branch.instance_id))
}

fn per_instance_paths(ctx: &Ctx<'_>, instance_id: i64) -> Result<GamePathsDto, String> {
    let stored = modlab_core::instance_root::stored_instance_root(ctx.core, instance_id)
        .map_err(|e| e.to_string())?;
    let slot_overrides = ctx
        .core
        .instance_path_overrides(instance_id)
        .map_err(|e| e.to_string())?;
    let slot = |id: &str| {
        slot_overrides
            .iter()
            .find(|(slot, _)| slot == id)
            .map(|(_, path)| PathBuf::from(path))
    };
    let root = fuse_path(stored, slot(INSTANCE_ROOT_SLOT));
    let mods_name = ctx.adapter.instance_subdir("mods_dir").unwrap_or("mods");
    let config_name = ctx
        .adapter
        .instance_subdir("config_dir")
        .unwrap_or("config");
    let mods = fuse_path(
        root.effective.as_ref().map(|path| path.join(mods_name)),
        slot("mods_dir"),
    );
    let config = fuse_path(
        root.effective.as_ref().map(|path| path.join(config_name)),
        slot("config_dir"),
    );
    let config_files = config
        .effective
        .as_deref()
        .map(loose_ini_files)
        .unwrap_or_default();
    Ok(GamePathsDto {
        game_id: ctx.adapter.id().to_string(),
        game_dir: owned_path(
            "game_dir",
            "Dossier de l'instance",
            &root,
            instance_update(instance_id, INSTANCE_ROOT_SLOT),
        ),
        mods_dir: owned_path(
            "mods_dir",
            &ctx.label("mods_dir", "Dossier mods/"),
            &mods,
            instance_update(instance_id, "mods_dir"),
        ),
        downloads_dir: downloads_path(ctx)?,
        config_dir: owned_path(
            "config_dir",
            "Dossier config/",
            &config,
            instance_update(instance_id, "config_dir"),
        ),
        ini_files: config_files,
        reference_dir: Some(game_dir(ctx)),
    })
}

fn instance_update(instance_id: i64, slot_id: &str) -> PathUpdate {
    PathUpdate::InstancePath {
        instance_id,
        slot_id: slot_id.to_string(),
    }
}

fn owned_path(
    id: &str,
    label: &str,
    fused: &modlab_core::game_adapter::FusedPath,
    update: PathUpdate,
) -> ManagedPathDto {
    let origin = if fused.override_path.is_some() {
        PathOrigin::Override
    } else if fused.detected.is_some() {
        PathOrigin::Default
    } else {
        PathOrigin::Missing
    };
    let effective = fused.effective.as_deref().map(show);
    location(
        id,
        label.to_string(),
        fused.detected.as_deref().map(show),
        fused.override_path.as_deref().map(show),
        effective.clone(),
        origin,
        effective,
        Some(update),
    )
}

/// Lecture tier (§7.2). Nothing is written; clearing an override is a
/// separate command.
#[tauri::command]
pub fn game_paths(
    state: tauri::State<super::AppState>,
    game_id: String,
    loadout_id: Option<i64>,
) -> Result<GamePathsDto, String> {
    let core = state.core.lock().map_err(|e| e.to_string())?;
    game_paths_for(&core, &state.data_root, &game_id, loadout_id)
}

/// Where an install for this branch should write, after the per-instance
/// folder exists and a human override has been applied. `None` when the
/// game keeps a shared install.
pub(crate) struct InstallTarget {
    pub instance_root: PathBuf,
    pub mods_dir: PathBuf,
    pub resource_packs: PathBuf,
    pub shader_packs: PathBuf,
}

pub(crate) fn install_target_for_loadout(
    core: &Core,
    data_root: &Path,
    loadout_id: i64,
) -> Result<Option<InstallTarget>, String> {
    let branch = loadout::list(core)
        .map_err(|e| e.to_string())?
        .into_iter()
        .find(|branch| branch.id == loadout_id)
        .ok_or_else(|| format!("branche {loadout_id} introuvable"))?;
    let adapter = super::adapters()
        .into_iter()
        .find(|adapter| adapter.id() == branch.game_id)
        .ok_or_else(|| format!("jeu inconnu : {}", branch.game_id))?;
    if matches!(
        adapter.instance_root_policy(),
        InstanceRootPolicy::SharedInstall
    ) {
        return Ok(None);
    }
    let stored = modlab_core::instance_root::ensure_instance_root(
        core,
        data_root,
        branch.instance_id,
        adapter.instance_root_policy(),
    )
    .map_err(|e| e.to_string())?
    .ok_or_else(|| "dossier d'instance absent".to_string())?;
    let overrides = core
        .instance_path_overrides(branch.instance_id)
        .map_err(|e| e.to_string())?;
    let slot = |id: &str| {
        overrides
            .iter()
            .find(|(slot, _)| slot == id)
            .map(|(_, path)| PathBuf::from(path))
    };
    let root = fuse_path(Some(stored), slot(INSTANCE_ROOT_SLOT))
        .effective
        .ok_or_else(|| "dossier d'instance absent".to_string())?;
    let mods_name = adapter.instance_subdir("mods_dir").unwrap_or("mods");
    let mods_dir = fuse_path(Some(root.join(mods_name)), slot("mods_dir"))
        .effective
        .unwrap_or_else(|| root.join(mods_name));
    let packs_name = adapter
        .instance_subdir("resourcepacks")
        .unwrap_or("resourcepacks");
    let shader_name = adapter
        .instance_subdir("shaderpacks")
        .unwrap_or("shaderpacks");
    Ok(Some(InstallTarget {
        instance_root: root.clone(),
        mods_dir,
        resource_packs: root.join(packs_name),
        shader_packs: root.join(shader_name),
    }))
}

fn game_dir(ctx: &Ctx<'_>) -> ManagedPathDto {
    let detected = ctx.slot("root").and_then(|slot| slot.detected.clone());
    let override_path = ctx.override_path("root").map(PathBuf::from);
    let fused = fuse_path(detected, override_path);
    let origin = if fused.override_path.is_some() {
        PathOrigin::Override
    } else if fused.detected.is_some() {
        PathOrigin::Detected
    } else {
        PathOrigin::Missing
    };
    let effective = fused.effective.as_deref().map(show);
    location(
        "game_dir",
        ctx.label("root", "Dossier du jeu"),
        fused.detected.as_deref().map(show),
        fused.override_path.as_deref().map(show),
        effective.clone(),
        origin,
        effective,
        Some(PathUpdate::GamePath {
            slot_id: "root".to_string(),
        }),
    )
}

/// A folder ModLab owns. Origin stays `default` whether or not it exists yet.
fn managed_dir(path: &Path, id: &str, label: &str) -> ManagedPathDto {
    let shown = show_abs(path);
    location(
        id,
        label.to_string(),
        Some(shown.clone()),
        None,
        Some(shown),
        PathOrigin::Default,
        None,
        None,
    )
}

/// A folder derived from the install. `detected` when it exists, `default`
/// when the path can be named but the directory is not there yet.
fn install_dir(path: &Path, id: &str, label: &str) -> ManagedPathDto {
    let origin = if path.is_dir() {
        PathOrigin::Detected
    } else {
        PathOrigin::Default
    };
    let shown = show_abs(path);
    location(
        id,
        label.to_string(),
        Some(shown.clone()),
        None,
        Some(shown),
        origin,
        None,
        None,
    )
}

fn in_install_config(path: Option<&Path>) -> (ManagedPathDto, Vec<IniFileDto>) {
    let Some(path) = path else {
        return (missing("config_dir", "Configs (.ini)"), Vec::new());
    };
    let dir = install_dir(path, "config_dir", "Configs (.ini)");
    let files = dir
        .effective
        .as_deref()
        .map(|path| loose_ini_files(Path::new(path)))
        .unwrap_or_default();
    (dir, files)
}

fn prefs_config(ctx: &Ctx<'_>, detected_dir: Option<PathBuf>) -> (ManagedPathDto, Vec<IniFileDto>) {
    let slot = ctx.adapter.prefs_file_slot().unwrap_or(PREFS_SLOT);
    let prefs_detected = ctx.slot(slot).and_then(|slot| slot.detected.clone());
    let prefs_override = ctx.override_path(slot).map(PathBuf::from);
    let override_dir = prefs_override.as_deref().and_then(parent_dir);
    let fused = fuse_path(detected_dir, override_dir);
    let origin = if fused.override_path.is_some() {
        PathOrigin::Override
    } else if fused.detected.is_some() {
        PathOrigin::Detected
    } else {
        PathOrigin::Missing
    };
    let dir = location(
        "config_dir",
        "Configs (.ini)".to_string(),
        fused.detected.as_deref().map(show),
        fused.override_path.as_deref().map(show),
        fused.effective.as_deref().map(show),
        origin,
        None,
        None,
    );
    let prefs = prefs_override.or(prefs_detected);
    let files = named_ini_files(
        ctx.adapter.known_config_files(),
        ctx.adapter.prefs_file_slot(),
        ctx.adapter.prefs_file_name(),
        fused.effective.as_deref(),
        prefs.as_deref(),
    );
    (dir, files)
}

fn downloads_path(ctx: &Ctx<'_>) -> Result<ManagedPathDto, String> {
    let game_id = ctx.adapter.id();
    let default_root = downloads::default_downloads_root(ctx.data_root);
    let override_root = ctx
        .core
        .game_path_override(downloads::SETTINGS_SCOPE, downloads::DOWNLOADS_DIR_SLOT)
        .map_err(|e| e.to_string())?;
    let fused = fuse_path(Some(default_root.clone()), override_root.map(PathBuf::from));
    let detected = downloads::downloads_dir(&default_root, Some(game_id));
    let override_game = fused
        .override_path
        .as_ref()
        .map(|root| downloads::downloads_dir(root, Some(game_id)));
    let effective_root = fused.effective.as_deref().unwrap_or(&default_root);
    let effective = downloads::downloads_dir(effective_root, Some(game_id));
    let edit_path = effective.parent().map(show);
    let origin = if fused.override_path.is_some() {
        PathOrigin::Override
    } else {
        PathOrigin::Default
    };
    Ok(location(
        "downloads_dir",
        "Téléchargements".to_string(),
        Some(show(&detected)),
        override_game.as_ref().map(|path| show(path)),
        Some(show(&effective)),
        origin,
        edit_path,
        Some(PathUpdate::DownloadsDir),
    ))
}

fn effective_root(ctx: &Ctx<'_>) -> Option<PathBuf> {
    let detected = ctx.slot("root").and_then(|slot| slot.detected.clone());
    let override_path = ctx.override_path("root").map(PathBuf::from);
    fuse_path(detected, override_path).effective
}

fn named_ini_files(
    names: &[&str],
    prefs_slot: Option<&str>,
    prefs_file_name: Option<&str>,
    config_dir: Option<&Path>,
    prefs: Option<&Path>,
) -> Vec<IniFileDto> {
    let on_disk = config_dir.map(ini_names_in).unwrap_or_default();
    let mut out = Vec::new();
    for name in names {
        let path = config_dir.map(|dir| dir.join(name));
        out.push(IniFileDto {
            slot_id: prefs_slot_for(prefs_slot, prefs_file_name, prefs, path.as_deref(), name),
            name: name.to_string(),
            present: path.as_ref().is_some_and(|file| file.is_file()),
            path: path.as_ref().map(|file| show(file)),
        });
    }
    if let Some(dir) = config_dir {
        for name in on_disk {
            if names
                .iter()
                .any(|canonical| canonical.eq_ignore_ascii_case(&name))
            {
                continue;
            }
            let path = dir.join(&name);
            out.push(IniFileDto {
                slot_id: prefs_slot_for(prefs_slot, prefs_file_name, prefs, Some(&path), ""),
                name,
                present: path.is_file(),
                path: Some(show(&path)),
            });
        }
    }
    let slotted = out.iter().any(|file| file.slot_id.as_deref() == prefs_slot);
    if !slotted {
        if let Some(prefs) = prefs {
            let name = prefs
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .filter(|n| !n.is_empty())
                .unwrap_or_else(|| "SkyrimPrefs.ini".to_string());
            out.push(IniFileDto {
                name,
                path: Some(show(prefs)),
                present: prefs.is_file(),
                slot_id: prefs_slot.map(str::to_string),
            });
        }
    }
    out
}

fn loose_ini_files(dir: &Path) -> Vec<IniFileDto> {
    if !dir.is_dir() {
        return Vec::new();
    }
    ini_names_in(dir)
        .into_iter()
        .map(|name| {
            let path = dir.join(&name);
            IniFileDto {
                name,
                present: path.is_file(),
                path: Some(show(&path)),
                slot_id: None,
            }
        })
        .collect()
}

fn prefs_slot_for(
    slot: Option<&str>,
    canonical_prefs: Option<&str>,
    prefs: Option<&Path>,
    row: Option<&Path>,
    canonical_name: &str,
) -> Option<String> {
    match (prefs, row) {
        (Some(prefs), Some(row)) if same_path(prefs, row) => slot.map(str::to_string),
        (None, _)
            if canonical_prefs.is_some_and(|name| canonical_name.eq_ignore_ascii_case(name)) =>
        {
            slot.map(str::to_string)
        }
        _ => None,
    }
}

fn ini_names_in(dir: &Path) -> Vec<String> {
    let Ok(read) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut names = Vec::new();
    for entry in read.flatten() {
        let Ok(ty) = entry.file_type() else {
            continue;
        };
        if ty.is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.to_ascii_lowercase().ends_with(".ini") {
            names.push(name);
        }
    }
    names.sort_by_key(|name| name.to_ascii_lowercase());
    names
}

fn parent_dir(path: &Path) -> Option<PathBuf> {
    let parent = path.parent()?;
    if parent.as_os_str().is_empty() {
        None
    } else {
        Some(parent.to_path_buf())
    }
}

fn same_path(a: &Path, b: &Path) -> bool {
    let fold = |path: &Path| {
        path.components()
            .map(|c| c.as_os_str().to_string_lossy().to_ascii_lowercase())
            .collect::<Vec<_>>()
    };
    fold(a) == fold(b)
}

fn show(path: &Path) -> String {
    path.display().to_string()
}

fn show_abs(path: &Path) -> String {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map(|cwd| cwd.join(path))
            .unwrap_or_else(|_| path.to_path_buf())
    };
    show(&native_separators(absolute))
}

#[cfg(windows)]
fn native_separators(path: PathBuf) -> PathBuf {
    PathBuf::from(path.to_string_lossy().replace('/', "\\"))
}

#[cfg(not(windows))]
fn native_separators(path: PathBuf) -> PathBuf {
    path
}

fn missing(id: &str, label: &str) -> ManagedPathDto {
    location(
        id,
        label.to_string(),
        None,
        None,
        None,
        PathOrigin::Missing,
        None,
        None,
    )
}

#[allow(clippy::too_many_arguments)]
fn location(
    id: &str,
    label: String,
    detected: Option<String>,
    override_path: Option<String>,
    effective: Option<String>,
    origin: PathOrigin,
    edit_path: Option<String>,
    update: Option<PathUpdate>,
) -> ManagedPathDto {
    let exists = effective
        .as_deref()
        .is_some_and(|path| Path::new(path).is_dir());
    ManagedPathDto {
        id: id.to_string(),
        label,
        kind: PathKind::Dir,
        detected,
        override_path,
        effective,
        exists,
        origin,
        edit_path,
        update,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::atomic::{AtomicU64, Ordering};

    fn scratch(tag: &str) -> PathBuf {
        static N: AtomicU64 = AtomicU64::new(0);
        let n = N.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "modlab-game-paths-{tag}-{}-{n}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn unknown_game_is_an_error() {
        let core = Core::open_in_memory().unwrap();
        let data = scratch("unknown");
        let err = game_paths_for(&core, &data, "nope", None).unwrap_err();
        assert!(err.contains("nope"), "{err}");
        let _ = fs::remove_dir_all(data);
    }

    #[test]
    fn skyrim_paths_follow_overrides_and_list_ini_files() {
        let core = Core::open_in_memory().unwrap();
        let data_root = scratch("skyrim-data");
        let game = scratch("skyrim-game");
        let downloads_root = scratch("skyrim-dl");
        let configs = scratch("skyrim-ini");
        fs::write(configs.join("Skyrim.ini"), b"[General]\n").unwrap();
        fs::write(configs.join("SkyrimPrefs.ini"), b"[Display]\n").unwrap();
        fs::write(configs.join("Extra.ini"), b"x").unwrap();
        fs::write(configs.join("notes.txt"), b"no").unwrap();
        fs::create_dir(configs.join("skip.ini")).unwrap();
        let prefs = configs.join("SkyrimPrefs.ini");

        core.set_game_path_override("skyrimse", "root", &game.display().to_string())
            .unwrap();
        core.set_game_path_override(
            downloads::SETTINGS_SCOPE,
            downloads::DOWNLOADS_DIR_SLOT,
            &downloads_root.display().to_string(),
        )
        .unwrap();
        core.set_game_path_override("skyrimse", PREFS_SLOT, &prefs.display().to_string())
            .unwrap();

        let dto = game_paths_for(&core, &data_root, "skyrimse", None).unwrap();

        assert_eq!(dto.game_dir.origin, PathOrigin::Override);
        assert_eq!(Path::new(dto.game_dir.effective.as_deref().unwrap()), game);
        assert!(dto.game_dir.exists);
        assert_eq!(dto.game_dir.edit_path, dto.game_dir.effective);
        assert_eq!(
            dto.game_dir.update,
            Some(PathUpdate::GamePath {
                slot_id: "root".into()
            })
        );

        assert_eq!(dto.mods_dir.id, "mods_dir");
        assert_eq!(dto.mods_dir.origin, PathOrigin::Default);
        assert!(dto.mods_dir.update.is_none());
        assert!(!dto.mods_dir.exists);
        assert_eq!(
            PathBuf::from(dto.mods_dir.effective.as_deref().unwrap()),
            data_root.join("mods")
        );

        assert_eq!(dto.downloads_dir.origin, PathOrigin::Override);
        assert_eq!(dto.downloads_dir.update, Some(PathUpdate::DownloadsDir));
        let downloads = PathBuf::from(dto.downloads_dir.effective.as_deref().unwrap());
        assert_eq!(downloads, downloads_root.join("skyrimse"));
        assert_eq!(
            PathBuf::from(dto.downloads_dir.edit_path.as_deref().unwrap()),
            downloads_root
        );
        assert!(!dto.downloads_dir.exists);

        assert_eq!(dto.config_dir.origin, PathOrigin::Override);
        assert!(dto.config_dir.update.is_none());
        assert_eq!(
            Path::new(dto.config_dir.effective.as_deref().unwrap()),
            configs
        );
        let names: Vec<_> = dto
            .ini_files
            .iter()
            .map(|file| file.name.as_str())
            .collect();
        assert_eq!(
            names,
            [
                "Skyrim.ini",
                "SkyrimPrefs.ini",
                "SkyrimCustom.ini",
                "Extra.ini"
            ]
        );
        let file = |name: &str| dto.ini_files.iter().find(|row| row.name == name).unwrap();
        assert!(file("Skyrim.ini").present);
        assert!(file("SkyrimPrefs.ini").present);
        assert_eq!(file("SkyrimPrefs.ini").slot_id.as_deref(), Some(PREFS_SLOT));
        assert!(!file("SkyrimCustom.ini").present);
        assert!(file("Extra.ini").present);
        assert!(file("Extra.ini").slot_id.is_none());

        let json = serde_json::to_value(&dto).unwrap();
        assert_eq!(json["gameDir"]["kind"], "dir");
        assert_eq!(json["gameDir"]["origin"], "override");
        assert_eq!(json["gameDir"]["update"]["via"], "gamePathOverride");
        assert_eq!(json["gameDir"]["update"]["slotId"], "root");
        assert_eq!(
            json["downloadsDir"]["update"]["via"],
            "downloadsDirOverride"
        );
        assert!(json["modsDir"]["update"].is_null());
        assert!(json["referenceDir"].is_null());
        assert_eq!(json["iniFiles"][1]["slotId"], PREFS_SLOT);

        for dir in [data_root, game, downloads_root, configs] {
            let _ = fs::remove_dir_all(dir);
        }
    }

    #[test]
    fn minecraft_paths_follow_the_instance_folder_not_the_launcher() {
        let core = Core::open_in_memory().unwrap();
        let data_root = scratch("mc-data");
        let created = loadout::create_instance(&core, "minecraft", "decube").unwrap();
        let target = install_target_for_loadout(&core, &data_root, created.id)
            .unwrap()
            .expect("minecraft owns a folder");
        fs::write(target.instance_root.join("config").join("fabric.ini"), b"x").unwrap();
        let launcher = scratch("mc-launcher");
        let decoy_mods = scratch("mc-decoy");
        core.set_game_path_override("minecraft", "root", &launcher.display().to_string())
            .unwrap();
        core.set_game_path_override("minecraft", "mods_dir", &decoy_mods.display().to_string())
            .unwrap();

        let dto = game_paths_for(&core, &data_root, "minecraft", Some(created.id)).unwrap();

        assert_eq!(dto.game_dir.label, "Dossier de l'instance");
        assert_eq!(
            Path::new(dto.game_dir.effective.as_deref().unwrap()),
            target.instance_root
        );
        assert_eq!(dto.game_dir.origin, PathOrigin::Default);
        assert!(matches!(
            dto.game_dir.update,
            Some(PathUpdate::InstancePath { .. })
        ));
        assert_eq!(
            PathBuf::from(dto.mods_dir.effective.clone().unwrap()),
            target.instance_root.join("mods")
        );
        assert_eq!(dto.mods_dir.origin, PathOrigin::Default);
        assert!(dto.mods_dir.update.is_some());
        assert_eq!(
            PathBuf::from(dto.config_dir.effective.clone().unwrap()),
            target.instance_root.join("config")
        );
        assert_eq!(dto.ini_files.len(), 1);
        assert_eq!(dto.ini_files[0].name, "fabric.ini");
        let reference = dto.reference_dir.expect("the launcher stays visible");
        assert_eq!(Path::new(reference.effective.as_deref().unwrap()), launcher);
        assert_ne!(
            Path::new(dto.game_dir.effective.as_deref().unwrap()),
            launcher
        );

        let custom = scratch("mc-custom-mods");
        core.set_instance_path_override(
            created.instance_id,
            "mods_dir",
            &custom.display().to_string(),
        )
        .unwrap();
        let overridden = game_paths_for(&core, &data_root, "minecraft", Some(created.id)).unwrap();
        assert_eq!(
            Path::new(overridden.mods_dir.effective.as_deref().unwrap()),
            custom
        );
        assert_eq!(overridden.mods_dir.origin, PathOrigin::Override);

        for dir in [data_root, launcher, decoy_mods, custom] {
            let _ = fs::remove_dir_all(dir);
        }
    }
}
