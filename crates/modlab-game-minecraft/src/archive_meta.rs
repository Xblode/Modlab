//! Facts a Minecraft jar states about itself.
//!
//! This is not [`crate::tool_archive`]: that module answers "is this file
//! the loader?". Almost every mod jar would match a loader manifest, and
//! that flag would be useless. Here a normal mod is the interesting case.
//!
//! Read locally, and only these places:
//! `fabric.mod.json`, `quilt.mod.json`, `META-INF/neoforge.mods.toml`,
//! `META-INF/mods.toml`, then the file name when none of those named a
//! Minecraft version. A jar that carries more than one of those files
//! reports every loader. `${file.jarVersion}` is read from `MANIFEST.MF`.
//! Nothing is downloaded and nothing is scraped. A value the file does not
//! state stays empty. There is no list of famous mods.

use modlab_core::game_adapter::{ArchiveMetadata, DeclaredDependency, DependencyKind};
use std::fs::File;
use std::io::{Cursor, Read, Seek};
use std::path::Path;
use zip::ZipArchive;

const TEXT_CAP: u64 = 65_536;

/// Read one jar or zip that is already on disk. `None` when the path is
/// missing or the file says nothing this module knows.
#[must_use]
pub fn read_path(path: &Path) -> Option<ArchiveMetadata> {
    if !path.is_file() {
        return None;
    }
    let file_name = path.file_name()?.to_string_lossy().into_owned();
    let meta = match File::open(path) {
        Ok(file) => match ZipArchive::new(file) {
            Ok(mut archive) => read_zip(&mut archive, &file_name),
            Err(_) => with_filename(ArchiveMetadata::default(), &file_name),
        },
        Err(_) => return None,
    };
    if meta.is_empty() {
        None
    } else {
        Some(meta)
    }
}

/// Ids this jar declares for itself, plus ids nested under `META-INF/jarjar/`.
///
/// Fabric: `fabric.mod.json` `"id"`. Forge and NeoForge: every `[[mods]]`
/// `modId`. Empty when the file does not say — the file name is not an id.
#[must_use]
pub fn own_ids(path: &Path) -> Vec<String> {
    if !path.is_file() {
        return Vec::new();
    }
    let Ok(file) = File::open(path) else {
        return Vec::new();
    };
    let Ok(mut archive) = ZipArchive::new(file) else {
        return Vec::new();
    };
    let mut ids = Vec::new();
    if let Some(text) = entry_text(&mut archive, "fabric.mod.json") {
        if let Some(id) = json_own_id(&text) {
            ids.push(id);
        }
    }
    if ids.is_empty() {
        if let Some(text) = entry_text(&mut archive, "quilt.mod.json") {
            if let Some(id) = quilt_own_id(&text) {
                ids.push(id);
            }
        }
    }
    if ids.is_empty() {
        for name in ["META-INF/neoforge.mods.toml", "META-INF/mods.toml"] {
            if let Some(text) = entry_text(&mut archive, name) {
                ids = toml_own_ids(&text);
                if !ids.is_empty() {
                    break;
                }
            }
        }
    }
    append_jarjar_ids(&mut archive, &mut ids);
    ids
}

fn quilt_own_id(text: &str) -> Option<String> {
    let json: serde_json::Value = serde_json::from_str(text).ok()?;
    let id = json
        .get("quilt_loader")
        .and_then(|loader| json_string(loader, "id"))
        .or_else(|| json_string(&json, "id"))?;
    let id = id.trim();
    if id.is_empty() {
        None
    } else {
        Some(id.to_string())
    }
}

fn append_jarjar_ids<R: Read + Seek>(archive: &mut ZipArchive<R>, ids: &mut Vec<String>) {
    let names: Vec<String> = (0..archive.len())
        .filter_map(|index| {
            let entry = archive.by_index(index).ok()?;
            let name = entry.name().replace('\\', "/");
            let lower = name.to_ascii_lowercase();
            if lower.starts_with("meta-inf/jarjar/")
                && lower.ends_with(".jar")
                && !entry.is_dir()
            {
                Some(name)
            } else {
                None
            }
        })
        .collect();
    for name in names {
        let Ok(mut entry) = archive.by_name(&name) else {
            continue;
        };
        let mut bytes = Vec::new();
        if entry.read_to_end(&mut bytes).is_err() || bytes.is_empty() {
            continue;
        }
        let Ok(mut nested) = ZipArchive::new(Cursor::new(bytes)) else {
            continue;
        };
        for id in nested_root_ids(&mut nested) {
            push_unique(ids, id);
        }
    }
}

fn nested_root_ids<R: Read + Seek>(archive: &mut ZipArchive<R>) -> Vec<String> {
    if let Some(text) = entry_text(archive, "fabric.mod.json") {
        if let Some(id) = json_own_id(&text) {
            return vec![id];
        }
    }
    if let Some(text) = entry_text(archive, "quilt.mod.json") {
        if let Some(id) = quilt_own_id(&text) {
            return vec![id];
        }
    }
    for name in ["META-INF/neoforge.mods.toml", "META-INF/mods.toml"] {
        if let Some(text) = entry_text(archive, name) {
            let ids = toml_own_ids(&text);
            if !ids.is_empty() {
                return ids;
            }
        }
    }
    Vec::new()
}

fn json_own_id(text: &str) -> Option<String> {
    let json: serde_json::Value = serde_json::from_str(text).ok()?;
    let id = json.get("id")?.as_str()?.trim();
    if id.is_empty() {
        None
    } else {
        Some(id.to_string())
    }
}

fn toml_own_ids(text: &str) -> Vec<String> {
    let Ok(doc) = toml::from_str::<toml::Value>(text) else {
        return Vec::new();
    };
    let Some(mods) = doc.get("mods").and_then(|value| value.as_array()) else {
        return Vec::new();
    };
    let mut ids = Vec::new();
    for item in mods {
        let Some(table) = item.as_table() else {
            continue;
        };
        let Some(id) = toml_string(table, "modId").or_else(|| toml_string(table, "modid")) else {
            continue;
        };
        if ids
            .iter()
            .any(|seen: &String| seen.eq_ignore_ascii_case(&id))
        {
            continue;
        }
        ids.push(id);
    }
    ids
}

fn read_zip<R: Read + Seek>(archive: &mut ZipArchive<R>, file_name: &str) -> ArchiveMetadata {
    let manifest = entry_text(archive, "META-INF/MANIFEST.MF");
    let jar_version = manifest.as_deref().and_then(implementation_version);
    let mut meta = ArchiveMetadata::default();
    if let Some(text) = entry_text(archive, "fabric.mod.json") {
        if let Some(part) = from_fabric(&text) {
            absorb(&mut meta, part);
            push_unique(&mut meta.loaders, "fabric".to_string());
        }
    }
    if let Some(text) = entry_text(archive, "quilt.mod.json") {
        if let Some(part) = from_quilt(&text) {
            absorb(&mut meta, part);
            push_unique(&mut meta.loaders, "quilt".to_string());
        }
    }
    if let Some(text) = entry_text(archive, "META-INF/neoforge.mods.toml") {
        if let Some(part) = from_toml(&text) {
            absorb(&mut meta, part);
            push_unique(&mut meta.loaders, "neoforge".to_string());
        }
    }
    if let Some(text) = entry_text(archive, "META-INF/mods.toml") {
        if let Some(part) = from_toml(&text) {
            absorb(&mut meta, part);
            push_unique(&mut meta.loaders, "forge".to_string());
        }
    }
    meta.version = resolve_jar_version(meta.version, jar_version.as_deref());
    with_filename(meta, file_name)
}

fn absorb(dst: &mut ArchiveMetadata, src: ArchiveMetadata) {
    if blank(&dst.name) {
        dst.name = src.name;
    }
    if version_blank_or_placeholder(&dst.version)
        && (src
            .version
            .as_ref()
            .is_some_and(|value| !is_placeholder(value))
            || dst.version.is_none())
    {
        dst.version = src.version;
    }
    for version in src.game_versions {
        push_unique(&mut dst.game_versions, version);
    }
    for dep in src.dependencies {
        push_unique_dep(&mut dst.dependencies, dep);
    }
    for loader in src.loaders {
        push_unique(&mut dst.loaders, loader);
    }
}

fn blank(value: &Option<String>) -> bool {
    value.as_ref().map(|s| s.trim().is_empty()).unwrap_or(true)
}

fn version_blank_or_placeholder(value: &Option<String>) -> bool {
    match value {
        None => true,
        Some(text) if text.trim().is_empty() || is_placeholder(text) => true,
        Some(_) => false,
    }
}

fn is_placeholder(value: &str) -> bool {
    value.contains('$') || value.contains('{') || value.contains('}')
}

fn implementation_version(text: &str) -> Option<String> {
    for line in text.lines() {
        let line = line.trim();
        let Some(rest) = line.strip_prefix("Implementation-Version:") else {
            continue;
        };
        let value = rest.trim();
        if !value.is_empty() && !is_placeholder(value) {
            return Some(value.to_string());
        }
    }
    None
}

fn resolve_jar_version(version: Option<String>, jar_version: Option<&str>) -> Option<String> {
    let version = version?;
    if !is_placeholder(&version) {
        return Some(version);
    }
    jar_version
        .map(str::trim)
        .filter(|value| !value.is_empty() && !is_placeholder(value))
        .map(str::to_string)
}

fn with_filename(mut meta: ArchiveMetadata, file_name: &str) -> ArchiveMetadata {
    meta.name = clean_text(meta.name);
    meta.version = clean_version(meta.version);
    if meta.game_versions.is_empty() {
        meta.game_versions = game_versions_in_name(file_name);
    }
    meta.dependencies = dedup_deps(meta.dependencies);
    meta
}

fn from_fabric(text: &str) -> Option<ArchiveMetadata> {
    let json: serde_json::Value = serde_json::from_str(text).ok()?;
    if !json.is_object() {
        return None;
    }
    let name = json_string(&json, "name").or_else(|| json_string(&json, "id"));
    let version = json_string(&json, "version");
    let (game_versions, dependencies) = fabric_all_deps(&json);
    Some(ArchiveMetadata {
        name,
        version,
        game_versions,
        dependencies,
        ..ArchiveMetadata::default()
    })
}

fn from_quilt(text: &str) -> Option<ArchiveMetadata> {
    let json: serde_json::Value = serde_json::from_str(text).ok()?;
    let Some(loader) = json.get("quilt_loader") else {
        return from_fabric(text);
    };
    if !loader.is_object() {
        return None;
    }
    let name = loader
        .get("metadata")
        .and_then(|meta| json_string(meta, "name"))
        .or_else(|| json_string(loader, "id"));
    let version = json_string(loader, "version");
    let (game_versions, dependencies) = quilt_all_deps(loader);
    Some(ArchiveMetadata {
        name,
        version,
        game_versions,
        dependencies,
        ..ArchiveMetadata::default()
    })
}

fn fabric_all_deps(json: &serde_json::Value) -> (Vec<String>, Vec<DeclaredDependency>) {
    let mut game_versions = Vec::new();
    let mut dependencies = Vec::new();
    take_fabric_map(
        json.get("depends"),
        DependencyKind::Required,
        &mut game_versions,
        &mut dependencies,
    );
    take_fabric_map(
        json.get("recommends"),
        DependencyKind::Optional,
        &mut game_versions,
        &mut dependencies,
    );
    take_fabric_map(
        json.get("suggests"),
        DependencyKind::Optional,
        &mut game_versions,
        &mut dependencies,
    );
    take_fabric_map(
        json.get("breaks"),
        DependencyKind::Incompatible,
        &mut game_versions,
        &mut dependencies,
    );
    take_fabric_map(
        json.get("conflicts"),
        DependencyKind::Incompatible,
        &mut game_versions,
        &mut dependencies,
    );
    (game_versions, dependencies)
}

fn take_fabric_map(
    value: Option<&serde_json::Value>,
    kind: DependencyKind,
    game_versions: &mut Vec<String>,
    dependencies: &mut Vec<DeclaredDependency>,
) {
    let Some(map) = value.and_then(|v| v.as_object()) else {
        return;
    };
    for (key, constraint) in map {
        if key.eq_ignore_ascii_case("minecraft") {
            if kind == DependencyKind::Required {
                extend_bounds(game_versions, constraint);
            }
        } else if !key.trim().is_empty() {
            push_unique_dep(
                dependencies,
                DeclaredDependency::with_kind(key.trim(), kind),
            );
        }
    }
}

fn quilt_all_deps(loader: &serde_json::Value) -> (Vec<String>, Vec<DeclaredDependency>) {
    let mut game_versions = Vec::new();
    let mut dependencies = Vec::new();
    take_quilt_list(
        loader.get("depends"),
        DependencyKind::Required,
        &mut game_versions,
        &mut dependencies,
    );
    take_quilt_list(
        loader.get("recommends"),
        DependencyKind::Optional,
        &mut game_versions,
        &mut dependencies,
    );
    take_quilt_list(
        loader.get("suggests"),
        DependencyKind::Optional,
        &mut game_versions,
        &mut dependencies,
    );
    take_quilt_list(
        loader.get("breaks"),
        DependencyKind::Incompatible,
        &mut game_versions,
        &mut dependencies,
    );
    take_quilt_list(
        loader.get("conflicts"),
        DependencyKind::Incompatible,
        &mut game_versions,
        &mut dependencies,
    );
    if game_versions.is_empty() && dependencies.is_empty() {
        if let Some(serde_json::Value::Object(_)) = loader.get("depends") {
            return fabric_all_deps(loader);
        }
    }
    (game_versions, dependencies)
}

fn take_quilt_list(
    value: Option<&serde_json::Value>,
    kind: DependencyKind,
    game_versions: &mut Vec<String>,
    dependencies: &mut Vec<DeclaredDependency>,
) {
    match value {
        Some(serde_json::Value::Array(items)) => {
            for item in items {
                let Some(id) = json_string(item, "id") else {
                    continue;
                };
                if id.eq_ignore_ascii_case("minecraft") {
                    if kind == DependencyKind::Required {
                        let versions = item.get("versions").or_else(|| item.get("version"));
                        if let Some(versions) = versions {
                            extend_bounds(game_versions, versions);
                        }
                    }
                } else {
                    push_unique_dep(dependencies, DeclaredDependency::with_kind(id, kind));
                }
            }
        }
        Some(serde_json::Value::Object(_)) => {
            take_fabric_map(value, kind, game_versions, dependencies);
        }
        _ => {}
    }
}

fn extend_bounds(slot: &mut Vec<String>, value: &serde_json::Value) {
    match value {
        serde_json::Value::String(text) => {
            for version in constraint_bounds(text) {
                push_unique(slot, version);
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                extend_bounds(slot, item);
            }
        }
        _ => {}
    }
}

fn from_toml(text: &str) -> Option<ArchiveMetadata> {
    let doc: toml::Value = toml::from_str(text).ok()?;
    let mut name = None;
    let mut version = None;
    let own = toml_own_ids(text);
    if let Some(mods) = doc.get("mods").and_then(|v| v.as_array()) {
        if let Some(first) = mods.first().and_then(|v| v.as_table()) {
            name = toml_string(first, "displayName")
                .or_else(|| toml_string(first, "display_name"))
                .or_else(|| toml_string(first, "modId"))
                .or_else(|| toml_string(first, "modid"));
            version = toml_string(first, "version");
        }
    }
    let (game_versions, dependencies) = toml_dependencies(doc.get("dependencies"), &own);
    if name.is_none() && version.is_none() && game_versions.is_empty() && dependencies.is_empty() {
        return None;
    }
    Some(ArchiveMetadata {
        name,
        version,
        game_versions,
        dependencies,
        ..ArchiveMetadata::default()
    })
}

/// Only `[[dependencies.<ownId>]]` blocks — other mods' tables in the same
/// file (jar-in-jar companions) are ignored.
fn toml_dependencies(
    value: Option<&toml::Value>,
    own_ids: &[String],
) -> (Vec<String>, Vec<DeclaredDependency>) {
    let Some(table) = value.and_then(|v| v.as_table()) else {
        return (Vec::new(), Vec::new());
    };
    let mut game_versions = Vec::new();
    let mut dependencies = Vec::new();
    for (owner, entries) in table {
        if !own_ids
            .iter()
            .any(|id| id.eq_ignore_ascii_case(owner.trim()))
        {
            continue;
        }
        match entries {
            toml::Value::Array(items) => {
                for item in items {
                    take_dep(item, &mut game_versions, &mut dependencies);
                }
            }
            toml::Value::Table(_) => take_dep(entries, &mut game_versions, &mut dependencies),
            _ => {}
        }
    }
    (game_versions, dependencies)
}

fn take_dep(
    value: &toml::Value,
    game_versions: &mut Vec<String>,
    dependencies: &mut Vec<DeclaredDependency>,
) {
    let Some(table) = value.as_table() else {
        return;
    };
    let Some(id) = toml_string(table, "modId").or_else(|| toml_string(table, "modid")) else {
        return;
    };
    if id.eq_ignore_ascii_case("minecraft") {
        let range = toml_string(table, "versionRange")
            .or_else(|| toml_string(table, "version"))
            .unwrap_or_default();
        for version in constraint_bounds(&range) {
            push_unique(game_versions, version);
        }
    } else {
        push_unique_dep(
            dependencies,
            DeclaredDependency::with_kind(id, toml_dep_kind(table)),
        );
    }
}

fn toml_dep_kind(table: &toml::map::Map<String, toml::Value>) -> DependencyKind {
    if let Some(kind) = toml_string(table, "type") {
        return match kind.to_ascii_lowercase().as_str() {
            "required" => DependencyKind::Required,
            "optional" => DependencyKind::Optional,
            "incompatible" => DependencyKind::Incompatible,
            "discouraged" => DependencyKind::Discouraged,
            _ => DependencyKind::Required,
        };
    }
    match table.get("mandatory").and_then(|value| value.as_bool()) {
        Some(true) => DependencyKind::Required,
        Some(false) => DependencyKind::Optional,
        None => DependencyKind::Required,
    }
}

fn json_string(value: &serde_json::Value, key: &str) -> Option<String> {
    let text = value.get(key)?.as_str()?.trim();
    if text.is_empty() {
        None
    } else {
        Some(text.to_string())
    }
}

fn toml_string(table: &toml::map::Map<String, toml::Value>, key: &str) -> Option<String> {
    let text = table.get(key)?.as_str()?.trim();
    if text.is_empty() {
        None
    } else {
        Some(text.to_string())
    }
}

fn clean_text(value: Option<String>) -> Option<String> {
    let text = value?.trim().to_string();
    if text.is_empty() {
        None
    } else {
        Some(text)
    }
}

fn clean_version(value: Option<String>) -> Option<String> {
    let text = clean_text(value)?;
    if text.contains('$') || text.contains('{') || text.contains('}') {
        None
    } else {
        Some(text)
    }
}

fn dedup_deps(values: Vec<DeclaredDependency>) -> Vec<DeclaredDependency> {
    let mut out = Vec::new();
    for dep in values {
        let trimmed = dep.id.trim();
        if trimmed.is_empty()
            || trimmed.eq_ignore_ascii_case("minecraft")
            || trimmed.eq_ignore_ascii_case("java")
        {
            continue;
        }
        push_unique_dep(
            &mut out,
            DeclaredDependency {
                id: trimmed.to_string(),
                kind: dep.kind,
            },
        );
    }
    out
}

fn push_unique_dep(slot: &mut Vec<DeclaredDependency>, dep: DeclaredDependency) {
    if slot
        .iter()
        .any(|seen| seen.id.eq_ignore_ascii_case(&dep.id))
    {
        return;
    }
    slot.push(dep);
}

/// Explicit bounds in a constraint. `>=1.20 <1.21` → `1.20`, `1.21`.
/// `1.20.x` keeps the written prefix `1.20`. A bare number with no dot
/// (`[4,)`) is a loader range, not a Minecraft version.
fn constraint_bounds(text: &str) -> Vec<String> {
    let bytes = text.as_bytes();
    let mut i = 0;
    let mut out = Vec::new();
    while i < bytes.len() {
        if bytes[i].is_ascii_digit() {
            let start = i;
            i += 1;
            while i < bytes.len() && (bytes[i].is_ascii_digit() || bytes[i] == b'.') {
                i += 1;
            }
            if i < bytes.len()
                && matches!(bytes[i], b'x' | b'X' | b'*')
                && i > start
                && bytes[i - 1] == b'.'
            {
                i += 1;
            }
            let token = text[start..i].trim_end_matches(['x', 'X', '*', '.']);
            if let Some(version) = dotted_version(token) {
                push_unique(&mut out, version);
            }
        } else {
            i += 1;
        }
    }
    out
}

fn dotted_version(token: &str) -> Option<String> {
    if token.is_empty() {
        return None;
    }
    let mut count = 0;
    for part in token.split('.') {
        if part.is_empty() || !part.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        count += 1;
    }
    if (2..=4).contains(&count) {
        Some(token.to_string())
    } else {
        None
    }
}

/// Minecraft-shaped tokens in a jar name (`1.12.2`, `1.16.5`, `mc1.20.1`).
/// A mod version like `1.2.3` or `7.6.1` does not match: the minor has to
/// look like a Minecraft release (1.7 through 1.30).
fn game_versions_in_name(file_name: &str) -> Vec<String> {
    let name = Path::new(file_name)
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| file_name.to_string());
    let name = if name.ends_with(".disabled") {
        name.trim_end_matches(".disabled").to_string()
    } else {
        name
    };
    let stem = name.trim_end_matches(".jar").trim_end_matches(".zip");
    let mut out = Vec::new();
    for token in stem.split(['-', '_', '+', ' ']) {
        if let Some(version) = minecraft_token(token) {
            if !out.iter().any(|seen: &String| seen == &version) {
                out.push(version);
            }
        }
    }
    out
}

fn minecraft_token(token: &str) -> Option<String> {
    let (body, forced) = if let Some(rest) = token
        .strip_prefix("mc")
        .or_else(|| token.strip_prefix("MC"))
    {
        (rest, true)
    } else {
        (token, false)
    };
    if forced {
        return dotted_version(body)
            .filter(|version| (2..=4).contains(&version.split('.').count()));
    }
    let mut parts = body.split('.');
    let major = parts.next()?;
    let minor = parts.next()?;
    let patch = parts.next();
    if parts.next().is_some() || major != "1" {
        return None;
    }
    if !minor.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let minor_n: u32 = minor.parse().ok()?;
    if !(7..=30).contains(&minor_n) {
        return None;
    }
    match patch {
        None => Some(format!("1.{minor}")),
        Some(patch)
            if !patch.is_empty()
                && patch.len() <= 2
                && patch.bytes().all(|b| b.is_ascii_digit()) =>
        {
            Some(format!("1.{minor}.{patch}"))
        }
        Some(_) => None,
    }
}

/// What a jar says that can feed a category, without becoming a stored label.
///
/// `mod_ids` and `names` are the manifest's own words. `library_badge` is
/// ModMenu's `custom.modmenu.badges` entry `library`. `description` is free
/// text — callers should only trust an unambiguous keyword in it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CategoryClues {
    pub mod_ids: Vec<String>,
    pub names: Vec<String>,
    pub description: Option<String>,
    pub library_badge: bool,
}

impl CategoryClues {
    pub fn absorb(&mut self, other: Self) {
        for id in other.mod_ids {
            push_unique(&mut self.mod_ids, id);
        }
        for name in other.names {
            push_unique(&mut self.names, name);
        }
        if self.description.is_none() {
            self.description = other.description;
        }
        self.library_badge |= other.library_badge;
    }
}

/// Read category clues from a jar or from a folder that has a manifest at
/// its root. Empty when the path says nothing.
#[must_use]
pub fn category_clues(path: &Path) -> CategoryClues {
    if path.is_dir() {
        return clues_from_dir(path);
    }
    if !path.is_file() {
        return CategoryClues::default();
    }
    let Ok(file) = File::open(path) else {
        return CategoryClues::default();
    };
    let Ok(mut archive) = ZipArchive::new(file) else {
        return CategoryClues::default();
    };
    if let Some(text) = entry_text(&mut archive, "fabric.mod.json") {
        if let Some(clues) = clues_from_fabric(&text) {
            return clues;
        }
    }
    if let Some(text) = entry_text(&mut archive, "quilt.mod.json") {
        if let Some(clues) = clues_from_quilt(&text) {
            return clues;
        }
    }
    for name in ["META-INF/neoforge.mods.toml", "META-INF/mods.toml"] {
        if let Some(text) = entry_text(&mut archive, name) {
            if let Some(clues) = clues_from_toml(&text) {
                return clues;
            }
        }
    }
    CategoryClues::default()
}

fn clues_from_dir(dir: &Path) -> CategoryClues {
    if let Some(text) = read_capped(&dir.join("fabric.mod.json")) {
        if let Some(clues) = clues_from_fabric(&text) {
            return clues;
        }
    }
    if let Some(text) = read_capped(&dir.join("quilt.mod.json")) {
        if let Some(clues) = clues_from_quilt(&text) {
            return clues;
        }
    }
    for name in ["META-INF/neoforge.mods.toml", "META-INF/mods.toml"] {
        if let Some(text) = read_capped(&dir.join(name)) {
            if let Some(clues) = clues_from_toml(&text) {
                return clues;
            }
        }
    }
    CategoryClues::default()
}

fn read_capped(path: &Path) -> Option<String> {
    let file = File::open(path).ok()?;
    let mut buf = String::new();
    file.take(TEXT_CAP).read_to_string(&mut buf).ok()?;
    if buf.is_empty() {
        None
    } else {
        Some(buf)
    }
}

fn clues_from_fabric(text: &str) -> Option<CategoryClues> {
    let json: serde_json::Value = serde_json::from_str(text).ok()?;
    if !json.is_object() {
        return None;
    }
    let mut clues = CategoryClues::default();
    if let Some(id) = json_string(&json, "id") {
        clues.mod_ids.push(id);
    }
    if let Some(name) = json_string(&json, "name") {
        clues.names.push(name);
    }
    clues.description = json_string(&json, "description");
    clues.library_badge = fabric_library_badge(&json);
    if clues.mod_ids.is_empty()
        && clues.names.is_empty()
        && clues.description.is_none()
        && !clues.library_badge
    {
        None
    } else {
        Some(clues)
    }
}

fn clues_from_quilt(text: &str) -> Option<CategoryClues> {
    let json: serde_json::Value = serde_json::from_str(text).ok()?;
    let Some(loader) = json.get("quilt_loader") else {
        return clues_from_fabric(text);
    };
    let mut clues = CategoryClues::default();
    if let Some(id) = json_string(loader, "id") {
        clues.mod_ids.push(id);
    }
    if let Some(name) = loader
        .get("metadata")
        .and_then(|meta| json_string(meta, "name"))
    {
        clues.names.push(name);
    }
    clues.description = loader
        .get("metadata")
        .and_then(|meta| json_string(meta, "description"));
    if clues.mod_ids.is_empty() && clues.names.is_empty() && clues.description.is_none() {
        None
    } else {
        Some(clues)
    }
}

fn fabric_library_badge(json: &serde_json::Value) -> bool {
    json.get("custom")
        .and_then(|value| value.get("modmenu"))
        .and_then(|value| value.get("badges"))
        .and_then(|value| value.as_array())
        .is_some_and(|badges| {
            badges.iter().any(|badge| {
                badge
                    .as_str()
                    .is_some_and(|text| text.eq_ignore_ascii_case("library"))
            })
        })
}

fn clues_from_toml(text: &str) -> Option<CategoryClues> {
    let doc: toml::Value = toml::from_str(text).ok()?;
    let mut clues = CategoryClues::default();
    if let Some(mods) = doc.get("mods").and_then(|value| value.as_array()) {
        for item in mods {
            let Some(table) = item.as_table() else {
                continue;
            };
            if let Some(id) = toml_string(table, "modId").or_else(|| toml_string(table, "modid")) {
                push_unique(&mut clues.mod_ids, id);
            }
            if let Some(name) =
                toml_string(table, "displayName").or_else(|| toml_string(table, "display_name"))
            {
                push_unique(&mut clues.names, name);
            }
            if clues.description.is_none() {
                clues.description = toml_string(table, "description");
            }
        }
    }
    if clues.mod_ids.is_empty() && clues.names.is_empty() && clues.description.is_none() {
        None
    } else {
        Some(clues)
    }
}

fn push_unique(slot: &mut Vec<String>, value: String) {
    if slot.iter().any(|seen| seen.eq_ignore_ascii_case(&value)) {
        return;
    }
    slot.push(value);
}

fn entry_text<R: Read + Seek>(archive: &mut ZipArchive<R>, name: &str) -> Option<String> {
    let entry = archive.by_name(name).ok()?;
    if entry.is_dir() {
        return None;
    }
    let mut buf = String::new();
    entry.take(TEXT_CAP).read_to_string(&mut buf).ok()?;
    if buf.is_empty() {
        None
    } else {
        Some(buf)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Cursor, Write};

    fn zip_bytes(entries: &[(&str, &str)]) -> Vec<u8> {
        let mut cursor = Cursor::new(Vec::new());
        let mut zip = zip::ZipWriter::new(&mut cursor);
        for (name, body) in entries {
            zip.start_file(*name, zip::write::SimpleFileOptions::default())
                .unwrap();
            zip.write_all(body.as_bytes()).unwrap();
        }
        zip.finish().unwrap();
        cursor.into_inner()
    }

    fn read_bytes(file_name: &str, bytes: &[u8]) -> ArchiveMetadata {
        let mut archive = ZipArchive::new(Cursor::new(bytes)).unwrap();
        read_zip(&mut archive, file_name)
    }

    #[test]
    fn two_jars_declare_two_different_game_versions() {
        let fabric = zip_bytes(&[(
            "fabric.mod.json",
            r#"{"schemaVersion":1,"id":"example","name":"Example","version":"1.0.0","depends":{"minecraft":"1.12.2","fabricloader":">=0.12.0","cloth":"*"}}"#,
        )]);
        let forge = zip_bytes(&[(
            "META-INF/mods.toml",
            "[[mods]]\nmodId=\"jei\"\nversion=\"7.6.1\"\ndisplayName=\"Just Enough Items\"\n\n[[dependencies.jei]]\nmodId=\"minecraft\"\nversionRange=\"[1.16.5,1.17)\"\n\n[[dependencies.jei]]\nmodId=\"forge\"\nmandatory=true\n",
        )]);

        let old = read_bytes("example-1.12.2.jar", &fabric);
        let new = read_bytes("jei-1.16.5.jar", &forge);

        assert_eq!(old.game_versions, vec!["1.12.2".to_string()]);
        assert_eq!(
            new.game_versions,
            vec!["1.16.5".to_string(), "1.17".to_string()]
        );
        assert_ne!(old.game_versions, new.game_versions);
        assert_eq!(new.loaders, vec!["forge".to_string()]);
        assert_eq!(old.loaders, vec!["fabric".to_string()]);
        assert_eq!(old.name.as_deref(), Some("Example"));
        assert_eq!(old.version.as_deref(), Some("1.0.0"));
        assert!(old
            .dependencies
            .iter()
            .any(|dep| dep.id == "fabricloader" && dep.kind == DependencyKind::Required));
        assert!(old
            .dependencies
            .iter()
            .any(|dep| dep.id == "cloth" && dep.kind == DependencyKind::Required));
        assert!(!old.dependencies.iter().any(|dep| dep.id == "minecraft"));
        assert_eq!(new.name.as_deref(), Some("Just Enough Items"));
        assert_eq!(new.version.as_deref(), Some("7.6.1"));
        assert_eq!(
            new.dependencies,
            vec![DeclaredDependency::required("forge")]
        );
        assert!(old.category.is_none());
        assert!(new.category.is_none());
    }

    #[test]
    fn a_jar_name_supplies_the_game_version_when_the_manifest_does_not() {
        let bare = zip_bytes(&[("META-INF/MANIFEST.MF", "Manifest-Version: 1.0\n")]);
        let old = read_bytes("widgets-1.12.2.jar", &bare);
        let new = read_bytes("widgets-1.16.5.jar", &bare);
        assert_eq!(old.game_versions, vec!["1.12.2".to_string()]);
        assert_eq!(new.game_versions, vec!["1.16.5".to_string()]);
        assert!(
            old.version.is_none(),
            "the game version is not a mod version"
        );
        assert!(old.name.is_none());
    }

    #[test]
    fn a_placeholder_version_is_not_stored() {
        let jar = zip_bytes(&[(
            "fabric.mod.json",
            r#"{"id":"example","name":"Example","version":"${file.jarVersion}","depends":{"minecraft":">=1.16.5 <1.17"}}"#,
        )]);
        let meta = read_bytes("example.jar", &jar);
        assert!(meta.version.is_none());
        assert_eq!(
            meta.game_versions,
            vec!["1.16.5".to_string(), "1.17".to_string()]
        );
    }

    #[test]
    fn a_placeholder_version_is_read_from_the_manifest() {
        let jar = zip_bytes(&[
            (
                "META-INF/mods.toml",
                "[[mods]]\nmodId=\"example\"\nversion=\"${file.jarVersion}\"\ndisplayName=\"Example\"\n\n[[dependencies.example]]\nmodId=\"minecraft\"\nversionRange=\"[1.20.1,1.21)\"\n",
            ),
            (
                "META-INF/MANIFEST.MF",
                "Manifest-Version: 1.0\nImplementation-Version: 4.2.0\n",
            ),
        ]);
        let meta = read_bytes("example.jar", &jar);
        assert_eq!(meta.version.as_deref(), Some("4.2.0"));
        assert_eq!(meta.loaders, vec!["forge".to_string()]);
        assert_eq!(
            meta.game_versions,
            vec!["1.20.1".to_string(), "1.21".to_string()]
        );
    }

    #[test]
    fn each_manifest_kind_names_its_loader_and_a_range_keeps_both_bounds() {
        let fabric = read_bytes(
            "fabric.jar",
            &zip_bytes(&[(
                "fabric.mod.json",
                r#"{"id":"sodium","name":"Sodium","version":"0.5.8","depends":{"minecraft":">=1.20 <1.21"}}"#,
            )]),
        );
        assert_eq!(fabric.loaders, vec!["fabric".to_string()]);
        assert_eq!(fabric.name.as_deref(), Some("Sodium"));
        assert_eq!(
            fabric.game_versions,
            vec!["1.20".to_string(), "1.21".to_string()]
        );

        let quilt = read_bytes(
            "quilt.jar",
            &zip_bytes(&[(
                "quilt.mod.json",
                r#"{"schema_version":1,"quilt_loader":{"id":"example","version":"1.2.3","metadata":{"name":"Example Quilt"},"depends":[{"id":"minecraft","versions":"~1.20.1"}]}}"#,
            )]),
        );
        assert_eq!(quilt.loaders, vec!["quilt".to_string()]);
        assert_eq!(quilt.name.as_deref(), Some("Example Quilt"));
        assert_eq!(quilt.version.as_deref(), Some("1.2.3"));
        assert_eq!(quilt.game_versions, vec!["1.20.1".to_string()]);

        let neo = read_bytes(
            "neo.jar",
            &zip_bytes(&[(
                "META-INF/neoforge.mods.toml",
                "[[mods]]\nmodId=\"jei\"\nversion=\"15.3.0.4\"\ndisplayName=\"Just Enough Items\"\n\n[[dependencies.jei]]\nmodId=\"minecraft\"\nversionRange=\"1.20.x\"\n",
            )]),
        );
        assert_eq!(neo.loaders, vec!["neoforge".to_string()]);
        assert_eq!(neo.game_versions, vec!["1.20".to_string()]);

        let both = read_bytes(
            "both.jar",
            &zip_bytes(&[
                (
                    "fabric.mod.json",
                    r#"{"id":"example","name":"Example","version":"1.0.0","depends":{"minecraft":["1.21","1.21.1"]}}"#,
                ),
                (
                    "quilt.mod.json",
                    r#"{"schema_version":1,"quilt_loader":{"id":"example","version":"1.0.0","metadata":{"name":"Example"}}}"#,
                ),
            ]),
        );
        assert_eq!(
            both.loaders,
            vec!["fabric".to_string(), "quilt".to_string()]
        );
        assert_eq!(
            both.game_versions,
            vec!["1.21".to_string(), "1.21.1".to_string()]
        );
    }

    #[test]
    fn the_adapter_reads_the_same_jar() {
        use crate::MinecraftAdapter;
        use modlab_core::game_adapter::GameAdapter;

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("example-1.16.5.jar");
        std::fs::write(
            &path,
            zip_bytes(&[(
                "META-INF/neoforge.mods.toml",
                "[[mods]]\nmodId=\"sodium\"\nversion=\"0.5.3\"\ndisplayName=\"Sodium\"\n\n[[dependencies.sodium]]\nmodId=\"minecraft\"\nversionRange=\"[1.16.5,1.17)\"\n",
            )]),
        )
        .unwrap();
        let meta = MinecraftAdapter
            .read_archive_metadata(&path)
            .expect("synthetic jar");
        assert_eq!(
            meta.game_versions,
            vec!["1.16.5".to_string(), "1.17".to_string()]
        );
        assert_eq!(meta.loaders, vec!["neoforge".to_string()]);
        assert_eq!(meta.version.as_deref(), Some("0.5.3"));
        assert_eq!(meta.name.as_deref(), Some("Sodium"));
    }

    /// Xaero Minimap-shaped toml: own optional deps kept, other modId's
    /// required `xaerolib` block ignored.
    #[test]
    fn xaero_like_toml_keeps_own_optional_and_skips_other_mod_block() {
        let toml = r#"
[[mods]]
modId="xaerominimap"
version="26.6.2"
displayName="Xaero's Minimap"

[[dependencies.xaerominimap]]
modId="neoforge"
type="required"
versionRange="[21,)"

[[dependencies.xaerominimap]]
modId="minecraft"
type="required"
versionRange="[1.21,)"

[[dependencies.xaerominimap]]
modId="openpartiesandclaims"
type="optional"

[[dependencies.xaerominimap]]
modId="xaeroworldmap"
type="optional"

[[dependencies.xaerobetterpvp]]
modId="xaerolib"
type="required"
"#;
        let jar = zip_bytes(&[("META-INF/neoforge.mods.toml", toml)]);
        let meta = read_bytes("xaerominimap.jar", &jar);
        let ids: Vec<&str> = meta.dependencies.iter().map(|d| d.id.as_str()).collect();
        assert_eq!(
            ids,
            ["neoforge", "openpartiesandclaims", "xaeroworldmap"]
        );
        assert!(!meta.dependencies.iter().any(|d| d.id == "xaerolib"));
        assert_eq!(
            meta.dependencies
                .iter()
                .find(|d| d.id == "neoforge")
                .map(|d| d.kind),
            Some(DependencyKind::Required)
        );
        assert_eq!(
            meta.dependencies
                .iter()
                .find(|d| d.id == "xaeroworldmap")
                .map(|d| d.kind),
            Some(DependencyKind::Optional)
        );
        assert_eq!(meta.game_versions, vec!["1.21".to_string()]);
    }

    #[test]
    fn own_ids_include_jarjar_nested_mod() {
        let nested = zip_bytes(&[(
            "META-INF/neoforge.mods.toml",
            "[[mods]]\nmodId=\"xaerolib\"\nversion=\"1.8.5\"\ndisplayName=\"XaeroLib\"\n",
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
                b"[[mods]]\nmodId=\"xaerominimap\"\nversion=\"1\"\ndisplayName=\"Minimap\"\n",
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
        let bytes = cursor.into_inner();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("minimap.jar");
        std::fs::write(&path, &bytes).unwrap();
        let ids = own_ids(&path);
        assert!(ids.iter().any(|id| id == "xaerominimap"));
        assert!(ids.iter().any(|id| id == "xaerolib"));
    }

    #[test]
    fn fabric_suggests_are_optional() {
        let jar = zip_bytes(&[(
            "fabric.mod.json",
            r#"{"id":"example","name":"Example","version":"1.0.0","depends":{"minecraft":"1.20.1","fabricloader":"*"},"suggests":{"modmenu":"*"},"breaks":{"optifabric":"*"}}"#,
        )]);
        let meta = read_bytes("example.jar", &jar);
        assert!(meta
            .dependencies
            .iter()
            .any(|d| d.id == "fabricloader" && d.kind == DependencyKind::Required));
        assert!(meta
            .dependencies
            .iter()
            .any(|d| d.id == "modmenu" && d.kind == DependencyKind::Optional));
        assert!(meta
            .dependencies
            .iter()
            .any(|d| d.id == "optifabric" && d.kind == DependencyKind::Incompatible));
    }
}
