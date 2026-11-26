//! Decide whether a mod row is SKSE. `classify_mod` is the only answer the
//! shell should forward: a loader exe at the top of an installed folder, or
//! `recognize` on a name / Nexus title / silverlock URL for an archive that
//! is not installed yet (including a download still in flight).
//!
//! Both signals read `NAME_RULES`. Address Library, Engine Fixes, an SKSE
//! plugin and a preloader do not match. This module does not install anything.
//! Copying the SE loader beside the game uses `SE_LOADER_FILE` from that same
//! table (`engine::LOADER_FILE`).
//!
//! There is no tool catalogue here. LOOT is an external program, not a mod
//! archive; only its official page is listed, so the shell can open it.

use std::path::Path;

/// A captured file this adapter is willing to file under a stable mod name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecognizedTool {
    /// Stable id (`"skse"`, `"sksevr"`). Not a display string.
    pub id: &'static str,
    /// What the mods table should show.
    pub display_name: &'static str,
    /// Parsed from the archive name when the silverlock pattern is there
    /// (`skse64_2_02_06` → `2.2.6`). `None` when the name has no version.
    pub version: Option<String>,
}

/// A page the shell may open in the system browser. Not a download.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OfficialToolPage {
    pub id: &'static str,
    pub name: &'static str,
    pub url: &'static str,
}

/// Pages the UI should open for Skyrim's script extender and load-order tool.
/// ModLab never downloads these itself.
pub const OFFICIAL_TOOL_PAGES: &[OfficialToolPage] = &[
    OfficialToolPage {
        id: "skse",
        name: "SKSE",
        url: "https://skse.silverlock.org/",
    },
    OfficialToolPage {
        id: "loot",
        name: "LOOT",
        url: "https://loot.github.io/",
    },
];

const SKSE_HOST: &str = "skse.silverlock.org";

/// One silverlock build. Longest `prefix` first, so `skse64` / `sksevr` are
/// tried before `skse_`. `loader` is the exe at the top of an extracted
/// folder. `wire` is the mod-row value (`"skse"` or `"skse-vr"`).
struct NameRule {
    prefix: &'static str,
    /// Stable id `recognize` returns (`"skse"`, `"sksevr"`).
    id: &'static str,
    wire: &'static str,
    display_name: &'static str,
    loader: &'static str,
}

const NAME_RULES: [NameRule; 3] = [
    NameRule {
        prefix: "skse64",
        id: "skse",
        wire: "skse",
        display_name: "SKSE",
        loader: "skse64_loader.exe",
    },
    NameRule {
        prefix: "sksevr",
        id: "sksevr",
        wire: "skse-vr",
        display_name: "SKSE VR",
        loader: "sksevr_loader.exe",
    },
    NameRule {
        prefix: "skse_",
        id: "skse",
        wire: "skse",
        display_name: "SKSE",
        loader: "skse_loader.exe",
    },
];

/// SE/AE loader. `engine::LOADER_FILE` is this same string.
pub const SE_LOADER_FILE: &str = NAME_RULES[0].loader;

/// SE/AE file prefix (`skse64`). Root files copied beside the game are
/// `{SE_NAME_PREFIX}_*.dll` and `{SE_NAME_PREFIX}_*.exe`.
pub const SE_NAME_PREFIX: &str = NAME_RULES[0].prefix;

/// `file_name` is the capture's own name (a path is tolerated). `source_url`
/// is the URL the download was captured from; an empty string means the
/// name is the only signal.
pub fn recognize(file_name: &str, source_url: &str) -> Option<RecognizedTool> {
    if let Some(tool) = recognize_stem(&archive_stem(file_name)) {
        return Some(tool);
    }
    let host = http_host(source_url)?;
    if !is_skse_site(&host) || !is_archive(file_name) {
        return None;
    }
    Some(se_tool(None))
}

/// What to file when the extracted folder holds `skse64_loader.exe`.
///
/// `archive_file_name` is the download (`Skyrim Script Extender (SKSE64)-…`
/// or an opaque CDN name). `content_dir_name` is the inner silverlock folder
/// (`skse64_2_02_08`), which disappears once the files are moved into the
/// mod directory. A version on any of those labels is kept. When none of
/// them name SKSE, the loader itself is still enough: the row is `SKSE`
/// with no invented version.
pub fn label_from_install(
    mod_name: &str,
    archive_file_name: &str,
    content_dir_name: &str,
) -> RecognizedTool {
    let mut found = Vec::new();
    for label in [archive_file_name, content_dir_name, mod_name] {
        if label.is_empty() {
            continue;
        }
        if let Some(tool) = recognize(label, "") {
            found.push(tool);
        }
    }
    if let Some(tool) = found.iter().find(|tool| tool.version.is_some()) {
        return tool.clone();
    }
    found.into_iter().next().unwrap_or(se_tool(None))
}

/// One mod row, as the shell sees it. Paths may point at an archive that is
/// not on disk yet (a download still in flight).
pub struct ModToolQuery<'a> {
    pub name: &'a str,
    pub source_path: &'a Path,
    pub archive_path: Option<&'a Path>,
    /// Version already stored on the row, when the mod declared one.
    pub stored_version: Option<&'a str>,
    /// Capture URL. Empty when this row has no download ledger entry.
    pub source_url: &'a str,
    /// Capture file name (`skse64_2_02_06.7z`). Empty when unknown.
    pub download_file_name: &'a str,
}

/// What `classify_mod` tells the mods list. `tool` is `"skse"` or `"skse-vr"`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModToolMark {
    pub tool: &'static str,
    pub version: Option<String>,
}

/// Whether this row is the extender. A loader exe in an installed folder
/// wins over the name. Otherwise the archive name, the Nexus title, or the
/// silverlock URL (via `recognize`) decides — including while the download
/// is still running. Companions (Address Library, Engine Fixes, a plugin, a
/// preloader) are `None`.
pub fn classify_mod(query: &ModToolQuery<'_>) -> Option<ModToolMark> {
    let recognized = recognized_from_query(query);
    let parsed = recognized.iter().find_map(|tool| tool.version.clone());
    let wire = if let Some(rule) = rule_with_loader(query.source_path) {
        rule.wire
    } else {
        let chosen = recognized
            .iter()
            .find(|tool| tool.version.is_some())
            .or_else(|| recognized.first())?;
        rule_by_id(chosen.id)?.wire
    };
    Some(ModToolMark {
        tool: wire,
        version: version_of(query.stored_version, parsed),
    })
}

/// SE root file copied beside `SkyrimSE.exe`: `{SE_NAME_PREFIX}_*.dll` or
/// `{SE_NAME_PREFIX}_*.exe`. Readmes stay in the mod folder.
pub fn is_se_root_file(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    let Some(rest) = lower.strip_prefix(SE_NAME_PREFIX) else {
        return false;
    };
    rest.starts_with('_') && (lower.ends_with(".dll") || lower.ends_with(".exe"))
}

fn se_tool(version: Option<String>) -> RecognizedTool {
    RecognizedTool {
        id: NAME_RULES[0].id,
        display_name: NAME_RULES[0].display_name,
        version,
    }
}

fn rule_with_loader(dir: &Path) -> Option<&'static NameRule> {
    if !dir.is_dir() {
        return None;
    }
    NAME_RULES
        .iter()
        .find(|rule| dir.join(rule.loader).is_file())
}

fn rule_by_id(id: &str) -> Option<&'static NameRule> {
    NAME_RULES.iter().find(|rule| rule.id == id)
}

fn recognized_from_query(query: &ModToolQuery<'_>) -> Vec<RecognizedTool> {
    let mut found = Vec::new();
    push_label(&mut found, query.download_file_name, query.source_url);
    if query.download_file_name.is_empty() && !query.source_url.is_empty() {
        if let Some(path) = query.archive_path {
            push_label(&mut found, &path.to_string_lossy(), query.source_url);
        }
        push_label(
            &mut found,
            &query.source_path.to_string_lossy(),
            query.source_url,
        );
    }
    push_label(&mut found, query.name, "");
    if let Some(path) = query.archive_path {
        push_label(&mut found, &path.to_string_lossy(), "");
    }
    push_label(&mut found, &query.source_path.to_string_lossy(), "");
    found
}

fn push_label(found: &mut Vec<RecognizedTool>, label: &str, url: &str) {
    if label.is_empty() {
        return;
    }
    if let Some(tool) = recognize(label, url) {
        found.push(tool);
    }
}

/// A version the row already declared wins. A leading `v` is dropped so the
/// cell can print its own. Otherwise the version parsed from a label.
fn version_of(stored: Option<&str>, parsed: Option<String>) -> Option<String> {
    clean_stored(stored).or(parsed)
}

fn clean_stored(stored: Option<&str>) -> Option<String> {
    let trimmed = stored?.trim();
    if trimmed.is_empty() {
        return None;
    }
    let bare = trimmed
        .strip_prefix('v')
        .or_else(|| trimmed.strip_prefix('V'))
        .unwrap_or(trimmed)
        .trim();
    if bare.is_empty() {
        None
    } else {
        Some(bare.to_string())
    }
}

fn recognize_stem(stem: &str) -> Option<RecognizedTool> {
    let lower = stem.to_ascii_lowercase();
    if is_companion(&lower) {
        return None;
    }
    if let Some((id, display_name, version)) = match_name(stem) {
        let version = short_version(version).or_else(|| loose_version(&lower));
        return Some(RecognizedTool {
            id,
            display_name,
            version,
        });
    }
    let (id, display_name) = product_kind(&lower)?;
    Some(RecognizedTool {
        id,
        display_name,
        version: loose_version(&lower),
    })
}

/// Address Library, Engine Fixes, and anything that is a plugin *for* SKSE.
/// Checked before the prefix, so `EngineFixes_SKSE64_…` and
/// `skse64_plugin_…` stay ordinary mods.
fn is_companion(lower: &str) -> bool {
    let tokens = tokens(lower);
    if tokens
        .iter()
        .any(|token| matches!(*token, "plugin" | "plugins" | "preloader"))
    {
        return true;
    }
    if tokens.windows(2).any(|pair| pair == ["address", "library"]) {
        return true;
    }
    if tokens
        .windows(2)
        .any(|pair| pair[0] == "engine" && matches!(pair[1], "fix" | "fixes"))
    {
        return true;
    }
    if tokens
        .iter()
        .any(|token| matches!(*token, "enginefix" | "enginefixes"))
    {
        return true;
    }
    tokens
        .windows(2)
        .any(|pair| pair[0] == "for" && matches!(pair[1], "skse" | "skse64" | "sksevr"))
}

/// Nexus and other titles that do not start with `skse64` / `sksevr` / `skse_`.
fn product_kind(lower: &str) -> Option<(&'static str, &'static str)> {
    let tokens = tokens(lower);
    if tokens.len() >= 4
        && tokens[0] == "skyrim"
        && tokens[1] == "vr"
        && tokens[2] == "script"
        && tokens[3] == "extender"
    {
        return Some(("sksevr", "SKSE VR"));
    }
    if tokens.len() >= 3
        && tokens[0] == "skyrim"
        && tokens[1] == "script"
        && tokens[2] == "extender"
    {
        if tokens
            .iter()
            .any(|token| *token == "sksevr" || *token == "vr")
        {
            return Some(("sksevr", "SKSE VR"));
        }
        return Some(("skse", "SKSE"));
    }
    let identity: Vec<&str> = tokens
        .iter()
        .copied()
        .filter(|token| !is_noise_token(token))
        .collect();
    match identity.as_slice() {
        ["skse"] | ["skse64"] | ["skse", "64"] => Some(("skse", "SKSE")),
        ["sksevr"] | ["skse", "vr"] => Some(("sksevr", "SKSE VR")),
        _ => None,
    }
}

fn is_noise_token(token: &str) -> bool {
    token.bytes().all(|byte| byte.is_ascii_digit())
        || matches!(
            token,
            "steam"
                | "gog"
                | "se"
                | "ae"
                | "anniversary"
                | "special"
                | "edition"
                | "beta"
                | "current"
        )
}

fn tokens(lower: &str) -> Vec<&str> {
    lower
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|token| !token.is_empty())
        .collect()
}

/// A prefix parse that starts with a Nexus mod id (`30379.2.2.6`) is not a
/// SKSE version. `None` lets the loose parser read `2.2.6` instead.
fn short_version(version: Option<String>) -> Option<String> {
    let version = version?;
    match version.split('.').next() {
        Some(first) if first.len() <= 2 => Some(version),
        _ => None,
    }
}

/// `2.2.8`, `2.02.06`, or Nexus `-30379-2-2-6-1701192802` → `2.2.6`.
/// A 4-digit piece (`1.6.1170`, a file id) is not a version component.
fn loose_version(lower: &str) -> Option<String> {
    dotted_triple(lower).or_else(|| dash_triple(lower))
}

fn dotted_triple(lower: &str) -> Option<String> {
    let bytes = lower.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        let at_boundary = index == 0 || !bytes[index - 1].is_ascii_digit();
        if at_boundary && bytes[index].is_ascii_digit() {
            if let Some(version) = dotted_at(lower, index) {
                return Some(version);
            }
        }
        index += 1;
    }
    None
}

fn dotted_at(lower: &str, start: usize) -> Option<String> {
    let (major, after_major) = take_part(lower, start)?;
    let after_dot = eat_dot(lower, after_major)?;
    let (minor, after_minor) = take_part(lower, after_dot)?;
    let after_dot = eat_dot(lower, after_minor)?;
    let (patch, end) = take_part(lower, after_dot)?;
    if end < lower.len() && lower.as_bytes()[end].is_ascii_digit() {
        return None;
    }
    Some(join_parts(&[major, minor, patch]))
}

fn eat_dot(lower: &str, index: usize) -> Option<usize> {
    if lower.as_bytes().get(index).copied() == Some(b'.') {
        Some(index + 1)
    } else {
        None
    }
}

fn take_part(lower: &str, start: usize) -> Option<(&str, usize)> {
    let bytes = lower.as_bytes();
    if start >= bytes.len() || !bytes[start].is_ascii_digit() {
        return None;
    }
    let mut end = start + 1;
    while end < bytes.len() && bytes[end].is_ascii_digit() {
        end += 1;
    }
    let part = &lower[start..end];
    if (1..=2).contains(&part.len()) {
        Some((part, end))
    } else {
        None
    }
}

fn dash_triple(lower: &str) -> Option<String> {
    let runs = digit_runs(lower);
    let mut found = None;
    if runs.len() < 3 {
        return None;
    }
    for index in 0..=runs.len() - 3 {
        let (major, minor, patch) = (runs[index], runs[index + 1], runs[index + 2]);
        if !is_version_part(major) || !is_version_part(minor) || !is_version_part(patch) {
            continue;
        }
        let prev_is_mod_id = index > 0 && (4..=6).contains(&runs[index - 1].len());
        let next = runs.get(index + 3).copied();
        let next_is_part = next.is_some_and(is_version_part);
        if next_is_part && !prev_is_mod_id {
            continue;
        }
        let next_is_file_id = next.is_some_and(|part| part.len() >= 7);
        let at_end = next.is_none() || next_is_file_id;
        if prev_is_mod_id || at_end {
            found = Some(join_parts(&[major, minor, patch]));
        }
    }
    found
}

fn is_version_part(part: &str) -> bool {
    (1..=2).contains(&part.len()) && part.bytes().all(|byte| byte.is_ascii_digit())
}

fn digit_runs(lower: &str) -> Vec<&str> {
    let bytes = lower.as_bytes();
    let mut runs = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        if !bytes[index].is_ascii_digit() {
            index += 1;
            continue;
        }
        let start = index;
        index += 1;
        while index < bytes.len() && bytes[index].is_ascii_digit() {
            index += 1;
        }
        runs.push(&lower[start..index]);
    }
    runs
}

fn join_parts(parts: &[&str]) -> String {
    parts
        .iter()
        .map(|part| {
            let trimmed = part.trim_start_matches('0');
            if trimmed.is_empty() {
                "0"
            } else {
                trimmed
            }
        })
        .collect::<Vec<_>>()
        .join(".")
}

fn match_name(stem: &str) -> Option<(&'static str, &'static str, Option<String>)> {
    let lower = stem.to_ascii_lowercase();
    for rule in &NAME_RULES {
        if lower.starts_with(rule.prefix) {
            return Some((
                rule.id,
                rule.display_name,
                version_after_prefix(&lower, rule.prefix),
            ));
        }
    }
    None
}

/// `skse64_2_02_06` → `2.2.6`. A trailing word (`_gog`, `_loader`) stops the
/// parse; the name match itself still holds.
fn version_after_prefix(stem_lower: &str, prefix: &str) -> Option<String> {
    let rest = stem_lower.get(prefix.len()..).unwrap_or("");
    let rest = rest.trim_start_matches(['_', '-', '.', ' ']);
    if rest.is_empty() {
        return None;
    }
    let mut parts: Vec<&str> = Vec::new();
    for component in rest.split(['_', '-', '.', ' ']) {
        if component.is_empty() {
            continue;
        }
        if !component.bytes().all(|b| b.is_ascii_digit()) || component.len() > 8 || parts.len() == 4
        {
            break;
        }
        let trimmed = component.trim_start_matches('0');
        parts.push(if trimmed.is_empty() { "0" } else { trimmed });
    }
    if parts.is_empty() {
        None
    } else {
        Some(parts.join("."))
    }
}

fn archive_stem(file_name: &str) -> String {
    let trimmed = file_name.trim();
    let base = Path::new(trimmed)
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| trimmed.to_string());
    // Only a real archive suffix. `Path::file_stem` would also eat the patch
    // of a dotted version (`Steam 2.2.8` → `Steam 2.2`).
    let path = Path::new(&base);
    let archive_ext = path
        .extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| matches!(ext.to_ascii_lowercase().as_str(), "7z" | "zip" | "rar"));
    if archive_ext {
        path.file_stem()
            .map(|stem| stem.to_string_lossy().into_owned())
            .unwrap_or(base)
    } else {
        base
    }
}

fn is_archive(file_name: &str) -> bool {
    let trimmed = file_name.trim();
    let Some(base) = Path::new(trimmed).file_name() else {
        return false;
    };
    Path::new(base)
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| matches!(e.to_ascii_lowercase().as_str(), "7z" | "zip" | "rar"))
}

fn is_skse_site(host: &str) -> bool {
    host == SKSE_HOST || host.ends_with(&format!(".{SKSE_HOST}"))
}

fn http_host(url: &str) -> Option<String> {
    let url = url.trim();
    let scheme_end = url.find("://")?;
    let scheme = &url[..scheme_end];
    if !scheme.eq_ignore_ascii_case("https") && !scheme.eq_ignore_ascii_case("http") {
        return None;
    }
    let rest = &url[scheme_end + 3..];
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    let authority = authority.rsplit('@').next().unwrap_or("");
    if authority.is_empty() {
        return None;
    }
    let host = if let Some(inner) = authority.strip_prefix('[') {
        inner.split(']').next()?
    } else {
        authority.split(':').next()?
    };
    let host = host.trim_end_matches('.').to_ascii_lowercase();
    if host.is_empty() {
        None
    } else {
        Some(host)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn name<'a>(file_name: &str) -> Option<RecognizedTool> {
        recognize(file_name, "")
    }

    #[test]
    fn skse64_archive_names_are_skse_with_a_dotted_version() {
        let tool = name("skse64_2_02_06.7z").expect("se build");
        assert_eq!(tool.id, "skse");
        assert_eq!(tool.display_name, "SKSE");
        assert_eq!(tool.version.as_deref(), Some("2.2.6"));

        let gog = name("SKSE64_2_02_06_gog.7z").expect("gog build");
        assert_eq!(gog.id, "skse");
        assert_eq!(gog.version.as_deref(), Some("2.2.6"));

        let loose = name("skse64_2_2_6.zip").expect("unpadded");
        assert_eq!(loose.version.as_deref(), Some("2.2.6"));
    }

    #[test]
    fn classic_and_vr_prefixes_are_the_same_family() {
        let classic = name("skse_1_07_03.7z").expect("classic");
        assert_eq!(classic.id, "skse");
        assert_eq!(classic.display_name, "SKSE");
        assert_eq!(classic.version.as_deref(), Some("1.7.3"));

        let vr = name("sksevr_2_00_12.7z").expect("vr");
        assert_eq!(vr.id, "sksevr");
        assert_eq!(vr.display_name, "SKSE VR");
        assert_eq!(vr.version.as_deref(), Some("2.0.12"));
    }

    #[test]
    fn a_bare_prefix_has_no_invented_version() {
        let tool = name("skse64.7z").expect("no version in the name");
        assert_eq!(tool.display_name, "SKSE");
        assert_eq!(tool.version, None);

        let loader = name(r"C:\downloads\skse64_loader.exe").expect("loader file");
        assert_eq!(loader.id, "skse");
        assert_eq!(loader.version, None);
    }

    #[test]
    fn a_name_that_only_mentions_skse_is_not_the_tool() {
        assert!(name("EngineFixes_SKSE64_1.6.1170.zip").is_none());
        assert!(name("Address Library-32444-1-6-1170.7z").is_none());
        assert!(name("Address Library for SKSE Plugins-32444-11-1700000000.7z").is_none());
        assert!(name("Engine Fixes - SKSE64 Preloader-17230-1-6-1170.7z").is_none());
        assert!(name("(Part 2) Engine Fixes - skse64 plugin-17230-6-1-1.7z").is_none());
        assert!(name("SSE Engine Fixes-17230-6-2-0.7z").is_none());
        assert!(name("notskse64.7z").is_none());
        assert!(name("lux.7z").is_none());
        assert!(recognize(
            "Lux-123-1-0-0.7z",
            "https://www.nexusmods.com/skyrimspecialedition/mods/30379"
        )
        .is_none());
    }

    #[test]
    fn nexus_skse64_titles_are_skse_and_keep_the_real_version() {
        let packed = name("Skyrim Script Extender (SKSE64)-30379-2-2-6-1701192802.7z")
            .expect("nexus file name");
        assert_eq!(packed.id, "skse");
        assert_eq!(packed.display_name, "SKSE");
        assert_eq!(packed.version.as_deref(), Some("2.2.6"));

        let from_cdn = recognize(
            "Skyrim Script Extender (SKSE64)-30379-2-2-6-1701192802.7z",
            "https://cf-files.nexusmods.com/skyrimspecialedition/30379/foo",
        )
        .expect("cdn url does not block the name");
        assert_eq!(from_cdn.version.as_deref(), Some("2.2.6"));

        let steam = name("Skyrim Script Extender (SKSE64) Steam-30379-2-2-6-1701192802.7z")
            .expect("steam build");
        assert_eq!(steam.version.as_deref(), Some("2.2.6"));

        let gog = name("Skyrim Script Extender (SKSE64) GOG-30379-2-02-06.7z").expect("gog build");
        assert_eq!(gog.version.as_deref(), Some("2.2.6"));

        let folder = name("Skyrim Script Extender (SKSE64) Steam 2.2.8").expect("inner folder");
        assert_eq!(folder.display_name, "SKSE");
        assert_eq!(folder.version.as_deref(), Some("2.2.8"));

        let padded = name("Skyrim Script Extender (SKSE64) 2.02.06").expect("dotted padded");
        assert_eq!(padded.version.as_deref(), Some("2.2.6"));

        let bare = name("Skyrim Script Extender (SKSE64).7z").expect("title only");
        assert_eq!(bare.display_name, "SKSE");
        assert_eq!(bare.version, None);

        // The mod id must not become the version just because the file starts
        // with the silverlock prefix.
        let prefixed = name("SKSE64-30379-2-2-6-1701192802.7z").expect("prefix plus nexus id");
        assert_eq!(prefixed.display_name, "SKSE");
        assert_eq!(prefixed.version.as_deref(), Some("2.2.6"));
    }

    #[test]
    fn a_bare_skse_name_and_the_vr_nexus_title_stay_in_the_family() {
        let classic = name("skse.7z").expect("classic stem");
        assert_eq!(classic.id, "skse");
        assert_eq!(classic.display_name, "SKSE");
        assert_eq!(classic.version, None);

        let vr = name("Skyrim VR Script Extender (SKSEVR)-30473-2-0-12-1700000000.7z")
            .expect("vr nexus title");
        assert_eq!(vr.id, "sksevr");
        assert_eq!(vr.display_name, "SKSE VR");
        assert_eq!(vr.version.as_deref(), Some("2.0.12"));
    }

    #[test]
    fn an_installed_loader_keeps_a_version_the_file_name_did_not_carry() {
        let from_folder = label_from_install("download", "download.7z", "skse64_2_02_08");
        assert_eq!(from_folder.display_name, "SKSE");
        assert_eq!(from_folder.version.as_deref(), Some("2.2.8"));

        let from_nexus = label_from_install(
            "Skyrim Script Extender (SKSE64)-30379-2-2-6-1701192802",
            "Skyrim Script Extender (SKSE64)-30379-2-2-6-1701192802.7z",
            "skse64_2_02_06",
        );
        assert_eq!(from_nexus.display_name, "SKSE");
        assert_eq!(from_nexus.version.as_deref(), Some("2.2.6"));

        let loader_only = label_from_install("file", "file.7z", "payload");
        assert_eq!(loader_only.id, "skse");
        assert_eq!(loader_only.display_name, "SKSE");
        assert_eq!(loader_only.version, None);
    }

    #[test]
    fn the_silverlock_host_labels_an_archive_even_when_the_name_does_not() {
        let tool = recognize("build.7z", "https://skse.silverlock.org/beta/current.7z")
            .expect("official site");
        assert_eq!(tool.id, "skse");
        assert_eq!(tool.display_name, "SKSE");
        assert_eq!(tool.version, None);

        let nested =
            recognize("build.zip", "https://files.skse.silverlock.org/x.zip").expect("subdomain");
        assert_eq!(nested.id, "skse");

        assert!(
            recognize("readme.txt", "https://skse.silverlock.org/").is_none(),
            "a page saved from the site is not an archive"
        );
        assert!(recognize("build.7z", "https://skse.silverlock.org.evil.test/build.7z").is_none());
        assert!(recognize("loot_0.24.1.7z", "https://loot.github.io/").is_none());
    }

    #[test]
    fn a_matching_name_wins_over_the_site_and_keeps_its_version() {
        let tool = recognize(
            "skse64_2_02_06.7z",
            "https://SKSE.Silverlock.Org:443/skse64_2_02_06.7z",
        )
        .expect("name and site");
        assert_eq!(tool.version.as_deref(), Some("2.2.6"));
    }

    #[test]
    fn official_pages_are_skse_and_loot() {
        assert_eq!(
            OFFICIAL_TOOL_PAGES
                .iter()
                .map(|p| (p.id, p.url))
                .collect::<Vec<_>>(),
            [
                ("skse", "https://skse.silverlock.org/"),
                ("loot", "https://loot.github.io/"),
            ]
        );
    }

    fn row<'a>(
        name: &'a str,
        source: &'a Path,
        archive: Option<&'a Path>,
        stored: Option<&'a str>,
        url: &'a str,
        download: &'a str,
    ) -> ModToolQuery<'a> {
        ModToolQuery {
            name,
            source_path: source,
            archive_path: archive,
            stored_version: stored,
            source_url: url,
            download_file_name: download,
        }
    }

    #[test]
    fn loader_names_come_from_the_same_prefix_table() {
        assert_eq!(SE_NAME_PREFIX, "skse64");
        assert_eq!(SE_LOADER_FILE, "skse64_loader.exe");
        assert_eq!(crate::engine::LOADER_FILE, SE_LOADER_FILE);
        assert_eq!(
            NAME_RULES
                .iter()
                .map(|rule| rule.loader)
                .collect::<Vec<_>>(),
            ["skse64_loader.exe", "sksevr_loader.exe", "skse_loader.exe"]
        );
        assert!(is_se_root_file("skse64_loader.exe"));
        assert!(is_se_root_file("skse64_1_7_104.dll"));
        assert!(is_se_root_file("SKSE64_steam_loader.DLL"));
        assert!(!is_se_root_file("skse64_readme.txt"));
        assert!(!is_se_root_file("sksevr_loader.exe"));
        assert!(!is_se_root_file("skse64.dll"));
    }

    #[test]
    fn classify_reads_silverlock_nexus_and_vr_names() {
        let se = Path::new("skse64_2_02_06.7z");
        let mark = classify_mod(&row(
            "skse64_2_02_06",
            se,
            Some(se),
            None,
            "",
            "skse64_2_02_06.7z",
        ))
        .expect("silverlock");
        assert_eq!(mark.tool, "skse");
        assert_eq!(mark.version.as_deref(), Some("2.2.6"));

        let nexus = Path::new("Skyrim Script Extender (SKSE64)-30379-2-2-6-1701192802.7z");
        let mark = classify_mod(&row(
            "Skyrim Script Extender (SKSE64)-30379-2-2-6-1701192802",
            nexus,
            Some(nexus),
            None,
            "https://cf-files.nexusmods.com/skyrimspecialedition/30379/foo",
            "Skyrim Script Extender (SKSE64)-30379-2-2-6-1701192802.7z",
        ))
        .expect("nexus");
        assert_eq!(mark.tool, "skse");
        assert_eq!(mark.version.as_deref(), Some("2.2.6"));

        let vr = Path::new("sksevr_2_00_12.7z");
        let mark = classify_mod(&row(
            "sksevr_2_00_12",
            vr,
            Some(vr),
            None,
            "",
            "sksevr_2_00_12.7z",
        ))
        .expect("vr archive");
        assert_eq!(mark.tool, "skse-vr");
        assert_eq!(mark.version.as_deref(), Some("2.0.12"));

        let vr_nexus = Path::new("Skyrim VR Script Extender (SKSEVR)-30473-2-0-12-1700000000.7z");
        let mark = classify_mod(&row(
            "Skyrim VR Script Extender (SKSEVR)-30473-2-0-12-1700000000",
            vr_nexus,
            Some(vr_nexus),
            None,
            "",
            "",
        ))
        .expect("vr nexus title");
        assert_eq!(mark.tool, "skse-vr");
        assert_eq!(mark.version.as_deref(), Some("2.0.12"));
    }

    #[test]
    fn a_download_in_flight_is_classified_from_the_capture() {
        let path = Path::new("F:/ModLab/downloads/build.7z");
        let mute = classify_mod(&row(
            "build",
            path,
            Some(path),
            None,
            "https://skse.silverlock.org/beta/current.7z",
            "build.7z",
        ))
        .expect("opaque silverlock download");
        assert_eq!(mute.tool, "skse");
        assert_eq!(mute.version, None);

        let named = Path::new("F:/ModLab/downloads/skse64_2_02_06.7z");
        let versioned = classify_mod(&row(
            "skse64_2_02_06",
            named,
            Some(named),
            None,
            "https://skse.silverlock.org/skse64_2_02_06.7z",
            "skse64_2_02_06.7z",
        ))
        .expect("named download");
        assert_eq!(versioned.tool, "skse");
        assert_eq!(versioned.version.as_deref(), Some("2.2.6"));
    }

    #[test]
    fn an_installed_loader_marks_the_row_even_when_the_archive_name_says_nothing() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(SE_LOADER_FILE), b"loader").unwrap();
        let archive = Path::new("file.7z");
        let mark = classify_mod(&row(
            "payload",
            dir.path(),
            Some(archive),
            None,
            "",
            "file.7z",
        ))
        .expect("loader");
        assert_eq!(mark.tool, "skse");
        assert_eq!(mark.version, None);

        let versioned = Path::new("skse64_2_02_08.7z");
        let mark = classify_mod(&row(
            "12",
            dir.path(),
            Some(versioned),
            None,
            "",
            "skse64_2_02_08.7z",
        ))
        .expect("loader keeps the archive version");
        assert_eq!(mark.tool, "skse");
        assert_eq!(mark.version.as_deref(), Some("2.2.8"));

        let declared = classify_mod(&row(
            "12",
            dir.path(),
            Some(versioned),
            Some("v2.2.6"),
            "",
            "skse64_2_02_08.7z",
        ))
        .expect("stored version wins");
        assert_eq!(declared.version.as_deref(), Some("2.2.6"));
    }

    #[test]
    fn companions_are_not_the_tool_and_a_vr_loader_is_skse_vr() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("sksevr_loader.exe"), b"loader").unwrap();
        let mark =
            classify_mod(&row("payload", dir.path(), None, None, "", "")).expect("vr loader");
        assert_eq!(mark.tool, "skse-vr");
        assert_eq!(mark.version, None);

        let absent = Path::new("not-a-folder");
        for name in [
            "EngineFixes_SKSE64_1.6.1170.zip",
            "Address Library-32444-1-6-1170.7z",
            "Address Library for SKSE Plugins-32444-11-1700000000.7z",
            "Engine Fixes - SKSE64 Preloader-17230-1-6-1170.7z",
            "(Part 2) Engine Fixes - skse64 plugin-17230-6-1-1.7z",
            "skse64_plugin_example.zip",
            "SSE Engine Fixes-17230-6-2-0.7z",
        ] {
            assert!(
                classify_mod(&row(name, absent, Some(Path::new(name)), None, "", name)).is_none(),
                "{name} is not SKSE"
            );
        }
    }
}
