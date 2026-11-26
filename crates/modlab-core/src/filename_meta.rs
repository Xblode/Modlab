//! Display name, version and remote id read from a download's file name.
//!
//! This does not know which mod it is looking at, and it does not know a
//! game. The dashed id-version-timestamp shape is a file-name convention.
//! Turning an id into a page address is the caller's job: the domain comes
//! from the game adapter, not from here.

use std::path::Path;

/// What a file name states on its own.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilenameFacts {
    pub name: String,
    /// `6.11`, `5.2SE`, `2.2.6`. Absent when the name has no clear version.
    pub version: Option<String>,
    /// Numeric id from the dashed file-name convention. Absent otherwise.
    pub nexus_mod_id: Option<String>,
}

/// A display name with `_` turned into spaces and a CamelCase word split
/// only where both sides are real words (`EngineFixes` → `Engine Fixes`).
/// `SkyUI` stays: the right side is an acronym, not a word.
#[must_use]
pub fn polish_display_name(raw: &str) -> String {
    collapse_ws(&split_camel_sure(&raw.replace('_', " ")))
}

/// `SkyUI-12604-6-11-1778020881 (3).7z` → name `SkyUI`, id `12604`, version `6.11`.
///
/// Also the spaced Nexus file name (`Name <id> <version> <stamp> <token>`)
/// and a trailing version (`Lux-2.2.6`, `Foo_1_2_3`, `Mod_v2`). A build with
/// a zero-padded piece (`skse64_2_02_06`) is left to the game adapter.
#[must_use]
pub fn parse_file_name(file_name: &str) -> FilenameFacts {
    let stem = stem_of(file_name);
    if let Some((name, id, version)) = nexus_shape(&stem) {
        return named(name, Some(version), Some(id));
    }
    if let Some((name, id, version)) = cdn_shape(&stem) {
        return named(name, Some(version), Some(id));
    }
    let (name, version) = peel_loose_version(&stem);
    named(name, version, None)
}

fn named(name: String, version: Option<String>, id: Option<String>) -> FilenameFacts {
    let name = match &version {
        Some(version) => strip_version_echo(&name, version),
        None => name,
    };
    let name = polish_display_name(&name);
    FilenameFacts {
        name: if name.is_empty() {
            "mod".to_string()
        } else {
            name
        },
        version,
        nexus_mod_id: id,
    }
}

/// True when `name` is still the file's own label (extension and Windows
/// ` (3)` suffix aside), so a later layer may replace it.
#[must_use]
pub fn is_raw_file_label(name: &str, file_name: &str) -> bool {
    let name = name.trim();
    if name.is_empty() {
        return false;
    }
    let base = file_only(file_name);
    let no_disabled = strip_disabled(&base);
    let no_ext = strip_one_extension(&no_disabled);
    if name == no_ext.trim() || name == base.trim() {
        return true;
    }
    let no_copy = strip_copy_suffixes(&no_ext);
    name == no_copy.trim()
}

fn stem_of(file_name: &str) -> String {
    let base = file_only(file_name);
    let base = strip_disabled(&base);
    let base = strip_one_extension(&base);
    strip_copy_suffixes(&base)
}

fn file_only(file_name: &str) -> String {
    Path::new(file_name)
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| file_name.trim().to_string())
}

fn strip_disabled(name: &str) -> String {
    name.strip_suffix(".disabled")
        .or_else(|| name.strip_suffix(".DISABLED"))
        .unwrap_or(name)
        .to_string()
}

fn strip_one_extension(name: &str) -> String {
    let path = Path::new(name);
    match (path.file_stem(), path.extension()) {
        (Some(stem), Some(ext)) => {
            let stem = stem.to_string_lossy();
            let ext = ext.to_string_lossy();
            if !stem.is_empty()
                && !ext.is_empty()
                && ext.len() <= 8
                && ext.chars().all(|c| c.is_ascii_alphanumeric())
            {
                return stem.into_owned();
            }
            name.to_string()
        }
        _ => name.to_string(),
    }
}

fn strip_copy_suffixes(name: &str) -> String {
    let mut current = name.trim().to_string();
    while let Some(next) = strip_one_copy(&current) {
        current = next;
    }
    current
}

/// Windows duplicate suffix: `name (3)`.
fn strip_one_copy(name: &str) -> Option<String> {
    let trimmed = name.trim_end();
    if !trimmed.ends_with(')') {
        return None;
    }
    let open = trimmed.rfind('(')?;
    if open == 0 {
        return None;
    }
    let inside = &trimmed[open + 1..trimmed.len() - 1];
    if inside.is_empty() || !inside.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let prefix = &trimmed[..open];
    if !prefix.ends_with(|c: char| c.is_whitespace()) {
        return None;
    }
    let before = prefix.trim_end();
    if before.is_empty() {
        None
    } else {
        Some(before.to_string())
    }
}

/// `Name-<modId>-<version-with-dashes>[-<10 digit timestamp>]`.
fn nexus_shape(stem: &str) -> Option<(String, String, String)> {
    let stem = stem.trim();
    if stem.is_empty() {
        return None;
    }
    let body = match stem.rsplit_once('-') {
        Some((head, tail)) if is_timestamp(tail) => head,
        _ => stem,
    };
    let tokens: Vec<&str> = body.split('-').collect();
    if tokens.len() < 3 {
        return None;
    }
    for id_at in 1..tokens.len() - 1 {
        let id = tokens[id_at];
        if !is_mod_id(id) {
            continue;
        }
        let version_parts = &tokens[id_at + 1..];
        if version_parts.is_empty()
            || version_parts.len() > 6
            || !version_parts.iter().copied().all(is_version_part)
        {
            continue;
        }
        let name = clean_display_name(&tokens[..id_at].join("-"));
        if name.is_empty() {
            continue;
        }
        return Some((name, id.to_string(), version_parts.join(".")));
    }
    None
}

fn is_timestamp(token: &str) -> bool {
    if token.len() != 10 || !token.bytes().all(|b| b.is_ascii_digit()) {
        return false;
    }
    token
        .parse::<u64>()
        .ok()
        .is_some_and(|n| n >= 1_000_000_000)
}

fn is_mod_id(token: &str) -> bool {
    !token.is_empty()
        && token.len() <= 8
        && token.bytes().all(|b| b.is_ascii_digit())
        && !token.starts_with('0')
}

/// `6`, `11`, `02`, `2SE`. A letter suffix stays on the numeric piece.
fn is_version_part(token: &str) -> bool {
    if token.is_empty() || token.len() > 12 {
        return false;
    }
    let mut chars = token.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !first.is_ascii_digit() {
        return false;
    }
    let mut letter = false;
    for c in chars {
        if c.is_ascii_digit() {
            if letter {
                return false;
            }
        } else if c.is_ascii_alphabetic() {
            letter = true;
        } else {
            return false;
        }
    }
    true
}

fn dotted_version(stem: &str) -> Option<String> {
    let mut found = None;
    for token in stem.split(['-', '_', '+', ' ']) {
        if let Some(version) = dotted_token(token) {
            found = Some(version);
        }
    }
    found
}

fn dotted_token(token: &str) -> Option<String> {
    let token = token
        .strip_prefix('v')
        .or_else(|| token.strip_prefix('V'))
        .unwrap_or(token);
    let parts: Vec<&str> = token.split('.').collect();
    if !(2..=4).contains(&parts.len()) {
        return None;
    }
    for (i, part) in parts.iter().enumerate() {
        if !is_version_part(part) {
            return None;
        }
        let has_letter = part.bytes().any(|b| b.is_ascii_alphabetic());
        if has_letter && i + 1 != parts.len() {
            return None;
        }
    }
    Some(token.to_string())
}

fn clean_display_name(raw: &str) -> String {
    collapse_ws(&raw.replace('_', " "))
}

fn collapse_ws(raw: &str) -> String {
    let mut out = String::new();
    let mut pending_space = false;
    for c in raw.chars() {
        if c.is_whitespace() {
            pending_space = true;
            continue;
        }
        if pending_space && !out.is_empty() {
            out.push(' ');
        }
        pending_space = false;
        out.push(c);
    }
    out
}

/// Split `engine|Fixes` when the right side contains a lowercase letter and
/// the left side is at least two letters. An all-caps tail (`Sky|UI`,
/// `True|HUD`) is left alone.
fn split_camel_sure(raw: &str) -> String {
    let chars: Vec<char> = raw.chars().collect();
    let mut out = String::new();
    for (index, ch) in chars.iter().copied().enumerate() {
        if index > 0 && camel_word_boundary(&chars, index) {
            out.push(' ');
        }
        out.push(ch);
    }
    out
}

fn camel_word_boundary(chars: &[char], index: usize) -> bool {
    let prev = chars[index - 1];
    let current = chars[index];
    if !prev.is_lowercase() || !current.is_uppercase() {
        return false;
    }
    if left_letters(chars, index) < 2 {
        return false;
    }
    right_word_has_lowercase(chars, index)
}

fn left_letters(chars: &[char], index: usize) -> usize {
    let mut count = 0;
    let mut cursor = index;
    while cursor > 0 {
        let ch = chars[cursor - 1];
        if !ch.is_alphabetic() {
            break;
        }
        count += 1;
        cursor -= 1;
    }
    count
}

fn right_word_has_lowercase(chars: &[char], index: usize) -> bool {
    let mut saw = false;
    for ch in chars.iter().copied().skip(index) {
        if !ch.is_alphabetic() {
            break;
        }
        if ch.is_lowercase() {
            saw = true;
        }
    }
    saw
}

/// Drop a whole token that is only the file version again (`2-08` next to
/// `2.08`, a leading `9.0.1`, a trailing `v13`). `SE` / `AE` / `VR` are not
/// versions, so they stay in the title.
fn strip_version_echo(name: &str, version: &str) -> String {
    let forms = version_forms(version);
    let kept: Vec<&str> = name
        .split_whitespace()
        .filter(|token| {
            !forms
                .iter()
                .any(|form| form.eq_ignore_ascii_case(token_key(token)))
        })
        .collect();
    kept.join(" ")
}

fn token_key(token: &str) -> &str {
    token.trim_matches(|c: char| matches!(c, '(' | ')' | '[' | ']' | ',' | '"' | '\''))
}

fn version_forms(version: &str) -> Vec<String> {
    let dashed = version.replace('.', "-");
    let underscored = version.replace('.', "_");
    vec![
        version.to_string(),
        dashed.clone(),
        underscored,
        format!("v{version}"),
        format!("V{version}"),
        format!("v{dashed}"),
        format!("V{dashed}"),
    ]
}

/// `Name 199639 1.6 2026-11-24T13-11Z s6Og04g9U` — the spaced Nexus file name.
fn cdn_shape(stem: &str) -> Option<(String, String, String)> {
    let mut tokens: Vec<&str> = stem.split_whitespace().filter(|t| !t.is_empty()).collect();
    if tokens.len() < 4 {
        return None;
    }
    if !is_file_token(tokens.last()?) {
        return None;
    }
    tokens.pop();
    if !is_iso_stamp(tokens.last()?) {
        return None;
    }
    tokens.pop();
    let version = tokens.last()?;
    if !is_cdn_version(version) {
        return None;
    }
    let version = version.to_string();
    tokens.pop();
    let id = tokens.last()?;
    if !is_mod_id(id) {
        return None;
    }
    let id = id.to_string();
    tokens.pop();
    if tokens.is_empty() {
        return None;
    }
    Some((tokens.join(" "), id, version))
}

fn is_file_token(token: &str) -> bool {
    let len = token.len();
    (8..=12).contains(&len)
        && token.bytes().all(|b| b.is_ascii_alphanumeric())
        && token.bytes().any(|b| b.is_ascii_digit())
        && token.bytes().any(|b| b.is_ascii_alphabetic())
}

/// `2026-11-24T13-11Z` — 17 characters, the Nexus download stamp.
fn is_iso_stamp(token: &str) -> bool {
    let b = token.as_bytes();
    b.len() == 17
        && b[4] == b'-'
        && b[7] == b'-'
        && b[10] == b'T'
        && b[13] == b'-'
        && b[16] == b'Z'
        && b[..4].iter().all(|c| c.is_ascii_digit())
        && b[5..7].iter().all(|c| c.is_ascii_digit())
        && b[8..10].iter().all(|c| c.is_ascii_digit())
        && b[11..13].iter().all(|c| c.is_ascii_digit())
        && b[14..16].iter().all(|c| c.is_ascii_digit())
}

fn is_cdn_version(token: &str) -> bool {
    let token = token
        .strip_prefix('v')
        .or_else(|| token.strip_prefix('V'))
        .unwrap_or(token);
    if token.is_empty() || token.len() > 16 {
        return false;
    }
    if token.bytes().all(|b| b.is_ascii_digit()) {
        return (1..=3).contains(&token.len()) && !token.starts_with('0');
    }
    dotted_token(token).is_some()
}

fn peel_loose_version(stem: &str) -> (String, Option<String>) {
    if let Some((name, version)) = peel_v_suffix(stem) {
        return (name, Some(version));
    }
    if let Some((name, version)) = peel_underscore_version(stem) {
        return (name, Some(version));
    }
    if let Some((name, version)) = peel_dotted_suffix(stem) {
        return (name, Some(version));
    }
    (stem.to_string(), dotted_version(stem))
}

/// `Mod_v2`, `Mod-v1.2`.
fn peel_v_suffix(stem: &str) -> Option<(String, String)> {
    let sep = stem.rfind(['_', '-', ' '])?;
    let tail = &stem[sep + 1..];
    let head = stem[..sep].trim();
    if head.is_empty() {
        return None;
    }
    let rest = tail
        .strip_prefix('v')
        .or_else(|| tail.strip_prefix('V'))
        .filter(|rest| rest.chars().next().is_some_and(|c| c.is_ascii_digit()))?;
    let version =
        if rest.bytes().all(|b| b.is_ascii_digit()) && !rest.starts_with('0') && rest.len() <= 4 {
            rest.to_string()
        } else {
            dotted_token(rest)?
        };
    Some((head.to_string(), version))
}

/// `Foo_1_2_3` → `1.2.3`. A zero-padded piece (`02`) refuses the whole suffix
/// so `skse64_2_02_06` stays with the adapter that already understands it.
fn peel_underscore_version(stem: &str) -> Option<(String, String)> {
    let parts: Vec<&str> = stem.split('_').collect();
    if parts.len() < 3 {
        return None;
    }
    let mut numeric = 0;
    for part in parts.iter().rev() {
        if is_plain_number(part) {
            numeric += 1;
        } else {
            break;
        }
    }
    if !(2..=4).contains(&numeric) || parts.len() == numeric {
        return None;
    }
    let name = parts[..parts.len() - numeric].join("_");
    if name.trim().is_empty() {
        return None;
    }
    let version = parts[parts.len() - numeric..].join(".");
    Some((name, version))
}

fn is_plain_number(part: &str) -> bool {
    !part.is_empty()
        && part.len() <= 4
        && part.bytes().all(|b| b.is_ascii_digit())
        && !(part.len() > 1 && part.starts_with('0'))
}

fn peel_dotted_suffix(stem: &str) -> Option<(String, String)> {
    let sep = stem.rfind(['-', '_', '+', ' '])?;
    let tail = &stem[sep + 1..];
    let head = stem[..sep].trim();
    if head.is_empty() {
        return None;
    }
    let version = dotted_token(tail)?;
    Some((head.to_string(), version))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skyui_nexus_name_with_a_windows_copy_suffix() {
        let facts = parse_file_name("SkyUI-12604-6-11-1778020881 (3).7z");
        assert_eq!(facts.name, "SkyUI");
        assert_eq!(facts.nexus_mod_id.as_deref(), Some("12604"));
        assert_eq!(facts.version.as_deref(), Some("6.11"));
        assert!(is_raw_file_label(
            "SkyUI-12604-6-11-1778020881 (3)",
            "SkyUI-12604-6-11-1778020881 (3).7z"
        ));
    }

    #[test]
    fn address_library_keeps_spaces_and_a_single_component_version() {
        let facts = parse_file_name("Address Library for SKSE Plugins-32444-11-1700000000.7z");
        assert_eq!(facts.name, "Address Library for SKSE Plugins");
        assert_eq!(facts.nexus_mod_id.as_deref(), Some("32444"));
        assert_eq!(facts.version.as_deref(), Some("11"));
    }

    #[test]
    fn a_letter_suffix_and_a_missing_timestamp_stay_in_the_version() {
        let facts = parse_file_name("Foo-15-5-2SE.zip");
        assert_eq!(facts.name, "Foo");
        assert_eq!(facts.nexus_mod_id.as_deref(), Some("15"));
        assert_eq!(facts.version.as_deref(), Some("5.2SE"));

        let dashed = parse_file_name("SkyUI-12604-6-11.7z");
        assert_eq!(dashed.name, "SkyUI");
        assert_eq!(dashed.version.as_deref(), Some("6.11"));
    }

    #[test]
    fn a_name_without_an_id_is_only_cleaned() {
        let plain = parse_file_name("some_cool_mod (3).zip");
        assert_eq!(plain.name, "some cool mod");
        assert_eq!(plain.nexus_mod_id, None);
        assert_eq!(plain.version, None);

        let dotted = parse_file_name("Lux-2.2.6.7z");
        assert_eq!(dotted.name, "Lux");
        assert_eq!(dotted.nexus_mod_id, None);
        assert_eq!(dotted.version.as_deref(), Some("2.2.6"));

        let letters = parse_file_name("Widget-5.2SE.zip");
        assert_eq!(letters.name, "Widget");
        assert_eq!(letters.version.as_deref(), Some("5.2SE"));

        let skse = parse_file_name("skse64_2_02_06.7z");
        assert_eq!(skse.name, "skse64 2 02 06");
        assert_eq!(skse.version, None);
        assert_eq!(skse.nexus_mod_id, None);
    }

    #[test]
    fn camel_case_splits_only_when_both_sides_are_words() {
        assert_eq!(parse_file_name("EngineFixes.zip").name, "Engine Fixes");
        assert_eq!(parse_file_name("SkyUI.zip").name, "SkyUI");
        assert_eq!(parse_file_name("TrueHUD.zip").name, "TrueHUD");
        assert_eq!(parse_file_name("BetterEnd-4.0.10.jar").name, "Better End");
        assert_eq!(
            parse_file_name("BetterEnd-4.0.10.jar").version.as_deref(),
            Some("4.0.10")
        );
    }

    #[test]
    fn a_marked_or_underscored_version_leaves_the_title() {
        let marked = parse_file_name("Mod_v2.zip");
        assert_eq!(marked.name, "Mod");
        assert_eq!(marked.version.as_deref(), Some("2"));

        let dotted = parse_file_name("Foo-v1.2.7z");
        assert_eq!(dotted.name, "Foo");
        assert_eq!(dotted.version.as_deref(), Some("1.2"));

        let underscored = parse_file_name("Foo_1_2_3.zip");
        assert_eq!(underscored.name, "Foo");
        assert_eq!(underscored.version.as_deref(), Some("1.2.3"));
    }

    #[test]
    fn edition_suffixes_stay_in_the_title() {
        let smim = parse_file_name("SMIM SE 2-08-659-2-08.7z");
        assert_eq!(smim.name, "SMIM SE");
        assert_eq!(smim.nexus_mod_id.as_deref(), Some("659"));
        assert_eq!(smim.version.as_deref(), Some("2.08"));

        let map =
            parse_file_name("9.0.1 A Quality World Map - Vivid with Stone Roads-5804-9-0-1.zip");
        assert_eq!(map.name, "A Quality World Map - Vivid with Stone Roads");
        assert_eq!(map.nexus_mod_id.as_deref(), Some("5804"));
        assert_eq!(map.version.as_deref(), Some("9.0.1"));

        let sky = parse_file_name("SkyUI_SE.zip");
        assert_eq!(sky.name, "SkyUI SE");
        assert_eq!(sky.version, None);
    }

    #[test]
    fn a_spaced_nexus_file_name_drops_the_stamp_and_the_token() {
        let civil = parse_file_name(
            "Civil War Questline Fixed AF 199639 1.6 2026-11-24T13-11Z s6Og04g9U.zip",
        );
        assert_eq!(civil.name, "Civil War Questline Fixed AF");
        assert_eq!(civil.nexus_mod_id.as_deref(), Some("199639"));
        assert_eq!(civil.version.as_deref(), Some("1.6"));

        let address = parse_file_name(
            "Address Library All in One (1.7.104.0) v13 32444 13 2026-08-27T15-29Z Ae46W7Fw2 (2).zip",
        );
        assert_eq!(address.name, "Address Library All in One (1.7.104.0)");
        assert_eq!(address.nexus_mod_id.as_deref(), Some("32444"));
        assert_eq!(address.version.as_deref(), Some("13"));

        let patch = parse_file_name(
            "Unofficial Skyrim Special Edition Patch 266 4.3.9c 2026-09-05T22-22Z lNkz9LChN.7z",
        );
        assert_eq!(patch.name, "Unofficial Skyrim Special Edition Patch");
        assert_eq!(patch.nexus_mod_id.as_deref(), Some("266"));
        assert_eq!(patch.version.as_deref(), Some("4.3.9c"));

        let engine = parse_file_name(
            "Engine Fixes 7.0.21 beta for Skyrim AE 1.7.99 17230 7.0.21 2026-08-24T16-52Z vlGiB9OkV.7z",
        );
        assert_eq!(engine.name, "Engine Fixes beta for Skyrim AE 1.7.99");
        assert_eq!(engine.nexus_mod_id.as_deref(), Some("17230"));
        assert_eq!(engine.version.as_deref(), Some("7.0.21"));

        let papyrus = parse_file_name(
            "PapyrusUtil AE SE - Scripting Utility Functions 13048 4.8 2026-08-30T01-51Z OyYrPu3oN.zip",
        );
        assert_eq!(
            papyrus.name,
            "Papyrus Util AE SE - Scripting Utility Functions"
        );
        assert_eq!(papyrus.nexus_mod_id.as_deref(), Some("13048"));
        assert_eq!(papyrus.version.as_deref(), Some("4.8"));
    }

    #[test]
    fn a_hyphenated_title_is_not_split_before_the_id() {
        let facts = parse_file_name("Engine Fixes - SKSE64 Preloader-17230-1-6-1170.7z");
        assert_eq!(facts.name, "Engine Fixes - SKSE64 Preloader");
        assert_eq!(facts.nexus_mod_id.as_deref(), Some("17230"));
        assert_eq!(facts.version.as_deref(), Some("1.6.1170"));
    }
}
