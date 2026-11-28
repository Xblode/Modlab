//! The Minecraft Game Adapter. Everything here is allowed to know what a
//! `.jar` is, what Fabric/Forge/NeoForge/Quilt are, where Prism or the
//! vanilla launcher put instances — none of that belongs in `modlab-core`.
//! See `.bframe/ARCHITECTURE.md`.
//!
//! Detection lives here; reading and writing an instance's `mods/` folder
//! lives in `instance`. Whether a jar *is* Fabric Loader, Forge or NeoForge
//! — rather than a mod that only runs on one of them — lives in
//! `tool_archive`. What a normal jar declares (name, version, game
//! version, dependencies) is read locally in `archive_meta` — no
//! CurseForge or Modrinth request. Still deliberately absent, and tracked
//! as such: the CurseForge/Modrinth Providers (need an official API key).
//! Play = in-process JVM (`launch`: ensure + spawn + wait). `official_launcher`
//! (profile + open desktop launcher) is the secondary / rescue path.
//! CurseForge/Modrinth Providers remain absent.

pub mod archive_meta;
pub mod branch_loader;
pub mod categories;
mod dependencies;
pub mod filename;
pub mod install;
pub mod instance;
pub mod launch;
pub mod loader_install;
pub mod msa;
pub mod official_launcher;
pub mod tool_archive;

use modlab_core::game_adapter::{
    ArchiveMetadata, ArchiveSource, CategoryHint, ClassifiedTool, DependencyMod, GameAdapter,
    GameCategory, GameInstall, InstanceRootPolicy, LocationQuery, ModDependencyReport,
    PrepareInstall, PreparedMod, SlotPath, StagedMod, ToolClassifyQuery,
};
use std::path::{Path, PathBuf};

pub struct MinecraftAdapter;

impl GameAdapter for MinecraftAdapter {
    fn id(&self) -> &'static str {
        "minecraft"
    }

    fn capabilities(&self) -> modlab_core::game_adapter::GameCapabilities {
        use modlab_core::game_adapter::{GameCapabilities, GameSection::*};
        GameCapabilities {
            short_name: "Minecraft",
            sections: vec![Dashboard, Mods, Packs, Worlds, Servers, Health],
            deployment_label: "dossier mods/",
            script_extender: None,
            load_order_tool: None,
            // Not an external manager like MO2 — Minecraft's own detected
            // instance *is* the migration source: its `mods/` folder is
            // already the real modlist. Clients map this id to a label the
            // same way they map any other capability id (ADR 0005).
            import_source: Some("minecraft-instance"),
        }
    }

    fn detect(&self) -> Option<GameInstall> {
        find_install().map(|root| GameInstall {
            id: "minecraft".to_string(),
            name: "Minecraft".to_string(),
            root,
        })
    }

    /// Path slots expose the desktop launcher (CurseForge Install or Mojang)
    /// for Mon jeu overrides and the secondary « Ouvrir le launcher » path.
    /// Jouer itself spawns Java via `launch` and does not need those slots.
    /// `mods_dir` under vanilla `.minecraft` only reports once it exists.
    fn path_slots(&self) -> Vec<modlab_core::game_adapter::PathSlot> {
        use modlab_core::game_adapter::{PathKind, PathSlot};
        let launcher = official_launcher::preferred_launcher();
        let root = launcher
            .as_ref()
            .map(|i| i.data_dir.clone())
            .or_else(find_install);
        let exe = launcher.map(|i| i.exe);
        vec![
            PathSlot {
                id: "root",
                label: "Dossier du launcher Minecraft",
                kind: PathKind::Dir,
                detected: root.clone(),
            },
            PathSlot {
                id: "launcher_exe",
                label: "Exécutable du launcher Minecraft",
                kind: PathKind::File,
                detected: exe.filter(|p| p.is_file()),
            },
            PathSlot {
                id: "mods_dir",
                label: "Dossier mods/",
                kind: PathKind::Dir,
                detected: root.as_ref().map(|r| r.join("mods")).filter(|p| p.is_dir()),
            },
        ]
    }

    fn mods_location(&self, query: &LocationQuery<'_>) -> modlab_core::game_adapter::ModsLocation {
        modlab_core::game_adapter::ModsLocation::InInstall(
            query.instance_root.map(instance::mods_dir),
        )
    }

    fn config_location(
        &self,
        query: &LocationQuery<'_>,
    ) -> modlab_core::game_adapter::ConfigLocation {
        modlab_core::game_adapter::ConfigLocation::InInstall(
            query.instance_root.map(instance::config_dir),
        )
    }

    fn instance_root_policy(&self) -> InstanceRootPolicy {
        InstanceRootPolicy::PerInstanceRoot {
            dir_name: "minecraft",
            subdirs: LAYOUT,
        }
    }

    fn instance_subdir(&self, slot_id: &str) -> Option<&'static str> {
        match slot_id {
            "mods_dir" => Some("mods"),
            "config_dir" => Some("config"),
            "resourcepacks" => Some("resourcepacks"),
            "shaderpacks" => Some("shaderpacks"),
            _ => None,
        }
    }

    fn classify_tool(&self, query: &ToolClassifyQuery<'_>) -> Option<ClassifiedTool> {
        branch_loader::classify_token(query.source_path)
            .or_else(|| tool_archive::classify_tool(query))
    }

    fn read_archive_metadata(&self, path: &Path) -> Option<ArchiveMetadata> {
        archive_meta::read_path(path)
    }

    fn filename_metadata(&self, file_name: &str) -> Option<ArchiveMetadata> {
        filename::parse(file_name)
    }

    fn categories(&self) -> &'static [GameCategory] {
        categories::CATEGORIES
    }

    fn resolve_category(&self, hint: &CategoryHint<'_>) -> Option<&'static str> {
        categories::resolve(hint)
    }

    fn resolve_dependencies(
        &self,
        mods: &[DependencyMod],
        _paths: &[SlotPath],
    ) -> Vec<ModDependencyReport> {
        dependencies::resolve(mods)
    }

    fn stage_archive(
        &self,
        data_root: &Path,
        mod_id: i64,
        source: &ArchiveSource<'_>,
    ) -> Result<StagedMod, String> {
        install::stage(data_root, mod_id, source)
    }

    fn prepare_install(&self, job: &PrepareInstall<'_>) -> Result<PreparedMod, String> {
        install::prepare(job)
    }

    fn retract_files(&self, data_root: &Path, mod_id: i64, source: &Path) -> Option<Vec<PathBuf>> {
        install::retract(data_root, mod_id, source)
    }
}

/// Empty directories created with a new Minecraft instance folder.
const LAYOUT: &[&str] = &["mods", "config", "resourcepacks", "shaderpacks", "saves"];

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
    fn capabilities_declare_expected_vocabulary() {
        use modlab_core::game_adapter::GameSection;
        let caps = MinecraftAdapter.capabilities();
        assert_eq!(caps.short_name, "Minecraft");
        assert_eq!(caps.deployment_label, "dossier mods/");
        assert_eq!(caps.script_extender, None);
        assert!(caps.sections.contains(&GameSection::Worlds));
        assert!(caps.load_order_tool.is_none());
        assert_eq!(
            caps.import_source,
            Some("minecraft-instance"),
            "the detected instance's own mods/ folder is the migration source"
        );
    }

    #[test]
    fn path_slots_declare_the_expected_ids() {
        let slots = MinecraftAdapter.path_slots();
        let ids: Vec<&str> = slots.iter().map(|s| s.id).collect();
        assert_eq!(ids, ["root", "launcher_exe", "mods_dir"]);
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
        if let Some(install) = MinecraftAdapter.detect() {
            assert_eq!(install.id, "minecraft");
            assert!(install.root.is_dir());
        }
    }
}
