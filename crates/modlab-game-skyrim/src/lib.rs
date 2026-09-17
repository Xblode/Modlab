//! The Skyrim Game Adapter. Everything here is allowed to know what an
//! `.esp` is, what SKSE is, what MO2's on-disk format looks like — none of
//! that belongs in `modlab-core`. See docs/01-architecture-decidee.md §3.1.

pub mod installer;
pub mod mo2;
#[cfg(windows)]
pub mod usvfs;

use modlab_core::game_adapter::{GameAdapter, GameInstall};
use std::path::{Path, PathBuf};

pub struct SkyrimAdapter;

impl GameAdapter for SkyrimAdapter {
    fn id(&self) -> &'static str {
        "skyrimse"
    }

    fn capabilities(&self) -> modlab_core::game_adapter::GameCapabilities {
        use modlab_core::game_adapter::{GameCapabilities, GameSection::*, GameTool};
        GameCapabilities {
            short_name: "Skyrim SE",
            sections: vec![Dashboard, Mods, Plugins, Archives, Directory, Saves, Health],
            deployment_label: "USVFS",
            script_extender: Some("SKSE"),
            load_order_tool: Some(GameTool { id: "loot", name: "LOOT" }),
        }
    }

    fn detect(&self) -> Option<GameInstall> {
        find_special_edition().map(|root| GameInstall {
            id: "skyrimse".to_string(),
            name: "Skyrim Special Edition".to_string(),
            root,
        })
    }
}

/// Scan likely Steam library locations across all drive letters for a
/// Skyrim Special Edition install. Good enough for a walking skeleton; a
/// real implementation would parse Steam's `libraryfolders.vdf` instead of
/// guessing common paths — noted as a real limitation, not fixed here
/// because it doesn't block anything Phase 1 actually needs yet.
fn find_special_edition() -> Option<PathBuf> {
    let relative_paths = [
        "SteamLibrary\\steamapps\\common\\Skyrim Special Edition",
        "Program Files (x86)\\Steam\\steamapps\\common\\Skyrim Special Edition",
    ];

    for drive in b'C'..=b'Z' {
        let drive = drive as char;
        for rel in &relative_paths {
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

/// Locate `usvfs_x64.dll` — env override first, then common MO2 install
/// paths on this machine (including Benjamin's known folder, whose
/// "Organazer" spelling is not a typo in the search).
pub fn find_usvfs_dll() -> Option<PathBuf> {
    if let Some(from_env) = std::env::var_os("MODLAB_USVFS_DLL") {
        let path = PathBuf::from(from_env);
        if path.is_file() {
            return Some(path);
        }
    }

    let mut candidates = Vec::new();
    candidates.push(PathBuf::from(
        r"F:\_Modding\Skyrim Modding\Mod Organazer\usvfs_x64.dll",
    ));
    candidates.push(PathBuf::from(
        r"F:\_Modding\Skyrim Modding\Mod Organizer\usvfs_x64.dll",
    ));

    if let Some(local) = std::env::var_os("LOCALAPPDATA") {
        candidates.push(PathBuf::from(local).join(r"ModOrganizer\usvfs_x64.dll"));
    }

    for drive in b'C'..=b'Z' {
        let drive = drive as char;
        candidates.push(PathBuf::from(format!(
            r"{drive}:\Modding\MO2\usvfs_x64.dll"
        )));
        candidates.push(PathBuf::from(format!(
            r"{drive}:\Program Files\Mod Organizer 2\usvfs_x64.dll"
        )));
    }

    candidates.into_iter().find(|p| p.is_file())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adapter_id_is_stable() {
        assert_eq!(SkyrimAdapter.id(), "skyrimse");
    }

    #[test]
    fn capabilities_declare_expected_vocabulary() {
        use modlab_core::game_adapter::GameSection;
        let caps = SkyrimAdapter.capabilities();
        assert_eq!(caps.short_name, "Skyrim SE");
        assert_eq!(caps.deployment_label, "USVFS");
        assert_eq!(caps.script_extender, Some("SKSE"));
        assert!(caps.sections.contains(&GameSection::Plugins));
        assert!(caps.sections.contains(&GameSection::Saves));
        let loot = caps.load_order_tool.expect("skyrim declares a load order tool");
        assert_eq!(loot.id, "loot");
    }

    #[test]
    fn finds_a_real_install_if_one_exists_on_this_machine() {
        // Not hermetic — checks the actual machine, same as `modlab detect`
        // will when a human runs it. A fixture-backed version belongs in
        // modlab-core's fixture tests once installation/deployment (which
        // actually need a fake game tree) land; detection alone doesn't
        // justify one yet.
        if let Some(install) = SkyrimAdapter.detect() {
            assert_eq!(install.id, "skyrimse");
            assert!(install.root.join("SkyrimSE.exe").is_file());
        }
    }
}
