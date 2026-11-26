//! What a Minecraft jar's file name states on its own.
//!
//! The Core already cleans a generic file name. This module only claims a
//! name it can actually split into a mod title, a mod version, a loader and
//! a Minecraft version. A Nexus id in the name is left to the Core.

use modlab_core::game_adapter::ArchiveMetadata;
use std::path::Path;

const LOADERS: &[&str] = &["fabric", "forge", "neoforge", "quilt"];

const DISPLAY: &[(&str, &str)] = &[
    ("jei", "Just Enough Items"),
    ("fabricapi", "Fabric API"),
    ("sodium", "Sodium"),
    ("journeymap", "JourneyMap"),
    ("appleskin", "AppleSkin"),
    ("iris", "Iris"),
    ("create", "Create"),
    ("betterend", "Better End"),
    ("betterendremastered", "Better End Remastered"),
];

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Word,
    Loader,
    Game,
    Version,
}

struct Tok {
    text: String,
    kind: Kind,
    /// Game version came from an `mc` prefix, so a look-alike `1.x` nearby
    /// is the mod's own version (`iris-…-1.11.7+mc26.3`).
    mc_prefix: bool,
}

/// `None` when the name is not a jar this module can split.
#[must_use]
pub fn parse(file_name: &str) -> Option<ArchiveMetadata> {
    if modlab_core::filename_meta::parse_file_name(file_name)
        .nexus_mod_id
        .is_some()
    {
        return None;
    }
    let stem = stem_of(file_name);
    if stem.is_empty() {
        return None;
    }
    let (mut loaders, mut games, rest) = take_brackets(&stem);
    let mut tokens = classify(&rest);
    promote_modern_games(&mut tokens);
    demote_classic_when_mc_present(&mut tokens);

    for token in &tokens {
        match token.kind {
            Kind::Loader => push_unique(&mut loaders, token.text.clone()),
            Kind::Game => push_unique(&mut games, token.text.clone()),
            Kind::Word | Kind::Version => {}
        }
    }
    let version = tokens.iter().rev().find_map(|token| {
        if token.kind == Kind::Version {
            Some(token.text.clone())
        } else {
            None
        }
    });
    let name = display_name(&tokens, &loaders)?;
    if loaders.is_empty() && games.is_empty() && version.is_none() {
        return None;
    }
    Some(ArchiveMetadata {
        name: Some(name),
        version,
        game_versions: games,
        loaders,
        ..ArchiveMetadata::default()
    })
}

fn stem_of(file_name: &str) -> String {
    let base = Path::new(file_name)
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| file_name.trim().to_string());
    let base = base
        .strip_suffix(".disabled")
        .or_else(|| base.strip_suffix(".DISABLED"))
        .unwrap_or(&base)
        .to_string();
    let base = strip_one_extension(&base);
    strip_copy_suffixes(&base)
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

fn take_brackets(input: &str) -> (Vec<String>, Vec<String>, String) {
    let mut rest = input.trim().to_string();
    let mut loaders = Vec::new();
    let mut games = Vec::new();
    while rest.starts_with('[') {
        let Some(end) = rest.find(']') else {
            break;
        };
        let inside = rest[1..end].trim();
        let after = rest[end + 1..].trim_start().to_string();
        if let Some(loader) = loader_id(inside) {
            push_unique(&mut loaders, loader);
        } else if let Some(game) = classic_or_mc(inside) {
            push_unique(&mut games, game);
        } else {
            break;
        }
        rest = after;
    }
    (loaders, games, rest)
}

fn classify(stem: &str) -> Vec<Tok> {
    stem.split(|c: char| c == '-' || c == '_' || c == '+' || c.is_whitespace())
        .filter(|token| !token.is_empty())
        .map(|token| {
            if let Some(loader) = loader_id(token) {
                return Tok {
                    text: loader,
                    kind: Kind::Loader,
                    mc_prefix: false,
                };
            }
            if let Some(game) = mc_game(token) {
                return Tok {
                    text: game,
                    kind: Kind::Game,
                    mc_prefix: true,
                };
            }
            if let Some(game) = classic_game(token) {
                return Tok {
                    text: game,
                    kind: Kind::Game,
                    mc_prefix: false,
                };
            }
            if let Some(version) = version_body(token) {
                return Tok {
                    text: version,
                    kind: Kind::Version,
                    mc_prefix: false,
                };
            }
            Tok {
                text: token.to_string(),
                kind: Kind::Word,
                mc_prefix: false,
            }
        })
        .collect()
}

fn promote_modern_games(tokens: &mut [Tok]) {
    let loader = tokens.iter().any(|token| token.kind == Kind::Loader);
    let other_version = tokens
        .iter()
        .any(|token| token.kind == Kind::Version && modern_game(&token.text).is_none());
    if !loader || !other_version {
        return;
    }
    for token in tokens.iter_mut() {
        if token.kind == Kind::Version && modern_game(&token.text).is_some() {
            token.kind = Kind::Game;
        }
    }
}

fn demote_classic_when_mc_present(tokens: &mut [Tok]) {
    let mc = tokens
        .iter()
        .any(|token| token.kind == Kind::Game && token.mc_prefix);
    if !mc {
        return;
    }
    for token in tokens.iter_mut() {
        if token.kind == Kind::Game && !token.mc_prefix {
            token.kind = Kind::Version;
        }
    }
}

fn display_name(tokens: &[Tok], loaders: &[String]) -> Option<String> {
    let titled: Vec<&str> = tokens
        .iter()
        .filter(|token| token.kind != Kind::Version && token.kind != Kind::Game)
        .map(|token| token.text.as_str())
        .collect();
    if titled.is_empty() {
        return None;
    }
    if let Some(known) = lookup_display(&norm_key(&titled.join(""))) {
        return Some(known.to_string());
    }
    let words: Vec<&str> = tokens
        .iter()
        .filter(|token| token.kind == Kind::Word)
        .map(|token| token.text.as_str())
        .collect();
    if let Some(known) = lookup_display(&norm_key(&words.join(""))) {
        return Some(known.to_string());
    }
    for loader in loaders {
        let with = norm_key(&format!("{loader}{}", words.join("")));
        if let Some(known) = lookup_display(&with) {
            return Some(known.to_string());
        }
    }
    if words.is_empty() {
        return None;
    }
    let joined = words.join(" ");
    let polished = modlab_core::filename_meta::polish_display_name(&joined);
    let titled = title_case_lower(&polished);
    if titled.is_empty() {
        None
    } else {
        Some(titled)
    }
}

fn lookup_display(key: &str) -> Option<&'static str> {
    DISPLAY
        .iter()
        .find(|(id, _)| *id == key)
        .map(|(_, name)| *name)
}

fn norm_key(raw: &str) -> String {
    raw.chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .flat_map(|ch| ch.to_lowercase())
        .collect()
}

fn title_case_lower(name: &str) -> String {
    name.split_whitespace()
        .map(|word| {
            if word.chars().any(|ch| ch.is_uppercase()) {
                return word.to_string();
            }
            let mut chars = word.chars();
            let Some(first) = chars.next() else {
                return String::new();
            };
            let mut out: String = first.to_uppercase().collect();
            out.push_str(chars.as_str());
            out
        })
        .filter(|word| !word.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

fn loader_id(token: &str) -> Option<String> {
    let lower = token.to_ascii_lowercase();
    LOADERS
        .iter()
        .find(|id| **id == lower)
        .map(|id| (*id).to_string())
}

fn classic_or_mc(token: &str) -> Option<String> {
    mc_game(token).or_else(|| classic_game(token))
}

fn mc_game(token: &str) -> Option<String> {
    let rest = token
        .strip_prefix("mc")
        .or_else(|| token.strip_prefix("MC"))?;
    let version = version_body(rest)?;
    let count = version.split('.').count();
    if (2..=4).contains(&count) {
        Some(version)
    } else {
        None
    }
}

fn classic_game(token: &str) -> Option<String> {
    let version = version_body(token)?;
    let mut parts = version.split('.');
    let major = parts.next()?;
    let minor = parts.next()?;
    let patch = parts.next();
    if parts.next().is_some() || major != "1" {
        return None;
    }
    let minor_n: u32 = minor.parse().ok()?;
    if !(7..=30).contains(&minor_n) {
        return None;
    }
    match patch {
        None => Some(version),
        Some(patch) if !patch.is_empty() && patch.bytes().all(|b| b.is_ascii_digit()) => {
            Some(version)
        }
        Some(_) => None,
    }
}

fn modern_game(token: &str) -> Option<String> {
    let version = version_body(token)?;
    let mut parts = version.split('.');
    let major = parts.next()?;
    let minor = parts.next()?;
    if parts.next().is_some() {
        return None;
    }
    if !minor.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let major_n: u32 = major.parse().ok()?;
    if (20..=99).contains(&major_n) {
        Some(version)
    } else {
        None
    }
}

fn version_body(token: &str) -> Option<String> {
    let token = if let Some(rest) = token.strip_prefix('v').or_else(|| token.strip_prefix('V')) {
        if rest.chars().next().is_some_and(|c| c.is_ascii_digit()) {
            rest
        } else {
            token
        }
    } else {
        token
    };
    let parts: Vec<&str> = token.split('.').collect();
    if !(2..=5).contains(&parts.len()) {
        return None;
    }
    let mut numeric = 0;
    for (index, part) in parts.iter().enumerate() {
        let last = index + 1 == parts.len();
        if part.is_empty() || part.len() > 8 {
            return None;
        }
        if part.bytes().all(|b| b.is_ascii_digit()) {
            if part.len() > 1 && part.starts_with('0') {
                return None;
            }
            numeric += 1;
            continue;
        }
        if last && single_letter(part) && numeric >= 2 {
            continue;
        }
        if last && trailing_letters(part) {
            numeric += 1;
            continue;
        }
        return None;
    }
    if numeric < 2 {
        return None;
    }
    Some(token.to_string())
}

fn single_letter(part: &str) -> bool {
    part.len() == 1 && part.chars().all(|c| c.is_ascii_alphabetic())
}

fn trailing_letters(part: &str) -> bool {
    let digits = part.chars().take_while(|c| c.is_ascii_digit()).count();
    if digits == 0 || digits == part.len() {
        return false;
    }
    let letters = &part[digits..];
    !letters.is_empty()
        && letters.len() <= 4
        && letters.chars().all(|c| c.is_ascii_alphabetic())
        && !(digits > 1 && part.starts_with('0'))
}

fn push_unique(slot: &mut Vec<String>, value: String) {
    if slot.iter().any(|seen| seen.eq_ignore_ascii_case(&value)) {
        return;
    }
    slot.push(value);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parsed(name: &str) -> ArchiveMetadata {
        parse(name).unwrap_or_else(|| panic!("expected a parse of {name}"))
    }

    #[test]
    fn jar_names_split_into_title_version_loader_and_game() {
        let sodium = parsed("sodium-fabric-0.5.8+mc1.20.1.jar");
        assert_eq!(sodium.name.as_deref(), Some("Sodium"));
        assert_eq!(sodium.version.as_deref(), Some("0.5.8"));
        assert_eq!(sodium.loaders, vec!["fabric".to_string()]);
        assert_eq!(sodium.game_versions, vec!["1.20.1".to_string()]);

        let jei = parsed("jei-1.20.1-forge-15.3.0.4.jar");
        assert_eq!(jei.name.as_deref(), Some("Just Enough Items"));
        assert_eq!(jei.version.as_deref(), Some("15.3.0.4"));
        assert_eq!(jei.loaders, vec!["forge".to_string()]);
        assert_eq!(jei.game_versions, vec!["1.20.1".to_string()]);

        let api = parsed("fabric-api-0.92.0+1.20.1.jar");
        assert_eq!(api.name.as_deref(), Some("Fabric API"));
        assert_eq!(api.version.as_deref(), Some("0.92.0"));
        assert_eq!(api.loaders, vec!["fabric".to_string()]);
        assert_eq!(api.game_versions, vec!["1.20.1".to_string()]);

        let create = parsed("create-1.20.1-0.5.1.f.jar");
        assert_eq!(create.name.as_deref(), Some("Create"));
        assert_eq!(create.version.as_deref(), Some("0.5.1.f"));
        assert!(create.loaders.is_empty());
        assert_eq!(create.game_versions, vec!["1.20.1".to_string()]);

        let end = parsed("BetterEnd-4.0.10.jar");
        assert_eq!(end.name.as_deref(), Some("Better End"));
        assert_eq!(end.version.as_deref(), Some("4.0.10"));
        assert!(end.game_versions.is_empty());

        let bracket = parsed("[1.20.1] Mod Name 1.2.jar");
        assert_eq!(bracket.name.as_deref(), Some("Mod Name"));
        assert_eq!(bracket.version.as_deref(), Some("1.2"));
        assert_eq!(bracket.game_versions, vec!["1.20.1".to_string()]);

        let neo = parsed("ModName-neoforge-1.21.1-2.0.jar");
        assert_eq!(neo.name.as_deref(), Some("Mod Name"));
        assert_eq!(neo.version.as_deref(), Some("2.0"));
        assert_eq!(neo.loaders, vec!["neoforge".to_string()]);
        assert_eq!(neo.game_versions, vec!["1.21.1".to_string()]);

        let map = parsed("journeymap-1.20.1-5.9.18-fabric.jar");
        assert_eq!(map.name.as_deref(), Some("JourneyMap"));
        assert_eq!(map.version.as_deref(), Some("5.9.18"));
        assert_eq!(map.loaders, vec!["fabric".to_string()]);
        assert_eq!(map.game_versions, vec!["1.20.1".to_string()]);
    }

    #[test]
    fn a_modern_minecraft_version_is_not_the_mod_version() {
        let jei = parsed("jei-26.3-neoforge-31.14.0.72.jar");
        assert_eq!(jei.name.as_deref(), Some("Just Enough Items"));
        assert_eq!(jei.version.as_deref(), Some("31.14.0.72"));
        assert_eq!(jei.loaders, vec!["neoforge".to_string()]);
        assert_eq!(jei.game_versions, vec!["26.3".to_string()]);

        let iris = parsed("iris-neoforge-1.11.7+mc26.3.jar");
        assert_eq!(iris.name.as_deref(), Some("Iris"));
        assert_eq!(iris.version.as_deref(), Some("1.11.7"));
        assert_eq!(iris.loaders, vec!["neoforge".to_string()]);
        assert_eq!(iris.game_versions, vec!["26.3".to_string()]);

        let apple = parsed("appleskin-fabric-mc26.3-3.0.10.jar");
        assert_eq!(apple.name.as_deref(), Some("AppleSkin"));
        assert_eq!(apple.version.as_deref(), Some("3.0.10"));
        assert_eq!(apple.loaders, vec!["fabric".to_string()]);
        assert_eq!(apple.game_versions, vec!["26.3".to_string()]);

        let remaster = parsed("BetterEndRemastered-30.5.8 (2).jar");
        assert_eq!(remaster.name.as_deref(), Some("Better End Remastered"));
        assert_eq!(remaster.version.as_deref(), Some("30.5.8"));
        assert!(remaster.loaders.is_empty());
        assert!(remaster.game_versions.is_empty());
    }

    #[test]
    fn a_bracketed_loader_is_not_part_of_the_title() {
        let meta = parsed("[Forge] Mod Name-1.2.jar");
        assert_eq!(meta.name.as_deref(), Some("Mod Name"));
        assert_eq!(meta.version.as_deref(), Some("1.2"));
        assert_eq!(meta.loaders, vec!["forge".to_string()]);
    }

    #[test]
    fn a_nexus_file_name_is_left_to_the_core() {
        assert!(parse("SkyUI-12604-6-11-1778020881.7z").is_none());
        assert!(parse("notes.txt").is_none());
    }
}
