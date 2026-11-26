//! Archive extraction — generic, any game, any provider. `.7z` via
//! `sevenz-rust` (spiked in Phase 0: extracts real Nexus archives cleanly,
//! no dependency on `7z.dll`). `.zip` via the `zip` crate. RAR isn't
//! supported — not spiked, and no real archive needing it has come up yet;
//! add it if that changes rather than guessing a library now.

use std::fs;
use std::io::Read;
use std::path::Path;

use crate::{CoreError, Result};

/// A 7z larger than this is not opened for a metadata peek. Solid archives
/// cannot skip ahead, and Info must stay responsive.
const SEVENZ_PEEK_MAX: u64 = 64 * 1024 * 1024;
const PEEK_ENTRY_MAX: u64 = 256 * 1024;

/// One small text entry pulled out of a zip or 7z.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PeekedEntry {
    pub path: String,
    pub bytes: Vec<u8>,
}

/// Read entries whose path ends with one of `suffixes` (`fomod/info.xml`,
/// `meta.ini`). Missing files, RAR and oversized 7z yield an empty list —
/// the caller still has the installed folder.
#[must_use]
pub fn peek_named_entries(archive_path: &Path, suffixes: &[&str]) -> Vec<PeekedEntry> {
    if !archive_path.is_file() || suffixes.is_empty() {
        return Vec::new();
    }
    let ext = archive_path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    match ext.as_str() {
        "zip" | "jar" => peek_zip(archive_path, suffixes),
        "7z" => peek_7z(archive_path, suffixes),
        _ => Vec::new(),
    }
}

fn peek_zip(archive_path: &Path, suffixes: &[&str]) -> Vec<PeekedEntry> {
    let Ok(file) = fs::File::open(archive_path) else {
        return Vec::new();
    };
    let Ok(mut zip) = zip::ZipArchive::new(file) else {
        return Vec::new();
    };
    let mut found = Vec::new();
    for index in 0..zip.len() {
        let Ok(mut entry) = zip.by_index(index) else {
            continue;
        };
        if !entry.is_file() || entry.size() > PEEK_ENTRY_MAX {
            continue;
        }
        let name = entry.name().replace('\\', "/");
        if !suffix_matches(&name, suffixes) {
            continue;
        }
        let mut bytes = Vec::new();
        if entry.read_to_end(&mut bytes).is_ok() {
            found.push(PeekedEntry { path: name, bytes });
        }
    }
    found
}

fn peek_7z(archive_path: &Path, suffixes: &[&str]) -> Vec<PeekedEntry> {
    let Ok(meta) = fs::metadata(archive_path) else {
        return Vec::new();
    };
    if meta.len() > SEVENZ_PEEK_MAX {
        return Vec::new();
    }
    let Ok(mut reader) =
        sevenz_rust::SevenZReader::open(archive_path, sevenz_rust::Password::empty())
    else {
        return Vec::new();
    };
    let mut found = Vec::new();
    let _ = reader.for_each_entries(|entry, reader| {
        let name = entry.name().replace('\\', "/");
        let take = !entry.is_directory()
            && entry.size <= PEEK_ENTRY_MAX
            && suffix_matches(&name, suffixes);
        if take {
            let mut bytes = Vec::new();
            if reader.read_to_end(&mut bytes).is_ok() {
                found.push(PeekedEntry { path: name, bytes });
            }
        } else if !entry.is_directory() {
            drain(reader);
        }
        Ok(true)
    });
    found
}

fn drain(reader: &mut dyn Read) {
    let mut buf = [0u8; 16 * 1024];
    loop {
        match reader.read(&mut buf) {
            Ok(0) | Err(_) => break,
            Ok(_) => {}
        }
    }
}

/// How many entry names [`list_entry_names`] will return. Past this the
/// list is cut and `truncated` is true — a caller must not treat a cut
/// list as "the archive contains only these".
const ENTRY_NAME_CAP: usize = 4_000;

/// File names inside a folder, a zip, a jar, or a 7z header.
///
/// Nothing is extracted and file bytes are not read. A 7z contributes
/// the names stored in its header. A missing path, a RAR, or a file that
/// is not an archive yields an empty list. `truncated` is true when the
/// cap stopped the walk early.
#[must_use]
pub fn list_entry_names(path: &Path) -> (Vec<String>, bool) {
    if path.is_dir() {
        return list_dir_names(path, ENTRY_NAME_CAP);
    }
    if !path.is_file() {
        return (Vec::new(), false);
    }
    let ext = path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    match ext.as_str() {
        "zip" | "jar" => list_zip_names(path, ENTRY_NAME_CAP),
        "7z" => list_7z_names(path, ENTRY_NAME_CAP),
        _ => (Vec::new(), false),
    }
}

fn list_zip_names(path: &Path, cap: usize) -> (Vec<String>, bool) {
    let Ok(file) = fs::File::open(path) else {
        return (Vec::new(), false);
    };
    let Ok(mut zip) = zip::ZipArchive::new(file) else {
        return (Vec::new(), false);
    };
    let mut names = Vec::new();
    let mut truncated = false;
    for index in 0..zip.len() {
        if names.len() >= cap {
            truncated = true;
            break;
        }
        let Ok(entry) = zip.by_index(index) else {
            continue;
        };
        if entry.is_dir() {
            continue;
        }
        names.push(entry.name().replace('\\', "/"));
    }
    (names, truncated)
}

fn list_7z_names(path: &Path, cap: usize) -> (Vec<String>, bool) {
    let Ok(reader) = sevenz_rust::SevenZReader::open(path, sevenz_rust::Password::empty()) else {
        return (Vec::new(), false);
    };
    let mut names = Vec::new();
    let mut truncated = false;
    for entry in &reader.archive().files {
        if entry.is_directory() {
            continue;
        }
        if names.len() >= cap {
            truncated = true;
            break;
        }
        names.push(entry.name().replace('\\', "/"));
    }
    (names, truncated)
}

fn list_dir_names(root: &Path, cap: usize) -> (Vec<String>, bool) {
    let mut names = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    let mut truncated = false;
    while let Some(dir) = stack.pop() {
        let Ok(read) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in read.flatten() {
            if names.len() >= cap {
                truncated = true;
                break;
            }
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            if kind.is_symlink() {
                continue;
            }
            if kind.is_dir() {
                stack.push(entry.path());
                continue;
            }
            if !kind.is_file() {
                continue;
            }
            if let Ok(rel) = entry.path().strip_prefix(root) {
                names.push(rel.to_string_lossy().replace('\\', "/"));
            }
        }
        if truncated {
            break;
        }
    }
    (names, truncated)
}

fn suffix_matches(path: &str, suffixes: &[&str]) -> bool {
    let path = path.to_ascii_lowercase();
    suffixes.iter().any(|suffix| {
        let suffix = suffix.replace('\\', "/").to_ascii_lowercase();
        path == suffix || path.ends_with(&format!("/{suffix}"))
    })
}

/// Extract every file in `archive_path` into `dest_dir` (created if
/// missing). Dispatches on file extension; anything else is a clean error,
/// not a silent no-op.
pub fn extract(archive_path: &Path, dest_dir: &Path) -> Result<()> {
    let ext = archive_path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or_default()
        .to_lowercase();

    match ext.as_str() {
        "7z" => extract_7z(archive_path, dest_dir),
        "zip" => extract_zip(archive_path, dest_dir),
        other => Err(CoreError::Archive(format!(
            "unsupported archive format: .{other} (from {})",
            archive_path.display()
        ))),
    }
}

fn extract_7z(archive_path: &Path, dest_dir: &Path) -> Result<()> {
    sevenz_rust::decompress_file(archive_path, dest_dir)
        .map_err(|e| CoreError::Archive(format!("7z extraction failed: {e}")))
}

fn extract_zip(archive_path: &Path, dest_dir: &Path) -> Result<()> {
    let file = fs::File::open(archive_path)?;
    let mut zip = zip::ZipArchive::new(file)
        .map_err(|e| CoreError::Archive(format!("not a valid zip file: {e}")))?;
    zip.extract(dest_dir)
        .map_err(|e| CoreError::Archive(format!("zip extraction failed: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn extracts_a_synthetic_zip() {
        let src_dir = tempfile::tempdir().unwrap();
        let zip_path = src_dir.path().join("test.zip");
        {
            let file = fs::File::create(&zip_path).unwrap();
            let mut writer = zip::ZipWriter::new(file);
            let options = zip::write::SimpleFileOptions::default();
            writer.start_file("meshes/thing.nif", options).unwrap();
            writer.write_all(b"fake mesh data").unwrap();
            writer.start_file("textures/thing.dds", options).unwrap();
            writer.write_all(b"fake texture data").unwrap();
            writer.finish().unwrap();
        }

        let dest_dir = tempfile::tempdir().unwrap();
        extract(&zip_path, dest_dir.path()).unwrap();

        assert_eq!(
            fs::read(dest_dir.path().join("meshes/thing.nif")).unwrap(),
            b"fake mesh data"
        );
        assert_eq!(
            fs::read(dest_dir.path().join("textures/thing.dds")).unwrap(),
            b"fake texture data"
        );
    }

    #[test]
    fn unsupported_extension_is_a_clean_error_not_a_panic() {
        let dir = tempfile::tempdir().unwrap();
        let fake = dir.path().join("archive.rar");
        fs::write(&fake, b"not really a rar").unwrap();

        let result = extract(&fake, dir.path());

        assert!(matches!(result, Err(CoreError::Archive(_))));
    }

    #[test]
    fn list_entry_names_reads_a_zip_and_a_folder_without_extracting() {
        let src_dir = tempfile::tempdir().unwrap();
        let zip_path = src_dir.path().join("test.zip");
        {
            let file = fs::File::create(&zip_path).unwrap();
            let mut writer = zip::ZipWriter::new(file);
            let options = zip::write::SimpleFileOptions::default();
            writer.start_file("textures/thing.dds", options).unwrap();
            writer.write_all(b"dds").unwrap();
            writer.finish().unwrap();
        }
        let (names, truncated) = list_entry_names(&zip_path);
        assert!(!truncated);
        assert_eq!(names, vec!["textures/thing.dds".to_string()]);

        let folder = src_dir.path().join("mod");
        fs::create_dir_all(folder.join("SKSE/Plugins")).unwrap();
        fs::write(folder.join("SKSE/Plugins/version.bin"), b"x").unwrap();
        let (names, truncated) = list_entry_names(&folder);
        assert!(!truncated);
        assert_eq!(names, vec!["SKSE/Plugins/version.bin".to_string()]);
    }
}
