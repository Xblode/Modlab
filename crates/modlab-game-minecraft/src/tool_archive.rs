//! Whether a mod row *is* Fabric Loader, Forge, or NeoForge.
//!
//! This is not [`crate::instance::read_jar`]. That function says which loader
//! a mod was built for (`fabric.mod.json`, `mods.toml`). Almost every mod
//! jar would match, and a loader flag on all of them is useless. Here the
//! jar has to be the loader or its installer.
//!
//! # Filename (no disk read)
//!
//! The stem is the file name, lowercased, with spaces and underscores turned
//! into hyphens. `.jar` / `.zip` / `.7z` and a trailing `.disabled` are
//! removed. A download that has not landed yet is classified from this
//! alone.
//!
//! * **Fabric** — stem is `fabric-loader` or `fabric-loader-<version>`.
//!   `fabric-api`, `fabric-language-kotlin`, and `fabric-installer` do not
//!   match. Fabric API is a mod that *depends* on the loader.
//! * **Forge** — stem starts with the official artifact
//!   `forge-<mc>-<forgeVersion>` (optional `-installer`, `-universal`,
//!   `-client`, `-server`, `-shim`), or a hyphen/dot token is exactly
//!   `minecraftforge`. `jei-1.20.1-forge-15.2.0` is a mod *for* Forge: it
//!   does not start with `forge-<mc>-<forgeVersion>`.
//! * **NeoForge** — stem is `neoforge` or `neoforge-<version>`, optional
//!   `-installer` / `-loader`. `sodium-neoforge-0.6.0` is a mod *for*
//!   NeoForge. An MDK (`*-mdk`) is a development kit, not a loader.
//!
//! A version is the token in that name. Nothing is filled in when the name
//! has none.
//!
//! # Jar contents (light zip read, nothing extracted)
//!
//! When a `.jar` or `.zip` is actually on disk, the central directory is
//! read in memory. Only a few small entries are decoded
//! (`fabric.mod.json`, `META-INF/MANIFEST.MF`, `META-INF/mods.toml`,
//! `META-INF/neoforge.mods.toml`), and only if those are not enough are
//! entry *names* scanned for loader packages. Nested jars are not opened.
//!
//! * **Fabric Loader** — `fabric.mod.json` `"id"` is `fabricloader`, or the
//!   jar contains `net/fabricmc/loader/`. Any other `fabric.mod.json`
//!   (Fabric API's id is `fabric-api`) is an ordinary mod, even if the
//!   file was named `fabric-loader-…`.
//! * **Forge** — `net/minecraftforge/installer/`, a `Main-Class` under
//!   `net.minecraftforge.installer`, or `net/minecraftforge/fml/` together
//!   with a `[[mods]]` `modId` of `forge`. A `mods.toml` whose own mod id
//!   is anything else is a Forge *mod*. A `modId = "forge"` under
//!   `[[dependencies.*]]` is that mod's dependency, not the loader.
//! * **NeoForge** — `net/neoforged/installer/`, `net/neoforged/fancymodloader/`,
//!   `net/neoforged/neoforge/`, `net/neoforged/fml/`, or a NeoForge installer
//!   `Main-Class`. `neoforge.mods.toml` alone is how a NeoForge *mod*
//!   declares itself; it is not the loader.
//!
//! Content wins over a contradictory file name, both ways: an opaque
//! `download.jar` that contains the loader is still the loader, and a
//! file named like the loader whose manifest is a normal mod is not.
//!
//! The version is the one in the file name when that name was recognized.
//! Otherwise it is the loader's own manifest version
//! (`fabric.mod.json`, `Implementation-Version`, or the loader `[[mods]]`
//! version). A placeholder (`${…}`) is ignored. The row's stored version
//! is not consulted: that column is a different fact, and copying it
//! would invent a loader version the file never stated.
//!
//! Quilt is a real loader and is intentionally not flagged here.

use modlab_core::game_adapter::{ClassifiedTool, ToolClassifyQuery};
use std::fs::File;
use std::io::Read;
use std::path::Path;

const TEXT_CAP: u64 = 65_536;

const ROLES: &[&str] = &[
    "installer",
    "universal",
    "client",
    "server",
    "shim",
    "loader",
];

struct NameHit {
    tool: &'static str,
    version: Option<String>,
}

#[derive(Default)]
struct JarFacts {
    fabric_id: Option<String>,
    fabric_version: Option<String>,
    /// `modId` values from `[[mods]]` only — not from dependency tables.
    own_mod_ids: Vec<String>,
    /// Version on the `[[mods]]` entry whose id is `forge` or `neoforge`.
    loader_toml_version: Option<String>,
    main_class: Option<String>,
    impl_version: Option<String>,
    has_install_profile: bool,
    has_fabric_loader_pkg: bool,
    has_forge_installer_pkg: bool,
    has_forge_fml_pkg: bool,
    has_neoforge_installer_pkg: bool,
    has_neoforge_loader_pkg: bool,
}

/// `Some` when this row is Fabric Loader, Forge, or NeoForge.
#[must_use]
pub fn classify_tool(query: &ToolClassifyQuery<'_>) -> Option<ClassifiedTool> {
    let named = name_hit(&stems_of(query));
    let facts = inspect_first(&readable_archives(query));
    combine(named, facts)
}

fn combine(name: Option<NameHit>, facts: Option<JarFacts>) -> Option<ClassifiedTool> {
    if let Some(facts) = &facts {
        if let Some(tool) = content_tool(facts) {
            return Some(ClassifiedTool {
                tool: tool.to_string(),
                version: resolve_version(name.as_ref(), Some(facts), tool),
            });
        }
        // A readable jar that is an ordinary mod cancels a file name that
        // only looked like a loader (`fabric-loader.jar` whose id is
        // `fabric-api`, an official-looking name wrapped around `sodium`).
        if is_ordinary_mod(facts) {
            return None;
        }
    }
    let name = name?;
    Some(ClassifiedTool {
        tool: name.tool.to_string(),
        version: name.version,
    })
}

fn resolve_version(name: Option<&NameHit>, facts: Option<&JarFacts>, tool: &str) -> Option<String> {
    if let Some(hit) = name {
        if hit.tool == tool {
            if let Some(version) = &hit.version {
                return Some(version.clone());
            }
        }
    }
    facts.and_then(|facts| content_version(facts, tool))
}

fn content_version(facts: &JarFacts, tool: &str) -> Option<String> {
    match tool {
        "fabric" => usable_version(facts.fabric_version.clone())
            .or_else(|| usable_version(facts.impl_version.clone())),
        "forge" | "neoforge" => usable_version(facts.loader_toml_version.clone())
            .or_else(|| usable_version(facts.impl_version.clone())),
        _ => None,
    }
}

/// `${file.jarVersion}` and similar build tokens are not versions.
fn usable_version(raw: Option<String>) -> Option<String> {
    let version = raw?.trim().to_string();
    if version.is_empty() || version.contains('$') || version.contains('{') || version.contains('}')
    {
        None
    } else {
        Some(version)
    }
}

fn content_tool(facts: &JarFacts) -> Option<&'static str> {
    if facts.fabric_id.as_deref() == Some("fabricloader") || facts.has_fabric_loader_pkg {
        return Some("fabric");
    }
    if is_ordinary_mod(facts) {
        return None;
    }
    if facts.has_neoforge_installer_pkg
        || facts.has_neoforge_loader_pkg
        || main_contains(facts, "neoforged.installer")
    {
        return Some("neoforge");
    }
    if facts.has_forge_installer_pkg
        || main_contains(facts, "minecraftforge.installer")
        || (facts.has_forge_fml_pkg && has_own_id(facts, "forge"))
        || (facts.has_install_profile && main_contains(facts, "minecraftforge"))
    {
        return Some("forge");
    }
    None
}

fn is_ordinary_mod(facts: &JarFacts) -> bool {
    if let Some(id) = facts.fabric_id.as_deref() {
        return id != "fabricloader";
    }
    !facts.own_mod_ids.is_empty()
        && facts
            .own_mod_ids
            .iter()
            .all(|id| id != "forge" && id != "neoforge")
}

fn has_own_id(facts: &JarFacts, id: &str) -> bool {
    facts.own_mod_ids.iter().any(|own| own == id)
}

fn main_contains(facts: &JarFacts, needle: &str) -> bool {
    facts
        .main_class
        .as_deref()
        .is_some_and(|class| class.to_ascii_lowercase().contains(needle))
}

fn name_hit(stems: &[String]) -> Option<NameHit> {
    let mut first = None;
    for stem in stems {
        let Some(hit) = match_stem(stem) else {
            continue;
        };
        if hit.version.is_some() {
            return Some(hit);
        }
        if first.is_none() {
            first = Some(hit);
        }
    }
    first
}

fn match_stem(stem: &str) -> Option<NameHit> {
    if stem.is_empty() || stem.ends_with("-mdk") {
        return None;
    }
    if let Some(hit) = fabric_from_stem(stem) {
        return Some(hit);
    }
    if let Some(hit) = neoforge_from_stem(stem) {
        return Some(hit);
    }
    forge_from_stem(stem)
}

fn fabric_from_stem(stem: &str) -> Option<NameHit> {
    let rest = if stem == "fabric-loader" {
        ""
    } else {
        stem.strip_prefix("fabric-loader-")?
    };
    if rest.is_empty() {
        return Some(NameHit {
            tool: "fabric",
            version: None,
        });
    }
    let body = strip_trailing_role(rest);
    if !is_numeric_version(body) {
        // `fabric-loader-api`, `fabric-installer`, `fabric-language-kotlin`.
        return None;
    }
    Some(NameHit {
        tool: "fabric",
        version: Some(body.to_string()),
    })
}

fn neoforge_from_stem(stem: &str) -> Option<NameHit> {
    let rest = if stem == "neoforge" {
        ""
    } else {
        stem.strip_prefix("neoforge-")?
    };
    let version = neoforge_version(rest)?;
    Some(NameHit {
        tool: "neoforge",
        version,
    })
}

fn neoforge_version(rest: &str) -> Option<Option<String>> {
    if rest.is_empty() || rest == "installer" || rest == "loader" {
        return Some(None);
    }
    let body = strip_trailing_role(rest);
    if body.is_empty() || body == "loader" || body == "installer" {
        return Some(None);
    }
    if let Some(version) = body.strip_prefix("loader-") {
        return is_numeric_version(version).then(|| Some(version.to_string()));
    }
    if is_numeric_version(body) {
        return Some(Some(body.to_string()));
    }
    forge_build_version(body).map(Some)
}

fn forge_from_stem(stem: &str) -> Option<NameHit> {
    if has_token(stem, "minecraftforge") {
        return Some(NameHit {
            tool: "forge",
            version: first_numeric_pair(stem),
        });
    }
    let rest = stem.strip_prefix("forge-")?;
    let version = forge_version_from_rest(rest)?;
    Some(NameHit {
        tool: "forge",
        version,
    })
}

fn forge_version_from_rest(rest: &str) -> Option<Option<String>> {
    if rest == "installer" {
        return Some(None);
    }
    let body = strip_trailing_role(rest);
    if body.is_empty() {
        return None;
    }
    forge_build_version(body).map(Some)
}

/// `1.20.1-47.2.0`, or the old `1.7.10-10.13.4.1614-1.7.10` where the
/// Minecraft version is repeated after the Forge build.
fn forge_build_version(body: &str) -> Option<String> {
    let parts: Vec<&str> = body.split('-').filter(|part| !part.is_empty()).collect();
    if parts.len() < 2 || !is_numeric_version(parts[0]) || !is_numeric_version(parts[1]) {
        return None;
    }
    for extra in &parts[2..] {
        if *extra != parts[0] && !is_role(extra) {
            return None;
        }
    }
    Some(format!("{}-{}", parts[0], parts[1]))
}

fn first_numeric_pair(stem: &str) -> Option<String> {
    let parts: Vec<&str> = stem.split('-').filter(|part| !part.is_empty()).collect();
    parts.windows(2).find_map(|pair| {
        if is_numeric_version(pair[0]) && is_numeric_version(pair[1]) {
            Some(format!("{}-{}", pair[0], pair[1]))
        } else {
            None
        }
    })
}

fn strip_trailing_role(rest: &str) -> &str {
    let Some((body, role)) = rest.rsplit_once('-') else {
        return if is_role(rest) { "" } else { rest };
    };
    if is_role(role) {
        body
    } else {
        rest
    }
}

fn is_role(value: &str) -> bool {
    ROLES.contains(&value)
}

fn has_token(stem: &str, token: &str) -> bool {
    stem.split(['-', '.']).any(|part| part == token)
}

/// `1.20.1`, `47.2.0`, `14.23.5.2860`, `0.16.9+local`. A bare integer is
/// not enough — loader versions in these names always have a dot.
fn is_numeric_version(value: &str) -> bool {
    let (main, build) = match value.split_once('+') {
        Some((main, build)) => (main, Some(build)),
        None => (value, None),
    };
    if main.is_empty()
        || !main.starts_with(|c: char| c.is_ascii_digit())
        || main.starts_with('.')
        || main.ends_with('.')
        || main.contains("..")
        || !main.contains('.')
        || !main.chars().all(|c| c.is_ascii_digit() || c == '.')
    {
        return false;
    }
    build.is_none_or(|build| {
        !build.is_empty()
            && build
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_')
    })
}

fn stems_of(query: &ToolClassifyQuery<'_>) -> Vec<String> {
    let mut stems = Vec::new();
    push_stem(&mut stems, query.download_file_name);
    push_stem(&mut stems, query.name);
    if let Some(path) = query.archive_path {
        push_stem(&mut stems, &path.to_string_lossy());
    }
    push_stem(&mut stems, &query.source_path.to_string_lossy());
    stems
}

fn push_stem(stems: &mut Vec<String>, label: &str) {
    let stem = stem_of(label);
    if stem.is_empty() || stems.contains(&stem) {
        return;
    }
    stems.push(stem);
}

fn stem_of(label: &str) -> String {
    let trimmed = label.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    let normalized = trimmed.replace('\\', "/");
    let file_name = Path::new(&normalized)
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or(normalized);
    let mut lower = file_name.replace([' ', '_'], "-").to_ascii_lowercase();
    if let Some(stripped) = lower.strip_suffix(".disabled") {
        lower = stripped.to_string();
    }
    for ext in [".jar", ".zip", ".7z"] {
        if let Some(stripped) = lower.strip_suffix(ext) {
            lower = stripped.to_string();
            break;
        }
    }
    while lower.contains("--") {
        lower = lower.replace("--", "-");
    }
    lower
}

fn readable_archives<'a>(query: &'a ToolClassifyQuery<'a>) -> Vec<&'a Path> {
    let mut paths = Vec::new();
    push_archive(&mut paths, query.source_path);
    if let Some(path) = query.archive_path {
        push_archive(&mut paths, path);
    }
    paths
}

fn push_archive<'a>(paths: &mut Vec<&'a Path>, path: &'a Path) {
    if !path.is_file() {
        return;
    }
    let name = path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_ascii_lowercase();
    let archive =
        name.ends_with(".jar") || name.ends_with(".jar.disabled") || name.ends_with(".zip");
    if archive && !paths.contains(&path) {
        paths.push(path);
    }
}

fn inspect_first(paths: &[&Path]) -> Option<JarFacts> {
    paths.iter().find_map(|path| inspect_jar(path))
}

fn inspect_jar(path: &Path) -> Option<JarFacts> {
    let file = File::open(path).ok()?;
    let mut archive = zip::ZipArchive::new(file).ok()?;
    Some(read_facts(&mut archive))
}

fn read_facts(archive: &mut zip::ZipArchive<File>) -> JarFacts {
    let mut facts = JarFacts::default();
    if let Some(text) = entry_text(archive, "fabric.mod.json") {
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) {
            facts.fabric_id = json_string(&json, "id").map(|id| id.to_ascii_lowercase());
            facts.fabric_version = json_string(&json, "version");
        }
    }
    if let Some(text) = entry_text(archive, "META-INF/MANIFEST.MF") {
        let (main_class, impl_version) = manifest_fields(&text);
        facts.main_class = main_class;
        facts.impl_version = impl_version;
    }
    for entry in ["META-INF/neoforge.mods.toml", "META-INF/mods.toml"] {
        let Some(text) = entry_text(archive, entry) else {
            continue;
        };
        for (id, version) in own_mods(&text) {
            if (id == "forge" || id == "neoforge") && facts.loader_toml_version.is_none() {
                facts.loader_toml_version = version;
            }
            facts.own_mod_ids.push(id);
        }
    }
    facts.has_install_profile = archive.by_name("install_profile.json").is_ok();
    if !facts_decisive(&facts) {
        scan_packages(archive, &mut facts);
    }
    facts
}

fn facts_decisive(facts: &JarFacts) -> bool {
    facts.fabric_id.as_deref() == Some("fabricloader")
        || is_ordinary_mod(facts)
        || main_contains(facts, "neoforged.installer")
        || main_contains(facts, "minecraftforge.installer")
}

fn json_string(value: &serde_json::Value, key: &str) -> Option<String> {
    let text = value.get(key)?.as_str()?.trim();
    if text.is_empty() {
        None
    } else {
        Some(text.to_string())
    }
}

fn manifest_fields(text: &str) -> (Option<String>, Option<String>) {
    let mut main_class = None;
    let mut impl_version = None;
    for line in text.lines() {
        let line = line.trim_end();
        if line.starts_with(' ') || line.starts_with('\t') {
            continue;
        }
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        let value = value.trim();
        if value.is_empty() {
            continue;
        }
        match key.to_ascii_lowercase().as_str() {
            "main-class" => main_class = Some(value.to_string()),
            "implementation-version" => impl_version = Some(value.to_string()),
            _ => {}
        }
    }
    (main_class, impl_version)
}

/// `modId` / `version` inside `[[mods]]` only. Dependency tables repeat
/// `modId = "forge"` for every Forge mod and must not count.
fn own_mods(text: &str) -> Vec<(String, Option<String>)> {
    let mut out = Vec::new();
    let mut in_mods = false;
    let mut current_id = None;
    let mut current_version = None;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if trimmed.starts_with('[') {
            if in_mods {
                if let Some(id) = current_id.take() {
                    out.push((id, current_version.take()));
                }
            }
            in_mods = trimmed == "[[mods]]" || trimmed == "[mods]";
            current_id = None;
            current_version = None;
            continue;
        }
        if !in_mods {
            continue;
        }
        if current_id.is_none() {
            if let Some(id) = assignment(trimmed, "modId") {
                current_id = Some(id.to_ascii_lowercase());
            }
        }
        if current_version.is_none() {
            current_version = assignment(trimmed, "version");
        }
    }
    if in_mods {
        if let Some(id) = current_id {
            out.push((id, current_version));
        }
    }
    out
}

fn assignment(trimmed: &str, key: &str) -> Option<String> {
    let rest = trimmed.strip_prefix(key)?.trim_start();
    let rest = rest.strip_prefix('=')?.trim();
    let value = strip_inline_comment(rest);
    let value = unquote(value);
    if value.is_empty() {
        None
    } else {
        Some(value)
    }
}

fn strip_inline_comment(value: &str) -> &str {
    let mut in_single = false;
    let mut in_double = false;
    for (index, ch) in value.char_indices() {
        match ch {
            '\'' if !in_double => in_single = !in_single,
            '"' if !in_single => in_double = !in_double,
            '#' if !in_single && !in_double => return value[..index].trim(),
            _ => {}
        }
    }
    value.trim()
}

fn unquote(value: &str) -> String {
    let value = value.trim().trim_end_matches(',').trim();
    let bytes = value.as_bytes();
    if bytes.len() >= 2
        && (bytes[0] == b'"' || bytes[0] == b'\'')
        && bytes[0] == bytes[bytes.len() - 1]
    {
        value[1..value.len() - 1].to_string()
    } else {
        value.to_string()
    }
}

fn entry_text(archive: &mut zip::ZipArchive<File>, name: &str) -> Option<String> {
    let mut entry = archive.by_name(name).ok()?;
    if entry.is_dir() {
        return None;
    }
    let mut buf = String::new();
    (&mut entry).take(TEXT_CAP).read_to_string(&mut buf).ok()?;
    if buf.is_empty() {
        None
    } else {
        Some(buf)
    }
}

fn scan_packages(archive: &zip::ZipArchive<File>, facts: &mut JarFacts) {
    for name in archive.file_names() {
        if has_prefix(name, "net/fabricmc/loader/") {
            facts.has_fabric_loader_pkg = true;
        } else if has_prefix(name, "net/minecraftforge/installer/") {
            facts.has_forge_installer_pkg = true;
        } else if has_prefix(name, "net/minecraftforge/fml/") {
            facts.has_forge_fml_pkg = true;
        } else if has_prefix(name, "net/neoforged/installer/") {
            facts.has_neoforge_installer_pkg = true;
        } else if has_prefix(name, "net/neoforged/fancymodloader/")
            || has_prefix(name, "net/neoforged/neoforge/")
            || has_prefix(name, "net/neoforged/fml/")
        {
            facts.has_neoforge_loader_pkg = true;
        }
    }
}

fn has_prefix(name: &str, prefix: &str) -> bool {
    let name = name.trim_start_matches(['/', '\\']);
    if name.starts_with(prefix) {
        return true;
    }
    name.contains('\\')
        && name
            .replace('\\', "/")
            .trim_start_matches('/')
            .starts_with(prefix)
}

#[cfg(test)]
mod tests {
    use super::*;
    use modlab_core::game_adapter::GameAdapter;
    use std::io::Write;

    fn query_name(file_name: &str) -> ToolClassifyQuery<'_> {
        ToolClassifyQuery {
            name: file_name,
            source_path: Path::new(file_name),
            archive_path: None,
            stored_version: Some("9.9.9"),
            source_url: "",
            download_file_name: "",
        }
    }

    fn assert_loader(file_name: &str, tool: &str, version: Option<&str>) {
        let got = classify_tool(&query_name(file_name))
            .unwrap_or_else(|| panic!("{file_name} should be {tool}"));
        assert_eq!(got.tool, tool, "{file_name}");
        assert_eq!(got.version.as_deref(), version, "{file_name}");
        assert_ne!(got.version.as_deref(), Some("9.9.9"), "{file_name}");
    }

    fn assert_not_loader(file_name: &str) {
        let got = classify_tool(&query_name(file_name));
        assert!(got.is_none(), "{file_name} is not a loader, got {got:?}");
    }

    #[test]
    fn file_names_pick_the_loader_and_not_a_mod_that_targets_one() {
        assert_loader("fabric-loader-0.16.9.jar", "fabric", Some("0.16.9"));
        assert_loader(
            "downloads/Fabric-Loader-0.15.11.JAR",
            "fabric",
            Some("0.15.11"),
        );
        assert_loader(
            "fabric-loader-0.16.9+local.jar",
            "fabric",
            Some("0.16.9+local"),
        );
        assert_loader("fabric-loader.jar", "fabric", None);
        assert_loader(
            "forge-1.20.1-47.2.0-installer.jar",
            "forge",
            Some("1.20.1-47.2.0"),
        );
        assert_loader(
            "forge-1.12.2-14.23.5.2860-universal.jar",
            "forge",
            Some("1.12.2-14.23.5.2860"),
        );
        assert_loader(
            "forge-1.7.10-10.13.4.1614-1.7.10.jar",
            "forge",
            Some("1.7.10-10.13.4.1614"),
        );
        assert_loader("Forge Installer.jar", "forge", None);
        assert_loader(
            "minecraftforge-universal-1.7.10-10.13.4.1614-1.7.10.jar",
            "forge",
            Some("1.7.10-10.13.4.1614"),
        );
        assert_loader(
            "neoforge-21.1.172-installer.jar",
            "neoforge",
            Some("21.1.172"),
        );
        assert_loader("neoforge-21.1.172.jar", "neoforge", Some("21.1.172"));
        assert_loader("neoforge-loader-4.0.41.jar", "neoforge", Some("4.0.41"));

        assert_not_loader("fabric-api-0.100.0+1.21.1.jar");
        assert_not_loader("fabric-language-kotlin-1.10.17+kotlin.2.0.0.jar");
        assert_not_loader("fabric-installer-1.0.1.jar");
        assert_not_loader("jei-1.20.1-forge-15.2.0.27.jar");
        assert_not_loader("sodium-neoforge-0.6.0.jar");
        assert_not_loader("examplemod-1.2.3.jar");
        assert_not_loader("neoforge-21.1.172-mdk.zip");
        assert_not_loader("forge-1.20.1-47.2.0-mdk.zip");
        assert_not_loader("quilt-loader-0.26.4.jar");
        assert_not_loader("Minecraft Forge.jar");
    }

    #[test]
    fn a_download_in_flight_uses_the_capture_name_not_the_stored_version() {
        let missing = Path::new("C:/no/such/modlab-capture.bin");
        let versioned = classify_tool(&ToolClassifyQuery {
            name: "capture",
            source_path: missing,
            archive_path: None,
            stored_version: Some("9.9.9"),
            source_url: "https://cdn.example.invalid/abc",
            download_file_name: "neoforge-21.1.172-installer.jar",
        })
        .expect("installer name");
        assert_eq!(versioned.tool, "neoforge");
        assert_eq!(versioned.version.as_deref(), Some("21.1.172"));

        let bare = classify_tool(&ToolClassifyQuery {
            name: "capture",
            source_path: missing,
            archive_path: None,
            stored_version: Some("9.9.9"),
            source_url: "",
            download_file_name: "fabric-loader.jar",
        })
        .expect("loader name without a version");
        assert_eq!(bare.tool, "fabric");
        assert_eq!(bare.version, None);
    }

    fn jar_with(dir: &Path, file_name: &str, entries: &[(&str, &str)]) -> std::path::PathBuf {
        let path = dir.join(file_name);
        let file = File::create(&path).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        for (entry_name, contents) in entries {
            zip.start_file(*entry_name, zip::write::SimpleFileOptions::default())
                .unwrap();
            zip.write_all(contents.as_bytes()).unwrap();
        }
        zip.finish().unwrap();
        path
    }

    fn query_jar<'a>(name: &'a str, path: &'a Path) -> ToolClassifyQuery<'a> {
        ToolClassifyQuery {
            name,
            source_path: path,
            archive_path: Some(path),
            stored_version: Some("9.9.9"),
            source_url: "https://example.invalid/mod",
            download_file_name: "",
        }
    }

    #[test]
    fn jar_contents_separate_the_loader_from_a_mod() {
        let dir = tempfile::tempdir().unwrap();

        let api = jar_with(
            dir.path(),
            "fabric-loader-9.9.9.jar",
            &[(
                "fabric.mod.json",
                r#"{"schemaVersion":1,"id":"fabric-api","name":"Fabric API","version":"0.100.0"}"#,
            )],
        );
        assert!(
            classify_tool(&query_jar("fabric-loader-9.9.9.jar", &api)).is_none(),
            "Fabric API is not Fabric Loader, even under that file name"
        );

        let ordinary = jar_with(
            dir.path(),
            "capture.jar",
            &[
                (
                    "META-INF/mods.toml",
                    "modLoader=\"javafml\"\nloaderVersion=\"[47,)\"\n\n[[mods]]\nmodId=\"examplemod\"\nversion=\"1.2.3\"\n\n[[dependencies.examplemod]]\nmodId=\"forge\"\nmandatory=true\n",
                ),
                ("net/minecraftforge/fml/Fake.class", ""),
            ],
        );
        assert!(
            classify_tool(&query_jar("capture.jar", &ordinary)).is_none(),
            "a Forge mod that depends on Forge is not the loader"
        );

        let neoforge_mod = jar_with(
            dir.path(),
            "neoforge-21.1.172.jar",
            &[(
                "META-INF/neoforge.mods.toml",
                "[[mods]]\nmodId=\"sodium\"\nversion=\"0.6.0\"\n",
            )],
        );
        assert!(
            classify_tool(&query_jar("neoforge-21.1.172.jar", &neoforge_mod)).is_none(),
            "a NeoForge mod toml is not the NeoForge loader"
        );

        let fabric = jar_with(
            dir.path(),
            "download.jar",
            &[(
                "fabric.mod.json",
                r#"{"schemaVersion":1,"id":"fabricloader","name":"Fabric Loader","version":"0.16.9"}"#,
            )],
        );
        let mark = classify_tool(&query_jar("download.jar", &fabric)).expect("fabric loader");
        assert_eq!(mark.tool, "fabric");
        assert_eq!(mark.version.as_deref(), Some("0.16.9"));

        let named = jar_with(
            dir.path(),
            "fabric-loader-0.16.9.jar",
            &[(
                "fabric.mod.json",
                r#"{"schemaVersion":1,"id":"fabricloader","version":"0.0.1"}"#,
            )],
        );
        let mark = classify_tool(&query_jar("fabric-loader-0.16.9.jar", &named)).expect("named");
        assert_eq!(mark.tool, "fabric");
        assert_eq!(mark.version.as_deref(), Some("0.16.9"));

        let placeholder = jar_with(
            dir.path(),
            "fabric-loader.jar",
            &[
                (
                    "fabric.mod.json",
                    r#"{"id":"fabricloader","version":"${version}"}"#,
                ),
                (
                    "META-INF/MANIFEST.MF",
                    "Implementation-Version: 0.15.11\r\n",
                ),
            ],
        );
        let mark = classify_tool(&query_jar("fabric-loader.jar", &placeholder)).expect("manifest");
        assert_eq!(mark.tool, "fabric");
        assert_eq!(mark.version.as_deref(), Some("0.15.11"));

        let no_version = jar_with(
            dir.path(),
            "fabric-loader.jar",
            &[(
                "fabric.mod.json",
                r#"{"id":"fabricloader","version":"${file.jarVersion}"}"#,
            )],
        );
        let mark = classify_tool(&query_jar("fabric-loader.jar", &no_version)).expect("no version");
        assert_eq!(mark.tool, "fabric");
        assert_eq!(mark.version, None);

        let installer = jar_with(
            dir.path(),
            "Minecraft Forge.jar",
            &[(
                "META-INF/MANIFEST.MF",
                "Main-Class: net.minecraftforge.installer.SimpleInstaller\r\nImplementation-Version: 47.2.0\r\n",
            )],
        );
        let mark = classify_tool(&query_jar("Minecraft Forge.jar", &installer)).expect("installer");
        assert_eq!(mark.tool, "forge");
        assert_eq!(mark.version.as_deref(), Some("47.2.0"));

        let joined = jar_with(
            dir.path(),
            "joined.jar",
            &[
                (
                    "META-INF/mods.toml",
                    "[[mods]]\nmodId=\"forge\"\nversion=\"${file.jarVersion}\"\n",
                ),
                ("META-INF/MANIFEST.MF", "Implementation-Version: 47.2.0\r\n"),
                ("net/minecraftforge/fml/loading/FMLLoader.class", ""),
            ],
        );
        let mark = classify_tool(&query_jar("joined.jar", &joined)).expect("forge jar");
        assert_eq!(mark.tool, "forge");
        assert_eq!(mark.version.as_deref(), Some("47.2.0"));

        let fancy = jar_with(
            dir.path(),
            "loader-4.0.41.jar",
            &[
                ("META-INF/MANIFEST.MF", "Implementation-Version: 4.0.41\r\n"),
                ("net/neoforged/fancymodloader/impl/Loader.class", ""),
            ],
        );
        let mark = classify_tool(&query_jar("loader-4.0.41.jar", &fancy)).expect("fancymodloader");
        assert_eq!(mark.tool, "neoforge");
        assert_eq!(mark.version.as_deref(), Some("4.0.41"));
    }

    #[test]
    fn the_adapter_forwards_classify_tool() {
        let mark = crate::MinecraftAdapter
            .classify_tool(&query_name("fabric-loader-0.16.9.jar"))
            .expect("fabric loader");
        assert_eq!(mark.tool, "fabric");
        assert_eq!(mark.version.as_deref(), Some("0.16.9"));
    }
}
