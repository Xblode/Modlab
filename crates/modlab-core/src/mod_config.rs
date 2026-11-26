//! Config files that belong to one mod's own folder.
//!
//! This is not the vanilla scan. That one walks the whole game `Data/`
//! directory and separates stock files from extras. Here the caller already
//! points at a single mod tree — the folder the install copied — and only
//! the files inside that tree are considered.
//!
//! A file counts when its extension is `.ini`, `.toml`, `.cfg`, `.json`,
//! `.yaml` or `.yml`. Paths such as `MCM/config/…` or `SKSE/Plugins/*.ini`
//! are included only because they sit in this tree and use one of those
//! extensions. No mod is recognized by name.
//!
//! Bookkeeping names (`meta.ini`, `desktop.ini`, …) are already dropped by
//! [`crate::conflicts::list_files_relative`], so they never show up here.

use std::fs;
use std::io;
use std::path::{Component, Path};

use crate::conflicts::list_files_relative;

/// Largest file whose bytes are returned for a read-only preview.
pub const MAX_CONFIG_PREVIEW_BYTES: u64 = 64 * 1024;

const CONFIG_EXTENSIONS: &[&str] = &["ini", "toml", "cfg", "json", "yaml", "yml"];

/// One config file found under a mod folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModConfigFile {
    /// File name only.
    pub name: String,
    /// Path relative to the mod folder, `/`-separated.
    pub relative_path: String,
    /// UTF-8 body when the file is small and textual. `None` when it is
    /// larger than [`MAX_CONFIG_PREVIEW_BYTES`], not valid UTF-8, or
    /// contains a NUL — the file is still listed.
    pub text: Option<String>,
}

/// Whether `path` (absolute or relative) has a config extension.
#[must_use]
pub fn is_config_file(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| {
            CONFIG_EXTENSIONS
                .iter()
                .any(|want| ext.eq_ignore_ascii_case(want))
        })
}

/// Config files directly under `root`, sorted by relative path.
///
/// `root` must be the mod's own directory. A missing or unreadable folder
/// is an error; a folder with no config files is an empty list.
pub fn list_mod_config_files(root: &Path) -> io::Result<Vec<ModConfigFile>> {
    let listed = list_files_relative(root)?;
    let root_canon = root.canonicalize().ok();
    let mut files = Vec::new();
    for relative in listed {
        if !is_plain_relative(&relative) || !is_config_file(&relative) {
            continue;
        }
        let text = root_canon
            .as_deref()
            .and_then(|canon| preview_text(canon, &root.join(&relative)));
        files.push(ModConfigFile {
            name: file_label(&relative),
            relative_path: display_relative(&relative),
            text,
        });
    }
    files.sort_by(|a, b| {
        a.relative_path
            .to_ascii_lowercase()
            .cmp(&b.relative_path.to_ascii_lowercase())
    });
    Ok(files)
}

fn is_plain_relative(path: &Path) -> bool {
    path.components().all(|c| matches!(c, Component::Normal(_)))
}

fn file_label(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn display_relative(path: &Path) -> String {
    path.components()
        .filter_map(|component| match component {
            Component::Normal(part) => Some(part.to_string_lossy()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/")
}

/// Read a small UTF-8 file that canonicalizes inside `root_canon`.
/// Anything else — too big, binary, or a link that leaves the mod folder —
/// yields `None` instead of failing the whole list.
fn preview_text(root_canon: &Path, file: &Path) -> Option<String> {
    let file_canon = file.canonicalize().ok()?;
    if !file_canon.starts_with(root_canon) {
        return None;
    }
    let meta = fs::metadata(&file_canon).ok()?;
    if !meta.is_file() || meta.len() > MAX_CONFIG_PREVIEW_BYTES {
        return None;
    }
    let bytes = fs::read(&file_canon).ok()?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > MAX_CONFIG_PREVIEW_BYTES
        || bytes.contains(&0)
    {
        return None;
    }
    String::from_utf8(bytes).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn config_extensions_are_recognized_case_insensitively() {
        assert!(is_config_file(Path::new("a.INI")));
        assert!(is_config_file(Path::new("SKSE/Plugins/a.toml")));
        assert!(is_config_file(Path::new("a.cfg")));
        assert!(is_config_file(Path::new("MCM/config/a.json")));
        assert!(is_config_file(Path::new("a.yaml")));
        assert!(is_config_file(Path::new("a.yml")));
        assert!(!is_config_file(Path::new("texture.dds")));
        assert!(!is_config_file(Path::new("a.esp")));
        assert!(!is_config_file(Path::new("a.dll")));
        assert!(!is_config_file(Path::new("a.ini.bak")));
        assert!(!is_config_file(Path::new("a.txt")));
        assert!(!is_config_file(Path::new("a.xml")));
    }

    #[test]
    fn lists_only_config_files_from_the_mod_tree() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("SkyUI.ini"), b"[Settings]\nbEnable=1\n").unwrap();
        fs::create_dir_all(dir.path().join("MCM/config")).unwrap();
        fs::write(
            dir.path().join("MCM/config/skyui.json"),
            b"{\"hotkey\":1}\n",
        )
        .unwrap();
        fs::create_dir_all(dir.path().join("SKSE/Plugins")).unwrap();
        fs::write(dir.path().join("SKSE/Plugins/SkyUI.ini"), b"iHotkey=26\n").unwrap();
        fs::write(dir.path().join("SKSE/Plugins/SkyUI.dll"), b"MZ").unwrap();
        fs::create_dir_all(dir.path().join("textures")).unwrap();
        fs::write(dir.path().join("textures/texture.dds"), b"DDS ").unwrap();
        fs::write(dir.path().join("SkyUI_SE.esp"), b"TES4").unwrap();
        fs::write(dir.path().join("meta.ini"), b"[General]\ninstalled=true").unwrap();

        let files = list_mod_config_files(dir.path()).unwrap();

        let paths: Vec<&str> = files
            .iter()
            .map(|file| file.relative_path.as_str())
            .collect();
        assert_eq!(
            paths,
            vec![
                "MCM/config/skyui.json",
                "SKSE/Plugins/SkyUI.ini",
                "SkyUI.ini",
            ]
        );
        assert_eq!(files[0].name, "skyui.json");
        assert_eq!(files[0].text.as_deref(), Some("{\"hotkey\":1}\n"));
        assert_eq!(files[1].name, "SkyUI.ini");
        assert_eq!(files[1].text.as_deref(), Some("iHotkey=26\n"));
        assert_eq!(files[2].text.as_deref(), Some("[Settings]\nbEnable=1\n"));
    }

    #[test]
    fn a_mod_without_config_files_returns_an_empty_list() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join("textures")).unwrap();
        fs::write(dir.path().join("textures/texture.dds"), b"DDS ").unwrap();
        fs::write(dir.path().join("readme.txt"), b"hello").unwrap();

        assert!(list_mod_config_files(dir.path()).unwrap().is_empty());
    }

    #[test]
    fn preview_stops_at_64_kib_and_skips_binary() {
        let dir = tempfile::tempdir().unwrap();
        let limit = usize::try_from(MAX_CONFIG_PREVIEW_BYTES).unwrap();
        fs::write(dir.path().join("exact.ini"), vec![b'a'; limit]).unwrap();
        fs::write(dir.path().join("over.ini"), vec![b'a'; limit + 1]).unwrap();
        fs::write(dir.path().join("bin.ini"), b"a\0b").unwrap();

        let files = list_mod_config_files(dir.path()).unwrap();
        let by_name = |name: &str| files.iter().find(|file| file.name == name).unwrap();

        assert_eq!(
            by_name("exact.ini").text.as_ref().map(String::len),
            Some(limit)
        );
        assert!(by_name("over.ini").text.is_none());
        assert!(by_name("bin.ini").text.is_none());
        assert_eq!(files.len(), 3);
    }
}
