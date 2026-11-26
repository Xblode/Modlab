//! Which dependency of an enabled mod is missing or turned off.
//!
//! Skyrim only. Two sources, and nothing else:
//! - each plugin's header masters, matched to the plugin file that provides
//!   that name on this branch (or to the game's own `Data/` folder);
//! - a DLL directly inside `SKSE/Plugins/`, which needs the SKSE mod
//!   (`tool` `skse` or `skse-vr`, or a folder that holds the loader exe).
//!
//! Official plugins (the five base names, plus `Skyrim.ccc` when it can be
//! read) are not dependencies. An unreadable plugin contributes none.
//! Masters packed in a BSA, FOMOD conditions and Nexus requirement lists
//! are not read.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use modlab_core::game_adapter::{
    DependencyKind, DependencyMod, DependencyState, ModDependencyReport, ResolvedDependency,
};

use crate::{engine, plugins};

const SKSE_NAME: &str = "SKSE";

#[derive(Clone)]
struct Provider {
    mod_id: i64,
    name: String,
    enabled: bool,
    position: i64,
}

impl Provider {
    fn from_mod(mod_: &DependencyMod) -> Self {
        Self {
            mod_id: mod_.mod_id,
            name: mod_.name.clone(),
            enabled: mod_.enabled,
            position: mod_.position,
        }
    }
}

/// `install_root` is the game root (`Skyrim.ccc`). `data_dir` is the real
/// `Data/` folder. Either may be absent: the five official names still apply,
/// and a master nobody provides is then missing rather than assumed present.
#[must_use]
pub(crate) fn resolve(
    mods: &[DependencyMod],
    install_root: Option<&Path>,
    data_dir: Option<&Path>,
) -> Vec<ModDependencyReport> {
    let official: HashSet<String> = plugins::official_plugin_names(install_root)
        .into_iter()
        .map(|name| name.to_lowercase())
        .collect();
    let game_files: HashSet<String> = data_dir
        .map(plugins::plugins_in)
        .unwrap_or_default()
        .into_iter()
        .map(|(name, _)| name.to_lowercase())
        .collect();

    let mut provided: HashMap<String, Vec<Provider>> = HashMap::new();
    for mod_ in mods {
        let Some(root) = content_root(mod_) else {
            continue;
        };
        for (name, _) in plugins::plugins_in(&root) {
            provided
                .entry(name.to_lowercase())
                .or_default()
                .push(Provider::from_mod(mod_));
        }
    }
    let skse_mods: Vec<Provider> = mods
        .iter()
        .filter(|mod_| is_skse_mod(mod_))
        .map(Provider::from_mod)
        .collect();

    let mut reports = Vec::new();
    for mod_ in mods {
        let Some(root) = content_root(mod_) else {
            continue;
        };
        let mut deps = Vec::new();
        let mut seen = HashSet::new();
        for (plugin_name, path) in plugins::plugins_in(&root) {
            let Some(masters) = plugins::read_masters(&path) else {
                continue;
            };
            let plugin_key = plugin_name.to_lowercase();
            for master in masters {
                let display = master_file_name(&master);
                if display.is_empty() {
                    continue;
                }
                let key = display.to_lowercase();
                if key == plugin_key || !seen.insert(key.clone()) || official.contains(&key) {
                    continue;
                }
                let providers = provided.get(&key).map(Vec::as_slice).unwrap_or(&[]);
                deps.push(resolve_requirement(
                    &display,
                    providers,
                    game_files.contains(&key),
                ));
            }
        }
        if has_skse_plugin_dll(&root)
            && !skse_mods
                .iter()
                .any(|provider| provider.mod_id == mod_.mod_id)
        {
            let others: Vec<Provider> = skse_mods
                .iter()
                .filter(|provider| provider.mod_id != mod_.mod_id)
                .cloned()
                .collect();
            deps.push(resolve_requirement(SKSE_NAME, &others, false));
        }
        if !deps.is_empty() {
            reports.push(ModDependencyReport {
                mod_id: mod_.mod_id,
                dependencies: deps,
            });
        }
    }
    reports
}

/// Directory the game treats as `Data/` for this mod. The engine mod is
/// reduced to its `Data` child, same as USVFS. A normal mod is its own
/// folder: a nested `Data/` is not the game's `Data/`.
fn content_root(mod_: &DependencyMod) -> Option<PathBuf> {
    if engine::is_engine_content(&mod_.source_path) {
        return engine::data_subdir(&mod_.source_path);
    }
    mod_.source_path.is_dir().then(|| mod_.source_path.clone())
}

fn is_skse_mod(mod_: &DependencyMod) -> bool {
    mod_.tool
        .as_deref()
        .is_some_and(|tool| tool == "skse" || tool == "skse-vr")
        || engine::is_engine_content(&mod_.source_path)
}

fn master_file_name(master: &str) -> String {
    let trimmed = master.trim();
    Path::new(trimmed)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(trimmed)
        .to_string()
}

fn resolve_requirement(
    name: &str,
    providers: &[Provider],
    game_provides: bool,
) -> ResolvedDependency {
    if let Some(provider) = best_provider(providers, true) {
        return from_provider(name, DependencyState::Active, provider);
    }
    if game_provides {
        return ResolvedDependency {
            name: name.to_string(),
            state: DependencyState::Active,
            provider_mod_id: None,
            provider_name: None,
            kind: DependencyKind::Required,
        };
    }
    if let Some(provider) = best_provider(providers, false) {
        return from_provider(name, DependencyState::Disabled, provider);
    }
    ResolvedDependency {
        name: name.to_string(),
        state: DependencyState::Missing,
        provider_mod_id: None,
        provider_name: None,
        kind: DependencyKind::Required,
    }
}

fn best_provider(providers: &[Provider], enabled: bool) -> Option<&Provider> {
    providers
        .iter()
        .filter(|provider| provider.enabled == enabled)
        .max_by_key(|provider| (provider.position, provider.mod_id))
}

fn from_provider(name: &str, state: DependencyState, provider: &Provider) -> ResolvedDependency {
    ResolvedDependency {
        name: name.to_string(),
        state,
        provider_mod_id: Some(provider.mod_id),
        provider_name: Some(provider.name.clone()),
        kind: DependencyKind::Required,
    }
}

/// `SKSE/Plugins/*.dll` directly under the mod's Data root. SKSE does not
/// load a DLL from a deeper folder, so this does not walk the tree.
fn has_skse_plugin_dll(root: &Path) -> bool {
    let Some(plugins) = child_dir(root, "SKSE").and_then(|skse| child_dir(&skse, "Plugins")) else {
        return false;
    };
    let Ok(read) = fs::read_dir(plugins) else {
        return false;
    };
    read.filter_map(Result::ok).any(|entry| {
        entry.path().is_file()
            && entry
                .path()
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("dll"))
    })
}

fn child_dir(parent: &Path, want: &str) -> Option<PathBuf> {
    fs::read_dir(parent).ok()?.find_map(|entry| {
        let entry = entry.ok()?;
        let is_dir = entry.file_type().ok()?.is_dir();
        if is_dir
            && entry
                .file_name()
                .to_string_lossy()
                .eq_ignore_ascii_case(want)
        {
            Some(entry.path())
        } else {
            None
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use modlab_core::game_adapter::{GameAdapter, SlotPath};

    fn plugin_bytes(masters: &[&[u8]]) -> Vec<u8> {
        let mut body = Vec::new();
        body.extend_from_slice(b"HEDR");
        body.extend_from_slice(&12u16.to_le_bytes());
        body.extend_from_slice(&1.7f32.to_le_bytes());
        body.extend_from_slice(&0u32.to_le_bytes());
        body.extend_from_slice(&0x800u32.to_le_bytes());
        for master in masters {
            body.extend_from_slice(b"MAST");
            body.extend_from_slice(&u16::try_from(master.len() + 1).unwrap().to_le_bytes());
            body.extend_from_slice(master);
            body.push(0);
            body.extend_from_slice(b"DATA");
            body.extend_from_slice(&8u16.to_le_bytes());
            body.extend_from_slice(&[0; 8]);
        }
        let mut out = Vec::new();
        out.extend_from_slice(b"TES4");
        out.extend_from_slice(&u32::try_from(body.len()).unwrap().to_le_bytes());
        out.extend_from_slice(&0u32.to_le_bytes());
        out.extend_from_slice(&0u32.to_le_bytes());
        out.extend_from_slice(&0u32.to_le_bytes());
        out.extend_from_slice(&44u16.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&body);
        out
    }

    fn put(dir: &Path, name: &str, masters: &[&[u8]]) {
        fs::create_dir_all(dir).unwrap();
        fs::write(dir.join(name), plugin_bytes(masters)).unwrap();
    }

    fn dep_mod(
        id: i64,
        name: &str,
        dir: &Path,
        enabled: bool,
        tool: Option<&str>,
    ) -> DependencyMod {
        DependencyMod {
            mod_id: id,
            name: name.to_string(),
            source_path: dir.to_path_buf(),
            archive_path: None,
            enabled,
            position: id,
            tool: tool.map(str::to_string),
            dependencies: Vec::new(),
        }
    }

    fn reports(
        mods: &[DependencyMod],
        root: Option<&Path>,
        data: Option<&Path>,
    ) -> Vec<ModDependencyReport> {
        let mut paths = Vec::new();
        if let Some(path) = root {
            paths.push(SlotPath {
                id: "root".to_string(),
                path: path.to_path_buf(),
            });
        }
        if let Some(path) = data {
            paths.push(SlotPath {
                id: "data_dir".to_string(),
                path: path.to_path_buf(),
            });
        }
        crate::SkyrimAdapter.resolve_dependencies(mods, &paths)
    }

    fn deps(found: &[ModDependencyReport], mod_id: i64) -> Vec<ResolvedDependency> {
        found
            .iter()
            .find(|report| report.mod_id == mod_id)
            .map(|report| report.dependencies.clone())
            .unwrap_or_default()
    }

    #[test]
    fn disabled_master_is_disabled_official_is_omitted_and_enabled_master_is_active() {
        let tmp = tempfile::tempdir().unwrap();
        let library = tmp.path().join("library");
        let patch = tmp.path().join("patch");
        let child = tmp.path().join("child");
        put(&library, "Library.esp", &[b"Skyrim.esm"]);
        put(&patch, "Patch.esp", &[b"Skyrim.esm"]);
        put(
            &child,
            "Child.esp",
            &[b"Skyrim.esm", b"Library.esp", b"Patch.esp"],
        );
        let found = reports(
            &[
                dep_mod(1, "Library", &library, false, None),
                dep_mod(2, "Patch", &patch, true, None),
                dep_mod(3, "Child", &child, true, None),
            ],
            None,
            None,
        );
        let child_deps = deps(&found, 3);
        assert!(
            child_deps
                .iter()
                .all(|dep| !dep.name.eq_ignore_ascii_case("Skyrim.esm")),
            "{child_deps:?}"
        );
        let library_dep = child_deps
            .iter()
            .find(|dep| dep.name == "Library.esp")
            .unwrap();
        assert_eq!(library_dep.state, DependencyState::Disabled);
        assert_eq!(library_dep.provider_mod_id, Some(1));
        assert_eq!(library_dep.provider_name.as_deref(), Some("Library"));
        let patch_dep = child_deps
            .iter()
            .find(|dep| dep.name == "Patch.esp")
            .unwrap();
        assert_eq!(patch_dep.state, DependencyState::Active);
        assert_eq!(patch_dep.provider_mod_id, Some(2));
    }

    #[test]
    fn unknown_master_is_missing() {
        let tmp = tempfile::tempdir().unwrap();
        let child = tmp.path().join("child");
        put(&child, "Child.esp", &[b"Skyrim.esm", b"Gone.esm"]);
        let found = reports(&[dep_mod(1, "Child", &child, true, None)], None, None);
        assert_eq!(
            deps(&found, 1),
            vec![ResolvedDependency {
                name: "Gone.esm".to_string(),
                state: DependencyState::Missing,
                provider_mod_id: None,
                provider_name: None,
                kind: DependencyKind::Required,
            }]
        );
    }

    #[test]
    fn a_master_that_ships_in_the_game_data_folder_is_active() {
        let tmp = tempfile::tempdir().unwrap();
        let data = tmp.path().join("Data");
        let child = tmp.path().join("child");
        put(&data, "Custom.esm", &[]);
        put(&child, "Child.esp", &[b"Skyrim.esm", b"Custom.esm"]);
        let found = reports(
            &[dep_mod(1, "Child", &child, true, None)],
            None,
            Some(&data),
        );
        assert_eq!(
            deps(&found, 1),
            vec![ResolvedDependency {
                name: "Custom.esm".to_string(),
                state: DependencyState::Active,
                provider_mod_id: None,
                provider_name: None,
                kind: DependencyKind::Required,
            }]
        );
    }

    #[test]
    fn skse_plugin_dll_is_missing_without_skse_and_disabled_when_skse_is_off() {
        let tmp = tempfile::tempdir().unwrap();
        let plugin = tmp.path().join("plugin");
        fs::create_dir_all(plugin.join("SKSE").join("Plugins")).unwrap();
        fs::write(plugin.join("SKSE").join("Plugins").join("foo.dll"), b"dll").unwrap();

        let missing = reports(&[dep_mod(1, "Plugin", &plugin, true, None)], None, None);
        assert_eq!(
            deps(&missing, 1),
            vec![ResolvedDependency {
                name: SKSE_NAME.to_string(),
                state: DependencyState::Missing,
                provider_mod_id: None,
                provider_name: None,
                kind: DependencyKind::Required,
            }]
        );

        let skse = tmp.path().join("skse");
        fs::create_dir_all(&skse).unwrap();
        let disabled = reports(
            &[
                dep_mod(1, "Plugin", &plugin, true, None),
                dep_mod(2, "SKSE64", &skse, false, Some("skse")),
            ],
            None,
            None,
        );
        let dep = &deps(&disabled, 1)[0];
        assert_eq!(dep.name, SKSE_NAME);
        assert_eq!(dep.state, DependencyState::Disabled);
        assert_eq!(dep.provider_mod_id, Some(2));
        assert_eq!(dep.provider_name.as_deref(), Some("SKSE64"));

        let enabled = reports(
            &[
                dep_mod(1, "Plugin", &plugin, true, None),
                dep_mod(2, "SKSE64", &skse, true, Some("skse")),
            ],
            None,
            None,
        );
        assert_eq!(deps(&enabled, 1)[0].state, DependencyState::Active);

        fs::create_dir_all(skse.join("SKSE").join("Plugins")).unwrap();
        fs::write(skse.join("SKSE").join("Plugins").join("self.dll"), b"dll").unwrap();
        let itself = reports(
            &[dep_mod(2, "SKSE64", &skse, true, Some("skse"))],
            None,
            None,
        );
        assert!(deps(&itself, 2).is_empty());
    }

    #[test]
    fn a_dll_outside_skse_plugins_is_not_a_dependency() {
        let tmp = tempfile::tempdir().unwrap();
        let plugin = tmp.path().join("plugin");
        fs::create_dir_all(plugin.join("Meshes")).unwrap();
        fs::write(plugin.join("Meshes").join("x.dll"), b"dll").unwrap();
        let found = reports(&[dep_mod(1, "Plugin", &plugin, true, None)], None, None);
        assert!(found.is_empty());
    }
}
