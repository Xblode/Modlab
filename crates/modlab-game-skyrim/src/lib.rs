//! The Skyrim Game Adapter. Everything here is allowed to know what an
//! `.esp` is, what SKSE is, what MO2's on-disk format looks like — none of
//! that belongs in `modlab-core`. See docs/01-architecture-decidee.md §3.1.

pub mod archive_meta;
pub mod categories;
mod dependencies;
pub mod engine;
pub mod fomod;
pub mod graphics;
pub mod installer;
pub mod mo2;
pub mod plugins;
pub mod skse;
pub mod tool_archive;
#[cfg(windows)]
pub mod usvfs;
pub mod vanilla;

use modlab_core::game_adapter::{
    ArchiveSource, CategoryHint, ClassifiedTool, DependencyMod, GameAdapter, GameCategory,
    GameInstall, LocationQuery, ModDependencyReport, PrepareInstall, PreparedMod, SlotPath,
    StagedMod, ToolClassifyQuery,
};
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
                detected: root.as_ref().map(|r| r.join("Data")).filter(|p| p.is_dir()),
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

    fn mods_location(&self, query: &LocationQuery<'_>) -> modlab_core::game_adapter::ModsLocation {
        modlab_core::game_adapter::ModsLocation::Managed(query.data_root.join("mods"))
    }

    fn config_location(
        &self,
        _query: &LocationQuery<'_>,
    ) -> modlab_core::game_adapter::ConfigLocation {
        let detected = my_games_dir().or_else(|| {
            find_prefs_ini().and_then(|path| path.parent().map(|dir| dir.to_path_buf()))
        });
        modlab_core::game_adapter::ConfigLocation::PrefsParent { detected }
    }

    fn known_config_files(&self) -> &'static [&'static str] {
        &CONFIG_INI_NAMES
    }

    fn prefs_file_slot(&self) -> Option<&'static str> {
        Some("prefs_ini")
    }

    fn prefs_file_name(&self) -> Option<&'static str> {
        Some("SkyrimPrefs.ini")
    }

    fn classify_tool(&self, query: &ToolClassifyQuery<'_>) -> Option<ClassifiedTool> {
        // The SKSE rules stay in `tool_archive`. This only forwards them.
        let mark = tool_archive::classify_mod(&tool_archive::ModToolQuery {
            name: query.name,
            source_path: query.source_path,
            archive_path: query.archive_path,
            stored_version: query.stored_version,
            source_url: query.source_url,
            download_file_name: query.download_file_name,
        })?;
        Some(ClassifiedTool {
            tool: mark.tool.to_string(),
            version: mark.version,
        })
    }

    fn categories(&self) -> &'static [GameCategory] {
        categories::CATEGORIES
    }

    fn resolve_category(&self, hint: &CategoryHint<'_>) -> Option<&'static str> {
        categories::resolve(hint)
    }

    fn read_archive_metadata(
        &self,
        path: &Path,
    ) -> Option<modlab_core::game_adapter::ArchiveMetadata> {
        archive_meta::read_path(path)
    }

    fn remote_page(&self, host: &str, remote_id: &str) -> Option<String> {
        if !host.eq_ignore_ascii_case("nexus") {
            return None;
        }
        let id = remote_id.trim();
        if id.is_empty() || id.starts_with('0') || !id.bytes().all(|byte| byte.is_ascii_digit()) {
            return None;
        }
        Some(format!(
            "https://www.nexusmods.com/skyrimspecialedition/mods/{id}"
        ))
    }

    fn resolve_dependencies(
        &self,
        mods: &[DependencyMod],
        paths: &[SlotPath],
    ) -> Vec<ModDependencyReport> {
        let path = |id: &str| {
            paths
                .iter()
                .find(|slot| slot.id == id)
                .map(|slot| slot.path.as_path())
        };
        dependencies::resolve(mods, path("root"), path("data_dir"))
    }

    fn stage_archive(
        &self,
        data_root: &Path,
        mod_id: i64,
        source: &ArchiveSource<'_>,
    ) -> Result<StagedMod, String> {
        let pending = modlab_core::download::ArchivedMod {
            name: source.name.to_string(),
            archive_path: source.archive_path.to_path_buf(),
        };
        let staged = installer::stage_archived(data_root, mod_id, &pending)?;
        Ok(StagedMod {
            name: staged.name,
            shape: shape_id(&staged.shape).to_string(),
            needs_wizard: staged.shape == installer::ArchiveShape::Fomod,
            staged_path: staged.staged_path,
            content_root: staged.content_root,
            refusal: None,
        })
    }

    fn prepare_install(&self, job: &PrepareInstall<'_>) -> Result<PreparedMod, String> {
        let pending = modlab_core::download::ArchivedMod {
            name: job.name.to_string(),
            archive_path: job.archive_path.to_path_buf(),
        };
        let plan = installer::archived_install_plan(&pending, job.content_root);
        let swap = installer::prepare_archived_tree(
            job.data_root,
            job.content_root,
            job.dest,
            plan.keep_game_root,
        )?;
        Ok(PreparedMod::Directory {
            dest: job.dest.to_path_buf(),
            swap,
            adjustments: plan.adjustments,
        })
    }

    fn prepare_guided(&self, job: &PrepareInstall<'_>) -> Result<PreparedMod, String> {
        let file_state = |file: &str| match (job.branch_file)(file) {
            modlab_core::game_adapter::BranchFileState::Active => fomod::FileState::Active,
            modlab_core::game_adapter::BranchFileState::Inactive => fomod::FileState::Inactive,
            modlab_core::game_adapter::BranchFileState::Missing => fomod::FileState::Missing,
        };
        let env = fomod::FomodEnv {
            file_state: &file_state,
            game_version: job.game_version,
        };
        let swap = installer::prepare_fomod_tree(
            job.data_root,
            job.content_root,
            job.picked,
            job.dest,
            &env,
        )?;
        Ok(PreparedMod::Directory {
            dest: job.dest.to_path_buf(),
            swap,
            adjustments: modlab_core::install::InstallAdjustments::default(),
        })
    }
}

fn shape_id(shape: &installer::ArchiveShape) -> &'static str {
    match shape {
        installer::ArchiveShape::Fomod => "fomod",
        installer::ArchiveShape::BareData => "bare-data",
        installer::ArchiveShape::WrappedData { .. } => "wrapped-data",
        installer::ArchiveShape::EngineRoot => "engine-root",
        installer::ArchiveShape::Unknown => "unknown",
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

/// Bethesda's three INI files in `My Games\Skyrim Special Edition`.
/// `SkyrimPrefs.ini` is the file the `prefs_ini` path slot points at.
pub const CONFIG_INI_NAMES: [&str; 3] = ["Skyrim.ini", "SkyrimPrefs.ini", "SkyrimCustom.ini"];

/// `Documents\My Games\Skyrim Special Edition` under a profile directory.
/// `USERPROFILE`-based rather than a real "known folder" API call: good
/// enough for a walking skeleton (same standard as `find_special_edition`'s
/// Steam-library scan). A Documents folder relocated by hand is simply
/// "introuvable" until a human overrides `prefs_ini`.
pub fn my_games_dir_from(profile: &Path) -> PathBuf {
    profile.join(r"Documents\My Games\Skyrim Special Edition")
}

/// The My Games folder when it exists on this machine. The INI files and
/// the Saves directory live inside it; either may be absent on its own.
pub fn my_games_dir() -> Option<PathBuf> {
    let profile = std::env::var_os("USERPROFILE")?;
    let path = my_games_dir_from(Path::new(&profile));
    path.is_dir().then_some(path)
}

/// `SkyrimPrefs.ini` — Bethesda writes it to the user's Documents folder,
/// never next to the game's own `.exe`, so this is a separate lookup from
/// `path_slots`' `root`.
pub fn find_prefs_ini() -> Option<PathBuf> {
    let path = my_games_dir()?.join("SkyrimPrefs.ini");
    path.is_file().then_some(path)
}

/// The `Saves` folder next to `SkyrimPrefs.ini`.
pub fn find_saves_dir() -> Option<PathBuf> {
    let path = my_games_dir()?.join("Saves");
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
    fn prefs_and_saves_live_under_my_games() {
        let games = my_games_dir_from(Path::new(r"C:\Users\dev"));
        assert_eq!(
            games.join("SkyrimPrefs.ini"),
            PathBuf::from(
                r"C:\Users\dev\Documents\My Games\Skyrim Special Edition\SkyrimPrefs.ini"
            )
        );
        assert_eq!(
            games.join("Saves"),
            PathBuf::from(r"C:\Users\dev\Documents\My Games\Skyrim Special Edition\Saves")
        );
        assert_eq!(
            CONFIG_INI_NAMES,
            ["Skyrim.ini", "SkyrimPrefs.ini", "SkyrimCustom.ini"]
        );
    }

    #[test]
    fn path_slots_declare_the_expected_ids() {
        let slots = SkyrimAdapter.path_slots();
        let ids: Vec<&str> = slots.iter().map(|s| s.id).collect();
        assert_eq!(
            ids,
            [
                "root",
                "exe",
                "data_dir",
                "usvfs_dll",
                "prefs_ini",
                "saves_dir"
            ]
        );
        // A file slot derived from a dir slot must never be reported detected
        // unless it actually exists on disk — `filter(.is_file())` above is
        // what this pins down.
        for slot in &slots {
            if let Some(p) = &slot.detected {
                assert!(
                    p.exists(),
                    "{} claims a path that doesn't exist: {p:?}",
                    slot.id
                );
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

    #[test]
    fn classify_tool_delegates_to_the_skse_rules() {
        let name = "skse64_2_02_06.7z";
        let mark = SkyrimAdapter
            .classify_tool(&ToolClassifyQuery {
                name,
                source_path: Path::new(name),
                archive_path: Some(Path::new(name)),
                stored_version: None,
                source_url: "",
                download_file_name: name,
            })
            .expect("skse archive");
        assert_eq!(mark.tool, "skse");
        assert_eq!(mark.version.as_deref(), Some("2.2.6"));
    }

    #[test]
    fn a_data_shaped_archive_still_installs_into_the_managed_mods_folder() {
        use modlab_core::download::{attach_to_loadout, capture};
        use modlab_core::game_adapter::{PrepareInstall, PreparedMod};
        use modlab_core::{loadout, Core};
        use std::fs;

        let data = tempfile::tempdir().unwrap();
        let core = Core::open_in_memory().unwrap();
        let created = loadout::create(&core, "skyrimse", "Test").unwrap();
        let archive = data.path().join("mod.zip");
        fs::write(&archive, b"zip").unwrap();
        let download = capture(
            &core,
            "https://example.test/mod.zip",
            "mod.zip",
            &archive,
            None,
        )
        .unwrap()
        .id();
        let mod_id = attach_to_loadout(&core, download, created.id, None).unwrap();
        let extracted = data.path().join("extracted");
        fs::create_dir_all(extracted.join("meshes")).unwrap();
        fs::write(extracted.join("meshes").join("a.nif"), b"mesh").unwrap();
        let dest = data.path().join("mods").join(mod_id.to_string());
        let prepared = SkyrimAdapter
            .prepare_install(&PrepareInstall {
                data_root: data.path(),
                mod_id,
                name: "Mesh mod",
                archive_path: &archive,
                content_root: &extracted,
                dest: &dest,
                instance_root: None,
                mods_dir: None,
                resource_packs: None,
                shader_packs: None,
                picked: &[],
                game_version: None,
                branch_file: &modlab_core::game_adapter::BranchFileState::missing,
            })
            .unwrap();
        let PreparedMod::Directory {
            dest,
            swap,
            adjustments,
        } = prepared
        else {
            panic!("skyrim still installs a directory tree");
        };
        modlab_core::install::commit_prepared_install(
            &core,
            created.id,
            mod_id,
            &dest,
            adjustments,
            swap,
        )
        .unwrap();
        assert_eq!(
            fs::read(dest.join("meshes").join("a.nif")).unwrap(),
            b"mesh"
        );
        assert!(!data.path().join("instances").exists());
        assert_eq!(
            loadout::entries(&core, created.id).unwrap()[0].status,
            loadout::STATUS_INSTALLED
        );
    }
}
