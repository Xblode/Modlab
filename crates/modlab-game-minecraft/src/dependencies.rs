//! Which declared dependency of a Minecraft mod is missing or turned off.
//!
//! A dependency is an id already stored on the mod, or re-read from the jar
//! when the file is still there (kinds survive a soft DB migration).
//! `minecraft` and `java` are ignored. The provider is the mod whose jar
//! declares that id — including nested `META-INF/jarjar/*.jar` — or, for
//! `fabricloader`, `forge` and `neoforge`, the mod classified as that loader.
//! Optional missing deps are resolved but are not badge problems; reverse
//! "used by" entries list other mods that depend on this one's own ids.

use std::collections::{HashMap, HashSet};

use modlab_core::game_adapter::{
    DeclaredDependency, DependencyKind, DependencyMod, DependencyState, ModDependencyReport,
    ResolvedDependency,
};

use crate::archive_meta;

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

#[must_use]
pub(crate) fn resolve(mods: &[DependencyMod]) -> Vec<ModDependencyReport> {
    let mut by_id: HashMap<String, Vec<Provider>> = HashMap::new();
    let mut by_tool: HashMap<String, Vec<Provider>> = HashMap::new();
    let mut own_by_mod: HashMap<i64, Vec<String>> = HashMap::new();
    for mod_ in mods {
        let provider = Provider::from_mod(mod_);
        let ids = ids_of(mod_);
        own_by_mod.insert(mod_.mod_id, ids.clone());
        for id in ids {
            by_id
                .entry(id.to_lowercase())
                .or_default()
                .push(provider.clone());
        }
        if let Some(tool) = mod_.tool.as_deref().filter(|tool| is_loader(tool)) {
            by_tool.entry(tool.to_string()).or_default().push(provider);
        }
    }

    let mut forward: HashMap<i64, Vec<ResolvedDependency>> = HashMap::new();
    for mod_ in mods {
        let mut deps = Vec::new();
        for declared in declared_deps(mod_) {
            deps.push(resolve_requirement(
                &declared.id,
                declared.kind,
                &providers_for(&declared.id, &by_id, &by_tool),
            ));
        }
        if !deps.is_empty() {
            forward.insert(mod_.mod_id, deps);
        }
    }

    // Reverse: other mods whose declared deps point at this mod's own ids.
    let mut used_by: HashMap<i64, Vec<ResolvedDependency>> = HashMap::new();
    for dependent in mods {
        for declared in declared_deps(dependent) {
            if ignored(&declared.id)
                || matches!(
                    declared.kind,
                    DependencyKind::Incompatible
                        | DependencyKind::Discouraged
                        | DependencyKind::UsedBy
                )
            {
                continue;
            }
            let key = declared.id.to_lowercase();
            for target in mods {
                if target.mod_id == dependent.mod_id {
                    continue;
                }
                let Some(own) = own_by_mod.get(&target.mod_id) else {
                    continue;
                };
                if !own.iter().any(|id| id.eq_ignore_ascii_case(&key)) {
                    continue;
                }
                let state = if dependent.enabled {
                    DependencyState::Active
                } else {
                    DependencyState::Disabled
                };
                let entry = ResolvedDependency {
                    name: dependent.name.clone(),
                    state,
                    provider_mod_id: Some(dependent.mod_id),
                    provider_name: Some(dependent.name.clone()),
                    kind: DependencyKind::UsedBy,
                };
                let list = used_by.entry(target.mod_id).or_default();
                if list
                    .iter()
                    .any(|seen| seen.provider_mod_id == entry.provider_mod_id)
                {
                    continue;
                }
                list.push(entry);
            }
        }
    }

    let mut reports = Vec::new();
    for mod_ in mods {
        let mut deps = forward.remove(&mod_.mod_id).unwrap_or_default();
        if let Some(mut reverse) = used_by.remove(&mod_.mod_id) {
            deps.append(&mut reverse);
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

fn ids_of(mod_: &DependencyMod) -> Vec<String> {
    let from_source = archive_meta::own_ids(&mod_.source_path);
    if !from_source.is_empty() {
        return from_source;
    }
    mod_.archive_path
        .as_deref()
        .filter(|path| *path != mod_.source_path)
        .map(archive_meta::own_ids)
        .unwrap_or_default()
}

/// Prefer a live jar read (correct kinds + ownId filter). Fall back to the
/// stored list so a deleted archive still resolves something.
fn declared_deps(mod_: &DependencyMod) -> Vec<DeclaredDependency> {
    if let Some(meta) = archive_meta::read_path(&mod_.source_path) {
        if !meta.dependencies.is_empty() {
            return filter_declared(meta.dependencies);
        }
    }
    if let Some(path) = mod_.archive_path.as_deref() {
        if path != mod_.source_path {
            if let Some(meta) = archive_meta::read_path(path) {
                if !meta.dependencies.is_empty() {
                    return filter_declared(meta.dependencies);
                }
            }
        }
    }
    filter_declared(mod_.dependencies.clone())
}

fn filter_declared(deps: Vec<DeclaredDependency>) -> Vec<DeclaredDependency> {
    let mut out = Vec::new();
    for dep in deps {
        let trimmed = dep.id.trim();
        if ignored(trimmed) {
            continue;
        }
        if out
            .iter()
            .any(|seen: &DeclaredDependency| seen.id.eq_ignore_ascii_case(trimmed))
        {
            continue;
        }
        out.push(DeclaredDependency {
            id: trimmed.to_string(),
            kind: dep.kind,
        });
    }
    out
}

fn ignored(id: &str) -> bool {
    id.is_empty() || id.eq_ignore_ascii_case("minecraft") || id.eq_ignore_ascii_case("java")
}

fn is_loader(tool: &str) -> bool {
    matches!(tool, "fabric" | "forge" | "neoforge")
}

fn loader_tool(id: &str) -> Option<&'static str> {
    if id.eq_ignore_ascii_case("fabricloader") {
        Some("fabric")
    } else if id.eq_ignore_ascii_case("forge") {
        Some("forge")
    } else if id.eq_ignore_ascii_case("neoforge") {
        Some("neoforge")
    } else {
        None
    }
}

fn providers_for(
    id: &str,
    by_id: &HashMap<String, Vec<Provider>>,
    by_tool: &HashMap<String, Vec<Provider>>,
) -> Vec<Provider> {
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    if let Some(list) = by_id.get(&id.to_lowercase()) {
        for provider in list {
            if seen.insert(provider.mod_id) {
                out.push(provider.clone());
            }
        }
    }
    if let Some(tool) = loader_tool(id) {
        if let Some(list) = by_tool.get(tool) {
            for provider in list {
                if seen.insert(provider.mod_id) {
                    out.push(provider.clone());
                }
            }
        }
    }
    out
}

fn resolve_requirement(
    name: &str,
    kind: DependencyKind,
    providers: &[Provider],
) -> ResolvedDependency {
    if let Some(provider) = best(providers, true) {
        return from_provider(name, DependencyState::Active, kind, provider);
    }
    if let Some(provider) = best(providers, false) {
        return from_provider(name, DependencyState::Disabled, kind, provider);
    }
    ResolvedDependency {
        name: name.to_string(),
        state: DependencyState::Missing,
        provider_mod_id: None,
        provider_name: None,
        kind,
    }
}

fn best(providers: &[Provider], enabled: bool) -> Option<&Provider> {
    providers
        .iter()
        .filter(|provider| provider.enabled == enabled)
        .max_by_key(|provider| (provider.position, provider.mod_id))
}

fn from_provider(
    name: &str,
    state: DependencyState,
    kind: DependencyKind,
    provider: &Provider,
) -> ResolvedDependency {
    ResolvedDependency {
        name: name.to_string(),
        state,
        provider_mod_id: Some(provider.mod_id),
        provider_name: Some(provider.name.clone()),
        kind,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use modlab_core::game_adapter::GameAdapter;
    use std::fs;
    use std::io::{Cursor, Write};
    use std::path::Path;

    fn write_jar(path: &Path, id: &str) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        let file = fs::File::create(path).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        zip.start_file("fabric.mod.json", zip::write::SimpleFileOptions::default())
            .unwrap();
        let body = format!(r#"{{"schemaVersion":1,"id":"{id}","name":"Fabric API"}}"#);
        zip.write_all(body.as_bytes()).unwrap();
        zip.finish().unwrap();
    }

    fn write_bytes(path: &Path, bytes: &[u8]) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, bytes).unwrap();
    }

    fn zip_bytes(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let mut cursor = Cursor::new(Vec::new());
        let mut zip = zip::ZipWriter::new(&mut cursor);
        for (name, body) in entries {
            zip.start_file(*name, zip::write::SimpleFileOptions::default())
                .unwrap();
            zip.write_all(body).unwrap();
        }
        zip.finish().unwrap();
        cursor.into_inner()
    }

    fn dep_mod(
        id: i64,
        name: &str,
        path: &Path,
        enabled: bool,
        tool: Option<&str>,
        dependencies: &[DeclaredDependency],
    ) -> DependencyMod {
        DependencyMod {
            mod_id: id,
            name: name.to_string(),
            source_path: path.to_path_buf(),
            archive_path: None,
            enabled,
            position: id,
            tool: tool.map(str::to_string),
            dependencies: dependencies.to_vec(),
        }
    }

    fn named<'a>(
        reports: &'a [ModDependencyReport],
        mod_id: i64,
        name: &str,
    ) -> &'a ResolvedDependency {
        reports
            .iter()
            .find(|report| report.mod_id == mod_id)
            .unwrap()
            .dependencies
            .iter()
            .find(|dep| dep.name == name)
            .unwrap()
    }

    #[test]
    fn disabled_fabric_api_is_disabled_and_minecraft_java_are_ignored() {
        let tmp = tempfile::tempdir().unwrap();
        let api = tmp.path().join("fabric-api.jar");
        write_jar(&api, "fabric-api");
        let loader = tmp.path().join("fabric-loader.jar");
        fs::write(&loader, b"not a jar").unwrap();
        let client = tmp.path().join("client.jar");

        let mods = vec![
            dep_mod(
                1,
                "My Mod",
                &client,
                true,
                None,
                &[
                    DeclaredDependency::required("fabric-api"),
                    DeclaredDependency::required("fabricloader"),
                    DeclaredDependency::required("minecraft"),
                    DeclaredDependency::required("Java"),
                ],
            ),
            dep_mod(2, "Fabric API", &api, false, None, &[]),
            dep_mod(3, "Fabric Loader", &loader, true, Some("fabric"), &[]),
        ];
        let found = crate::MinecraftAdapter.resolve_dependencies(&mods, &[]);
        let fabric_api = named(&found, 1, "fabric-api");
        assert_eq!(fabric_api.state, DependencyState::Disabled);
        assert_eq!(fabric_api.kind, DependencyKind::Required);
        assert_eq!(fabric_api.provider_mod_id, Some(2));
        assert_eq!(fabric_api.provider_name.as_deref(), Some("Fabric API"));
        let loader_dep = named(&found, 1, "fabricloader");
        assert_eq!(loader_dep.state, DependencyState::Active);
        assert_eq!(loader_dep.provider_mod_id, Some(3));
        let names: Vec<&str> = found
            .iter()
            .find(|report| report.mod_id == 1)
            .unwrap()
            .dependencies
            .iter()
            .filter(|dep| dep.kind != DependencyKind::UsedBy)
            .map(|dep| dep.name.as_str())
            .collect();
        assert_eq!(names, ["fabric-api", "fabricloader"]);

        let mut enabled = mods.clone();
        enabled[1].enabled = true;
        let again = crate::MinecraftAdapter.resolve_dependencies(&enabled, &[]);
        assert_eq!(
            named(&again, 1, "fabric-api").state,
            DependencyState::Active
        );

        let alone = dep_mod(
            1,
            "My Mod",
            &client,
            true,
            None,
            &[
                DeclaredDependency::required("fabric-api"),
                DeclaredDependency::required("minecraft"),
                DeclaredDependency::required("java"),
            ],
        );
        let missing = crate::MinecraftAdapter.resolve_dependencies(&[alone], &[]);
        assert_eq!(
            named(&missing, 1, "fabric-api").state,
            DependencyState::Missing
        );
        assert!(named(&missing, 1, "fabric-api").provider_mod_id.is_none());
    }

    #[test]
    fn optional_missing_is_not_required_and_reverse_used_by_works() {
        let tmp = tempfile::tempdir().unwrap();
        let neo_toml = br#"
[[mods]]
modId="xaerominimap"
version="1"
displayName="Minimap"

[[dependencies.xaerominimap]]
modId="neoforge"
type="required"

[[dependencies.xaerominimap]]
modId="xaeroworldmap"
type="optional"

[[dependencies.xaerobetterpvp]]
modId="xaerolib"
type="required"
"#;
        let minimap = tmp.path().join("minimap.jar");
        write_bytes(
            &minimap,
            &zip_bytes(&[("META-INF/neoforge.mods.toml", neo_toml)]),
        );

        let world_toml = br#"
[[mods]]
modId="xaeroworldmap"
version="1"
displayName="World Map"

[[dependencies.xaeroworldmap]]
modId="xaerominimap"
type="optional"
"#;
        let world = tmp.path().join("worldmap.jar");
        write_bytes(
            &world,
            &zip_bytes(&[("META-INF/neoforge.mods.toml", world_toml)]),
        );

        let mods = vec![
            dep_mod(1, "Xaero's Minimap", &minimap, true, None, &[]),
            dep_mod(2, "Xaero's World Map", &world, true, None, &[]),
            dep_mod(
                3,
                "NeoForge",
                &tmp.path().join("neoforge.jar"),
                true,
                Some("neoforge"),
                &[],
            ),
        ];
        fs::write(tmp.path().join("neoforge.jar"), b"not a jar").unwrap();

        let reports = crate::MinecraftAdapter.resolve_dependencies(&mods, &[]);
        let mini = reports.iter().find(|r| r.mod_id == 1).unwrap();
        assert!(
            !mini.dependencies.iter().any(|d| d.name == "xaerolib"),
            "other modId block must not leak"
        );
        let optional = named(&reports, 1, "xaeroworldmap");
        assert_eq!(optional.kind, DependencyKind::Optional);
        assert_eq!(optional.state, DependencyState::Active);

        let alone = vec![
            dep_mod(1, "Xaero's Minimap", &minimap, true, None, &[]),
            dep_mod(
                3,
                "NeoForge",
                &tmp.path().join("neoforge.jar"),
                true,
                Some("neoforge"),
                &[],
            ),
        ];
        let alone_reports = crate::MinecraftAdapter.resolve_dependencies(&alone, &[]);
        let missing_optional = named(&alone_reports, 1, "xaeroworldmap");
        assert_eq!(missing_optional.state, DependencyState::Missing);
        assert_eq!(missing_optional.kind, DependencyKind::Optional);

        let used = named(&reports, 1, "Xaero's World Map");
        assert_eq!(used.kind, DependencyKind::UsedBy);
        assert_eq!(used.state, DependencyState::Active);
        assert_eq!(used.provider_mod_id, Some(2));
    }

    #[test]
    fn jarjar_provider_satisfies_required_dep() {
        let tmp = tempfile::tempdir().unwrap();
        let nested = zip_bytes(&[(
            "META-INF/neoforge.mods.toml",
            b"[[mods]]\nmodId=\"xaerolib\"\nversion=\"1\"\ndisplayName=\"Lib\"\n",
        )]);
        let mut cursor = Cursor::new(Vec::new());
        {
            let mut zip = zip::ZipWriter::new(&mut cursor);
            zip.start_file(
                "META-INF/neoforge.mods.toml",
                zip::write::SimpleFileOptions::default(),
            )
            .unwrap();
            zip.write_all(
                br#"
[[mods]]
modId="xaerobetterpvp"
version="1"
displayName="Better PVP"

[[dependencies.xaerobetterpvp]]
modId="xaerolib"
type="required"
"#,
            )
            .unwrap();
            zip.start_file(
                "META-INF/jarjar/xaerolib.jar",
                zip::write::SimpleFileOptions::default(),
            )
            .unwrap();
            zip.write_all(&nested).unwrap();
            zip.finish().unwrap();
        }
        let host = tmp.path().join("betterpvp.jar");
        write_bytes(&host, &cursor.into_inner());

        let mods = vec![dep_mod(1, "Better PVP", &host, true, None, &[])];
        let reports = crate::MinecraftAdapter.resolve_dependencies(&mods, &[]);
        let lib = named(&reports, 1, "xaerolib");
        assert_eq!(lib.state, DependencyState::Active);
        assert_eq!(lib.kind, DependencyKind::Required);
        assert_eq!(lib.provider_mod_id, Some(1));
    }
}
