//! TEMPORARY Skyrim detection.
//!
//! This is explicitly NOT the `GameAdapter` architecture from
//! docs/01-architecture-decidee.md §3.1 — the Core is not supposed to know
//! what "Skyrim" is. It exists only so the Phase 0 walking-skeleton milestone
//! ("la CLI détecte Skyrim") has something real behind it. `skyrim-dev`
//! should replace this with a proper adapter and delete this module.

use std::path::{Path, PathBuf};

/// Scan likely Steam library locations across all drive letters for a
/// Skyrim Special Edition install. Good enough for a walking skeleton;
/// a real implementation would parse Steam's `libraryfolders.vdf` instead
/// of guessing common paths.
pub fn find_skyrim_special_edition() -> Option<PathBuf> {
    let candidates = ["SteamLibrary\\steamapps\\common\\Skyrim Special Edition"];
    let program_files_candidates =
        ["Program Files (x86)\\Steam\\steamapps\\common\\Skyrim Special Edition"];

    for drive in b'C'..=b'Z' {
        let drive = drive as char;
        for rel in candidates.iter().chain(program_files_candidates.iter()) {
            let path = PathBuf::from(format!("{drive}:\\{rel}"));
            if looks_like_skyrim_install(&path) {
                return Some(path);
            }
        }
    }
    None
}

fn looks_like_skyrim_install(path: &Path) -> bool {
    path.join("SkyrimSE.exe").is_file() && path.join("Data").is_dir()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_a_real_install_if_one_exists_on_this_machine() {
        // Not a hermetic test — deliberately checks the actual machine, the
        // way `modlab detect` will when a human runs it. Skipped implicitly
        // (assertion just documents intent) if no Skyrim SE happens to be
        // installed; CI/fixture-based detection tests come with the real
        // GameAdapter later.
        if let Some(path) = find_skyrim_special_edition() {
            assert!(path.join("SkyrimSE.exe").is_file());
        }
    }
}
