//! The Minecraft Game Adapter. Everything here is allowed to know what a
//! `.jar` is, what Fabric/Forge/NeoForge/Quilt are, where Prism or the
//! vanilla launcher put instances — none of that belongs in `modlab-core`.
//! See `docs/01-architecture-decidee.md` §3.1.
//!
//! Phase 2 first slice: detection only. Install, CurseForge/Modrinth
//! Providers, and launch come once the UI can actually show a second game
//! next to Skyrim — that is the point of this crate existing now.

use modlab_core::game_adapter::{GameAdapter, GameInstall};
use std::path::{Path, PathBuf};

pub struct MinecraftAdapter;

impl GameAdapter for MinecraftAdapter {
    fn id(&self) -> &'static str {
        "minecraft"
    }

    fn detect(&self) -> Option<GameInstall> {
        find_install().map(|root| GameInstall {
            id: "minecraft".to_string(),
            name: "Minecraft".to_string(),
            root,
        })
    }
}

/// Vanilla `.minecraft`, then common Prism / Modrinth / CurseForge app
/// data folders. Same "guess likely paths" approach as the Skyrim adapter
/// — parsing launcher JSON comes later.
fn find_install() -> Option<PathBuf> {
    let mut candidates = Vec::new();

    if let Some(appdata) = std::env::var_os("APPDATA") {
        let appdata = PathBuf::from(appdata);
        candidates.push(appdata.join(".minecraft"));
        candidates.push(appdata.join("PrismLauncher"));
        candidates.push(appdata.join("ModrinthApp"));
        candidates.push(appdata.join("CurseForge"));
    }

    for drive in b'C'..=b'Z' {
        let drive = drive as char;
        candidates.push(PathBuf::from(format!(
            "{drive}:\\Program Files\\Prism Launcher"
        )));
        candidates.push(PathBuf::from(format!("{drive}:\\Games\\Minecraft")));
    }

    candidates.into_iter().find(|p| looks_like_minecraft(p))
}

fn looks_like_minecraft(path: &Path) -> bool {
    if !path.is_dir() {
        return false;
    }
    // Vanilla launcher data.
    if path.join("launcher_profiles.json").is_file() || path.join("versions").is_dir() {
        return true;
    }
    // Prism / MultiMC-style instance root.
    if path.join("prismlauncher.cfg").is_file() || path.join("instances").is_dir() {
        return true;
    }
    // CurseForge app data often has a `minecraft` child.
    path.join("minecraft").is_dir()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adapter_id_is_stable() {
        assert_eq!(MinecraftAdapter.id(), "minecraft");
    }

    #[test]
    fn finds_a_real_install_if_one_exists_on_this_machine() {
        if let Some(install) = MinecraftAdapter.detect() {
            assert_eq!(install.id, "minecraft");
            assert!(install.root.is_dir());
        }
    }
}
