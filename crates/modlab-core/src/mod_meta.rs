//! Per-field provenance for a mod row.
//!
//! A layer may fill a field that is empty, or replace one that came from a
//! less reliable layer. `user` is never replaced: a name or a tag the human
//! typed stays. Rank, highest last: filename, page, archive, api, user.

use std::collections::BTreeMap;

use crate::filename_meta::{self, FilenameFacts};
use crate::game_adapter::{ArchiveMetadata, DeclaredDependency};
use crate::{
    decode_dependencies, encode_dependencies, encode_string_list, Core, CoreError, ModSheet, Result,
};

/// Where a stored value came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Layer {
    Filename,
    Page,
    Archive,
    Api,
    User,
}

impl Layer {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Filename => "filename",
            Self::Page => "page",
            Self::Archive => "archive",
            Self::Api => "api",
            Self::User => "user",
        }
    }

    fn rank(self) -> u8 {
        match self {
            Self::Filename => 1,
            Self::Page => 2,
            Self::Archive => 3,
            Self::Api => 4,
            Self::User => 5,
        }
    }

    fn parse(raw: &str) -> Option<Self> {
        match raw {
            "filename" => Some(Self::Filename),
            "page" => Some(Self::Page),
            "archive" => Some(Self::Archive),
            "api" => Some(Self::Api),
            "user" => Some(Self::User),
            _ => None,
        }
    }
}

/// Fields a layer wants to write. `None` means "this layer has nothing to
/// say". `clear_category` is the human emptying their own tag.
#[derive(Debug, Clone, Default)]
pub struct Patch {
    pub name: Option<String>,
    pub version: Option<String>,
    pub category: Option<String>,
    pub clear_category: bool,
    pub category_raw: Option<String>,
    pub category_nexus_id: Option<String>,
    pub source_host: Option<String>,
    pub page_url: Option<String>,
    pub file_url: Option<String>,
    pub nexus_mod_id: Option<String>,
    pub author: Option<String>,
    pub website: Option<String>,
    pub game_versions: Option<Vec<String>>,
    pub dependencies: Option<Vec<DeclaredDependency>>,
    /// Opaque loader ids (`fabric`, `forge`, …). The Core stores them and
    /// does not decide what they mean.
    pub loaders: Option<Vec<String>>,
}

/// Write `patch` onto `mod_id`.
///
/// `seed` is only for a row this process just inserted: an absent source
/// may be stamped even when the name is already the parsed one. A later
/// refresh passes `seed = false`, so a name that is no longer the raw file
/// label is left alone when nobody recorded who set it.
pub fn apply_layer(
    core: &Core,
    mod_id: i64,
    layer: Layer,
    patch: &Patch,
    file_names: &[String],
    seed: bool,
) -> Result<()> {
    let Some(mut sheet) = core.select_mod_sheet(mod_id)? else {
        return Err(CoreError::Message(format!("mod {mod_id} introuvable")));
    };
    let mut sources = parse_sources(sheet.field_sources.as_deref());
    let mut changed = false;

    if let Some(value) = clean(patch.name.as_deref()) {
        if allows(
            layer,
            sources.get("name").copied(),
            &sheet.name,
            value,
            name_floor(&sheet.name, file_names),
            seed,
        ) {
            sheet.name = value.to_string();
            sources.insert("name", layer);
            changed = true;
        }
    }
    if let Some(value) = clean(patch.version.as_deref()) {
        if !is_placeholder(value)
            && allows(
                layer,
                sources.get("version").copied(),
                sheet.version.as_deref().unwrap_or(""),
                value,
                version_floor(sheet.version.as_deref().unwrap_or(""), file_names),
                seed,
            )
        {
            sheet.version = Some(value.to_string());
            sources.insert("version", layer);
            changed = true;
        }
    }
    if patch.clear_category && layer == Layer::User {
        sheet.category = None;
        sources.insert("category", Layer::User);
        changed = true;
    } else if let Some(value) = clean(patch.category.as_deref()) {
        if allows(
            layer,
            sources.get("category").copied(),
            sheet.category.as_deref().unwrap_or(""),
            value,
            Layer::User.rank(),
            seed,
        ) {
            sheet.category = Some(value.to_string());
            sources.insert("category", layer);
            changed = true;
        }
    }
    changed |= write_text(
        &mut sheet.category_raw,
        &mut sources,
        "category_raw",
        layer,
        patch.category_raw.as_deref(),
        0,
        seed,
    );
    changed |= write_text(
        &mut sheet.category_nexus_id,
        &mut sources,
        "category_nexus_id",
        layer,
        patch.category_nexus_id.as_deref(),
        0,
        seed,
    );
    changed |= write_text(
        &mut sheet.source_host,
        &mut sources,
        "source_host",
        layer,
        patch.source_host.as_deref(),
        Layer::Page.rank(),
        seed,
    );
    changed |= write_text(
        &mut sheet.page_url,
        &mut sources,
        "page_url",
        layer,
        patch.page_url.as_deref(),
        Layer::Page.rank(),
        seed,
    );
    changed |= write_text(
        &mut sheet.file_url,
        &mut sources,
        "file_url",
        layer,
        patch.file_url.as_deref(),
        Layer::Page.rank(),
        seed,
    );
    changed |= write_text(
        &mut sheet.nexus_mod_id,
        &mut sources,
        "nexus_mod_id",
        layer,
        patch.nexus_mod_id.as_deref(),
        0,
        seed,
    );
    changed |= write_text(
        &mut sheet.author,
        &mut sources,
        "author",
        layer,
        patch.author.as_deref(),
        0,
        seed,
    );
    changed |= write_text(
        &mut sheet.website,
        &mut sources,
        "website",
        layer,
        patch.website.as_deref(),
        0,
        seed,
    );
    if let Some(values) = &patch.game_versions {
        changed |= write_list(
            &mut sheet.game_versions,
            &mut sources,
            "game_versions",
            layer,
            values,
            seed,
        )?;
    }
    if let Some(values) = &patch.dependencies {
        changed |= write_dep_list(
            &mut sheet.dependencies,
            &mut sources,
            "dependencies",
            layer,
            values,
            seed,
        )?;
    }
    if let Some(values) = &patch.loaders {
        changed |= write_list(
            &mut sheet.loaders,
            &mut sources,
            "loaders",
            layer,
            values,
            seed,
        )?;
    }

    if changed {
        sheet.field_sources = Some(encode_sources(&sources));
        core.save_mod_sheet(mod_id, &sheet)?;
    }
    Ok(())
}

/// Filename layer, then an optional archive read. `keep_tool_label` leaves
/// a loader's existing name and version alone (SKSE stays SKSE).
#[allow(clippy::too_many_arguments)]
pub fn refresh_stored(
    core: &Core,
    mod_id: i64,
    file_names: &[String],
    derived_host: Option<&str>,
    derived_page: Option<&str>,
    archive: Option<&ArchiveMetadata>,
    filename_extra: Option<&ArchiveMetadata>,
    keep_tool_label: bool,
) -> Result<()> {
    let sheet = core
        .select_mod_sheet(mod_id)?
        .ok_or_else(|| CoreError::Message(format!("mod {mod_id} introuvable")))?;
    let facts = best_facts(file_names);
    let mut filename = patch_from_facts(&facts);
    if let Some(extra) = filename_extra {
        merge_filename_extra(&mut filename, extra);
    }
    if let Some(host) = clean(derived_host) {
        filename.source_host = Some(host.to_string());
    }
    if let Some(page) = clean(derived_page) {
        filename.page_url = Some(page.to_string());
    }
    if keep_tool_label {
        protect_tool(&sheet, &mut filename, file_names);
    }
    apply_layer(core, mod_id, Layer::Filename, &filename, file_names, false)?;

    if let Some(archive) = archive {
        let mut patch = patch_from_archive(archive);
        let sheet = core
            .select_mod_sheet(mod_id)?
            .ok_or_else(|| CoreError::Message(format!("mod {mod_id} introuvable")))?;
        // A host the browser or a previous layer already stored stays. The
        // archive may still fill one that is empty.
        if sheet
            .source_host
            .as_deref()
            .is_some_and(|host| !host.trim().is_empty())
        {
            patch.source_host = None;
        }
        if keep_tool_label {
            protect_tool(&sheet, &mut patch, file_names);
        }
        apply_layer(core, mod_id, Layer::Archive, &patch, file_names, false)?;
    }
    Ok(())
}

fn protect_tool(sheet: &ModSheet, patch: &mut Patch, file_names: &[String]) {
    let raw_name = file_names
        .iter()
        .any(|file| filename_meta::is_raw_file_label(&sheet.name, file));
    if !sheet.name.trim().is_empty() && !raw_name {
        patch.name = None;
    }
    if sheet
        .version
        .as_deref()
        .is_some_and(|v| !v.trim().is_empty())
    {
        patch.version = None;
    }
}

pub fn patch_from_facts(facts: &FilenameFacts) -> Patch {
    Patch {
        name: Some(facts.name.clone()),
        version: facts.version.clone(),
        nexus_mod_id: facts.nexus_mod_id.clone(),
        ..Patch::default()
    }
}

pub fn patch_from_archive(meta: &ArchiveMetadata) -> Patch {
    let category_raw = clean(meta.category_raw.as_deref())
        .or_else(|| clean(meta.category.as_deref()))
        .map(str::to_string);
    Patch {
        name: clean(meta.name.as_deref())
            .filter(|value| !is_placeholder(value))
            .map(filename_meta::polish_display_name)
            .filter(|value| !value.is_empty()),
        version: clean(meta.version.as_deref())
            .filter(|v| !is_placeholder(v))
            .map(str::to_string),
        category_raw,
        category_nexus_id: clean(meta.category_nexus_id.as_deref()).map(str::to_string),
        source_host: clean(meta.source_host.as_deref()).map(str::to_string),
        page_url: clean(meta.page_url.as_deref()).map(str::to_string),
        nexus_mod_id: clean(meta.nexus_mod_id.as_deref()).map(str::to_string),
        author: clean(meta.author.as_deref())
            .filter(|v| !is_placeholder(v))
            .map(str::to_string),
        website: clean(meta.website.as_deref()).map(str::to_string),
        game_versions: if meta.game_versions.is_empty() {
            None
        } else {
            Some(meta.game_versions.clone())
        },
        dependencies: if meta.dependencies.is_empty() {
            None
        } else {
            Some(meta.dependencies.clone())
        },
        loaders: if meta.loaders.is_empty() {
            None
        } else {
            Some(meta.loaders.clone())
        },
        ..Patch::default()
    }
}

fn merge_filename_extra(patch: &mut Patch, extra: &ArchiveMetadata) {
    if let Some(name) = clean(extra.name.as_deref())
        .filter(|value| !is_placeholder(value))
        .map(filename_meta::polish_display_name)
        .filter(|value| !value.is_empty())
    {
        patch.name = Some(name);
    }
    if let Some(version) = clean(extra.version.as_deref()).filter(|value| !is_placeholder(value)) {
        patch.version = Some(version.to_string());
    }
    if !extra.game_versions.is_empty() {
        patch.game_versions = Some(extra.game_versions.clone());
    }
    if !extra.loaders.is_empty() {
        patch.loaders = Some(extra.loaders.clone());
    }
}

fn best_facts(file_names: &[String]) -> FilenameFacts {
    let mut fallback = None;
    for name in file_names {
        if name.trim().is_empty() {
            continue;
        }
        let facts = filename_meta::parse_file_name(name);
        if facts.nexus_mod_id.is_some() {
            return facts;
        }
        if fallback.is_none() {
            fallback = Some(facts);
        }
    }
    fallback.unwrap_or_else(|| filename_meta::parse_file_name(""))
}

fn write_text(
    slot: &mut Option<String>,
    sources: &mut BTreeMap<&'static str, Layer>,
    key: &'static str,
    layer: Layer,
    incoming: Option<&str>,
    legacy_floor: u8,
    seed: bool,
) -> bool {
    let Some(value) = clean(incoming) else {
        return false;
    };
    let current = slot.as_deref().unwrap_or("");
    if !allows(
        layer,
        sources.get(key).copied(),
        current,
        value,
        legacy_floor,
        seed,
    ) {
        return false;
    }
    *slot = Some(value.to_string());
    sources.insert(key, layer);
    true
}

fn write_list(
    slot: &mut Vec<String>,
    sources: &mut BTreeMap<&'static str, Layer>,
    key: &'static str,
    layer: Layer,
    incoming: &[String],
    seed: bool,
) -> Result<bool> {
    let cleaned = encode_string_list(incoming)?;
    let Some(encoded) = cleaned else {
        return Ok(false);
    };
    let current_encoded = encode_string_list(slot)?.unwrap_or_default();
    let floor = if slot.is_empty() {
        0
    } else {
        Layer::Archive.rank()
    };
    if !allows(
        layer,
        sources.get(key).copied(),
        &current_encoded,
        &encoded,
        floor,
        seed,
    ) {
        return Ok(false);
    }
    *slot = decode_owned(&encoded);
    sources.insert(key, layer);
    Ok(true)
}

fn write_dep_list(
    slot: &mut Vec<DeclaredDependency>,
    sources: &mut BTreeMap<&'static str, Layer>,
    key: &'static str,
    layer: Layer,
    incoming: &[DeclaredDependency],
    seed: bool,
) -> Result<bool> {
    let cleaned = encode_dependencies(incoming)?;
    let Some(encoded) = cleaned else {
        return Ok(false);
    };
    let current_encoded = encode_dependencies(slot)?.unwrap_or_default();
    let floor = if slot.is_empty() {
        0
    } else {
        Layer::Archive.rank()
    };
    if !allows(
        layer,
        sources.get(key).copied(),
        &current_encoded,
        &encoded,
        floor,
        seed,
    ) {
        return Ok(false);
    }
    *slot = decode_dependencies(Some(encoded));
    sources.insert(key, layer);
    Ok(true)
}

fn decode_owned(raw: &str) -> Vec<String> {
    serde_json::from_str::<Vec<String>>(raw)
        .unwrap_or_default()
        .into_iter()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

fn allows(
    layer: Layer,
    recorded: Option<Layer>,
    current: &str,
    incoming: &str,
    legacy_floor: u8,
    seed: bool,
) -> bool {
    if incoming.trim().is_empty() {
        return false;
    }
    if layer == Layer::User {
        return true;
    }
    if let Some(recorded) = recorded {
        if current.trim().is_empty() {
            return recorded != Layer::User;
        }
        // The same weak layer may replace its own previous guess. A later
        // archive read can correct a name it stored as `EngineFixes`, and a
        // better file-name parse can replace a stamp it stored whole. `user`
        // is not in this pair, so a typed name stays.
        if layer == recorded && matches!(layer, Layer::Filename | Layer::Archive) {
            return current.trim() != incoming.trim();
        }
        return layer.rank() > recorded.rank();
    }
    if current.trim().is_empty() || seed {
        return true;
    }
    layer.rank() > legacy_floor
}

fn name_floor(current: &str, file_names: &[String]) -> u8 {
    if file_names
        .iter()
        .any(|file| filename_meta::is_raw_file_label(current, file))
    {
        0
    } else {
        Layer::User.rank()
    }
}

fn version_floor(current: &str, file_names: &[String]) -> u8 {
    let current = current.trim();
    if current.is_empty() {
        return 0;
    }
    let from_file = file_names
        .iter()
        .any(|file| filename_meta::parse_file_name(file).version.as_deref() == Some(current));
    if from_file {
        Layer::Filename.rank()
    } else {
        Layer::User.rank()
    }
}

fn clean(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|s| !s.is_empty())
}

fn is_placeholder(value: &str) -> bool {
    value.contains('$') || value.contains('{') || value.contains('}')
}

fn parse_sources(raw: Option<&str>) -> BTreeMap<&'static str, Layer> {
    let mut out = BTreeMap::new();
    let Some(raw) = raw.filter(|s| !s.trim().is_empty()) else {
        return out;
    };
    let Ok(map) = serde_json::from_str::<BTreeMap<String, String>>(raw) else {
        return out;
    };
    for (key, value) in map {
        let Some(layer) = Layer::parse(&value) else {
            continue;
        };
        let key: &'static str = match key.as_str() {
            "name" => "name",
            "version" => "version",
            "category" => "category",
            "category_raw" => "category_raw",
            "category_nexus_id" => "category_nexus_id",
            "source_host" => "source_host",
            "page_url" => "page_url",
            "file_url" => "file_url",
            "nexus_mod_id" => "nexus_mod_id",
            "author" => "author",
            "website" => "website",
            "game_versions" => "game_versions",
            "dependencies" => "dependencies",
            "loaders" => "loaders",
            _ => continue,
        };
        out.insert(key, layer);
    }
    out
}

fn encode_sources(sources: &BTreeMap<&'static str, Layer>) -> String {
    let map: BTreeMap<&str, &str> = sources
        .iter()
        .map(|(key, layer)| (*key, layer.as_str()))
        .collect();
    serde_json::to_string(&map).unwrap_or_else(|_| "{}".to_string())
}

/// Whether the stored category was chosen by the human (`field_sources.category = "user"`).
#[must_use]
pub fn category_is_user(field_sources: Option<&str>) -> bool {
    parse_sources(field_sources).get("category") == Some(&Layer::User)
}

/// Which of these mods have a category the human chose.
pub fn category_user_flags(core: &Core, mod_ids: &[i64]) -> Result<Vec<(i64, bool)>> {
    Ok(core
        .select_field_sources(mod_ids)?
        .into_iter()
        .map(|(id, raw)| (id, category_is_user(raw.as_deref())))
        .collect())
}

/// Read one field's recorded layer. `None` when the row or the key is absent.
pub fn recorded_layer(core: &Core, mod_id: i64, key: &str) -> Result<Option<Layer>> {
    let Some(sheet) = core.select_mod_sheet(mod_id)? else {
        return Ok(None);
    };
    Ok(parse_sources(sheet.field_sources.as_deref())
        .get(key)
        .copied())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_user_category_is_not_replaced_or_refilled() {
        let core = Core::open_in_memory().unwrap();
        let id = core.insert_archived_mod("A", "a.7z", None, None).unwrap();
        apply_layer(
            &core,
            id,
            Layer::User,
            &Patch {
                category: Some("magic".to_string()),
                ..Patch::default()
            },
            &[],
            false,
        )
        .unwrap();
        apply_layer(
            &core,
            id,
            Layer::Archive,
            &Patch {
                category: Some("combat".to_string()),
                ..Patch::default()
            },
            &[],
            false,
        )
        .unwrap();
        let facts = core.select_mod_facts(id).unwrap().unwrap();
        assert_eq!(facts.category.as_deref(), Some("magic"));
        assert_eq!(
            recorded_layer(&core, id, "category").unwrap(),
            Some(Layer::User)
        );

        apply_layer(
            &core,
            id,
            Layer::User,
            &Patch {
                clear_category: true,
                ..Patch::default()
            },
            &[],
            false,
        )
        .unwrap();
        apply_layer(
            &core,
            id,
            Layer::Archive,
            &Patch {
                category: Some("audio".to_string()),
                ..Patch::default()
            },
            &[],
            false,
        )
        .unwrap();
        let facts = core.select_mod_facts(id).unwrap().unwrap();
        assert!(facts.category.is_none());
    }

    #[test]
    fn a_filename_layer_may_replace_its_own_guess_and_not_a_user_name() {
        let core = Core::open_in_memory().unwrap();
        let id = core
            .insert_archived_mod(
                "Civil War Questline Fixed AF 199639 1.6 2026-11-24T13-11Z s6Og04g9U",
                "civil.zip",
                None,
                None,
            )
            .unwrap();
        core.set_field_sources(id, Some(r#"{"name":"filename"}"#))
            .unwrap();
        let file = "Civil War Questline Fixed AF 199639 1.6 2026-11-24T13-11Z s6Og04g9U.zip";
        refresh_stored(
            &core,
            id,
            &[file.to_string()],
            None,
            None,
            None,
            None,
            false,
        )
        .unwrap();
        let sheet = core.select_mod_sheet(id).unwrap().unwrap();
        assert_eq!(sheet.name, "Civil War Questline Fixed AF");
        assert_eq!(sheet.version.as_deref(), Some("1.6"));
        assert_eq!(sheet.nexus_mod_id.as_deref(), Some("199639"));

        apply_layer(
            &core,
            id,
            Layer::User,
            &Patch {
                name: Some("Guerre civile".to_string()),
                ..Patch::default()
            },
            &[],
            false,
        )
        .unwrap();
        refresh_stored(
            &core,
            id,
            &[file.to_string()],
            None,
            None,
            None,
            None,
            false,
        )
        .unwrap();
        let sheet = core.select_mod_sheet(id).unwrap().unwrap();
        assert_eq!(sheet.name, "Guerre civile");
    }

    #[test]
    fn loaders_are_stored_with_their_layer_and_an_archive_name_can_be_polished() {
        let core = Core::open_in_memory().unwrap();
        let id = core
            .insert_archived_mod("EngineFixes", "engine.7z", None, None)
            .unwrap();
        apply_layer(
            &core,
            id,
            Layer::Archive,
            &Patch {
                name: Some("EngineFixes".to_string()),
                ..Patch::default()
            },
            &[],
            true,
        )
        .unwrap();
        refresh_stored(
            &core,
            id,
            &["engine.7z".to_string()],
            None,
            None,
            Some(&crate::game_adapter::ArchiveMetadata {
                name: Some("EngineFixes".to_string()),
                loaders: vec!["forge".to_string()],
                game_versions: vec!["1.20".to_string(), "1.20.4".to_string()],
                ..crate::game_adapter::ArchiveMetadata::default()
            }),
            None,
            false,
        )
        .unwrap();
        let sheet = core.select_mod_sheet(id).unwrap().unwrap();
        assert_eq!(sheet.name, "Engine Fixes");
        assert_eq!(sheet.loaders, vec!["forge".to_string()]);
        assert_eq!(
            sheet.game_versions,
            vec!["1.20".to_string(), "1.20.4".to_string()]
        );
        assert_eq!(
            recorded_layer(&core, id, "loaders").unwrap(),
            Some(Layer::Archive)
        );
        assert_eq!(
            recorded_layer(&core, id, "name").unwrap(),
            Some(Layer::Archive)
        );
    }
}
