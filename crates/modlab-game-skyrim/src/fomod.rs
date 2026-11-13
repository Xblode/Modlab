//! The FOMOD guided installer: parses `fomod/ModuleConfig.xml` and answers
//! the two questions a wizard needs — what to ask the human, and what file
//! list a given set of answers produces.
//!
//! Deliberately not the full FOMOD spec:
//! - **Visibility conditions** (`visible` patterns on a step/group/plugin,
//!   keyed to an earlier flag or a dependency) are not evaluated — every
//!   step and every plugin is always shown. Showing an irrelevant option is
//!   a minor annoyance; silently hiding the wrong one because a condition
//!   was mis-evaluated is a mod installed wrong.
//! - **`moduleDependencies`** (a version/plugin-presence gate before the
//!   installer even starts) is not read: checking it needs plugin-record
//!   reading, which this project doesn't have yet
//!   (`docs/01-architecture-decidee.md` §4).
//! - **`conditionalFileInstalls`** (files chosen by pattern-matching the
//!   answers, on top of what each plugin's own `<files>` already lists) is
//!   not read. A FOMOD that relies on it installs an incomplete file set —
//!   better caught by testing than silently guessed at here.
//! - Images (`<image path=.../>`) are parsed by nobody; the wizard is names
//!   and descriptions only.
//!
//! All three are real, scoped follow-ups, not accidents — each one *fails
//! safe* (asks the human something they can still answer correctly, or
//! installs less rather than installs wrong).

use roxmltree::{Document, Node};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileMapping {
    /// Relative to the archive's content root (the folder that directly
    /// contains `fomod/`).
    pub source: PathBuf,
    /// Relative to the installed mod's own root (what ends up under
    /// `Data/`).
    pub destination: PathBuf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GroupType {
    SelectExactlyOne,
    SelectAtMostOne,
    SelectAtLeastOne,
    SelectAny,
    SelectAll,
}

impl GroupType {
    fn parse(s: &str) -> Self {
        match s {
            "SelectAtMostOne" => Self::SelectAtMostOne,
            "SelectAtLeastOne" => Self::SelectAtLeastOne,
            "SelectAny" => Self::SelectAny,
            "SelectAll" => Self::SelectAll,
            // "SelectExactlyOne" and anything unrecognized: the safest
            // fallback is the most restrictive real option, not a guess.
            _ => Self::SelectExactlyOne,
        }
    }

    /// Whether `count` selected plugins satisfies this group.
    pub fn accepts(self, count: usize) -> bool {
        match self {
            Self::SelectExactlyOne => count == 1,
            Self::SelectAtMostOne => count <= 1,
            Self::SelectAtLeastOne => count >= 1,
            Self::SelectAny => true,
            Self::SelectAll => true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plugin {
    pub id: String,
    pub name: String,
    pub description: String,
    pub files: Vec<FileMapping>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Group {
    pub id: String,
    pub name: String,
    pub group_type: GroupType,
    pub plugins: Vec<Plugin>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Step {
    pub id: String,
    pub name: String,
    pub groups: Vec<Group>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Module {
    pub name: String,
    /// Installed no matter what the human picks.
    pub required_files: Vec<FileMapping>,
    pub steps: Vec<Step>,
}

/// Read and parse a `ModuleConfig.xml` file from disk, decoding whichever
/// encoding it turns out to be in.
///
/// Real FOMOD authoring tools do not agree on UTF-8: a popular one ("FOMOD
/// Creation Kit") emits UTF-16LE with a BOM, confirmed against a real
/// downloaded archive while building this — `String::from_utf8` on those
/// bytes doesn't error, it just decodes as mostly null bytes and silently
/// yields a module with no steps, which looks like "nothing to configure"
/// rather than the parsing bug it actually is. Worth detecting properly
/// rather than assuming UTF-8 and hoping.
pub fn read_module_config(path: &Path) -> Result<Module, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("lecture de ModuleConfig.xml : {e}"))?;
    let xml = decode_xml_bytes(&bytes)?;
    parse(&xml)
}

fn decode_xml_bytes(bytes: &[u8]) -> Result<String, String> {
    if let Some(rest) = bytes.strip_prefix(&[0xFF, 0xFE]) {
        return Ok(decode_utf16(rest, true));
    }
    if let Some(rest) = bytes.strip_prefix(&[0xFE, 0xFF]) {
        return Ok(decode_utf16(rest, false));
    }
    let rest = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(bytes);
    String::from_utf8(rest.to_vec())
        .map_err(|_| "ModuleConfig.xml n'est ni UTF-8 ni UTF-16 reconnu".to_string())
}

fn decode_utf16(bytes: &[u8], little_endian: bool) -> String {
    let units: Vec<u16> = bytes
        .chunks_exact(2)
        .map(|c| {
            if little_endian {
                u16::from_le_bytes([c[0], c[1]])
            } else {
                u16::from_be_bytes([c[0], c[1]])
            }
        })
        .collect();
    String::from_utf16_lossy(&units)
}

/// Parse a `ModuleConfig.xml`. Refuses rather than guessing at anything the
/// file itself is malformed about; a missing *optional* element (no
/// `<description>`, no `<requiredInstallFiles>`) is not an error.
pub fn parse(xml: &str) -> Result<Module, String> {
    let doc = Document::parse(xml).map_err(|e| format!("ModuleConfig.xml invalide : {e}"))?;
    let config = doc.root_element();
    if config.tag_name().name() != "config" {
        return Err("ModuleConfig.xml : élément racine <config> introuvable".to_string());
    }

    let name = child(config, "moduleName")
        .map(text)
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "Mod sans nom".to_string());

    let required_files = child(config, "requiredInstallFiles")
        .map(collect_files)
        .unwrap_or_default();

    let mut steps = Vec::new();
    if let Some(install_steps) = child(config, "installSteps") {
        for (si, step_node) in ordered_children(install_steps, "installStep").enumerate() {
            let step_id = format!("s{si}");
            let step_name = attr(step_node, "name").unwrap_or_else(|| format!("Étape {}", si + 1));
            let mut groups = Vec::new();

            if let Some(ofg) = child(step_node, "optionalFileGroups") {
                for (gi, group_node) in ordered_children(ofg, "group").enumerate() {
                    let group_id = format!("{step_id}-g{gi}");
                    let group_name =
                        attr(group_node, "name").unwrap_or_else(|| format!("Groupe {}", gi + 1));
                    let group_type =
                        GroupType::parse(attr(group_node, "type").as_deref().unwrap_or(""));
                    let mut plugins = Vec::new();

                    if let Some(plugins_node) = child(group_node, "plugins") {
                        for (pi, plugin_node) in
                            ordered_children(plugins_node, "plugin").enumerate()
                        {
                            let plugin_id = format!("{group_id}-p{pi}");
                            let plugin_name = attr(plugin_node, "name")
                                .unwrap_or_else(|| format!("Option {}", pi + 1));
                            let description = child(plugin_node, "description")
                                .map(text)
                                .unwrap_or_default();
                            let files = child(plugin_node, "files")
                                .map(collect_files)
                                .unwrap_or_default();
                            plugins.push(Plugin {
                                id: plugin_id,
                                name: plugin_name,
                                description,
                                files,
                            });
                        }
                    }

                    groups.push(Group {
                        id: group_id,
                        name: group_name,
                        group_type,
                        plugins,
                    });
                }
            }

            steps.push(Step {
                id: step_id,
                name: step_name,
                groups,
            });
        }
    }

    Ok(Module {
        name,
        required_files,
        steps,
    })
}

/// The file list a set of answers produces: every required file, plus every
/// selected plugin's files. `picked` is the set of plugin ids the human
/// checked — `resolve` does not validate group rules (`GroupType::accepts`
/// does that against the UI's own selection state before this ever runs).
pub fn resolve(module: &Module, picked: &[String]) -> Vec<FileMapping> {
    let mut files = module.required_files.clone();
    for step in &module.steps {
        for group in &step.groups {
            for plugin in &group.plugins {
                if picked.iter().any(|p| p == &plugin.id) {
                    files.extend(plugin.files.iter().cloned());
                }
            }
        }
    }
    files
}

/// Case-insensitively locate `fomod/ModuleConfig.xml` under `content_root`
/// — real archives disagree on casing (`FOMOD/`, `Fomod/moduleconfig.xml`…)
/// and this project already treats that as a fact of life, not an error
/// (`installer::classify` does the same `eq_ignore_ascii_case` dance).
pub fn find_module_config(content_root: &Path) -> Option<PathBuf> {
    let fomod_dir = std::fs::read_dir(content_root)
        .ok()?
        .filter_map(|e| e.ok())
        .find(|e| {
            e.file_type().map(|t| t.is_dir()).unwrap_or(false)
                && e.file_name()
                    .to_string_lossy()
                    .eq_ignore_ascii_case("fomod")
        })?
        .path();
    std::fs::read_dir(&fomod_dir)
        .ok()?
        .filter_map(|e| e.ok())
        .find(|e| {
            e.file_name()
                .to_string_lossy()
                .eq_ignore_ascii_case("moduleconfig.xml")
        })
        .map(|e| e.path())
}

fn child<'a, 'input>(node: Node<'a, 'input>, tag: &str) -> Option<Node<'a, 'input>> {
    node.children()
        .find(|n| n.is_element() && n.has_tag_name(tag))
}

fn attr(node: Node, name: &str) -> Option<String> {
    node.attribute(name).map(|s| s.to_string())
}

fn text(node: Node) -> String {
    node.text().unwrap_or("").trim().to_string()
}

/// Children of `parent` named `tag`, in the order the human should see them
/// — `order="Explicit"` (or an absent/unknown `order`) keeps document order;
/// `Ascending`/`Descending` sort by name, which is the only ordering rule
/// simple enough to be worth implementing without also modelling FOMOD's
/// image/index sort keys.
fn ordered_children<'a, 'input: 'a>(
    parent: Node<'a, 'input>,
    tag: &'a str,
) -> Box<dyn Iterator<Item = Node<'a, 'input>> + 'a> {
    let mut items: Vec<Node> = parent
        .children()
        .filter(|n| n.is_element() && n.has_tag_name(tag))
        .collect();
    match attr(parent, "order").as_deref() {
        Some("Descending") => items.sort_by(|a, b| name_of(b).cmp(&name_of(a))),
        Some("Ascending") => items.sort_by(|a, b| name_of(a).cmp(&name_of(b))),
        _ => {}
    }
    Box::new(items.into_iter())
}

fn name_of(node: &Node) -> String {
    attr(*node, "name").unwrap_or_default()
}

fn collect_files(files_node: Node) -> Vec<FileMapping> {
    files_node
        .children()
        .filter(|n| n.is_element())
        .filter_map(|n| {
            let source = attr(n, "source")?;
            let destination = attr(n, "destination").unwrap_or_default();
            // An empty `destination` means "same relative layout as
            // `source`" — the FOMOD spec's own convention, not a default we
            // invented.
            let destination = if destination.is_empty() {
                source.clone()
            } else {
                destination
            };
            match n.tag_name().name() {
                "file" | "folder" => Some(FileMapping {
                    source: PathBuf::from(source.replace('\\', "/")),
                    destination: PathBuf::from(destination.replace('\\', "/")),
                }),
                _ => None,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SIMPLE: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<config>
  <moduleName>Lux</moduleName>
  <requiredInstallFiles>
    <folder source="Core" destination=""/>
  </requiredInstallFiles>
  <installSteps order="Explicit">
    <installStep name="Choisir la variante">
      <optionalFileGroups order="Explicit">
        <group name="Intensité" type="SelectExactlyOne">
          <plugins order="Explicit">
            <plugin name="Standard">
              <description>Luminosité par défaut</description>
              <files><folder source="Standard" destination=""/></files>
              <typeDescriptor><type name="Optional"/></typeDescriptor>
            </plugin>
            <plugin name="Sombre">
              <description>Plus sombre</description>
              <files><folder source="Sombre" destination=""/></files>
              <typeDescriptor><type name="Optional"/></typeDescriptor>
            </plugin>
          </plugins>
        </group>
      </optionalFileGroups>
    </installStep>
  </installSteps>
</config>"#;

    #[test]
    fn parses_module_name_and_required_files() {
        let module = parse(SIMPLE).unwrap();
        assert_eq!(module.name, "Lux");
        assert_eq!(module.required_files.len(), 1);
        assert_eq!(module.required_files[0].source, PathBuf::from("Core"));
    }

    #[test]
    fn parses_one_step_one_group_two_plugins() {
        let module = parse(SIMPLE).unwrap();
        assert_eq!(module.steps.len(), 1);
        let group = &module.steps[0].groups[0];
        assert_eq!(group.name, "Intensité");
        assert_eq!(group.group_type, GroupType::SelectExactlyOne);
        assert_eq!(group.plugins.len(), 2);
        assert_eq!(group.plugins[0].name, "Standard");
        assert_eq!(group.plugins[1].description, "Plus sombre");
    }

    #[test]
    fn ids_are_stable_and_addressable() {
        let module = parse(SIMPLE).unwrap();
        assert_eq!(module.steps[0].id, "s0");
        assert_eq!(module.steps[0].groups[0].id, "s0-g0");
        assert_eq!(module.steps[0].groups[0].plugins[1].id, "s0-g0-p1");
    }

    #[test]
    fn resolve_includes_required_files_plus_the_picked_plugin() {
        let module = parse(SIMPLE).unwrap();
        let files = resolve(&module, &["s0-g0-p1".to_string()]);

        assert_eq!(
            files.len(),
            2,
            "the required folder plus the picked plugin's"
        );
        assert!(files.iter().any(|f| f.source == PathBuf::from("Core")));
        assert!(files.iter().any(|f| f.source == PathBuf::from("Sombre")));
        assert!(!files.iter().any(|f| f.source == PathBuf::from("Standard")));
    }

    #[test]
    fn resolve_with_nothing_picked_is_just_the_required_files() {
        let module = parse(SIMPLE).unwrap();
        let files = resolve(&module, &[]);
        assert_eq!(files.len(), 1);
    }

    #[test]
    fn group_type_accepts_matches_the_five_fomod_rules() {
        assert!(GroupType::SelectExactlyOne.accepts(1));
        assert!(!GroupType::SelectExactlyOne.accepts(0));
        assert!(!GroupType::SelectExactlyOne.accepts(2));

        assert!(GroupType::SelectAtMostOne.accepts(0));
        assert!(GroupType::SelectAtMostOne.accepts(1));
        assert!(!GroupType::SelectAtMostOne.accepts(2));

        assert!(!GroupType::SelectAtLeastOne.accepts(0));
        assert!(GroupType::SelectAtLeastOne.accepts(1));
        assert!(GroupType::SelectAtLeastOne.accepts(5));

        assert!(GroupType::SelectAny.accepts(0));
        assert!(GroupType::SelectAny.accepts(5));

        assert!(GroupType::SelectAll.accepts(0));
        assert!(GroupType::SelectAll.accepts(99));
    }

    #[test]
    fn unknown_group_type_falls_back_to_the_strictest_real_option() {
        assert_eq!(
            GroupType::parse("SomethingNewInTheSpec"),
            GroupType::SelectExactlyOne
        );
    }

    #[test]
    fn empty_destination_means_same_relative_layout_as_source() {
        let xml = r#"<config>
          <requiredInstallFiles>
            <file source="readme.txt" destination=""/>
          </requiredInstallFiles>
        </config>"#;
        let module = parse(xml).unwrap();
        assert_eq!(
            module.required_files[0].destination,
            PathBuf::from("readme.txt")
        );
    }

    #[test]
    fn missing_root_element_is_refused_not_defaulted() {
        let err = parse("<notconfig></notconfig>").unwrap_err();
        assert!(err.contains("config"));
    }

    #[test]
    fn reads_a_real_utf16_module_config_without_garbling_it() {
        // Real bytes a popular FOMOD authoring tool ("FOMOD Creation Kit")
        // actually produces — UTF-16LE with a BOM. Encoding the fixture by
        // hand rather than assuming `read_module_config` would somehow be
        // tested by the UTF-8 fixtures above, which it explicitly is not.
        let xml = r#"<config><moduleName>Café Mod</moduleName></config>"#;
        let mut bytes = vec![0xFF, 0xFE];
        for unit in xml.encode_utf16() {
            bytes.extend_from_slice(&unit.to_le_bytes());
        }
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("ModuleConfig.xml");
        std::fs::write(&path, &bytes).unwrap();

        let module = read_module_config(&path).unwrap();

        assert_eq!(module.name, "Café Mod");
    }

    #[test]
    fn reads_a_plain_utf8_module_config_with_no_bom() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("ModuleConfig.xml");
        std::fs::write(&path, SIMPLE).unwrap();

        assert_eq!(read_module_config(&path).unwrap().name, "Lux");
    }

    #[test]
    fn finds_module_config_regardless_of_casing() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("FOMOD")).unwrap();
        std::fs::write(dir.path().join("FOMOD/moduleconfig.xml"), SIMPLE).unwrap();

        let found = find_module_config(dir.path()).unwrap();

        assert_eq!(found, dir.path().join("FOMOD").join("moduleconfig.xml"));
    }

    #[test]
    fn find_module_config_is_none_without_a_fomod_folder() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("readme.txt"), "hi").unwrap();
        assert_eq!(find_module_config(dir.path()), None);
    }

    #[test]
    fn ascending_order_sorts_plugins_by_name() {
        let xml = r#"<config><installSteps order="Explicit">
          <installStep name="S">
            <optionalFileGroups order="Explicit">
              <group name="G" type="SelectAny">
                <plugins order="Ascending">
                  <plugin name="Zebra"><files/></plugin>
                  <plugin name="Alpha"><files/></plugin>
                </plugins>
              </group>
            </optionalFileGroups>
          </installStep>
        </installSteps></config>"#;
        let module = parse(xml).unwrap();
        let names: Vec<_> = module.steps[0].groups[0]
            .plugins
            .iter()
            .map(|p| p.name.as_str())
            .collect();
        assert_eq!(names, vec!["Alpha", "Zebra"]);
    }
}
