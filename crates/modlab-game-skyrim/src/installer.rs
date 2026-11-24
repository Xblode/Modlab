//! Given an already-extracted archive, work out where its files belong —
//! see `docs/faisabilite.md` A3 and `docs/01-architecture-decidee.md` §9 for the
//! reasoning. This only *classifies*; it doesn't move or copy a single
//! file. Placement (actually deploying into a mod's folder) is a later,
//! separate step, same "read/decide, then act" split as the MO2 importer.
//!
//! Deliberately narrow for now: recognizes the common shapes (bare Data
//! content, a wrapped `Data/` folder, a FOMOD needing the guided installer
//! that doesn't exist yet) and says `Unknown` rather than guessing for
//! anything else — including root-level files (SKSE loaders, ENB DLLs,
//! `d3d11.dll`) which `docs/faisabilite.md` A3 covers but this doesn't yet.

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
        match dirs.as_slice() {
            [only] if files.is_empty() => current = current.join(only),
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
    let source = match detected.shape {
        ArchiveShape::WrappedData { data_dir } if !keep_game_root => {
            detected.content_root.join(data_dir)
        }
        _ => detected.content_root,
    };
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent)?;
    }
    // A rename inside the same data root is atomic and instant; the copy
    // fallback covers a staging area that landed on another volume.
    if fs::rename(&source, dest).is_err() {
        copy_dir(&source, dest)?;
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
    } else {
        ArchiveShape::Unknown
    }
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
}
