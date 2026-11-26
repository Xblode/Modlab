//! Given an already-extracted archive, work out where its files belong —
//! see `docs/faisabilite.md` A3 and `docs/01-architecture-decidee.md` §9 for the
//! reasoning. This only *classifies*; it doesn't move or copy a single
//! file. Placement (actually deploying into a mod's folder) is a later,
//! separate step, same "read/decide, then act" split as the MO2 importer.
//!
//! Recognizes the common shapes (bare Data content, a wrapped `Data/`
//! folder, a FOMOD, and SKSE's own loader at the content root) and says
//! `Unknown` rather than guessing for anything else — including ENB DLLs
//! and `d3d11.dll`, which `docs/faisabilite.md` A3 covers but this doesn't
//! yet. A readme (or another loose note) sitting next to a single folder
//! is not a shape: it is skipped so the folder underneath can be classified.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// What an extracted archive's real content looks like, and where it
/// actually starts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DetectedArchive {
    pub shape: ArchiveShape,
    /// Where the real content starts — may be several levels below the
    /// extraction root the archive wraps everything in single-child
    /// folders. Confirmed against a real archive, not assumed: SKSE's own
    /// `.7z` extracts to `<root>/Skyrim Script Extender (SKSE64).../
    /// skse64_2_02_08/`, two folders of pure wrapping before any real file.
    pub content_root: PathBuf,
}

/// What an archive's real content root looks like.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArchiveShape {
    /// Has `fomod/ModuleConfig.xml` — needs the guided FOMOD installer,
    /// which isn't built yet. Detected so callers can say "not supported
    /// yet" honestly instead of installing it wrong.
    Fomod,
    /// Data-folder-shaped content directly at the content root (`meshes/`,
    /// `textures/`, `scripts/`, `interface/`, `SKSE/`, or a
    /// `.esp`/`.esm`/`.esl` file) — install as-is under the mod's `Data/`.
    BareData,
    /// The content root contains a folder literally named `Data` —
    /// install its contents under the mod's `Data/`.
    WrappedData { data_dir: String },
    /// `skse64_loader.exe` at the content root and no `Data/` folder. The
    /// layout is the game root, not Data content — see `engine`.
    EngineRoot,
    /// Nothing recognized.
    Unknown,
}

const DATA_SHAPED_SUBDIRS: &[&str] = &[
    "meshes",
    "textures",
    "scripts",
    "interface",
    "skse",
    "sound",
    "seq",
];
const PLUGIN_EXTENSIONS: &[&str] = &["esp", "esm", "esl"];

/// Classify an extracted directory. At each candidate root, tries to
/// recognize a shape *first*; only if that fails, and the level is just a
/// single wrapping folder with nothing else in it, descends one level and
/// tries again. Recognizing before descending matters: a `fomod/` or
/// `Data/` folder that happens to be the *only* thing at some level must
/// stop there, not be treated as "just more wrapping" and stepped into —
/// an earlier version of this function got exactly that backwards and
/// mIS-classified a bare FOMOD archive as `Unknown`.
pub fn detect_shape(extracted_root: &Path) -> io::Result<DetectedArchive> {
    let mut current = extracted_root.to_path_buf();
    loop {
        let (dirs, files) = list_entries(&current)?;
        let shape = classify(&dirs, &files);

        if shape != ArchiveShape::Unknown {
            return Ok(DetectedArchive {
                shape,
                content_root: current,
            });
        }
        // A readme, a preview image, `desktop.ini`: not a mod. One folder
        // next to only those is still a wrapper, the same as a level that
        // holds nothing else at all.
        let only_noise = files.iter().all(|name| is_wrapper_noise(name));
        match (dirs.as_slice(), only_noise) {
            ([only], true) => current = current.join(only),
            _ => {
                return Ok(DetectedArchive {
                    shape: ArchiveShape::Unknown,
                    content_root: current,
                })
            }
        }
    }
}

/// Move an archive's real content into a mod's own folder (`dest`), laid out
/// the way USVFS expects: the mod folder is linked straight onto the game's
/// `Data/`, so its *contents* must already be Data-relative.
///
/// For a `WrappedData` archive that means moving the inner `Data/` folder, not
/// the folder that contains it — otherwise files deploy as `Data/Data/...`.
/// `keep_game_root` is for the engine mod (SKSE), recognised by its content
/// (`engine::is_engine_content`: a `skse64_loader.exe` at the content root,
/// never by the mod's name): its root files (loader, `skse64_*.dll`) are copied
/// into the game's install root (`engine::install_root_files`), so they must
/// stay beside `Data/` and the layout is left untouched.
///
/// `content_root` is re-classified here (it is already a recognized root, so
/// no descent happens) to keep callers from having to carry the shape around.
pub fn place_content(content_root: &Path, dest: &Path, keep_game_root: bool) -> io::Result<()> {
    let detected = detect_shape(content_root)?;
    let unwrap_data = match &detected.shape {
        ArchiveShape::WrappedData { data_dir } if !keep_game_root => Some(data_dir.clone()),
        _ => None,
    };
    let source = match &unwrap_data {
        Some(data_dir) => detected.content_root.join(data_dir),
        None => detected.content_root.clone(),
    };
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent)?;
    }
    // A rename inside the same data root is atomic and instant; the copy
    // fallback covers a staging area that landed on another volume.
    if fs::rename(&source, dest).is_err() {
        copy_dir(&source, dest)?;
    }
    // Unwrapping `Data/` must not drop a plugin or archive that was sitting
    // beside that folder. Those files belong at the mod root (= Data/ once
    // USVFS links it). The engine mod never takes this branch: its loader
    // stays beside `Data/`.
    if matches!(detected.shape, ArchiveShape::WrappedData { .. }) && !keep_game_root {
        copy_data_siblings(&detected.content_root, dest)?;
    }
    Ok(())
}

/// Plugin and Bethesda-archive names that the game loads from `Data/` itself,
/// so they cannot stay outside the folder `place_content` just unwrapped.
fn is_data_root_file(name: &str) -> bool {
    Path::new(name)
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|ext| {
            matches!(
                ext.to_ascii_lowercase().as_str(),
                "esp" | "esm" | "esl" | "bsa" | "ba2"
            )
        })
}

fn copy_data_siblings(content_root: &Path, dest: &Path) -> io::Result<()> {
    let (_, files) = list_entries(content_root)?;
    for name in files {
        if !is_data_root_file(&name) {
            continue;
        }
        let to = dest.join(&name);
        if to.exists() {
            continue;
        }
        fs::copy(content_root.join(&name), &to)?;
    }
    Ok(())
}

/// Recursively copy a directory tree.
pub fn copy_dir(from: &Path, to: &Path) -> io::Result<()> {
    fs::create_dir_all(to)?;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir(&entry.path(), &target)?;
        } else {
            fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}

fn classify(dirs: &[String], files: &[String]) -> ArchiveShape {
    if dirs.iter().any(|d| d.eq_ignore_ascii_case("fomod")) {
        ArchiveShape::Fomod
    } else if let Some(data_dir) = dirs.iter().find(|d| d.eq_ignore_ascii_case("data")) {
        ArchiveShape::WrappedData {
            data_dir: data_dir.clone(),
        }
    } else if has_data_shaped_content(dirs, files) {
        ArchiveShape::BareData
    } else if files
        .iter()
        .any(|f| f.eq_ignore_ascii_case(crate::engine::LOADER_FILE))
    {
        ArchiveShape::EngineRoot
    } else {
        ArchiveShape::Unknown
    }
}

/// A file that does not make a directory a mod by itself: notes, licenses,
/// preview images, and the two bookkeeping names Windows and Explorer drop
/// into folders. Used only to decide whether a single child folder is still
/// just wrapping.
fn is_wrapper_noise(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    if lower == "thumbs.db" || lower == "desktop.ini" {
        return true;
    }
    Path::new(&lower)
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|ext| {
            matches!(
                ext,
                "txt" | "md" | "pdf" | "url" | "html" | "htm" | "rtf" | "jpg" | "jpeg" | "png"
            )
        })
}

fn list_entries(dir: &Path) -> io::Result<(Vec<String>, Vec<String>)> {
    let mut dirs = Vec::new();
    let mut files = Vec::new();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().to_string();
        if entry.file_type()?.is_dir() {
            dirs.push(name);
        } else {
            files.push(name);
        }
    }
    Ok((dirs, files))
}

fn has_data_shaped_content(dirs: &[String], files: &[String]) -> bool {
    let has_data_shaped_subdir = dirs.iter().any(|d| {
        DATA_SHAPED_SUBDIRS
            .iter()
            .any(|known| d.eq_ignore_ascii_case(known))
    });
    let has_plugin_file = files.iter().any(|f| {
        Path::new(f)
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|ext| {
                PLUGIN_EXTENSIONS
                    .iter()
                    .any(|known| ext.eq_ignore_ascii_case(known))
            })
    });
    has_data_shaped_subdir || has_plugin_file
}

/// An archive extracted into the data root's staging area, classified but
/// not installed. The shell turns this into the wizard-or-install decision.
pub struct StagedArchive {
    pub name: String,
    pub shape: ArchiveShape,
    pub staged_path: PathBuf,
    pub content_root: PathBuf,
}

/// Unpack `mod_id`'s archive into `<data root>/staging/<id>` and classify it.
/// A previous staging directory is moved to the trash, not deleted.
pub fn stage_archive(
    core: &modlab_core::Core,
    data_root: &Path,
    mod_id: i64,
) -> Result<StagedArchive, String> {
    let pending = modlab_core::download::archived_mod(core, mod_id).map_err(|e| e.to_string())?;
    stage_archived(data_root, mod_id, &pending)
}

/// Disk half of [`stage_archive`]. No database access: the caller looks the
/// archive up, drops its lock, then extracts.
pub fn stage_archived(
    data_root: &Path,
    mod_id: i64,
    pending: &modlab_core::download::ArchivedMod,
) -> Result<StagedArchive, String> {
    let staged = data_root.join("staging").join(mod_id.to_string());
    if staged.exists() {
        modlab_core::files::move_to_trash(data_root, &staged).map_err(|e| e.to_string())?;
    }
    fs::create_dir_all(&staged).map_err(|e| e.to_string())?;
    modlab_core::archive::extract(&pending.archive_path, &staged).map_err(|e| e.to_string())?;
    let detected =
        detect_shape(&staged).map_err(|e| format!("lecture de l'archive extraite : {e}"))?;
    Ok(StagedArchive {
        name: pending.name.clone(),
        shape: detected.shape,
        staged_path: staged,
        content_root: detected.content_root,
    })
}

/// What [`install_archived`] needs besides the archive path. Computed from
/// the content root (a directory listing) and the archive row — no copy.
pub struct ArchivedInstallPlan {
    pub keep_game_root: bool,
    pub adjustments: modlab_core::install::InstallAdjustments,
}

pub fn archived_install_plan(
    pending: &modlab_core::download::ArchivedMod,
    content_root: &Path,
) -> ArchivedInstallPlan {
    let keep_game_root = crate::engine::is_engine_content(content_root);
    let tool_label = keep_game_root.then(|| {
        let archive_name = pending
            .archive_path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("");
        let dir_name = content_root
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("");
        crate::tool_archive::label_from_install(&pending.name, archive_name, dir_name)
    });
    ArchivedInstallPlan {
        keep_game_root,
        adjustments: modlab_core::install::InstallAdjustments {
            rename_to: tool_label.as_ref().and_then(|tool| {
                (pending.name != tool.display_name).then(|| tool.display_name.to_string())
            }),
            version_if_empty: tool_label.as_ref().and_then(|tool| tool.version.clone()),
        },
    }
}

/// Build the new mod tree beside `dest`. Does not touch the database.
/// Failure leaves `dest` as it was; whatever was written goes to the trash.
pub fn prepare_archived_tree(
    data_root: &Path,
    content_root: &Path,
    dest: &Path,
    keep_game_root: bool,
) -> Result<modlab_core::files::DirSwap, String> {
    modlab_core::files::DirSwap::replace_with(data_root, dest, |staged| {
        place_content(content_root, staged, keep_game_root)
            .map_err(|e| modlab_core::CoreError::Message(e.to_string()))
    })
    .map_err(|e| e.to_string())
}

/// Place `content_root` into `dest` and mark the mod installed.
///
/// The new tree is built beside `dest` and renamed into place. The previous
/// folder, if any, goes to the trash. A failure restores both the folder
/// and the database row.
pub fn install_archived(
    core: &modlab_core::Core,
    data_root: &Path,
    loadout_id: i64,
    mod_id: i64,
    content_root: &Path,
    dest: &Path,
) -> Result<(), String> {
    let pending = modlab_core::download::archived_mod(core, mod_id).map_err(|e| e.to_string())?;
    // The engine mod is told by its content (a loader exe at the content
    // root), never by its name. The inner folder name is read first:
    // `place_content` renames that directory and the version in it would
    // otherwise be lost.
    let plan = archived_install_plan(&pending, content_root);
    let swap = prepare_archived_tree(data_root, content_root, dest, plan.keep_game_root)?;
    modlab_core::install::commit_prepared_install(
        core,
        loadout_id,
        mod_id,
        dest,
        plan.adjustments,
        swap,
    )
    .map_err(|e| e.to_string())?;
    discard_staging(data_root, mod_id);
    Ok(())
}

fn checked_fomod_files(
    content_root: &Path,
    picked: &[String],
    env: &crate::fomod::FomodEnv<'_>,
) -> Result<Vec<crate::fomod::FileMapping>, String> {
    let config_path = crate::fomod::find_module_config(content_root)
        .ok_or_else(|| "fomod/ModuleConfig.xml introuvable dans l'archive".to_string())?;
    let module = crate::fomod::read_module_config(&config_path)?;
    // Re-reads the script and settles the selection again. The wizard's
    // checks are a preview; flags, required plugins and conditional files
    // are decided here.
    crate::fomod::install_plan(content_root, &module, picked, env)
}

/// Copy the files the wizard picked into a sibling of `dest`. Group rules,
/// flags and conditional files are settled before anything is written.
/// No database access.
pub fn prepare_fomod_tree(
    data_root: &Path,
    content_root: &Path,
    picked: &[String],
    dest: &Path,
    env: &crate::fomod::FomodEnv<'_>,
) -> Result<modlab_core::files::DirSwap, String> {
    let files = checked_fomod_files(content_root, picked, env)?;
    modlab_core::files::DirSwap::replace_with(data_root, dest, |staged| {
        fs::create_dir_all(staged).map_err(|e| modlab_core::CoreError::Message(e.to_string()))?;
        for mapping in &files {
            let from = content_root.join(&mapping.source);
            let to = staged.join(&mapping.destination);
            if let Some(parent) = to.parent() {
                fs::create_dir_all(parent)
                    .map_err(|e| modlab_core::CoreError::Message(e.to_string()))?;
            }
            fs::copy(&from, &to).map_err(|e| {
                modlab_core::CoreError::Message(format!(
                    "copie de {} : {e}",
                    mapping.source.display()
                ))
            })?;
        }
        Ok(())
    })
    .map_err(|e| e.to_string())
}

/// Copy the FOMOD files the wizard picked into `dest`, after re-checking
/// each group's rules. Same rollback as [`install_archived`].
#[allow(clippy::too_many_arguments)]
pub fn install_fomod(
    core: &modlab_core::Core,
    data_root: &Path,
    loadout_id: i64,
    mod_id: i64,
    content_root: &Path,
    picked: &[String],
    dest: &Path,
    env: &crate::fomod::FomodEnv<'_>,
) -> Result<(), String> {
    modlab_core::download::archived_mod(core, mod_id).map_err(|e| e.to_string())?;
    let swap = prepare_fomod_tree(data_root, content_root, picked, dest, env)?;
    modlab_core::install::commit_prepared_install(
        core,
        loadout_id,
        mod_id,
        dest,
        modlab_core::install::InstallAdjustments::default(),
        swap,
    )
    .map_err(|e| e.to_string())?;
    discard_staging(data_root, mod_id);
    Ok(())
}

pub fn discard_staging(data_root: &Path, mod_id: i64) {
    let staging = data_root.join("staging").join(mod_id.to_string());
    if staging.exists() {
        let _ = modlab_core::files::move_to_trash(data_root, &staging);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path as StdPath;

    fn make_tree(entries: &[&str]) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        for rel in entries {
            let path = dir.path().join(rel);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).unwrap();
            }
            fs::write(path, b"placeholder").unwrap();
        }
        dir
    }

    #[test]
    fn detects_fomod() {
        let dir = make_tree(&["fomod/ModuleConfig.xml", "fomod/info.xml"]);
        let detected = detect_shape(dir.path()).unwrap();
        assert_eq!(detected.shape, ArchiveShape::Fomod);
        assert_eq!(detected.content_root, dir.path());
    }

    #[test]
    fn detects_wrapped_data_folder() {
        let dir = make_tree(&["Data/meshes/thing.nif"]);
        let detected = detect_shape(dir.path()).unwrap();
        assert_eq!(
            detected.shape,
            ArchiveShape::WrappedData {
                data_dir: "Data".into()
            }
        );
    }

    #[test]
    fn detects_bare_data_via_known_subdir() {
        let dir = make_tree(&["meshes/thing.nif", "textures/thing.dds"]);
        assert_eq!(
            detect_shape(dir.path()).unwrap().shape,
            ArchiveShape::BareData
        );
    }

    #[test]
    fn detects_bare_data_via_plugin_file_at_root() {
        let dir = make_tree(&["MyMod.esp"]);
        assert_eq!(
            detect_shape(dir.path()).unwrap().shape,
            ArchiveShape::BareData
        );
    }

    #[test]
    fn unrecognized_shape_is_unknown_not_a_guess() {
        let dir = make_tree(&["readme.txt", "some_installer.exe"]);
        assert_eq!(
            detect_shape(dir.path()).unwrap().shape,
            ArchiveShape::Unknown
        );
    }

    #[test]
    fn fomod_takes_priority_even_with_a_data_folder_present() {
        // A FOMOD can ship a Data/ folder alongside fomod/ for the
        // installer script to pull from — the guided installer must own
        // this case, not the bare/wrapped heuristics.
        let dir = make_tree(&["fomod/ModuleConfig.xml", "Data/meshes/thing.nif"]);
        assert_eq!(detect_shape(dir.path()).unwrap().shape, ArchiveShape::Fomod);
    }

    #[test]
    fn descends_through_nested_single_child_wrapping_folders() {
        // Mirrors the real SKSE archive's actual shape, found by testing
        // against it directly: two levels of "just one folder in here"
        // before any real content. A detector that only looks at the
        // extraction root misses this entirely.
        let dir = make_tree(&[
            "Skyrim Script Extender (SKSE64) Steam 2.2.8/skse64_2_02_08/skse64_loader.exe",
            "Skyrim Script Extender (SKSE64) Steam 2.2.8/skse64_2_02_08/skse64_1_6_1170.dll",
            "Skyrim Script Extender (SKSE64) Steam 2.2.8/skse64_2_02_08/Data/Scripts/placeholder.pex",
        ]);

        let detected = detect_shape(dir.path()).unwrap();

        assert_eq!(
            detected.content_root,
            dir.path()
                .join("Skyrim Script Extender (SKSE64) Steam 2.2.8")
                .join("skse64_2_02_08")
        );
        assert_eq!(
            detected.shape,
            ArchiveShape::WrappedData {
                data_dir: "Data".into()
            }
        );
    }

    #[test]
    fn wrapped_data_is_placed_without_a_nested_data_folder() {
        // Regression: the mod folder is linked onto Data/, so moving the
        // folder that *contains* Data/ deployed files as Data/Data/...
        let staged = make_tree(&["Data/meshes/thing.nif", "Data/MyMod.esp", "readme.txt"]);
        let out = tempfile::tempdir().unwrap();
        let dest = out.path().join("mods").join("1");

        place_content(staged.path(), &dest, false).unwrap();

        assert!(dest.join("meshes/thing.nif").is_file());
        assert!(dest.join("MyMod.esp").is_file());
        assert!(!dest.join("Data").exists());
    }

    #[test]
    fn wrapped_data_keeps_game_root_layout_for_the_engine_mod() {
        // SKSE: loader exe beside Data/, copied whole into the install root.
        let staged = make_tree(&["skse64_loader.exe", "Data/Scripts/placeholder.pex"]);
        let out = tempfile::tempdir().unwrap();
        let dest = out.path().join("mods").join("2");

        place_content(staged.path(), &dest, true).unwrap();

        assert!(dest.join("skse64_loader.exe").is_file());
        assert!(dest.join("Data/Scripts/placeholder.pex").is_file());
    }

    #[test]
    fn engine_layout_is_chosen_by_content_not_by_the_mods_name() {
        // The install command decides `keep_game_root` from the staged content.
        // "Address Library for SKSE Plugins" is a plain Data mod despite its name:
        // its SKSE/Plugins folder must end up at the mod root (= Data/SKSE/Plugins
        // once USVFS links the mod onto Data/), not be kept as a game-root layout.
        let skse = make_tree(&["skse64_loader.exe", "Data/Scripts/placeholder.pex"]);
        let address_library = make_tree(&["Data/SKSE/Plugins/x.dll"]);
        assert!(crate::engine::is_engine_content(
            &detect_shape(skse.path()).unwrap().content_root
        ));
        let detected = detect_shape(address_library.path()).unwrap();
        let keep_game_root = crate::engine::is_engine_content(&detected.content_root);
        assert!(!keep_game_root);

        let out = tempfile::tempdir().unwrap();
        let dest = out.path().join("mods").join("4");
        place_content(address_library.path(), &dest, keep_game_root).unwrap();

        assert!(dest.join("SKSE/Plugins/x.dll").is_file());
        assert!(!dest.join("Data").exists());
    }

    #[test]
    fn bare_data_is_placed_as_is() {
        let staged = make_tree(&["meshes/thing.nif"]);
        let out = tempfile::tempdir().unwrap();
        let dest = out.path().join("mods").join("3");

        place_content(staged.path(), &dest, false).unwrap();

        assert!(dest.join("meshes/thing.nif").is_file());
    }

    #[test]
    fn a_readme_beside_one_folder_does_not_hide_the_mod() {
        // The download lands as `readme.txt` plus the real folder. Stopping
        // at the readme used to report Unknown, so the shell refused to
        // install and the mod stayed an inactive archive.
        let staged = make_tree(&[
            "readme.txt",
            "My Mod/Data/MyMod.esp",
            "My Mod/Data/meshes/thing.nif",
        ]);
        let detected = detect_shape(staged.path()).unwrap();
        assert_eq!(
            detected.shape,
            ArchiveShape::WrappedData {
                data_dir: "Data".into()
            }
        );

        let out = tempfile::tempdir().unwrap();
        let dest = out.path().join("mods").join("9");
        let keep = crate::engine::is_engine_content(&detected.content_root);
        place_content(&detected.content_root, &dest, keep).unwrap();

        assert!(dest.join("MyMod.esp").is_file());
        assert!(dest.join("meshes/thing.nif").is_file());
        assert!(!dest.join("Data").exists());
        assert!(!dest.join("readme.txt").exists());
    }

    #[test]
    fn a_plugin_sitting_beside_data_is_kept_at_the_mod_root() {
        let staged = make_tree(&[
            "MyMod.esp",
            "MyMod.bsa",
            "Data/meshes/thing.nif",
            "readme.txt",
        ]);
        let out = tempfile::tempdir().unwrap();
        let dest = out.path().join("mods").join("8");

        place_content(staged.path(), &dest, false).unwrap();

        assert!(dest.join("MyMod.esp").is_file());
        assert!(dest.join("MyMod.bsa").is_file());
        assert!(dest.join("meshes/thing.nif").is_file());
        assert!(!dest.join("readme.txt").exists());
    }

    #[test]
    fn a_loader_without_a_data_folder_is_the_engine_not_unknown() {
        let staged = make_tree(&["skse64_loader.exe", "skse64_1_7_104.dll", "readme.txt"]);
        let detected = detect_shape(staged.path()).unwrap();
        assert_eq!(detected.shape, ArchiveShape::EngineRoot);
        assert!(crate::engine::is_engine_content(&detected.content_root));

        let out = tempfile::tempdir().unwrap();
        let dest = out.path().join("mods").join("7");
        place_content(&detected.content_root, &dest, true).unwrap();

        assert!(dest.join("skse64_loader.exe").is_file());
        assert!(dest.join("skse64_1_7_104.dll").is_file());
    }

    #[test]
    fn a_readme_does_not_hide_a_nested_skse_loader() {
        let staged = make_tree(&[
            "notes.txt",
            "Skyrim Script Extender/skse64_2_02_06/skse64_loader.exe",
            "Skyrim Script Extender/skse64_2_02_06/skse64_1_7_104.dll",
            "Skyrim Script Extender/skse64_2_02_06/Data/Scripts/Actor.pex",
        ]);
        let detected = detect_shape(staged.path()).unwrap();
        assert!(
            detected.content_root.join("skse64_loader.exe").is_file(),
            "loader must be at the content root, got {}",
            detected.content_root.display()
        );
        assert!(crate::engine::is_engine_content(&detected.content_root));

        let out = tempfile::tempdir().unwrap();
        let dest = out.path().join("mods").join("6");
        place_content(&detected.content_root, &dest, true).unwrap();

        assert!(dest.join("skse64_loader.exe").is_file());
        assert!(dest.join("Data/Scripts/Actor.pex").is_file());
        assert!(!crate::engine::is_engine_content(&dest.join("Data")));
    }

    #[test]
    fn detects_the_real_skse_archive_if_present() {
        // Same spirit as the other real-machine tests this session.
        let real_archive = StdPath::new(
            "F:/_Modding/Skyrim Modding/_Download/1.6.11/Skyrim Script Extender (SKSE64) Steam 2.2.8.7z",
        );
        if !real_archive.is_file() {
            return;
        }
        let dest_dir = tempfile::tempdir().unwrap();
        modlab_core::archive::extract(real_archive, dest_dir.path()).unwrap();

        let detected = detect_shape(dest_dir.path()).unwrap();

        assert!(
            detected.content_root.join("skse64_loader.exe").is_file(),
            "resolved content root should be where the loader actually is: {}",
            detected.content_root.display()
        );
    }

    #[test]
    fn install_failure_leaves_the_game_tree_and_uninstall_keeps_the_files() {
        let root = tempfile::tempdir().unwrap();
        let game = root.path().join("Game").join("Data");
        fs::create_dir_all(&game).unwrap();
        fs::write(game.join("Skyrim.esm"), b"VANILLA").unwrap();

        let content = root.path().join("content");
        fs::create_dir_all(content.join("meshes")).unwrap();
        fs::write(content.join("meshes").join("a.nif"), b"mesh").unwrap();
        let archive = root.path().join("mod.zip");
        fs::write(&archive, b"archive").unwrap();

        let core = modlab_core::Core::open_in_memory().unwrap();
        let loadout = modlab_core::loadout::create(&core, "skyrimse", "Test").unwrap();
        let download_id = modlab_core::download::capture(
            &core,
            "https://example.test/mod.zip",
            "mod.zip",
            &archive,
            None,
        )
        .unwrap()
        .id();
        let mod_id =
            modlab_core::download::attach_to_loadout(&core, download_id, loadout.id, None).unwrap();
        let dest = root.path().join("mods").join(mod_id.to_string());

        let missing = root.path().join("missing-content");
        let err = install_archived(&core, root.path(), loadout.id, mod_id, &missing, &dest);
        assert!(err.is_err(), "{err:?}");
        assert!(!dest.exists(), "a failed install leaves no partial folder");
        assert_eq!(fs::read(game.join("Skyrim.esm")).unwrap(), b"VANILLA");
        assert_eq!(
            modlab_core::loadout::entries(&core, loadout.id).unwrap()[0].status,
            modlab_core::loadout::STATUS_ARCHIVED
        );

        install_archived(&core, root.path(), loadout.id, mod_id, &content, &dest).unwrap();
        assert_eq!(
            fs::read(dest.join("meshes").join("a.nif")).unwrap(),
            b"mesh"
        );
        assert_eq!(fs::read(game.join("Skyrim.esm")).unwrap(), b"VANILLA");

        let trashed =
            modlab_core::install::uninstall_mod(&core, root.path(), loadout.id, mod_id, &dest)
                .unwrap()
                .expect("extracted files moved to the trash");
        assert!(!dest.exists());
        assert_eq!(
            fs::read(trashed.join("meshes").join("a.nif")).unwrap(),
            b"mesh"
        );
        assert_eq!(fs::read(game.join("Skyrim.esm")).unwrap(), b"VANILLA");
        assert_eq!(
            modlab_core::loadout::entries(&core, loadout.id).unwrap()[0].status,
            modlab_core::loadout::STATUS_ARCHIVED
        );
    }

    #[test]
    fn fomod_priority_keeps_the_higher_file_and_rolls_back_with_the_swap() {
        let root = tempfile::tempdir().unwrap();
        let content = root.path().join("content");
        fs::create_dir_all(content.join("fomod")).unwrap();
        fs::write(content.join("low.txt"), b"low").unwrap();
        fs::write(content.join("high.txt"), b"high").unwrap();
        fs::write(
            content.join("fomod").join("ModuleConfig.xml"),
            r#"<config><requiredInstallFiles>
              <file source="low.txt" destination="out.txt" priority="1"/>
              <file source="high.txt" destination="out.txt" priority="8"/>
            </requiredInstallFiles></config>"#,
        )
        .unwrap();
        let dest = root.path().join("mods").join("1");
        let env = crate::fomod::FomodEnv {
            file_state: &|_| crate::fomod::FileState::Missing,
            game_version: None,
        };
        let swap = prepare_fomod_tree(root.path(), &content, &[], &dest, &env).unwrap();
        assert_eq!(fs::read(dest.join("out.txt")).unwrap(), b"high");
        swap.commit();
    }
}
