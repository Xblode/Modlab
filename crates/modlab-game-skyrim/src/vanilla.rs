//! Read-only check of what in a Skyrim install is not part of a vanilla
//! `Data/` folder, plus config files kept apart from that list.
//!
//! Nothing here writes, renames, or deletes. A clean result is both lists
//! empty — this module never reports files the baseline is missing, only
//! names that are present and outside it.
//!
//! # What counts as vanilla `Data/`
//!
//! A stock Special Edition `Data/` is a flat set of masters and archives
//! (no loose folders). The baseline is:
//!
//! - the five official masters and their BSAs, including `Skyrim - Patch.bsa`
//!   and `Skyrim - Textures0.bsa` … `Textures8.bsa`, `Meshes0` / `Meshes1`;
//! - one voice archive per language, `Skyrim - Voices_<lang><digit>.bsa`
//!   (`en0`, `fr0`, …);
//! - the four Creations shipped to every 1.6 owner (Fishing, Rare Curios,
//!   Saints & Seducers, Survival Mode) and `_ResourcePack`, each plugin
//!   with its `.bsa`;
//! - any plugin named in `Skyrim.ccc` at the install root, plus a `.bsa`
//!   whose stem is exactly that plugin's stem.
//!
//! Paid Anniversary archives are therefore vanilla when the install's own
//! `Skyrim.ccc` lists them, and foreign when it does not. An archive whose
//! name does not share the plugin stem (or a name this list does not know,
//! such as a later official file we have not recorded) is reported.
//!
//! # Shape of the result
//!
//! Top-level names only. A foreign directory is one entry; its loose files
//! are not expanded. Config files (`.ini`, `.toml`, `.cfg`, `.json`,
//! `.yaml`, `.yml`) are not repeated in `extras`: a top-level config is
//! only in `configs`, and a config inside a foreign directory is listed
//! there as well as the directory itself in `extras`.
//!
//! The search for those nested configs does not enter asset trees
//! (`meshes`, `textures`, `scripts`, …) and stops eight directories below
//! the top-level name. `Documents\My Games\…` is separate (`Location::Documents`):
//! the game's own INIs, `ControlMap_Custom.txt`, and the `Saves` / `Logs`
//! folders are not reported. Other files there are configs. `Saves` is
//! never walked.

use std::collections::HashSet;
use std::fs;
use std::io;
use std::path::Path;

use encoding_rs::WINDOWS_1252;

/// How far below a top-level `Data/` name we still look for config files.
const MAX_CONFIG_DEPTH: u32 = 8;

/// Masters, base archives, and the Creations every updated 1.6 install has.
const BASE_FILES: &[&str] = &[
    "Skyrim.esm",
    "Update.esm",
    "Dawnguard.esm",
    "HearthFires.esm",
    "Dragonborn.esm",
    "Skyrim - Animations.bsa",
    "Skyrim - Interface.bsa",
    "Skyrim - Meshes0.bsa",
    "Skyrim - Meshes1.bsa",
    "Skyrim - Misc.bsa",
    "Skyrim - Shaders.bsa",
    "Skyrim - Sounds.bsa",
    "Skyrim - Textures0.bsa",
    "Skyrim - Textures1.bsa",
    "Skyrim - Textures2.bsa",
    "Skyrim - Textures3.bsa",
    "Skyrim - Textures4.bsa",
    "Skyrim - Textures5.bsa",
    "Skyrim - Textures6.bsa",
    "Skyrim - Textures7.bsa",
    "Skyrim - Textures8.bsa",
    "Skyrim - Patch.bsa",
    "Update.bsa",
    "Dawnguard.bsa",
    "HearthFires.bsa",
    "Dragonborn.bsa",
    "_ResourcePack.esl",
    "_ResourcePack.bsa",
    "ccBGSSSE001-Fish.esm",
    "ccBGSSSE001-Fish.bsa",
    "ccBGSSSE025-AdvDSGS.esm",
    "ccBGSSSE025-AdvDSGS.bsa",
    "ccBGSSSE037-Curios.esl",
    "ccBGSSSE037-Curios.bsa",
    "ccQDRSSE001-SurvivalMode.esl",
    "ccQDRSSE001-SurvivalMode.bsa",
];

/// Folders whose contents are assets. The folder itself is still an extra
/// when it sits at the top of `Data/`; we just don't walk it looking for
/// an `.ini` buried under thousands of loose files.
const ASSET_DIRS: &[&str] = &[
    "actors",
    "animations",
    "facegendata",
    "geometry",
    "grass",
    "lodsettings",
    "meshes",
    "music",
    "scripts",
    "seq",
    "shadersfx",
    "sound",
    "sounds",
    "strings",
    "terrain",
    "textures",
    "trees",
    "video",
    "videos",
];

/// Written by the game when the player remaps controls. Not one of
/// [`crate::CONFIG_INI_NAMES`], which are the three INI files, but still
/// stock for this folder.
const CONTROL_MAP: &str = "ControlMap_Custom.txt";

/// Folders in that same directory that are the game's, not mod config.
const DOCUMENT_SKIP_DIRS: &[&str] = &["Saves", "Logs"];

/// Where a reported path is relative to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Location {
    /// The game's `Data/` folder.
    Data,
    /// The folder that contains `SkyrimPrefs.ini`.
    Documents,
    /// The install root (`Skyrim.ccc` only).
    Install,
}

/// A top-level `Data/` name that is not in the vanilla baseline.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataExtra {
    /// Relative to `Data/`, `/`-separated. A directory is not expanded.
    pub path: String,
    pub kind: ExtraKind,
}

/// Directory or file. A symlink is reported as a file and never followed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ExtraKind {
    Dir,
    File,
}

/// A config file, either in `Data/` or next to the game INIs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigFile {
    /// Relative to `location`, `/`-separated.
    pub path: String,
    pub location: Location,
}

/// One entry the scan could not read, or a directory that resolved outside
/// the folder being scanned.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unreadable {
    pub path: String,
    pub location: Location,
    pub problem: ReadProblem,
}

/// Why one entry was skipped.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReadProblem {
    Io(String),
    /// `canonicalize` left the directory the scan started from.
    EscapesRoot,
}

/// Everything past the vanilla baseline. Both lists can be empty.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VanillaScan {
    pub extras: Vec<DataExtra>,
    pub configs: Vec<ConfigFile>,
    pub unreadable: Vec<Unreadable>,
    /// `true` when `documents_dir` existed and its entries were read.
    /// `false` when it was absent — not the same as "no extra config".
    pub documents_scanned: bool,
}

/// List what in `data_dir` exceeds the vanilla baseline, and config files
/// under `documents_dir` when that directory exists.
///
/// `install_root` is only read for `Skyrim.ccc` (Creation Club plugin
/// names). A missing file is normal for a 1.5 install and is not an error.
/// `documents_dir` is skipped when it is `None` or not a directory; that
/// does not fail the `Data/` scan.
///
/// Returns an error when `data_dir` itself is not a directory. Does not
/// create it.
pub fn scan(
    data_dir: &Path,
    install_root: Option<&Path>,
    documents_dir: Option<&Path>,
) -> io::Result<VanillaScan> {
    if !data_dir.is_dir() {
        return Err(io::Error::new(io::ErrorKind::NotFound, "not a directory"));
    }
    let root_canon = data_dir.canonicalize()?;
    let mut scan = VanillaScan {
        extras: Vec::new(),
        configs: Vec::new(),
        unreadable: Vec::new(),
        documents_scanned: false,
    };
    let (names, stems) = vanilla_tables(install_root, &mut scan.unreadable);
    scan_data_entries(data_dir, &root_canon, &names, &stems, &mut scan)?;
    if let Some(dir) = documents_dir.filter(|d| d.is_dir()) {
        scan_documents(dir, &mut scan);
    }
    sort_scan(&mut scan);
    Ok(scan)
}

fn vanilla_tables(
    install_root: Option<&Path>,
    issues: &mut Vec<Unreadable>,
) -> (HashSet<String>, HashSet<String>) {
    let mut names = HashSet::with_capacity(BASE_FILES.len() + 32);
    for name in BASE_FILES {
        names.insert((*name).to_ascii_lowercase());
    }
    let mut stems = HashSet::new();
    load_ccc(install_root, &mut names, &mut stems, issues);
    (names, stems)
}

/// Same Windows-1252 reading as the load-order code (`plugins.rs`): Bethesda
/// writes `Skyrim.ccc` in that code page, one plugin name per line.
fn load_ccc(
    install_root: Option<&Path>,
    names: &mut HashSet<String>,
    stems: &mut HashSet<String>,
    issues: &mut Vec<Unreadable>,
) {
    let Some(root) = install_root else {
        return;
    };
    let path = root.join("Skyrim.ccc");
    let bytes = match fs::read(&path) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return,
        Err(e) => {
            issues.push(Unreadable {
                path: "Skyrim.ccc".to_string(),
                location: Location::Install,
                problem: ReadProblem::Io(e.to_string()),
            });
            return;
        }
    };
    let (text, _, _) = WINDOWS_1252.decode(&bytes);
    let text = text.trim_start_matches('\u{feff}');
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        remember_ccc_plugin(line, names, stems);
    }
}

fn remember_ccc_plugin(line: &str, names: &mut HashSet<String>, stems: &mut HashSet<String>) {
    if line == "." || line == ".." || line.contains(['/', '\\']) {
        return;
    }
    let lower = line.to_ascii_lowercase();
    if let Some((stem, ext)) = lower.rsplit_once('.') {
        if matches!(ext, "esm" | "esl" | "esp") && !stem.is_empty() {
            stems.insert(stem.to_string());
        }
    }
    names.insert(lower);
}

fn is_vanilla_file(name: &str, names: &HashSet<String>, stems: &HashSet<String>) -> bool {
    let lower = name.to_ascii_lowercase();
    if names.contains(&lower) || is_voice_bsa(&lower) {
        return true;
    }
    match lower.rsplit_once('.') {
        Some((stem, "bsa")) => stems.contains(stem),
        _ => false,
    }
}

fn is_voice_bsa(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    let Some(rest) = lower.strip_prefix("skyrim - voices_") else {
        return false;
    };
    let Some(rest) = rest.strip_suffix(".bsa") else {
        return false;
    };
    let len = rest.len();
    if !(3..=4).contains(&len) {
        return false;
    }
    let (lang, index) = rest.split_at(len - 1);
    lang.chars().all(|c| c.is_ascii_alphabetic()) && index.chars().all(|c| c.is_ascii_digit())
}

fn is_config_file(name: &str) -> bool {
    Path::new(name)
        .extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| {
            ["ini", "toml", "cfg", "json", "yaml", "yml"]
                .iter()
                .any(|want| ext.eq_ignore_ascii_case(want))
        })
}

fn is_asset_dir(name: &str) -> bool {
    ASSET_DIRS.iter().any(|dir| name.eq_ignore_ascii_case(dir))
}

fn is_document_file(name: &str) -> bool {
    name.eq_ignore_ascii_case(CONTROL_MAP)
        || crate::CONFIG_INI_NAMES
            .iter()
            .any(|file| name.eq_ignore_ascii_case(file))
}

fn is_document_skip_dir(name: &str) -> bool {
    DOCUMENT_SKIP_DIRS
        .iter()
        .any(|dir| name.eq_ignore_ascii_case(dir))
}

fn join_rel(parent: &str, name: &str) -> String {
    if parent.is_empty() {
        name.to_string()
    } else {
        format!("{parent}/{name}")
    }
}

fn stays_inside(root_canon: &Path, path: &Path) -> io::Result<bool> {
    Ok(path.canonicalize()?.starts_with(root_canon))
}

fn entry_name(entry: &fs::DirEntry) -> String {
    entry.file_name().to_string_lossy().into_owned()
}

fn push_io(scan: &mut VanillaScan, path: String, location: Location, err: io::Error) {
    scan.unreadable.push(Unreadable {
        path,
        location,
        problem: ReadProblem::Io(err.to_string()),
    });
}

fn scan_data_entries(
    data_dir: &Path,
    root_canon: &Path,
    names: &HashSet<String>,
    stems: &HashSet<String>,
    scan: &mut VanillaScan,
) -> io::Result<()> {
    let read = fs::read_dir(data_dir)?;
    for entry in read {
        let entry = match entry {
            Ok(entry) => entry,
            Err(e) => {
                push_io(scan, "(entry)".to_string(), Location::Data, e);
                continue;
            }
        };
        let name = entry_name(&entry);
        let ft = match entry.file_type() {
            Ok(ft) => ft,
            Err(e) => {
                push_io(scan, name, Location::Data, e);
                continue;
            }
        };
        let path = entry.path();
        if ft.is_symlink() {
            // A link wearing a vanilla name is still not the stock file.
            if is_config_file(&name) {
                scan.configs.push(ConfigFile {
                    path: name,
                    location: Location::Data,
                });
            } else {
                scan.extras.push(DataExtra {
                    path: name,
                    kind: ExtraKind::File,
                });
            }
            continue;
        }
        if ft.is_dir() {
            scan.extras.push(DataExtra {
                path: name.clone(),
                kind: ExtraKind::Dir,
            });
            if is_asset_dir(&name) {
                continue;
            }
            match stays_inside(root_canon, &path) {
                Ok(true) => collect_configs(&path, root_canon, &name, 0, Location::Data, scan),
                Ok(false) => scan.unreadable.push(Unreadable {
                    path: name,
                    location: Location::Data,
                    problem: ReadProblem::EscapesRoot,
                }),
                Err(e) => push_io(scan, name, Location::Data, e),
            }
            continue;
        }
        if is_vanilla_file(&name, names, stems) {
            continue;
        }
        if is_config_file(&name) {
            scan.configs.push(ConfigFile {
                path: name,
                location: Location::Data,
            });
        } else {
            scan.extras.push(DataExtra {
                path: name,
                kind: ExtraKind::File,
            });
        }
    }
    Ok(())
}

fn collect_configs(
    dir: &Path,
    root_canon: &Path,
    rel: &str,
    depth: u32,
    location: Location,
    scan: &mut VanillaScan,
) {
    if depth >= MAX_CONFIG_DEPTH {
        return;
    }
    let read = match fs::read_dir(dir) {
        Ok(read) => read,
        Err(e) => {
            push_io(scan, rel.to_string(), location, e);
            return;
        }
    };
    for entry in read {
        let entry = match entry {
            Ok(entry) => entry,
            Err(e) => {
                push_io(scan, rel.to_string(), location, e);
                continue;
            }
        };
        let name = entry_name(&entry);
        let child_rel = join_rel(rel, &name);
        let ft = match entry.file_type() {
            Ok(ft) => ft,
            Err(e) => {
                push_io(scan, child_rel, location, e);
                continue;
            }
        };
        let path = entry.path();
        if ft.is_symlink() {
            if is_config_file(&name) {
                scan.configs.push(ConfigFile {
                    path: child_rel,
                    location,
                });
            }
            continue;
        }
        if ft.is_dir() {
            if is_asset_dir(&name) {
                continue;
            }
            match stays_inside(root_canon, &path) {
                Ok(true) => {
                    collect_configs(&path, root_canon, &child_rel, depth + 1, location, scan);
                }
                Ok(false) => scan.unreadable.push(Unreadable {
                    path: child_rel,
                    location,
                    problem: ReadProblem::EscapesRoot,
                }),
                Err(e) => push_io(scan, child_rel, location, e),
            }
            continue;
        }
        if is_config_file(&name) {
            scan.configs.push(ConfigFile {
                path: child_rel,
                location,
            });
        }
    }
}

fn scan_documents(dir: &Path, scan: &mut VanillaScan) {
    let root_canon = match dir.canonicalize() {
        Ok(path) => path,
        Err(e) => {
            push_io(scan, ".".to_string(), Location::Documents, e);
            return;
        }
    };
    let read = match fs::read_dir(dir) {
        Ok(read) => read,
        Err(e) => {
            push_io(scan, ".".to_string(), Location::Documents, e);
            return;
        }
    };
    scan.documents_scanned = true;
    for entry in read {
        let entry = match entry {
            Ok(entry) => entry,
            Err(e) => {
                push_io(scan, ".".to_string(), Location::Documents, e);
                continue;
            }
        };
        let name = entry_name(&entry);
        let ft = match entry.file_type() {
            Ok(ft) => ft,
            Err(e) => {
                push_io(scan, name, Location::Documents, e);
                continue;
            }
        };
        if (ft.is_dir() || ft.is_symlink()) && is_document_skip_dir(&name) {
            continue;
        }
        if ft.is_symlink() {
            if !is_document_file(&name) {
                scan.configs.push(ConfigFile {
                    path: name,
                    location: Location::Documents,
                });
            }
            continue;
        }
        let path = entry.path();
        if ft.is_dir() {
            match stays_inside(&root_canon, &path) {
                Ok(true) => {
                    collect_configs(&path, &root_canon, &name, 0, Location::Documents, scan);
                }
                Ok(false) => scan.unreadable.push(Unreadable {
                    path: name,
                    location: Location::Documents,
                    problem: ReadProblem::EscapesRoot,
                }),
                Err(e) => push_io(scan, name, Location::Documents, e),
            }
            continue;
        }
        if is_document_file(&name) {
            continue;
        }
        scan.configs.push(ConfigFile {
            path: name,
            location: Location::Documents,
        });
    }
}

fn sort_scan(scan: &mut VanillaScan) {
    let lower = |path: &str| path.to_lowercase();
    scan.extras.sort_by(|a, b| {
        a.kind
            .cmp(&b.kind)
            .then_with(|| lower(&a.path).cmp(&lower(&b.path)))
    });
    scan.configs.sort_by(|a, b| {
        a.location
            .cmp(&b.location)
            .then_with(|| lower(&a.path).cmp(&lower(&b.path)))
    });
    scan.unreadable.sort_by(|a, b| {
        a.location
            .cmp(&b.location)
            .then_with(|| lower(&a.path).cmp(&lower(&b.path)))
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn touch(path: &Path, bytes: &[u8]) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, bytes).unwrap();
    }

    fn tree(dir: &Path) -> Vec<(String, Vec<u8>)> {
        let mut out = Vec::new();
        fn walk(root: &Path, dir: &Path, out: &mut Vec<(String, Vec<u8>)>) {
            for entry in fs::read_dir(dir).unwrap() {
                let entry = entry.unwrap();
                let path = entry.path();
                if path.is_dir() {
                    walk(root, &path, out);
                } else {
                    let rel = path
                        .strip_prefix(root)
                        .unwrap()
                        .to_string_lossy()
                        .replace('\\', "/");
                    out.push((rel, fs::read(&path).unwrap()));
                }
            }
        }
        if dir.is_dir() {
            walk(dir, dir, &mut out);
        }
        out.sort();
        out
    }

    #[test]
    fn config_extensions_and_voice_packs_match_only_the_stock_names() {
        assert!(is_config_file("a.INI"));
        assert!(is_config_file("a.toml"));
        assert!(is_config_file("a.cfg"));
        assert!(is_config_file("a.json"));
        assert!(is_config_file("a.yaml"));
        assert!(is_config_file("a.yml"));
        assert!(!is_config_file("a.ini.bak"));
        assert!(!is_config_file("a.xml"));
        assert!(!is_config_file("a.txt"));
        assert!(!is_config_file("a.esp"));

        assert!(is_voice_bsa("Skyrim - Voices_en0.bsa"));
        assert!(is_voice_bsa("Skyrim - Voices_FR1.bsa"));
        assert!(is_voice_bsa("skyrim - voices_zht0.bsa"));
        assert!(!is_voice_bsa("Skyrim - Voices_english0.bsa"));
        assert!(!is_voice_bsa("Skyrim - Voices_en0.bsa.bak"));
        assert!(!is_voice_bsa("Skyrim - Voices_en.bsa"));
        assert!(!is_voice_bsa("Voices_en0.bsa"));
    }

    #[test]
    fn official_masters_are_not_extras() {
        let tmp = tempfile::tempdir().unwrap();
        let data = tmp.path().join("Data");
        fs::create_dir(&data).unwrap();
        for name in crate::plugins::OFFICIAL_PLUGINS {
            fs::write(data.join(name), b"esm").unwrap();
        }
        let scan = scan(&data, None, None).unwrap();
        assert!(scan.extras.is_empty(), "{:?}", scan.extras);
        assert!(scan.configs.is_empty(), "{:?}", scan.configs);
    }

    #[test]
    fn texture_and_mesh_archive_indexes_stop_at_the_stock_range() {
        let tmp = tempfile::tempdir().unwrap();
        let data = tmp.path().join("Data");
        fs::create_dir(&data).unwrap();
        for i in 0..=9 {
            fs::write(data.join(format!("Skyrim - Textures{i}.bsa")), b"x").unwrap();
        }
        for name in [
            "Skyrim - Meshes0.bsa",
            "Skyrim - Meshes1.bsa",
            "Skyrim - Meshes2.bsa",
        ] {
            fs::write(data.join(name), b"x").unwrap();
        }
        let scan = scan(&data, None, None).unwrap();
        assert_eq!(
            scan.extras,
            vec![
                DataExtra {
                    path: "Skyrim - Meshes2.bsa".into(),
                    kind: ExtraKind::File,
                },
                DataExtra {
                    path: "Skyrim - Textures9.bsa".into(),
                    kind: ExtraKind::File,
                },
            ]
        );
    }

    #[test]
    fn free_creation_club_files_are_vanilla_without_a_ccc_file() {
        let tmp = tempfile::tempdir().unwrap();
        let data = tmp.path().join("Data");
        fs::create_dir(&data).unwrap();
        for name in [
            "_ResourcePack.esl",
            "_ResourcePack.bsa",
            "ccBGSSSE001-Fish.esm",
            "ccBGSSSE001-Fish.bsa",
            "ccBGSSSE025-AdvDSGS.esm",
            "ccBGSSSE025-AdvDSGS.bsa",
            "ccBGSSSE037-Curios.esl",
            "ccBGSSSE037-Curios.bsa",
            "ccQDRSSE001-SurvivalMode.esl",
            "ccQDRSSE001-SurvivalMode.bsa",
        ] {
            fs::write(data.join(name), b"x").unwrap();
        }
        let scan = scan(&data, Some(tmp.path()), None).unwrap();
        assert!(scan.extras.is_empty(), "{:?}", scan.extras);
        assert!(scan.unreadable.is_empty(), "{:?}", scan.unreadable);
    }

    #[test]
    fn lists_data_extras_and_keeps_configs_apart_without_writing() {
        let tmp = tempfile::tempdir().unwrap();
        let game = tmp.path().join("game");
        let data = game.join("Data");
        let docs = tmp.path().join("docs");
        fs::create_dir_all(&data).unwrap();
        fs::create_dir_all(&docs).unwrap();

        touch(
            &game.join("Skyrim.ccc"),
            b"# comment\r\nccTest001-Thing.esl\r\n\r\nccAbsent.esl\r\n",
        );
        for name in [
            "SKYRIM.ESM",
            "HearthFires.bsa",
            "Dawnguard.bsa",
            "Skyrim - Patch.bsa",
            "Skyrim - Voices_FR0.bsa",
            "ccBGSSSE001-Fish.esm",
            "ccBGSSSE001-Fish.bsa",
            "ccTest001-Thing.esl",
            "ccTest001-Thing.bsa",
        ] {
            touch(&data.join(name), b"vanilla");
        }
        touch(&data.join("ccTest001-Thing.esp"), b"same-stem-but-esp");
        touch(&data.join("Mod.esp"), b"mod");
        touch(
            &data.join("Skyrim - Voices_english0.bsa"),
            b"not-a-language-pack",
        );
        touch(&data.join("a.cfg"), b"cfg");
        touch(&data.join("foo.ini"), b"ini");
        touch(&data.join("MCM/Config/iHUD.ini"), b"mcm");
        touch(&data.join("SKSE/Plugins/EngineFixes.ini"), b"ef");
        touch(&data.join("SKSE/Plugins/foo.toml"), b"toml");
        touch(&data.join("SKSE/meshes/skip.ini"), b"skip");
        touch(&data.join("Meshes/hidden.ini"), b"hidden");
        touch(&data.join("Meshes/armor/a.nif"), b"nif");
        touch(&data.join("Update.esm/mod.ini"), b"dir-named-like-a-master");

        touch(&docs.join("Skyrim.ini"), b"ini");
        touch(&docs.join("SkyrimPrefs.ini"), b"prefs");
        touch(&docs.join("SkyrimCustom.ini"), b"custom");
        touch(&docs.join("ControlMap_Custom.txt"), b"keys");
        touch(&docs.join("Skyrim.ini.bak"), b"backup");
        touch(&docs.join("notes.ini"), b"notes");
        touch(&docs.join("Saves/Quick.ess"), b"save");
        touch(&docs.join("Saves/cheat.ini"), b"nope");
        touch(&docs.join("Logs/Script/Papyrus.0.log"), b"log");
        touch(&docs.join("Logs/foo.ini"), b"nope");
        touch(&docs.join("SKSE/skse.log"), b"log");
        touch(&docs.join("SKSE/skse.ini"), b"skse");
        touch(&docs.join("SKSE/Plugins/crash.ini"), b"crash");

        let before_game = tree(&game);
        let before_docs = tree(&docs);
        let scan = scan(&data, Some(&game), Some(&docs)).unwrap();
        assert_eq!(
            tree(&game),
            before_game,
            "scan must not write under the game"
        );
        assert_eq!(
            tree(&docs),
            before_docs,
            "scan must not write under Documents"
        );

        assert_eq!(
            scan.extras,
            vec![
                DataExtra {
                    path: "MCM".into(),
                    kind: ExtraKind::Dir,
                },
                DataExtra {
                    path: "Meshes".into(),
                    kind: ExtraKind::Dir,
                },
                DataExtra {
                    path: "SKSE".into(),
                    kind: ExtraKind::Dir,
                },
                DataExtra {
                    path: "Update.esm".into(),
                    kind: ExtraKind::Dir,
                },
                DataExtra {
                    path: "ccTest001-Thing.esp".into(),
                    kind: ExtraKind::File,
                },
                DataExtra {
                    path: "Mod.esp".into(),
                    kind: ExtraKind::File,
                },
                DataExtra {
                    path: "Skyrim - Voices_english0.bsa".into(),
                    kind: ExtraKind::File,
                },
            ]
        );
        assert_eq!(
            scan.configs,
            vec![
                ConfigFile {
                    path: "a.cfg".into(),
                    location: Location::Data,
                },
                ConfigFile {
                    path: "foo.ini".into(),
                    location: Location::Data,
                },
                ConfigFile {
                    path: "MCM/Config/iHUD.ini".into(),
                    location: Location::Data,
                },
                ConfigFile {
                    path: "SKSE/Plugins/EngineFixes.ini".into(),
                    location: Location::Data,
                },
                ConfigFile {
                    path: "SKSE/Plugins/foo.toml".into(),
                    location: Location::Data,
                },
                ConfigFile {
                    path: "Update.esm/mod.ini".into(),
                    location: Location::Data,
                },
                ConfigFile {
                    path: "notes.ini".into(),
                    location: Location::Documents,
                },
                ConfigFile {
                    path: "SKSE/Plugins/crash.ini".into(),
                    location: Location::Documents,
                },
                ConfigFile {
                    path: "SKSE/skse.ini".into(),
                    location: Location::Documents,
                },
                ConfigFile {
                    path: "Skyrim.ini.bak".into(),
                    location: Location::Documents,
                },
            ]
        );
        assert!(scan.documents_scanned);
        assert!(scan.unreadable.is_empty(), "{:?}", scan.unreadable);
    }

    #[test]
    fn refuses_a_file_and_does_not_create_a_missing_documents_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let file = tmp.path().join("not-a-dir");
        fs::write(&file, b"keep").unwrap();
        assert!(scan(&file, None, None).is_err());
        assert_eq!(fs::read(&file).unwrap(), b"keep");

        let data = tmp.path().join("Data");
        fs::create_dir(&data).unwrap();
        fs::write(data.join("Mod.esp"), b"m").unwrap();
        let missing = tmp.path().join("no-docs");
        let scan = scan(&data, None, Some(&missing)).unwrap();
        assert!(!scan.documents_scanned);
        assert_eq!(
            scan.extras,
            vec![DataExtra {
                path: "Mod.esp".into(),
                kind: ExtraKind::File,
            }]
        );
        assert!(!missing.exists());
    }

    #[cfg(windows)]
    #[test]
    fn a_link_out_of_data_is_not_followed() {
        let tmp = tempfile::tempdir().unwrap();
        let data = tmp.path().join("Data");
        let outside = tmp.path().join("outside");
        fs::create_dir_all(&data).unwrap();
        fs::create_dir_all(&outside).unwrap();
        fs::write(outside.join("secret.ini"), b"secret").unwrap();
        let link = data.join("link");
        if std::os::windows::fs::symlink_dir(&outside, &link).is_err() {
            return;
        }
        let scan = scan(&data, None, None).unwrap();
        assert!(
            scan.configs.iter().all(|c| !c.path.contains("secret")),
            "{:?}",
            scan.configs
        );
        assert!(
            scan.extras.iter().any(|e| e.path == "link"),
            "{:?}",
            scan.extras
        );
        assert_eq!(fs::read(outside.join("secret.ini")).unwrap(), b"secret");
    }
}
