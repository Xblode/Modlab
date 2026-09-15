//! Archive extraction — generic, any game, any provider. `.7z` via
//! `sevenz-rust` (spiked in Phase 0: extracts real Nexus archives cleanly,
//! no dependency on `7z.dll`). `.zip` via the `zip` crate. RAR isn't
//! supported — not spiked, and no real archive needing it has come up yet;
//! add it if that changes rather than guessing a library now.

use std::fs;
use std::path::Path;

use crate::{CoreError, Result};

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
    use std::path::Path as StdPath;

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
    fn extracts_a_real_skse_archive_if_present() {
        // Same spirit as the other real-machine tests this session:
        // skipped gracefully if this machine doesn't have it. Deliberately
        // doesn't assert *where* files end up — this crate is game-agnostic
        // and shouldn't know SKSE's archive layout; modlab-game-skyrim's
        // installer tests cover that. Just: extraction produces real files.
        let real_archive = StdPath::new(
            "F:/_Modding/Skyrim Modding/_Download/1.6.11/Skyrim Script Extender (SKSE64) Steam 2.2.8.7z",
        );
        if !real_archive.is_file() {
            return;
        }
        let dest_dir = tempfile::tempdir().unwrap();
        extract(real_archive, dest_dir.path()).unwrap();
        assert!(
            walkdir_has_any_file(dest_dir.path()),
            "expected at least one extracted file somewhere under {}",
            dest_dir.path().display()
        );
    }

    fn walkdir_has_any_file(dir: &StdPath) -> bool {
        fs::read_dir(dir).unwrap().any(|entry| {
            let entry = entry.unwrap();
            let path = entry.path();
            path.is_file() || (path.is_dir() && walkdir_has_any_file(&path))
        })
    }
}
