//! Facts a Skyrim archive or installed folder states about itself.
//!
//! Read locally, and only these places: `fomod/info.xml` and a Mod
//! Organizer `meta.ini` sitting next to the mod. Nothing is downloaded.
//! A value the file does not state stays empty.

use std::fs;
use std::path::{Path, PathBuf};

use modlab_core::game_adapter::ArchiveMetadata;

const INFO_SUFFIX: &str = "fomod/info.xml";
const META_SUFFIX: &str = "meta.ini";

/// Read one archive or one installed mod folder. `None` when the path is
/// missing or neither file says anything this module knows.
#[must_use]
pub fn read_path(path: &Path) -> Option<ArchiveMetadata> {
    let meta = if path.is_dir() {
        read_dir(path)
    } else if path.is_file() {
        read_archive(path)
    } else {
        return None;
    };
    if meta.is_empty() {
        None
    } else {
        Some(meta)
    }
}

fn read_dir(dir: &Path) -> ArchiveMetadata {
    let info = find_in_tree(dir, "fomod", "info.xml").and_then(|path| fs::read(&path).ok());
    let ini = find_named(dir, "meta.ini").and_then(|path| fs::read(&path).ok());
    let mut meta = merge(info.as_deref(), ini.as_deref());
    add_plugin_name(dir, &mut meta);
    meta
}

fn read_archive(path: &Path) -> ArchiveMetadata {
    let entries = modlab_core::archive::peek_named_entries(path, &[INFO_SUFFIX, META_SUFFIX]);
    let info = shortest(&entries, INFO_SUFFIX).map(|entry| entry.bytes.as_slice());
    let ini = shortest(&entries, META_SUFFIX).map(|entry| entry.bytes.as_slice());
    let mut meta = merge(info, ini);
    add_plugin_name(path, &mut meta);
    meta
}

/// One plugin's file name, when the archive never wrote a title. Several
/// plugins, or a list cut short, stay empty — guessing would pick the wrong
/// one. The five vanilla masters are not the mod's plugin.
fn add_plugin_name(path: &Path, meta: &mut ArchiveMetadata) {
    if meta
        .name
        .as_ref()
        .is_some_and(|name| !name.trim().is_empty())
    {
        return;
    }
    if heavy_7z(path) {
        return;
    }
    let (names, truncated) = modlab_core::archive::list_entry_names(path);
    if truncated {
        return;
    }
    meta.name = sole_plugin_name(&names);
}

fn heavy_7z(path: &Path) -> bool {
    let ext = path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or_default();
    if !ext.eq_ignore_ascii_case("7z") {
        return false;
    }
    fs::metadata(path)
        .map(|meta| meta.len() > 64 * 1024 * 1024)
        .unwrap_or(true)
}

fn sole_plugin_name(paths: &[String]) -> Option<String> {
    let mut found = None;
    for path in paths {
        let file = path.rsplit(['/', '\\']).next().unwrap_or(path);
        let lower = file.to_ascii_lowercase();
        if !(lower.ends_with(".esp") || lower.ends_with(".esm") || lower.ends_with(".esl")) {
            continue;
        }
        if is_vanilla_plugin(&lower) {
            continue;
        }
        if found.is_some() {
            return None;
        }
        found = Some(file.to_string());
    }
    let file = found?;
    let stem = Path::new(&file)
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())?;
    let name = modlab_core::filename_meta::polish_display_name(&stem);
    if name.is_empty() {
        None
    } else {
        Some(name)
    }
}

fn is_vanilla_plugin(file: &str) -> bool {
    matches!(
        file,
        "skyrim.esm" | "update.esm" | "dawnguard.esm" | "hearthfires.esm" | "dragonborn.esm"
    )
}

fn shortest<'a>(
    entries: &'a [modlab_core::archive::PeekedEntry],
    suffix: &str,
) -> Option<&'a modlab_core::archive::PeekedEntry> {
    entries
        .iter()
        .filter(|entry| ends_with_suffix(&entry.path, suffix))
        .min_by_key(|entry| entry.path.len())
}

fn ends_with_suffix(path: &str, suffix: &str) -> bool {
    let path = path.replace('\\', "/").to_ascii_lowercase();
    let suffix = suffix.replace('\\', "/").to_ascii_lowercase();
    path == suffix || path.ends_with(&format!("/{suffix}"))
}

fn find_named(dir: &Path, file_name: &str) -> Option<PathBuf> {
    named_in(dir, file_name).or_else(|| {
        fs::read_dir(dir).ok()?.find_map(|entry| {
            let entry = entry.ok()?;
            let path = entry.path();
            if entry.file_type().ok()?.is_dir() {
                named_in(&path, file_name)
            } else {
                None
            }
        })
    })
}

fn find_in_tree(dir: &Path, folder: &str, file_name: &str) -> Option<PathBuf> {
    info_in(dir, folder, file_name).or_else(|| {
        fs::read_dir(dir).ok()?.find_map(|entry| {
            let entry = entry.ok()?;
            if entry.file_type().ok()?.is_dir() {
                info_in(&entry.path(), folder, file_name)
            } else {
                None
            }
        })
    })
}

fn info_in(dir: &Path, folder: &str, file_name: &str) -> Option<PathBuf> {
    let fomod = fs::read_dir(dir).ok()?.find_map(|entry| {
        let entry = entry.ok()?;
        if entry.file_type().ok()?.is_dir()
            && entry
                .file_name()
                .to_string_lossy()
                .eq_ignore_ascii_case(folder)
        {
            Some(entry.path())
        } else {
            None
        }
    })?;
    named_in(&fomod, file_name)
}

fn named_in(dir: &Path, file_name: &str) -> Option<PathBuf> {
    fs::read_dir(dir).ok()?.find_map(|entry| {
        let entry = entry.ok()?;
        if entry
            .file_name()
            .to_string_lossy()
            .eq_ignore_ascii_case(file_name)
        {
            Some(entry.path())
        } else {
            None
        }
    })
}

fn merge(info_xml: Option<&[u8]>, meta_ini: Option<&[u8]>) -> ArchiveMetadata {
    let mut meta = ArchiveMetadata::default();
    if let Some(bytes) = info_xml {
        apply_info(&mut meta, &decode_text(bytes));
    }
    if let Some(bytes) = meta_ini {
        apply_ini(&mut meta, &decode_text(bytes));
    }
    meta
}

fn apply_info(meta: &mut ArchiveMetadata, xml: &str) {
    let Ok(doc) = roxmltree::Document::parse(xml) else {
        return;
    };
    if meta.name.is_none() {
        meta.name = tag_text(&doc, &["Name", "moduleName"]);
    }
    if meta.version.is_none() {
        meta.version = tag_text(&doc, &["Version"]);
    }
    if meta.author.is_none() {
        meta.author = tag_text(&doc, &["Author"]);
    }
    if meta.website.is_none() {
        meta.website = tag_text(&doc, &["Website"]);
    }
    if meta.category_raw.is_none() {
        let groups = group_labels(&doc);
        if !groups.is_empty() {
            meta.category_raw = Some(groups.join(", "));
        }
    }
}

fn apply_ini(meta: &mut ArchiveMetadata, text: &str) {
    let values = ini_values(text);
    if meta.nexus_mod_id.is_none() {
        meta.nexus_mod_id = values
            .get("modid")
            .filter(|id| is_positive_int(id))
            .cloned();
    }
    if meta.version.is_none() {
        meta.version = values.get("version").cloned();
    }
    // `nexusCategory` is the Nexus id. `category` is MO2's own field and is
    // often "-1," even when Nexus had a real number. Prefer the Nexus key.
    let nexus_category = values.get("nexuscategory").map(String::as_str);
    let mo2_category = values.get("category").map(String::as_str);
    if meta.category_nexus_id.is_none() {
        meta.category_nexus_id = nexus_category
            .and_then(first_category_id)
            .or_else(|| mo2_category.and_then(first_category_id));
    }
    if meta.category_raw.is_none() {
        let text = nexus_category
            .filter(|value| first_category_id(value).is_some())
            .or(mo2_category);
        if let Some(category) = text {
            meta.category_raw = Some(category.to_string());
        }
    }
    if meta.page_url.is_none() {
        meta.page_url = values.get("url").filter(|url| is_http(url)).cloned();
    }
    if meta.source_host.is_none() && (meta.nexus_mod_id.is_some() || meta.page_url.is_some()) {
        meta.source_host = Some("nexus".to_string());
    }
}

fn tag_text(doc: &roxmltree::Document<'_>, names: &[&str]) -> Option<String> {
    doc.descendants().find_map(|node| {
        if !node.is_element() {
            return None;
        }
        let tag = node.tag_name().name();
        if !names.iter().any(|name| tag.eq_ignore_ascii_case(name)) {
            return None;
        }
        usable(node.text().unwrap_or(""))
    })
}

fn group_labels(doc: &roxmltree::Document<'_>) -> Vec<String> {
    let mut labels = Vec::new();
    for node in doc.descendants() {
        if !node.is_element() || !node.tag_name().name().eq_ignore_ascii_case("element") {
            continue;
        }
        let under_groups = node.ancestors().any(|ancestor| {
            ancestor.is_element() && ancestor.tag_name().name().eq_ignore_ascii_case("groups")
        });
        if !under_groups {
            continue;
        }
        let text = node.text().unwrap_or("").trim();
        let text = if text.is_empty() {
            node.attribute("name").unwrap_or("").trim()
        } else {
            text
        };
        if let Some(label) = usable(text) {
            if !labels.iter().any(|existing| existing == &label) {
                labels.push(label);
            }
        }
    }
    labels
}

fn ini_values(text: &str) -> std::collections::BTreeMap<String, String> {
    let mut map = std::collections::BTreeMap::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty()
            || line.starts_with('[')
            || line.starts_with('#')
            || line.starts_with(';')
        {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim().to_ascii_lowercase();
        let value = value.trim().trim_matches('"').trim();
        if key.is_empty() || value.is_empty() {
            continue;
        }
        map.entry(key).or_insert_with(|| value.to_string());
    }
    map
}

fn first_category_id(raw: &str) -> Option<String> {
    let first = raw.split([',', ';']).next()?.trim();
    if is_positive_int(first) {
        Some(first.to_string())
    } else {
        None
    }
}

fn is_positive_int(value: &str) -> bool {
    !value.is_empty()
        && value != "0"
        && !value.starts_with('-')
        && value.bytes().all(|b| b.is_ascii_digit())
}

fn is_http(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    lower.starts_with("https://") || lower.starts_with("http://")
}

fn usable(text: &str) -> Option<String> {
    let text = text.trim();
    if text.is_empty() || text.contains('$') || text.contains('{') || text.contains('}') {
        None
    } else {
        Some(text.to_string())
    }
}

/// UTF-8, or UTF-16 (with or without a BOM). FOMOD tools often write UTF-16 LE.
fn decode_text(bytes: &[u8]) -> String {
    if let Some(rest) = bytes.strip_prefix(&[0xFF, 0xFE]) {
        return decode_with(encoding_rs::UTF_16LE, rest);
    }
    if let Some(rest) = bytes.strip_prefix(&[0xFE, 0xFF]) {
        return decode_with(encoding_rs::UTF_16BE, rest);
    }
    let bytes = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(bytes);
    if looks_like_utf16_le(bytes) {
        return decode_with(encoding_rs::UTF_16LE, bytes);
    }
    decode_with(encoding_rs::UTF_8, bytes)
}

fn decode_with(encoding: &'static encoding_rs::Encoding, bytes: &[u8]) -> String {
    let (text, _, _) = encoding.decode(bytes);
    text.into_owned()
}

fn looks_like_utf16_le(bytes: &[u8]) -> bool {
    if bytes.len() < 8 || !bytes.len().is_multiple_of(2) {
        return false;
    }
    let sample = &bytes[..bytes.len().min(80)];
    let highs = sample
        .iter()
        .skip(1)
        .step_by(2)
        .filter(|b| **b == 0)
        .count();
    highs * 2 >= sample.len() / 2
}

#[cfg(test)]
mod tests {
    use super::*;

    fn utf16_le(text: &str) -> Vec<u8> {
        let mut out = vec![0xFF, 0xFE];
        for unit in text.encode_utf16() {
            out.extend(unit.to_le_bytes());
        }
        out
    }

    const INFO: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<fomod>
  <Name>SkyUI</Name>
  <Author>schlangster</Author>
  <Version>5.2SE</Version>
  <Website>https://example.test/skyui</Website>
  <Groups>
    <element>User Interface</element>
    <element>Gameplay</element>
  </Groups>
</fomod>
"#;

    #[test]
    fn info_xml_utf8_and_utf16_fill_the_same_fields() {
        let utf8 = merge(Some(INFO.as_bytes()), None);
        let utf16 = merge(Some(&utf16_le(INFO)), None);
        for meta in [&utf8, &utf16] {
            assert_eq!(meta.name.as_deref(), Some("SkyUI"));
            assert_eq!(meta.version.as_deref(), Some("5.2SE"));
            assert_eq!(meta.author.as_deref(), Some("schlangster"));
            assert_eq!(meta.website.as_deref(), Some("https://example.test/skyui"));
            assert_eq!(
                meta.category_raw.as_deref(),
                Some("User Interface, Gameplay")
            );
        }
    }

    #[test]
    fn meta_ini_fills_the_id_and_does_not_cover_info_xml() {
        let ini = "\
[General]
modid=12604
version=6.11
category=51
url=\"https://www.nexusmods.com/skyrimspecialedition/mods/12604\"
nexusFileStatus=1
";
        let meta = merge(Some(INFO.as_bytes()), Some(ini.as_bytes()));
        assert_eq!(meta.name.as_deref(), Some("SkyUI"));
        assert_eq!(meta.version.as_deref(), Some("5.2SE"));
        assert_eq!(meta.nexus_mod_id.as_deref(), Some("12604"));
        assert_eq!(meta.category_nexus_id.as_deref(), Some("51"));
        assert_eq!(
            meta.category_raw.as_deref(),
            Some("User Interface, Gameplay")
        );
        assert_eq!(
            meta.page_url.as_deref(),
            Some("https://www.nexusmods.com/skyrimspecialedition/mods/12604")
        );
        assert_eq!(meta.source_host.as_deref(), Some("nexus"));

        let only_ini = merge(None, Some(ini.as_bytes()));
        assert_eq!(only_ini.version.as_deref(), Some("6.11"));
        assert_eq!(only_ini.category_raw.as_deref(), Some("51"));
        assert_eq!(only_ini.category_nexus_id.as_deref(), Some("51"));

        let both = "\
[General]
category=\"-1,\"
nexusCategory=42
";
        let from_nexus = merge(None, Some(both.as_bytes()));
        assert_eq!(from_nexus.category_nexus_id.as_deref(), Some("42"));
        assert_eq!(from_nexus.category_raw.as_deref(), Some("42"));
    }

    #[test]
    fn a_folder_and_a_zip_are_both_read() {
        let dir = tempfile::tempdir().unwrap();
        let fomod = dir.path().join("fomod");
        fs::create_dir(&fomod).unwrap();
        fs::write(fomod.join("info.xml"), INFO).unwrap();
        fs::write(dir.path().join("meta.ini"), "modid=12604\ncategory=51\n").unwrap();
        let from_dir = read_path(dir.path()).expect("folder");
        assert_eq!(from_dir.name.as_deref(), Some("SkyUI"));
        assert_eq!(from_dir.nexus_mod_id.as_deref(), Some("12604"));
        assert_eq!(from_dir.version.as_deref(), Some("5.2SE"));
    }

    #[test]
    fn one_plugin_names_a_mod_that_has_no_title_and_two_do_not() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("SkyUI_SE.esp"), b"esp").unwrap();
        fs::write(dir.path().join("Skyrim.esm"), b"esm").unwrap();
        let meta = read_path(dir.path()).expect("one plugin");
        assert_eq!(meta.name.as_deref(), Some("SkyUI SE"));

        fs::write(dir.path().join("Second.esp"), b"esp").unwrap();
        assert!(read_path(dir.path()).is_none());
    }

    #[test]
    fn an_info_xml_title_wins_over_the_plugin_file() {
        let dir = tempfile::tempdir().unwrap();
        let fomod = dir.path().join("fomod");
        fs::create_dir(&fomod).unwrap();
        fs::write(fomod.join("info.xml"), INFO).unwrap();
        fs::write(dir.path().join("OtherName.esp"), b"esp").unwrap();
        let meta = read_path(dir.path()).expect("info");
        assert_eq!(meta.name.as_deref(), Some("SkyUI"));
    }
}
