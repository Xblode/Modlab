//! Direct-to-disk handling for Skyrim's own engine mod (SKSE).
//!
//! Every other mod is deployed only into the virtual session USVFS builds at
//! launch (`usvfs::deploy_enabled_mods`) — nothing on the real install ever
//! changes. SKSE is the one deliberate exception: `skse64_loader.exe` and its
//! DLL must sit for real, directly in the game's own install root, exactly
//! like a manual "extract SKSE into the game folder" install would put them
//! — that's what `skse64_loader.exe` finds *beside itself* when it starts,
//! before it ever gets a chance to see anything USVFS would have virtually
//! linked in. Toggling it on/off in the Mods table physically copies or
//! removes those files, so "back to a vanilla install" is instant and never
//! needs a relaunch.
//!
//! This still fits "everything reversible" (`CLAUDE.md`): nothing here ever
//! deletes irreplaceable data. Disabling only removes files this module
//! itself copied moments (or days) ago; the mod's own extracted copy under
//! ModLab's `mods/` folder (`source`, below) is untouched and re-enabling
//! recreates byte-identical files from it.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Name-match heuristic — the same stopgap the shell's "Engine & Indicators"
/// section uses on the frontend, not yet the real "known mods" registry
/// (tracked separately, see ClickUp). Duplicated once per language on
/// purpose rather than shared: there is no code boundary between Rust and
/// TypeScript to share it through, and it is meant to be replaced whole once
/// that registry lands, not threaded through more call sites than it has to be.
pub fn is_engine_mod(name: &str) -> bool {
    name.to_lowercase().contains("skse")
}

/// Every file under `source`, as a path relative to `source` — what
/// `install_direct` copies, and what `uninstall_direct` removes again.
/// Recomputed both times rather than persisted anywhere: `source` is the
/// mod's own extracted content and does not change between an enable and
/// the next disable.
fn relative_files(source: &Path) -> io::Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    walk(source, source, &mut out)?;
    return Ok(out);

    fn walk(base: &Path, dir: &Path, out: &mut Vec<PathBuf>) -> io::Result<()> {
        for entry in fs::read_dir(dir)? {
            let path = entry?.path();
            if path.is_dir() {
                walk(base, &path, out)?;
            } else if let Ok(rel) = path.strip_prefix(base) {
                out.push(rel.to_path_buf());
            }
        }
        Ok(())
    }
}

/// Copy every file `source` provides directly into `install_root` — a real
/// write, the one exception to "USVFS only" this module exists for. Existing
/// files at the destination are overwritten (re-enabling after an update to
/// the same mod folder should pick up the new bytes, not silently keep the
/// old ones).
pub fn install_direct(source: &Path, install_root: &Path) -> io::Result<()> {
    for rel in relative_files(source)? {
        let to = install_root.join(&rel);
        if let Some(parent) = to.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::copy(source.join(&rel), &to)?;
    }
    Ok(())
}

/// Remove exactly the files `install_direct` would copy for this same
/// `source` — restores `install_root` to vanilla for this mod's part of it.
/// Best-effort per file: a file already missing (never installed, or removed
/// by hand) is not an error, since the end state — that file absent from
/// `install_root` — is exactly what was asked for either way.
pub fn uninstall_direct(source: &Path, install_root: &Path) -> io::Result<()> {
    for rel in relative_files(source)? {
        let _ = fs::remove_file(install_root.join(&rel));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_engine_mod_matches_skse_case_insensitively() {
        assert!(is_engine_mod("SKSE64"));
        assert!(is_engine_mod("Address Library for SKSE Plugins"));
        assert!(!is_engine_mod("SkyUI"));
    }

    #[test]
    fn install_then_uninstall_round_trips_to_an_empty_install_root() {
        let source = tempfile::tempdir().unwrap();
        fs::write(source.path().join("skse64_loader.exe"), b"fake loader").unwrap();
        fs::create_dir_all(source.path().join("Data/SKSE/Plugins")).unwrap();
        fs::write(
            source.path().join("Data/SKSE/Plugins/skse64_steam_loader.dll"),
            b"fake dll",
        )
        .unwrap();

        let install_root = tempfile::tempdir().unwrap();
        install_direct(source.path(), install_root.path()).unwrap();

        assert!(install_root.path().join("skse64_loader.exe").is_file());
        assert!(install_root
            .path()
            .join("Data/SKSE/Plugins/skse64_steam_loader.dll")
            .is_file());
        assert_eq!(
            fs::read(install_root.path().join("skse64_loader.exe")).unwrap(),
            b"fake loader"
        );

        uninstall_direct(source.path(), install_root.path()).unwrap();

        assert!(!install_root.path().join("skse64_loader.exe").exists());
        assert!(!install_root
            .path()
            .join("Data/SKSE/Plugins/skse64_steam_loader.dll")
            .exists());
    }

    #[test]
    fn uninstall_on_files_never_installed_is_not_an_error() {
        let source = tempfile::tempdir().unwrap();
        fs::write(source.path().join("skse64_loader.exe"), b"fake loader").unwrap();
        let install_root = tempfile::tempdir().unwrap();

        uninstall_direct(source.path(), install_root.path()).unwrap();
    }

    #[test]
    fn reinstalling_overwrites_a_changed_file() {
        let source = tempfile::tempdir().unwrap();
        fs::write(source.path().join("skse64_loader.exe"), b"v1").unwrap();
        let install_root = tempfile::tempdir().unwrap();

        install_direct(source.path(), install_root.path()).unwrap();
        fs::write(source.path().join("skse64_loader.exe"), b"v2").unwrap();
        install_direct(source.path(), install_root.path()).unwrap();

        assert_eq!(
            fs::read(install_root.path().join("skse64_loader.exe")).unwrap(),
            b"v2"
        );
    }
}
