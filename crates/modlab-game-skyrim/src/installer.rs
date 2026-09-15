//! Given an already-extracted archive, work out where its files belong —
//! see `docs/idees.md` §A3 and `docs/01-architecture-decidee.md` §9 for the
//! reasoning. This only *classifies*; it doesn't move or copy a single
//! file. Placement (actually deploying into a mod's folder) is a later,
//! separate step, same "read/decide, then act" split as the MO2 importer.
//!
//! Deliberately narrow for now: recognizes the common shapes (bare Data
//! content, a wrapped `Data/` folder, a FOMOD needing the guided installer
//! that doesn't exist yet) and says `Unknown` rather than guessing for
//! anything else — including root-level files (SKSE loaders, ENB DLLs,
//! `d3d11.dll`) which `docs/idees.md` §A3 covers but this doesn't yet.

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
