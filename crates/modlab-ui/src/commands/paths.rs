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
pub(crate) fn game_path_slots(
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
            let override_path = overrides
                .iter()
                .find(|(id, _)| id == slot.id)
                .map(|(_, path)| PathBuf::from(path));
            let fused = fuse_path(slot.detected, override_path);
            GamePathSlotDto {
                id: slot.id.to_string(),
                label: slot.label.to_string(),
                kind: slot.kind,
                detected: fused.detected.map(|path| path.display().to_string()),
                override_path: fused.override_path.map(|path| path.display().to_string()),
                effective: fused.effective.map(|path| path.display().to_string()),
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
pub(crate) fn set_game_path_override(
    state: State<AppState>,
    game_id: String,
    slot_id: String,
    path: String,
) -> Result<Vec<GamePathSlotDto>, String> {
    let path = path.trim();
    if path.is_empty() {
        return Err("Le chemin ne peut pas être vide.".into());
    }
    let path_buf = PathBuf::from(path);
    {
        let core = state.core.lock().map_err(|e| e.to_string())?;
        crate::ipc_paths::validate_game_path_override(
            &core,
            &state.data_root,
            &game_id,
            &slot_id,
            &path_buf,
        )?;
        core.set_game_path_override(&game_id, &slot_id, &path_buf.display().to_string())
            .map_err(|e| e.to_string())?;
    }
    game_path_slots(state, game_id)
}

/// Point one slot of one instance at a folder chosen by hand. Does not move
/// files that are already installed. For the instance folder itself, the
/// game's empty layout directories are created in the chosen folder.
#[tauri::command]
pub(crate) fn set_instance_path_override(
    state: State<AppState>,
    instance_id: i64,
    slot_id: String,
    path: String,
) -> Result<(), String> {
    let path = path.trim();
    if path.is_empty() {
        return Err("Le chemin ne peut pas être vide.".into());
    }
    let core = state.core.lock().map_err(|e| e.to_string())?;
    let game_id = loadout::list(&core)
        .map_err(|e| e.to_string())?
        .into_iter()
        .find(|branch| branch.instance_id == instance_id)
        .map(|branch| branch.game_id)
        .ok_or_else(|| format!("instance {instance_id} introuvable"))?;
    core.set_instance_path_override(instance_id, &slot_id, path)
        .map_err(|e| e.to_string())?;
    if slot_id == modlab_core::game_adapter::INSTANCE_ROOT_SLOT {
        if let Some(adapter) = crate::adapters()
            .into_iter()
            .find(|adapter| adapter.id() == game_id)
        {
            if let modlab_core::game_adapter::InstanceRootPolicy::PerInstanceRoot {
                subdirs, ..
            } = adapter.instance_root_policy()
            {
                let root = PathBuf::from(path);
                std::fs::create_dir_all(&root).map_err(|e| e.to_string())?;
                for name in subdirs {
                    std::fs::create_dir_all(root.join(name)).map_err(|e| e.to_string())?;
                }
            }
        }
    }
    let mut tx = transaction::Transaction::new(&core, "instance.path");
    tx.step(format!(
        "chemin « {slot_id} » de l'instance {instance_id} : {path}"
    ));
    tx.commit().map_err(|e| e.to_string())?;
    Ok(())
}

/// Drop a human override for one instance slot. The stored folder stays.
#[tauri::command]
pub(crate) fn clear_instance_path_override(
    state: State<AppState>,
    instance_id: i64,
    slot_id: String,
) -> Result<(), String> {
    let core = state.core.lock().map_err(|e| e.to_string())?;
    core.clear_instance_path_override(instance_id, &slot_id)
        .map_err(|e| e.to_string())?;
    let mut tx = transaction::Transaction::new(&core, "instance.path");
    tx.step(format!(
        "chemin « {slot_id} » de l'instance {instance_id} rétabli"
    ));
    tx.commit().map_err(|e| e.to_string())?;
    Ok(())
}

/// Drop a human override for one slot, falling back to the adapter's own
/// detection again.
#[tauri::command]
pub(crate) fn clear_game_path_override(
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

/// One child of the active game's install folder (path slot `root`).
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameInstallEntryDto {
    pub name: String,
    pub kind: PathKind,
}

/// One directory under the game install `root`. An empty `relative` (or only
/// `.`) is `root` itself. Names are appended one component at a time: a raw
/// `join` would let `\Windows` or `C:foo` replace the root on Windows.
/// `..` is refused even when it would stay inside — the Répertoire tab names
/// the folder it wants, it never walks upward. A symlink or junction that
/// resolves outside `root` is refused after canonicalization.
fn install_subdir(root: &Path, relative: &str) -> Result<PathBuf, String> {
    const OUTSIDE: &str = "Hors du dossier d'installation";
    if !root.is_dir() {
        return Err(format!("Pas un dossier : {}", root.display()));
    }
    if relative.contains('\0') {
        return Err(OUTSIDE.into());
    }
    let root_canon = root
        .canonicalize()
        .map_err(|e| format!("Lecture impossible : {e}"))?;
    if relative.is_empty() {
        return Ok(root_canon);
    }
    let rel = Path::new(relative);
    if rel.is_absolute() {
        return Err(OUTSIDE.into());
    }
    let mut joined = root_canon.clone();
    let mut saw_name = false;
    for comp in rel.components() {
        match comp {
            Component::Normal(part) => {
                joined.push(part);
                saw_name = true;
            }
            Component::CurDir => {}
            Component::ParentDir | Component::Prefix(_) | Component::RootDir => {
                return Err(OUTSIDE.into());
            }
        }
    }
    if !saw_name {
        return Ok(root_canon);
    }
    let target = match joined.canonicalize() {
        Ok(path) => path,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Err("Lecture impossible : dossier introuvable".into());
        }
        Err(e) => return Err(format!("Lecture impossible : {e}")),
    };
    if !target.starts_with(&root_canon) {
        return Err(OUTSIDE.into());
    }
    if !target.is_dir() {
        return Err(format!("Pas un dossier : {}", target.display()));
    }
    Ok(target)
}

/// Immediate children of a directory. Directories first, then files, each
/// group ordered by name. Not recursive: the Répertoire tab asks for the
/// one folder it is showing.
fn read_install_entries(root: &Path) -> Result<Vec<GameInstallEntryDto>, String> {
    if !root.is_dir() {
        return Err(format!("Pas un dossier : {}", root.display()));
    }
    let read = std::fs::read_dir(root).map_err(|e| format!("Lecture impossible : {e}"))?;
    let mut rows = Vec::new();
    for entry in read {
        let entry = entry.map_err(|e| format!("Lecture impossible : {e}"))?;
        let Ok(ty) = entry.file_type() else {
            continue;
        };
        rows.push(GameInstallEntryDto {
            name: entry.file_name().to_string_lossy().into_owned(),
            kind: if ty.is_dir() {
                PathKind::Dir
            } else {
                PathKind::File
            },
        });
    }
    rows.sort_by(|a, b| {
        let rank = |kind: PathKind| match kind {
            PathKind::Dir => 0,
            PathKind::File => 1,
        };
        rank(a.kind)
            .cmp(&rank(b.kind))
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    Ok(rows)
}

/// The Répertoire tab: one folder of this game's install directory (slot
/// `root`), honouring a human override the same way `game_path_slots` does.
/// `relative_path` is relative to that root; empty lists the root itself.
/// Read-only. An unknown game is an error; no install path is an empty
/// list — the UI already says so from the slot itself. A path that leaves
/// the install folder is an error.
#[tauri::command]
pub(crate) fn game_install_entries(
    state: State<AppState>,
    game_id: String,
    relative_path: Option<String>,
) -> Result<Vec<GameInstallEntryDto>, String> {
    let adapter = adapters()
        .into_iter()
        .find(|a| a.id() == game_id)
        .ok_or_else(|| format!("jeu inconnu : {game_id}"))?;
    let core = state.core.lock().map_err(|e| e.to_string())?;
    let Some(root) = resolve_slot(&core, adapter, "root") else {
        return Ok(Vec::new());
    };
    let relative = relative_path.as_deref().unwrap_or("");
    let dir = install_subdir(&root, relative)?;
    read_install_entries(&dir)
}

/// One top-level `Data/` name that is not part of a vanilla Skyrim install.
/// A directory is not expanded; config files inside it are in `configs`.
#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct VanillaExtraDto {
    /// Relative to `Data/`, `/`-separated.
    pub path: String,
    pub kind: PathKind,
}

/// Where a config path is relative to.
#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum VanillaLocation {
    Data,
    Documents,
    Install,
}

/// A config file kept apart from [`VanillaExtraDto`].
#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct VanillaConfigDto {
    /// Relative to `location`, `/`-separated.
    pub path: String,
    pub location: VanillaLocation,
}

/// One path the scan could not read.
#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct VanillaUnreadableDto {
    pub path: String,
    pub location: VanillaLocation,
    pub reason: String,
}

/// Read-only vanilla check. `scanned` is false when no `Data/` path is
/// known — distinct from a scan that found nothing. `documents_scanned`
/// is false when the INI folder could not be opened. Neither list is a
/// promise that stock files are present.
#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct VanillaScanDto {
    pub scanned: bool,
    pub documents_scanned: bool,
    pub extras: Vec<VanillaExtraDto>,
    pub configs: Vec<VanillaConfigDto>,
    pub unreadable: Vec<VanillaUnreadableDto>,
}

impl VanillaScanDto {
    fn not_scanned() -> Self {
        Self {
            scanned: false,
            documents_scanned: false,
            extras: Vec::new(),
            configs: Vec::new(),
            unreadable: Vec::new(),
        }
    }
}

fn vanilla_location(location: vanilla::Location) -> VanillaLocation {
    match location {
        vanilla::Location::Data => VanillaLocation::Data,
        vanilla::Location::Documents => VanillaLocation::Documents,
        vanilla::Location::Install => VanillaLocation::Install,
    }
}

fn vanilla_scan_dto(scan: vanilla::VanillaScan) -> VanillaScanDto {
    VanillaScanDto {
        scanned: true,
        documents_scanned: scan.documents_scanned,
        extras: scan
            .extras
            .into_iter()
            .map(|extra| VanillaExtraDto {
                path: extra.path,
                kind: match extra.kind {
                    vanilla::ExtraKind::Dir => PathKind::Dir,
                    vanilla::ExtraKind::File => PathKind::File,
                },
            })
            .collect(),
        configs: scan
            .configs
            .into_iter()
            .map(|config| VanillaConfigDto {
                path: config.path,
                location: vanilla_location(config.location),
            })
            .collect(),
        unreadable: scan
            .unreadable
            .into_iter()
            .map(|entry| VanillaUnreadableDto {
                path: entry.path,
                location: vanilla_location(entry.location),
                reason: match entry.problem {
                    vanilla::ReadProblem::Io(err) => format!("Lecture impossible : {err}"),
                    vanilla::ReadProblem::EscapesRoot => "Hors du dossier analysé".to_string(),
                },
            })
            .collect(),
    }
}

/// `Data/`, install root (for `Skyrim.ccc`), and the INI folder. The INI
/// folder follows the same precedence as `game_paths`: parent of the
/// `prefs_ini` slot (override wins), otherwise `my_games_dir()`.
struct VanillaPaths {
    data: Option<PathBuf>,
    root: Option<PathBuf>,
    documents: Option<PathBuf>,
}

fn vanilla_paths(core: &Core, game_id: &str) -> Result<VanillaPaths, String> {
    let adapter = adapters()
        .into_iter()
        .find(|adapter| adapter.id() == game_id)
        .ok_or_else(|| format!("jeu inconnu : {game_id}"))?;
    if adapter.id() != SkyrimAdapter.id() {
        return Err("Le contrôle vanilla n'est pas disponible pour ce jeu.".into());
    }
    let prefs = resolve_slot(core, adapter, "prefs_ini");
    let documents = prefs
        .as_deref()
        .and_then(|path| path.parent().map(Path::to_path_buf))
        .or_else(modlab_game_skyrim::my_games_dir);
    Ok(VanillaPaths {
        data: resolve_slot(core, adapter, "data_dir"),
        root: resolve_slot(core, adapter, "root"),
        documents,
    })
}

fn finish_vanilla_scan(
    data_dir: Option<&Path>,
    install_root: Option<&Path>,
    documents_dir: Option<&Path>,
) -> Result<VanillaScanDto, String> {
    let Some(data_dir) = data_dir else {
        return Ok(VanillaScanDto::not_scanned());
    };
    if !data_dir.is_dir() {
        return Err(format!("Pas un dossier : {}", data_dir.display()));
    }
    vanilla::scan(data_dir, install_root, documents_dir)
        .map(vanilla_scan_dto)
        .map_err(|e| format!("Lecture impossible : {e}"))
}

#[cfg(test)]
fn run_vanilla_scan(core: &Core, game_id: &str) -> Result<VanillaScanDto, String> {
    let paths = vanilla_paths(core, game_id)?;
    finish_vanilla_scan(
        paths.data.as_deref(),
        paths.root.as_deref(),
        paths.documents.as_deref(),
    )
}

/// What in this game's `Data/` (and, separately, its INI folder) is not
/// part of a vanilla Skyrim install. Read-only (Lecture tier, §7.2):
/// nothing is deleted or written. Unknown game is an error. No `Data/`
/// path is an empty scan (`scanned: false`), not an error. Only Skyrim SE
/// has a baseline; other adapters are an error.
/// Path lookup takes the database lock; the directory walk does not, and
/// it runs off the UI thread.
#[tauri::command]
pub(crate) async fn game_vanilla_scan(
    app: tauri::AppHandle,
    game_id: String,
) -> Result<VanillaScanDto, String> {
    off_ui(move || {
        let state = app.state::<AppState>();
        let paths = {
            let core = state.core.lock().map_err(|e| e.to_string())?;
            vanilla_paths(&core, &game_id)?
        };
        finish_vanilla_scan(
            paths.data.as_deref(),
            paths.root.as_deref(),
            paths.documents.as_deref(),
        )
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
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
    fn read_install_entries_lists_the_install_folder() {
        let dir = scratch("install-entries");
        fs::write(dir.join("SkyrimSE.exe"), b"x").unwrap();
        fs::write(dir.join("b.txt"), b"b").unwrap();
        fs::create_dir(dir.join("Data")).unwrap();

        let rows = read_install_entries(&dir).unwrap();
        let names: Vec<_> = rows.iter().map(|r| r.name.as_str()).collect();
        assert_eq!(names, ["Data", "b.txt", "SkyrimSE.exe"]);
        assert!(matches!(rows[0].kind, PathKind::Dir));
        assert!(matches!(rows[1].kind, PathKind::File));

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn install_subdir_lists_a_child_and_refuses_to_leave_the_root() {
        let dir = scratch("install-sub");
        fs::write(dir.join("readme.txt"), b"r").unwrap();
        fs::create_dir_all(dir.join("Data").join("meshes")).unwrap();
        fs::write(dir.join("Data").join("meshes").join("a.nif"), b"a").unwrap();
        let outside = scratch("install-sub-out");
        fs::write(outside.join("secret.txt"), b"s").unwrap();

        let nested = install_subdir(&dir, "Data/meshes").unwrap();
        let names: Vec<_> = read_install_entries(&nested)
            .unwrap()
            .into_iter()
            .map(|row| row.name)
            .collect();
        assert_eq!(names, ["a.nif"]);

        let via_backslash = install_subdir(&dir, r"Data\meshes").unwrap();
        assert_eq!(read_install_entries(&via_backslash).unwrap().len(), 1);
        let via_dot = install_subdir(&dir, "./Data/meshes").unwrap();
        assert_eq!(read_install_entries(&via_dot).unwrap().len(), 1);

        let root = install_subdir(&dir, "").unwrap();
        let root_names: Vec<_> = read_install_entries(&root)
            .unwrap()
            .into_iter()
            .map(|row| row.name)
            .collect();
        assert_eq!(root_names, ["Data", "readme.txt"]);

        for bad in [
            "..",
            r"..\..",
            "Data/../..",
            "Data/../../outside",
            "../install-sub-out",
        ] {
            let err = install_subdir(&dir, bad).unwrap_err();
            assert!(
                err.contains("Hors du dossier d'installation"),
                "{bad}: {err}"
            );
        }

        let err = install_subdir(&dir, &outside.display().to_string()).unwrap_err();
        assert!(err.contains("Hors du dossier d'installation"), "{err}");

        #[cfg(windows)]
        {
            for bad in [r"\Windows", r"C:\Windows", "C:Windows"] {
                let err = install_subdir(&dir, bad).unwrap_err();
                assert!(
                    err.contains("Hors du dossier d'installation"),
                    "{bad}: {err}"
                );
            }
            // A junction or symlink that points outside the install folder
            // must not be listable. Creating one needs the privilege; when
            // the OS refuses, the component checks above still hold.
            let link = dir.join("link-out");
            if std::os::windows::fs::symlink_dir(&outside, &link).is_ok() {
                let err = install_subdir(&dir, "link-out").unwrap_err();
                assert!(err.contains("Hors du dossier d'installation"), "{err}");
            }
        }

        let err = install_subdir(&dir, "readme.txt").unwrap_err();
        assert!(err.contains("Pas un dossier"), "{err}");
        let err = install_subdir(&dir, "no/such").unwrap_err();
        assert!(err.contains("dossier introuvable"), "{err}");

        let _ = fs::remove_dir_all(&dir);
        let _ = fs::remove_dir_all(&outside);
    }

    #[test]
    fn vanilla_scan_uses_path_overrides_and_keeps_configs_apart() {
        let core = Core::open_in_memory().unwrap();
        let root = scratch("vanilla-game");
        let docs = scratch("vanilla-docs");
        let data = root.join("Data");
        fs::create_dir_all(&data).unwrap();
        fs::write(root.join("Skyrim.ccc"), b"ccFromCcc.esl\r\n").unwrap();
        fs::write(data.join("Skyrim.esm"), b"vanilla").unwrap();
        fs::write(data.join("ccFromCcc.esl"), b"cc").unwrap();
        fs::write(data.join("ccFromCcc.bsa"), b"bsa").unwrap();
        fs::write(data.join("OnlyInFixture.esp"), b"mod").unwrap();
        fs::write(data.join("custom.ini"), b"cfg").unwrap();
        fs::write(docs.join("Skyrim.ini"), b"ini").unwrap();
        fs::write(docs.join("extra.ini"), b"extra").unwrap();

        core.set_game_path_override("skyrimse", "root", &root.display().to_string())
            .unwrap();
        core.set_game_path_override("skyrimse", "data_dir", &data.display().to_string())
            .unwrap();
        core.set_game_path_override(
            "skyrimse",
            "prefs_ini",
            &docs.join("SkyrimPrefs.ini").display().to_string(),
        )
        .unwrap();

        let dto = run_vanilla_scan(&core, "skyrimse").unwrap();
        assert!(dto.scanned);
        assert!(dto.documents_scanned);
        assert_eq!(
            dto.extras,
            vec![VanillaExtraDto {
                path: "OnlyInFixture.esp".into(),
                kind: PathKind::File,
            }]
        );
        assert_eq!(
            dto.configs,
            vec![
                VanillaConfigDto {
                    path: "custom.ini".into(),
                    location: VanillaLocation::Data,
                },
                VanillaConfigDto {
                    path: "extra.ini".into(),
                    location: VanillaLocation::Documents,
                },
            ]
        );
        assert_eq!(fs::read(data.join("OnlyInFixture.esp")).unwrap(), b"mod");

        let wire = serde_json::to_value(&dto).unwrap();
        assert_eq!(wire["scanned"], true);
        assert_eq!(wire["documentsScanned"], true);
        assert_eq!(wire["extras"][0]["kind"], "file");
        assert_eq!(wire["extras"][0]["path"], "OnlyInFixture.esp");
        assert_eq!(wire["configs"][0]["location"], "data");
        assert_eq!(wire["configs"][1]["location"], "documents");
        assert_eq!(wire["unreadable"], serde_json::json!([]));

        let err = run_vanilla_scan(&core, "minecraft").unwrap_err();
        assert!(err.contains("pas disponible"), "{err}");
        let err = run_vanilla_scan(&core, "nope").unwrap_err();
        assert!(err.contains("jeu inconnu"), "{err}");

        core.set_game_path_override(
            "skyrimse",
            "data_dir",
            &root.join("missing").display().to_string(),
        )
        .unwrap();
        let err = run_vanilla_scan(&core, "skyrimse").unwrap_err();
        assert!(err.contains("Pas un dossier"), "{err}");

        let empty = finish_vanilla_scan(None, None, None).unwrap();
        let wire = serde_json::to_value(&empty).unwrap();
        assert_eq!(wire["scanned"], false);
        assert_eq!(wire["documentsScanned"], false);
        assert_eq!(wire["extras"], serde_json::json!([]));
        assert_eq!(wire["configs"], serde_json::json!([]));

        let _ = fs::remove_dir_all(&root);
        let _ = fs::remove_dir_all(&docs);
    }

    #[test]
    fn vanilla_scan_dto_translates_read_problems() {
        let dto = vanilla_scan_dto(vanilla::VanillaScan {
            extras: Vec::new(),
            configs: Vec::new(),
            documents_scanned: false,
            unreadable: vec![
                vanilla::Unreadable {
                    path: "link".into(),
                    location: vanilla::Location::Data,
                    problem: vanilla::ReadProblem::EscapesRoot,
                },
                vanilla::Unreadable {
                    path: "SKSE".into(),
                    location: vanilla::Location::Documents,
                    problem: vanilla::ReadProblem::Io("access denied".into()),
                },
            ],
        });
        assert!(dto.scanned);
        assert_eq!(dto.unreadable[0].reason, "Hors du dossier analysé");
        assert_eq!(
            dto.unreadable[1].reason,
            "Lecture impossible : access denied"
        );
        let wire = serde_json::to_value(&dto).unwrap();
        assert_eq!(wire["unreadable"][0]["location"], "data");
        assert_eq!(wire["unreadable"][1]["location"], "documents");
        assert_eq!(wire["unreadable"][0]["path"], "link");
    }
}
