//! The Skyrim Game Adapter. Everything here is allowed to know what an
//! `.esp` is, what SKSE is, what MO2's on-disk format looks like — none of
//! that belongs in `modlab-core`. See docs/01-architecture-decidee.md §3.1.

pub mod engine;
pub mod fomod;
pub mod graphics;
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
            load_order_tool: Some(GameTool {
                id: "loot",
                name: "LOOT",
            }),
            import_source: Some("mo2"),
        }
    }

    fn detect(&self) -> Option<GameInstall> {
        find_special_edition().map(|root| GameInstall {
            id: "skyrimse".to_string(),
            name: "Skyrim Special Edition".to_string(),
            root,
        })
    }

    /// Skyrim is found from its own `.exe` (see `find_special_edition`), so
    /// the exe and `Data/` are just that root plus a fixed suffix; the USVFS
    /// DLL is a wholly separate lookup (MO2's own install, not Skyrim's).
    fn path_slots(&self) -> Vec<modlab_core::game_adapter::PathSlot> {
        use modlab_core::game_adapter::{PathKind, PathSlot};
        let root = find_special_edition();
        vec![
            PathSlot {
                id: "root",
                label: "Dossier d'installation",
                kind: PathKind::Dir,
                detected: root.clone(),
            },
            PathSlot {
                id: "exe",
                label: "Exécutable du jeu (SkyrimSE.exe)",
                kind: PathKind::File,
                detected: root
                    .as_ref()
                    .map(|r| r.join("SkyrimSE.exe"))
                    .filter(|p| p.is_file()),
            },
            PathSlot {
                id: "data_dir",
                label: "Dossier Data",
                kind: PathKind::Dir,
                detected: root
                    .as_ref()
                    .map(|r| r.join("Data"))
                    .filter(|p| p.is_dir()),
            },
            PathSlot {
                id: "usvfs_dll",
                label: "usvfs_x64.dll (Mod Organizer 2)",
                kind: PathKind::File,
                detected: find_usvfs_dll(),
            },
            PathSlot {
                id: "prefs_ini",
                label: "SkyrimPrefs.ini",
                kind: PathKind::File,
                detected: find_prefs_ini(),
            },
            PathSlot {
                id: "saves_dir",
                label: "Dossier des sauvegardes",
                kind: PathKind::Dir,
                detected: find_saves_dir(),
            },
        ]
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

/// `SkyrimPrefs.ini` — Bethesda writes it to the user's Documents folder,
/// never next to the game's own `.exe`, so this is a separate lookup from
/// `path_slots`' `root`. `USERPROFILE`-based rather than a real "known
/// folder" API call: good enough for a walking skeleton (same standard this
/// crate already applies to `find_special_edition`'s Steam-library scan),
/// wrong only for a Documents folder someone has relocated by hand — that
/// case just shows up as "introuvable" and a human overrides the path
/// instead, same recovery every other slot already offers.
pub fn find_prefs_ini() -> Option<PathBuf> {
    let profile = std::env::var_os("USERPROFILE")?;
    let path = PathBuf::from(profile)
        .join(r"Documents\My Games\Skyrim Special Edition\SkyrimPrefs.ini");
    path.is_file().then_some(path)
}

/// The `Saves` folder next to `SkyrimPrefs.ini` — same "good enough for a
/// walking skeleton" `USERPROFILE` lookup, same recovery (a human override)
/// if Documents was relocated.
pub fn find_saves_dir() -> Option<PathBuf> {
    let profile = std::env::var_os("USERPROFILE")?;
    let path =
        PathBuf::from(profile).join(r"Documents\My Games\Skyrim Special Edition\Saves");
    path.is_dir().then_some(path)
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
        let loot = caps
            .load_order_tool
            .expect("skyrim declares a load order tool");
        assert_eq!(loot.id, "loot");
        assert_eq!(
            caps.import_source,
            Some("mo2"),
            "Skyrim SE is migratable from Mod Organizer 2"
        );
    }

    #[test]
    fn path_slots_declare_the_expected_ids() {
        let slots = SkyrimAdapter.path_slots();
        let ids: Vec<&str> = slots.iter().map(|s| s.id).collect();
        assert_eq!(
            ids,
            ["root", "exe", "data_dir", "usvfs_dll", "prefs_ini", "saves_dir"]
        );
        // A file slot derived from a dir slot must never be reported detected
        // unless it actually exists on disk — `filter(.is_file())` above is
        // what this pins down.
        for slot in &slots {
            if let Some(p) = &slot.detected {
                assert!(p.exists(), "{} claims a path that doesn't exist: {p:?}", slot.id);
            }
        }
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
