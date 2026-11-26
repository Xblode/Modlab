//! The FOMOD guided installer: parses `fomod/ModuleConfig.xml` (schema 5.x)
//! and turns a set of answers into the files to copy.
//!
//! What this covers, the way Mod Organizer 2 and Vortex do:
//! - group rules (`SelectExactlyOne`, `SelectAtMostOne`, `SelectAtLeastOne`,
//!   `SelectAny`, `SelectAll`);
//! - plugin `typeDescriptor` (`type` or `dependencyType` — first matching
//!   pattern wins, otherwise `defaultType`);
//! - flags set by the selected plugins, and `installStep` `visible`
//!   (an invisible step is skipped, both for the questions and for its files);
//! - `order` on steps, groups and plugins (`Ascending` when the attribute is
//!   absent, then `Descending`, `Explicit`);
//! - `requiredInstallFiles`, the chosen plugins' `file`/`folder` entries
//!   (`priority`, `alwaysInstall`, `installIfUsable`), then
//!   `conditionalFileInstalls` (every matching pattern, after the flags are
//!   final). A higher `priority` wins a shared destination; a tie keeps the
//!   later entry. An empty folder destination means "children at the mod root";
//! - module and plugin images, read back only from inside the extracted tree.
//!
//! Dependencies inside a `dependencyType`, a `visible` block or a conditional
//! pattern are `flagDependency`, `fileDependency` (`Active` / `Inactive` /
//! `Missing`, asked of the branch the caller describes), `gameDependency`
//! (installed version is at least the required one) and `fommDependency`
//! (always satisfied — ModLab is not FOMM and does not pretend to a FOMM
//! version). Groups nest with `operator` `And` (the default) or `Or`.
//!
//! Still not handled, on purpose:
//! - `moduleDependencies` does not block the wizard. The same dependency
//!   language is evaluated for steps and plugins, but a failing module gate
//!   does not refuse to open the installer.
//! - `showFade` on `moduleImage` is ignored.
//! - A file contributed by an enabled mod counts as `Active` even when the
//!   load order later marks that plugin inactive (missing master, plugin
//!   cap). Mods are enabled or not; there is no separate per-plugin switch.
//! - An image is served as bytes. It is not decoded, resized or colour-managed.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Component, Path, PathBuf};

use roxmltree::{Document, Node};

/// Largest image the wizard will hand to the WebView, in bytes.
const IMAGE_MAX_BYTES: u64 = 8 * 1024 * 1024;

const NOT_USABLE_REASON: &str = "Indisponible avec la configuration actuelle.";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileMapping {
    /// Relative to the archive's content root (the folder that directly
    /// contains `fomod/`).
    pub source: PathBuf,
    /// Relative to the installed mod's own root (what ends up under `Data/`).
    pub destination: PathBuf,
    /// Higher wins when two entries share a destination. Ties keep the later one.
    pub priority: i32,
    /// Install even when the plugin itself is not selected.
    pub always_install: bool,
    /// Install when the plugin's effective type is anything but `NotUsable`,
    /// even if the plugin is not selected.
    pub install_if_usable: bool,
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
    fn parse(value: &str) -> Self {
        match value {
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
            Self::SelectAny | Self::SelectAll => true,
        }
    }
}

/// Effective plugin type after `dependencyType` patterns have been tried.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PluginTypeName {
    Required,
    Optional,
    Recommended,
    NotUsable,
    CouldBeUsable,
}

impl PluginTypeName {
    /// Stable id for the wizard (`"not-usable"`, `"could-be-usable"`, …).
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Required => "required",
            Self::Optional => "optional",
            Self::Recommended => "recommended",
            Self::NotUsable => "not-usable",
            Self::CouldBeUsable => "could-be-usable",
        }
    }

    fn parse(name: Option<&str>) -> Self {
        match name.unwrap_or("").trim().to_ascii_lowercase().as_str() {
            "required" => Self::Required,
            "recommended" => Self::Recommended,
            "notusable" => Self::NotUsable,
            "couldbeusable" => Self::CouldBeUsable,
            _ => Self::Optional,
        }
    }
}

/// How a `fileDependency` expects to find a data-relative path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileState {
    Active,
    Inactive,
    Missing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Op {
    And,
    Or,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Dep {
    Flag {
        name: String,
        value: String,
    },
    File {
        file: String,
        state: FileState,
    },
    Game {
        version: String,
    },
    /// FOMM is not installed here. The dependency is treated as satisfied.
    Fomm,
    Group {
        op: Op,
        children: Vec<Dep>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TypeChoice {
    default: PluginTypeName,
    /// Tried in document order. The first match wins.
    patterns: Vec<(Dep, PluginTypeName)>,
}

impl Default for TypeChoice {
    fn default() -> Self {
        Self {
            default: PluginTypeName::Optional,
            patterns: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Conditional {
    when: Dep,
    files: Vec<FileMapping>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleImage {
    pub path: String,
    /// `showImage`. Absent means show.
    pub show: bool,
    /// Positive `height` only. Zero, negative and absent mean "let the layout decide".
    pub height: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plugin {
    pub id: String,
    pub name: String,
    pub description: String,
    pub image: Option<String>,
    pub files: Vec<FileMapping>,
    flags: Vec<(String, String)>,
    type_choice: TypeChoice,
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
    visible: Option<Dep>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Module {
    pub name: String,
    pub image: Option<ModuleImage>,
    /// Installed no matter what the human picks.
    pub required_files: Vec<FileMapping>,
    pub steps: Vec<Step>,
    conditional: Vec<Conditional>,
}

/// What the wizard shows for one plugin after flags and dependencies are applied.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginView {
    pub id: String,
    pub type_name: PluginTypeName,
    pub selected: bool,
    pub locked: bool,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroupView {
    pub id: String,
    /// French sentence when the group is visible and its rule is not met.
    pub hint: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StepView {
    pub id: String,
    pub visible: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct View {
    pub steps: Vec<StepView>,
    pub groups: Vec<GroupView>,
    pub plugins: Vec<PluginView>,
}

/// `Defaults` pre-checks `Recommended` (and forces `Required` / `SelectAll`).
/// `Strict` keeps the caller's checks, then applies the same forces — this is
/// what an install uses, so a recommended option the human cleared stays cleared.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionMode {
    Defaults,
    Strict,
}

/// Facts the dependency evaluator cannot see on its own.
pub struct FomodEnv<'a> {
    pub file_state: &'a dyn Fn(&str) -> FileState,
    /// Installed game version (`major.minor.build.revision`), when known.
    pub game_version: Option<&'a str>,
}

struct Settled {
    view: View,
    flags: HashMap<String, String>,
}

/// Read and parse a `ModuleConfig.xml` file from disk, decoding whichever
/// encoding it turns out to be in.
///
/// Real FOMOD authoring tools do not agree on UTF-8: a popular one ("FOMOD
/// Creation Kit") emits UTF-16LE with a BOM, confirmed against a real
/// downloaded archive while building this — `String::from_utf8` on those
/// bytes doesn't error, it just decodes as mostly null bytes and silently
/// yields a module with no steps, which looks like "nothing to configure"
/// rather than the parsing bug it actually is.
pub fn read_module_config(path: &Path) -> Result<Module, String> {
    let bytes = fs::read(path).map_err(|e| format!("lecture de ModuleConfig.xml : {e}"))?;
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
    let (pairs, _) = bytes.as_chunks::<2>();
    let units: Vec<u16> = pairs
        .iter()
        .map(|pair| {
            if little_endian {
                u16::from_le_bytes(*pair)
            } else {
                u16::from_be_bytes(*pair)
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
        .map(node_text)
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
                    groups.push(parse_group(group_node, &step_id, gi));
                }
            }
            steps.push(Step {
                id: step_id,
                name: step_name,
                groups,
                visible: parse_visible(step_node),
            });
        }
    }

    let conditional = child(config, "conditionalFileInstalls")
        .map(parse_conditional)
        .unwrap_or_default();

    Ok(Module {
        name,
        image: parse_module_image(config),
        required_files,
        steps,
        conditional,
    })
}

fn parse_group(group_node: Node, step_id: &str, gi: usize) -> Group {
    let group_id = format!("{step_id}-g{gi}");
    let group_name = attr(group_node, "name").unwrap_or_else(|| format!("Groupe {}", gi + 1));
    let group_type = GroupType::parse(attr(group_node, "type").as_deref().unwrap_or(""));
    let mut plugins = Vec::new();
    if let Some(plugins_node) = child(group_node, "plugins") {
        for (pi, plugin_node) in ordered_children(plugins_node, "plugin").enumerate() {
            plugins.push(parse_plugin(plugin_node, &group_id, pi));
        }
    }
    Group {
        id: group_id,
        name: group_name,
        group_type,
        plugins,
    }
}

fn parse_plugin(plugin_node: Node, group_id: &str, pi: usize) -> Plugin {
    let image = child(plugin_node, "image")
        .and_then(|node| attr(node, "path"))
        .filter(|path| !path.trim().is_empty());
    Plugin {
        id: format!("{group_id}-p{pi}"),
        name: attr(plugin_node, "name").unwrap_or_else(|| format!("Option {}", pi + 1)),
        description: child(plugin_node, "description")
            .map(node_text)
            .unwrap_or_default(),
        image,
        files: child(plugin_node, "files")
            .map(collect_files)
            .unwrap_or_default(),
        flags: parse_flags(plugin_node),
        type_choice: parse_type_choice(plugin_node),
    }
}

fn parse_type_choice(plugin_node: Node) -> TypeChoice {
    let Some(descriptor) = child(plugin_node, "typeDescriptor") else {
        return TypeChoice::default();
    };
    if let Some(dependency) = child(descriptor, "dependencyType") {
        let default_name = child(dependency, "defaultType").and_then(|node| attr(node, "name"));
        let default = default_name.as_deref();
        let mut patterns = Vec::new();
        if let Some(list) = child(dependency, "patterns") {
            for pattern in list
                .children()
                .filter(|node| node.is_element() && node.has_tag_name("pattern"))
            {
                let when = child(pattern, "dependencies")
                    .map(parse_dependencies)
                    .unwrap_or(Dep::Group {
                        op: Op::And,
                        children: Vec::new(),
                    });
                let name = child(pattern, "type").and_then(|node| attr(node, "name"));
                patterns.push((when, PluginTypeName::parse(name.as_deref())));
            }
        }
        return TypeChoice {
            default: PluginTypeName::parse(default),
            patterns,
        };
    }
    let simple = child(descriptor, "type").and_then(|node| attr(node, "name"));
    TypeChoice {
        default: PluginTypeName::parse(simple.as_deref()),
        patterns: Vec::new(),
    }
}

fn parse_flags(plugin_node: Node) -> Vec<(String, String)> {
    let Some(list) = child(plugin_node, "conditionFlags") else {
        return Vec::new();
    };
    list.children()
        .filter(|node| node.is_element() && node.has_tag_name("flag"))
        .filter_map(|flag| {
            let name = attr(flag, "name")?.trim().to_string();
            if name.is_empty() {
                return None;
            }
            Some((name, node_text(flag)))
        })
        .collect()
}

fn parse_visible(step_node: Node) -> Option<Dep> {
    let visible = child(step_node, "visible")?;
    if let Some(dependencies) = child(visible, "dependencies") {
        return Some(parse_dependencies(dependencies));
    }
    let children: Vec<Dep> = visible
        .children()
        .filter(|node| node.is_element())
        .filter_map(parse_dep_child)
        .collect();
    if children.is_empty() {
        None
    } else {
        Some(Dep::Group {
            op: Op::And,
            children,
        })
    }
}

fn parse_conditional(node: Node) -> Vec<Conditional> {
    let parent = child(node, "patterns").unwrap_or(node);
    parent
        .children()
        .filter(|child_node| child_node.is_element() && child_node.has_tag_name("pattern"))
        .map(|pattern| Conditional {
            when: child(pattern, "dependencies")
                .map(parse_dependencies)
                .unwrap_or(Dep::Group {
                    op: Op::And,
                    children: Vec::new(),
                }),
            files: child(pattern, "files")
                .map(collect_files)
                .unwrap_or_default(),
        })
        .collect()
}

fn parse_dependencies(node: Node) -> Dep {
    let children = node
        .children()
        .filter(|child_node| child_node.is_element())
        .filter_map(parse_dep_child)
        .collect();
    Dep::Group {
        op: parse_op(node),
        children,
    }
}

fn parse_dep_child(node: Node) -> Option<Dep> {
    match node.tag_name().name() {
        "dependencies" => Some(parse_dependencies(node)),
        "flagDependency" => {
            let name = attr(node, "flag")?.trim().to_string();
            if name.is_empty() {
                return None;
            }
            let value = attr(node, "value").unwrap_or_default();
            Some(Dep::Flag {
                name,
                value: value.trim().to_string(),
            })
        }
        "fileDependency" => {
            let file = attr(node, "file")?.trim().to_string();
            if file.is_empty() {
                return None;
            }
            Some(Dep::File {
                file,
                state: parse_file_state(node),
            })
        }
        "gameDependency" => Some(Dep::Game {
            version: attr(node, "version").unwrap_or_default(),
        }),
        "fommDependency" => Some(Dep::Fomm),
        _ => None,
    }
}

fn parse_op(node: Node) -> Op {
    match attr(node, "operator").as_deref().map(str::trim) {
        Some(value) if value.eq_ignore_ascii_case("or") => Op::Or,
        _ => Op::And,
    }
}

fn parse_file_state(node: Node) -> FileState {
    match attr(node, "state").as_deref().map(str::trim) {
        Some(value) if value.eq_ignore_ascii_case("inactive") => FileState::Inactive,
        Some(value) if value.eq_ignore_ascii_case("missing") => FileState::Missing,
        _ => FileState::Active,
    }
}

fn parse_module_image(config: Node) -> Option<ModuleImage> {
    let node = child(config, "moduleImage")?;
    let path = attr(node, "path")
        .or_else(|| child(node, "image").and_then(|image| attr(image, "path")))
        .filter(|path| !path.trim().is_empty())?;
    let height = attr(node, "height")
        .and_then(|value| value.trim().parse::<i32>().ok())
        .filter(|value| *value > 0)
        .and_then(|value| u32::try_from(value).ok());
    Some(ModuleImage {
        path,
        show: attr_bool(node, "showImage", true),
        height,
    })
}

/// Plugin states for `picked`, after flags, visibility and forced types.
///
/// The result is what the wizard renders. [`resolve`] uses `Strict` so an
/// install does not resurrect a `Recommended` option the human turned off.
pub fn evaluate(
    module: &Module,
    picked: &[String],
    env: &FomodEnv<'_>,
    mode: SelectionMode,
) -> View {
    settle(module, picked, env, mode).view
}

/// The logical file list: required files, files of the settled selection
/// (plus `alwaysInstall` / `installIfUsable`), then every conditional pattern
/// whose dependencies match. Group rules are checked here — an invalid
/// visible group refuses the whole list. Folders are not expanded yet; see
/// [`install_plan`].
pub fn resolve(
    module: &Module,
    picked: &[String],
    env: &FomodEnv<'_>,
) -> Result<Vec<FileMapping>, String> {
    let settled = settle(module, picked, env, SelectionMode::Strict);
    let messages: Vec<&str> = settled
        .view
        .groups
        .iter()
        .filter_map(|group| group.hint.as_deref())
        .collect();
    if !messages.is_empty() {
        return Err(messages.join(" "));
    }

    let mut files = module.required_files.clone();
    for step in &module.steps {
        for group in &step.groups {
            for plugin in &group.plugins {
                let Some(state) = settled
                    .view
                    .plugins
                    .iter()
                    .find(|item| item.id == plugin.id)
                else {
                    continue;
                };
                let usable = state.type_name != PluginTypeName::NotUsable;
                for file in &plugin.files {
                    if state.selected || file.always_install || (file.install_if_usable && usable) {
                        files.push(file.clone());
                    }
                }
            }
        }
    }
    for pattern in &module.conditional {
        if eval_dep(&pattern.when, &settled.flags, env) {
            files.extend(pattern.files.iter().cloned());
        }
    }
    Ok(files)
}

/// [`resolve`] expanded to real files, with one winner per destination.
///
/// A source that leaves `content_root` (`..`, an absolute path, a symlink
/// whose target is outside) is refused before anything is copied.
pub fn install_plan(
    content_root: &Path,
    module: &Module,
    picked: &[String],
    env: &FomodEnv<'_>,
) -> Result<Vec<FileMapping>, String> {
    let logical = resolve(module, picked, env)?;
    let root_canon =
        fs::canonicalize(content_root).map_err(|e| format!("archive extraite illisible : {e}"))?;
    let mut seq = 0u32;
    let mut planned = Vec::new();
    for mapping in &logical {
        expand_mapping(content_root, &root_canon, mapping, &mut seq, &mut planned)?;
    }
    Ok(collapse(planned)
        .into_iter()
        .map(|file| FileMapping {
            source: file.source,
            destination: file.destination,
            priority: file.priority,
            always_install: false,
            install_if_usable: false,
        })
        .collect())
}

fn settle(module: &Module, picked: &[String], env: &FomodEnv<'_>, mode: SelectionMode) -> Settled {
    let defaults = mode == SelectionMode::Defaults;
    let mut selected: HashSet<String> = picked.iter().cloned().collect();
    let mut seen = HashSet::new();
    for _ in 0..12 {
        let flags = collect_flags(module, &selected);
        let next = desired_selection(module, &selected, &flags, env, defaults);
        if next == selected {
            break;
        }
        let mut key: Vec<&str> = next.iter().map(String::as_str).collect();
        key.sort_unstable();
        if !seen.insert(key.join("\u{0}")) {
            let flags = collect_flags(module, &selected);
            selected = desired_selection(module, &selected, &flags, env, false);
            break;
        }
        selected = next;
    }
    let flags = collect_flags(module, &selected);
    let view = build_view(module, &selected, &flags, env);
    Settled { view, flags }
}

fn desired_selection(
    module: &Module,
    selected: &HashSet<String>,
    flags: &HashMap<String, String>,
    env: &FomodEnv<'_>,
    defaults: bool,
) -> HashSet<String> {
    let mut next = HashSet::new();
    for step in &module.steps {
        if !step_visible(step, flags, env) {
            continue;
        }
        for group in &step.groups {
            match group.group_type {
                GroupType::SelectAll => {
                    for plugin in &group.plugins {
                        let kind = effective_type(&plugin.type_choice, flags, env);
                        if kind != PluginTypeName::NotUsable {
                            next.insert(plugin.id.clone());
                        }
                    }
                }
                GroupType::SelectExactlyOne | GroupType::SelectAtMostOne => {
                    if let Some(id) = radio_choice(group, selected, flags, env, defaults) {
                        next.insert(id);
                    }
                }
                GroupType::SelectAtLeastOne | GroupType::SelectAny => {
                    for plugin in &group.plugins {
                        let kind = effective_type(&plugin.type_choice, flags, env);
                        if kind == PluginTypeName::NotUsable {
                            continue;
                        }
                        let want = kind == PluginTypeName::Required
                            || selected.contains(&plugin.id)
                            || (defaults && kind == PluginTypeName::Recommended);
                        if want {
                            next.insert(plugin.id.clone());
                        }
                    }
                }
            }
        }
    }
    next
}

fn radio_choice(
    group: &Group,
    selected: &HashSet<String>,
    flags: &HashMap<String, String>,
    env: &FomodEnv<'_>,
    defaults: bool,
) -> Option<String> {
    let mut required = None;
    let mut recommended = None;
    let mut chosen = None;
    for plugin in &group.plugins {
        let kind = effective_type(&plugin.type_choice, flags, env);
        if kind == PluginTypeName::NotUsable {
            continue;
        }
        if kind == PluginTypeName::Required && required.is_none() {
            required = Some(plugin.id.clone());
        }
        if kind == PluginTypeName::Recommended && recommended.is_none() {
            recommended = Some(plugin.id.clone());
        }
        if chosen.is_none() && selected.contains(&plugin.id) {
            chosen = Some(plugin.id.clone());
        }
    }
    if required.is_some() {
        return required;
    }
    if chosen.is_some() {
        return chosen;
    }
    if defaults {
        recommended
    } else {
        None
    }
}

fn build_view(
    module: &Module,
    selected: &HashSet<String>,
    flags: &HashMap<String, String>,
    env: &FomodEnv<'_>,
) -> View {
    let mut steps = Vec::new();
    let mut groups = Vec::new();
    let mut plugins = Vec::new();
    for step in &module.steps {
        let visible = step_visible(step, flags, env);
        steps.push(StepView {
            id: step.id.clone(),
            visible,
        });
        for group in &step.groups {
            let typed: Vec<(&Plugin, PluginTypeName)> = group
                .plugins
                .iter()
                .map(|plugin| (plugin, effective_type(&plugin.type_choice, flags, env)))
                .collect();
            let has_required = typed
                .iter()
                .any(|(_, kind)| *kind == PluginTypeName::Required);
            let count = typed
                .iter()
                .filter(|(plugin, kind)| {
                    visible && *kind != PluginTypeName::NotUsable && selected.contains(&plugin.id)
                })
                .count();
            groups.push(GroupView {
                id: group.id.clone(),
                hint: if visible {
                    group_hint(group, count)
                } else {
                    None
                },
            });
            for (plugin, kind) in typed {
                let is_selected =
                    visible && kind != PluginTypeName::NotUsable && selected.contains(&plugin.id);
                plugins.push(PluginView {
                    id: plugin.id.clone(),
                    type_name: kind,
                    selected: is_selected,
                    locked: is_locked(group.group_type, kind, has_required, visible),
                    reason: (kind == PluginTypeName::NotUsable)
                        .then(|| NOT_USABLE_REASON.to_string()),
                });
            }
        }
    }
    View {
        steps,
        groups,
        plugins,
    }
}

fn is_locked(
    group_type: GroupType,
    kind: PluginTypeName,
    has_required: bool,
    visible: bool,
) -> bool {
    if !visible || kind == PluginTypeName::NotUsable || kind == PluginTypeName::Required {
        return true;
    }
    if group_type == GroupType::SelectAll {
        return true;
    }
    has_required
        && matches!(
            group_type,
            GroupType::SelectExactlyOne | GroupType::SelectAtMostOne
        )
}

fn group_hint(group: &Group, count: usize) -> Option<String> {
    let invalid = match group.group_type {
        GroupType::SelectExactlyOne => count != 1,
        GroupType::SelectAtMostOne => count > 1,
        GroupType::SelectAtLeastOne => count < 1,
        GroupType::SelectAny | GroupType::SelectAll => false,
    };
    if !invalid {
        return None;
    }
    let detail = match group.group_type {
        GroupType::SelectExactlyOne => "choisis une option",
        GroupType::SelectAtLeastOne => "choisis au moins une option",
        GroupType::SelectAtMostOne => "une option au plus",
        GroupType::SelectAny | GroupType::SelectAll => "sélection invalide",
    };
    Some(format!("« {} » : {detail}.", group.name))
}

fn step_visible(step: &Step, flags: &HashMap<String, String>, env: &FomodEnv<'_>) -> bool {
    match &step.visible {
        None => true,
        Some(dep) => eval_dep(dep, flags, env),
    }
}

fn effective_type(
    choice: &TypeChoice,
    flags: &HashMap<String, String>,
    env: &FomodEnv<'_>,
) -> PluginTypeName {
    for (when, kind) in &choice.patterns {
        if eval_dep(when, flags, env) {
            return *kind;
        }
    }
    choice.default
}

fn collect_flags(module: &Module, selected: &HashSet<String>) -> HashMap<String, String> {
    let mut flags = HashMap::new();
    for step in &module.steps {
        for group in &step.groups {
            for plugin in &group.plugins {
                if !selected.contains(&plugin.id) {
                    continue;
                }
                for (name, value) in &plugin.flags {
                    flags.insert(name.to_lowercase(), value.clone());
                }
            }
        }
    }
    flags
}

fn eval_dep(dep: &Dep, flags: &HashMap<String, String>, env: &FomodEnv<'_>) -> bool {
    match dep {
        Dep::Flag { name, value } => flags
            .get(&name.to_lowercase())
            .is_some_and(|current| current.eq_ignore_ascii_case(value)),
        Dep::File { file, state } => (env.file_state)(file) == *state,
        Dep::Game { version } => env
            .game_version
            .is_some_and(|installed| version_at_least(installed, version)),
        Dep::Fomm => true,
        Dep::Group { op, children } => match op {
            Op::And => children.iter().all(|child| eval_dep(child, flags, env)),
            Op::Or => children.iter().any(|child| eval_dep(child, flags, env)),
        },
    }
}

/// Numeric components, missing pieces counting as zero. `installed >= required`.
fn version_at_least(installed: &str, required: &str) -> bool {
    let installed = version_parts(installed);
    let required = version_parts(required);
    let len = installed.len().max(required.len());
    for index in 0..len {
        let left = installed.get(index).copied().unwrap_or(0);
        let right = required.get(index).copied().unwrap_or(0);
        if left > right {
            return true;
        }
        if left < right {
            return false;
        }
    }
    true
}

fn version_parts(value: &str) -> Vec<u64> {
    value
        .split(|ch: char| !ch.is_ascii_digit())
        .filter(|part| !part.is_empty())
        .filter_map(|part| part.parse().ok())
        .collect()
}

struct Planned {
    source: PathBuf,
    destination: PathBuf,
    priority: i32,
    seq: u32,
}

fn expand_mapping(
    content_root: &Path,
    root_canon: &Path,
    mapping: &FileMapping,
    seq: &mut u32,
    out: &mut Vec<Planned>,
) -> Result<(), String> {
    if !relative_stays_inside(&mapping.source) {
        return Err(format!(
            "chemin source refusé : {}",
            mapping.source.display()
        ));
    }
    let folder_abs = content_root.join(&mapping.source);
    let canon = fs::canonicalize(&folder_abs)
        .map_err(|_| format!("fichier FOMOD introuvable : {}", mapping.source.display()))?;
    if !canon.starts_with(root_canon) {
        return Err(format!(
            "chemin source refusé : {}",
            mapping.source.display()
        ));
    }
    if canon.is_dir() {
        let ctx = ExpandCtx {
            content_root,
            root_canon,
            folder_abs: &folder_abs,
            source_prefix: &mapping.source,
            dest_prefix: &mapping.destination,
            priority: mapping.priority,
        };
        walk_folder(&folder_abs, &ctx, seq, out)
    } else {
        let destination = if mapping.destination.as_os_str().is_empty() {
            PathBuf::from(
                mapping
                    .source
                    .file_name()
                    .unwrap_or(mapping.source.as_os_str()),
            )
        } else {
            mapping.destination.clone()
        };
        push_planned(
            mapping.source.clone(),
            destination,
            mapping.priority,
            seq,
            out,
        );
        Ok(())
    }
}

struct ExpandCtx<'a> {
    content_root: &'a Path,
    root_canon: &'a Path,
    folder_abs: &'a Path,
    source_prefix: &'a Path,
    dest_prefix: &'a Path,
    priority: i32,
}

fn walk_folder(
    dir: &Path,
    ctx: &ExpandCtx<'_>,
    seq: &mut u32,
    out: &mut Vec<Planned>,
) -> Result<(), String> {
    let read = fs::read_dir(dir)
        .map_err(|e| format!("lecture de {} : {e}", ctx.source_prefix.display()))?;
    for entry in read {
        let entry =
            entry.map_err(|e| format!("lecture de {} : {e}", ctx.source_prefix.display()))?;
        let file_type = entry
            .file_type()
            .map_err(|e| format!("lecture de {} : {e}", ctx.source_prefix.display()))?;
        let path = entry.path();
        if file_type.is_dir() {
            walk_folder(&path, ctx, seq, out)?;
            continue;
        }
        let inside = path
            .strip_prefix(ctx.folder_abs)
            .map_err(|_| format!("chemin source refusé : {}", ctx.source_prefix.display()))?;
        let source = forward(ctx.source_prefix.join(inside));
        let destination = forward(ctx.dest_prefix.join(inside));
        if !relative_stays_inside(&source) {
            return Err(format!("chemin source refusé : {}", source.display()));
        }
        let file_canon = fs::canonicalize(ctx.content_root.join(&source))
            .map_err(|_| format!("fichier FOMOD introuvable : {}", source.display()))?;
        if !file_canon.starts_with(ctx.root_canon) {
            return Err(format!("chemin source refusé : {}", source.display()));
        }
        push_planned(source, destination, ctx.priority, seq, out);
    }
    Ok(())
}

fn push_planned(
    source: PathBuf,
    destination: PathBuf,
    priority: i32,
    seq: &mut u32,
    out: &mut Vec<Planned>,
) {
    let current = *seq;
    *seq += 1;
    out.push(Planned {
        source: forward(source),
        destination: forward(destination),
        priority,
        seq: current,
    });
}

fn collapse(files: Vec<Planned>) -> Vec<Planned> {
    let mut best: HashMap<String, Planned> = HashMap::new();
    let mut order = Vec::new();
    for file in files {
        let key = dest_key(&file.destination);
        if let Some(previous) = best.get(&key) {
            let wins = file.priority > previous.priority
                || (file.priority == previous.priority && file.seq > previous.seq);
            if wins {
                best.insert(key, file);
            }
        } else {
            order.push(key.clone());
            best.insert(key, file);
        }
    }
    order
        .into_iter()
        .filter_map(|key| best.remove(&key))
        .collect()
}

fn dest_key(path: &Path) -> String {
    path.components()
        .filter_map(|component| match component {
            Component::Normal(part) => Some(part.to_string_lossy().to_lowercase()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/")
}

fn forward(path: PathBuf) -> PathBuf {
    PathBuf::from(path.to_string_lossy().replace('\\', "/"))
}

fn relative_stays_inside(path: &Path) -> bool {
    let text = path.to_string_lossy();
    if text.contains(':') || text.starts_with('/') || text.starts_with('\\') {
        return false;
    }
    path.components()
        .all(|component| matches!(component, Component::Normal(_) | Component::CurDir))
}

/// Case-insensitive lookup of files the active branch and the game `Data/`
/// folder already provide. Enabled mods and the game data folder are
/// `Active`; a file that exists only in a disabled mod is `Inactive`.
pub struct BranchSnapshot {
    data_dir: Option<PathBuf>,
    enabled: Vec<ModRoot>,
    disabled: Vec<ModRoot>,
    cache: RefCell<HashMap<String, FileState>>,
}

struct ModRoot {
    path: PathBuf,
    data_child: Option<PathBuf>,
}

impl ModRoot {
    fn new(path: PathBuf) -> Self {
        let data_child = fs::read_dir(&path).ok().and_then(|read| {
            read.flatten().find_map(|entry| {
                let is_data = entry
                    .file_name()
                    .to_string_lossy()
                    .eq_ignore_ascii_case("data")
                    && entry.path().is_dir();
                is_data.then(|| entry.path())
            })
        });
        Self { path, data_child }
    }
}

impl BranchSnapshot {
    pub fn new(data_dir: Option<PathBuf>, enabled: Vec<PathBuf>, disabled: Vec<PathBuf>) -> Self {
        Self {
            data_dir,
            enabled: enabled.into_iter().map(ModRoot::new).collect(),
            disabled: disabled.into_iter().map(ModRoot::new).collect(),
            cache: RefCell::new(HashMap::new()),
        }
    }

    pub fn state(&self, file: &str) -> FileState {
        let key = file.trim().replace('\\', "/").to_lowercase();
        if let Some(hit) = self.cache.borrow().get(&key).copied() {
            return hit;
        }
        let found = self.lookup(file);
        self.cache.borrow_mut().insert(key, found);
        found
    }

    fn lookup(&self, file: &str) -> FileState {
        let Some(rel) = normalize_data_rel(file) else {
            return FileState::Missing;
        };
        if self.data_has(&rel) || self.roots_have(&self.enabled, &rel) {
            return FileState::Active;
        }
        if self.roots_have(&self.disabled, &rel) {
            return FileState::Inactive;
        }
        FileState::Missing
    }

    fn data_has(&self, rel: &str) -> bool {
        self.data_dir
            .as_ref()
            .is_some_and(|dir| exists_ci(dir, rel))
    }

    fn roots_have(&self, roots: &[ModRoot], rel: &str) -> bool {
        roots.iter().any(|root| {
            exists_ci(&root.path, rel)
                || root
                    .data_child
                    .as_ref()
                    .is_some_and(|dir| exists_ci(dir, rel))
        })
    }
}

fn normalize_data_rel(raw: &str) -> Option<String> {
    let normalized = raw.trim().replace('\\', "/");
    let mut parts = Vec::new();
    for part in normalized.split('/') {
        if part.is_empty() || part == "." {
            continue;
        }
        if part == ".." {
            return None;
        }
        parts.push(part);
    }
    if parts
        .first()
        .is_some_and(|part| part.eq_ignore_ascii_case("data"))
    {
        parts.remove(0);
    }
    if parts.is_empty() {
        None
    } else {
        Some(parts.join("/"))
    }
}

fn exists_ci(root: &Path, rel: &str) -> bool {
    if !root.is_dir() {
        return false;
    }
    let mut current = root.to_path_buf();
    for part in rel.split('/') {
        if part.is_empty() || part == "." || part == ".." {
            return false;
        }
        let Ok(read) = fs::read_dir(&current) else {
            return false;
        };
        let mut found = None;
        for entry in read.flatten() {
            if entry
                .file_name()
                .to_string_lossy()
                .eq_ignore_ascii_case(part)
            {
                found = Some(entry.path());
                break;
            }
        }
        let Some(next) = found else {
            return false;
        };
        current = next;
    }
    current.exists()
}

/// Bytes of one image inside `content_root`. `..`, an absolute path, a
/// symlink that leaves the folder, a foreign extension or a file over
/// [`IMAGE_MAX_BYTES`] is refused.
pub fn read_fomod_image(content_root: &Path, relative: &str) -> Result<Vec<u8>, String> {
    read_fomod_image_limited(content_root, relative, IMAGE_MAX_BYTES)
}

fn read_fomod_image_limited(
    content_root: &Path,
    relative: &str,
    max_bytes: u64,
) -> Result<Vec<u8>, String> {
    let rel = sanitize_image_rel(relative)?;
    let ext = rel.extension().and_then(|ext| ext.to_str()).unwrap_or("");
    if !is_image_ext(ext) {
        return Err("format d'image non pris en charge".to_string());
    }
    let file = resolve_image(content_root, &rel)?;
    let meta = fs::metadata(&file).map_err(|_| "image introuvable".to_string())?;
    if !meta.is_file() {
        return Err("image introuvable".to_string());
    }
    if meta.len() > max_bytes {
        return Err("image trop volumineuse".to_string());
    }
    fs::read(&file).map_err(|e| format!("lecture de l'image : {e}"))
}

/// `data:image/...;base64,...` for a WebView `<img>`. The extension picks the
/// media type; the bytes are not inspected.
pub fn image_data_url(bytes: &[u8], relative: &str) -> String {
    let normalized = relative.replace('\\', "/");
    let ext = Path::new(&normalized)
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let mime = match ext.as_str() {
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "bmp" => "image/bmp",
        "webp" => "image/webp",
        _ => "image/png",
    };
    format!("data:{mime};base64,{}", base64_encode(bytes))
}

fn sanitize_image_rel(relative: &str) -> Result<PathBuf, String> {
    let trimmed = relative.trim();
    if trimmed.is_empty()
        || trimmed.starts_with('/')
        || trimmed.starts_with('\\')
        || trimmed.contains(':')
    {
        return Err("chemin d'image refusé".to_string());
    }
    let normalized = trimmed.replace('\\', "/");
    let mut clean = PathBuf::new();
    for component in Path::new(&normalized).components() {
        match component {
            Component::Normal(part) => clean.push(part),
            Component::CurDir => {}
            _ => return Err("chemin d'image refusé".to_string()),
        }
    }
    if clean.as_os_str().is_empty() {
        return Err("chemin d'image refusé".to_string());
    }
    Ok(clean)
}

fn resolve_image(content_root: &Path, rel: &Path) -> Result<PathBuf, String> {
    let mut current = content_root.to_path_buf();
    for component in rel.components() {
        let Component::Normal(name) = component else {
            return Err("chemin d'image refusé".to_string());
        };
        let name = name.to_string_lossy();
        let found = fs::read_dir(&current)
            .map_err(|_| "image introuvable".to_string())?
            .flatten()
            .find(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .eq_ignore_ascii_case(&name)
            });
        let Some(entry) = found else {
            return Err("image introuvable".to_string());
        };
        current = entry.path();
    }
    let root_canon =
        fs::canonicalize(content_root).map_err(|_| "chemin d'image refusé".to_string())?;
    let file_canon = fs::canonicalize(&current).map_err(|_| "image introuvable".to_string())?;
    if !file_canon.starts_with(&root_canon) {
        return Err("chemin d'image refusé".to_string());
    }
    Ok(file_canon)
}

fn is_image_ext(ext: &str) -> bool {
    matches!(
        ext.to_ascii_lowercase().as_str(),
        "png" | "jpg" | "jpeg" | "gif" | "bmp" | "webp"
    )
}

fn base64_encode(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    let (chunks, rest) = bytes.as_chunks::<3>();
    for chunk in chunks {
        let n = (u32::from(chunk[0]) << 16) | (u32::from(chunk[1]) << 8) | u32::from(chunk[2]);
        out.push(TABLE[((n >> 18) & 0x3F) as usize] as char);
        out.push(TABLE[((n >> 12) & 0x3F) as usize] as char);
        out.push(TABLE[((n >> 6) & 0x3F) as usize] as char);
        out.push(TABLE[(n & 0x3F) as usize] as char);
    }
    if rest.len() == 1 {
        let n = u32::from(rest[0]) << 16;
        out.push(TABLE[((n >> 18) & 0x3F) as usize] as char);
        out.push(TABLE[((n >> 12) & 0x3F) as usize] as char);
        out.push('=');
        out.push('=');
    } else if rest.len() == 2 {
        let n = (u32::from(rest[0]) << 16) | (u32::from(rest[1]) << 8);
        out.push(TABLE[((n >> 18) & 0x3F) as usize] as char);
        out.push(TABLE[((n >> 12) & 0x3F) as usize] as char);
        out.push(TABLE[((n >> 6) & 0x3F) as usize] as char);
        out.push('=');
    }
    out
}

/// Case-insensitively locate `fomod/ModuleConfig.xml` under `content_root`.
/// Real archives disagree on casing (`FOMOD/`, `Fomod/moduleconfig.xml`).
pub fn find_module_config(content_root: &Path) -> Option<PathBuf> {
    let fomod_dir = fs::read_dir(content_root)
        .ok()?
        .flatten()
        .find(|entry| {
            entry.file_type().map(|kind| kind.is_dir()).unwrap_or(false)
                && entry
                    .file_name()
                    .to_string_lossy()
                    .eq_ignore_ascii_case("fomod")
        })?
        .path();
    fs::read_dir(&fomod_dir)
        .ok()?
        .flatten()
        .find(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .eq_ignore_ascii_case("moduleconfig.xml")
        })
        .map(|entry| entry.path())
}

fn child<'a, 'input>(node: Node<'a, 'input>, tag: &str) -> Option<Node<'a, 'input>> {
    node.children()
        .find(|n| n.is_element() && n.has_tag_name(tag))
}

fn attr(node: Node, name: &str) -> Option<String> {
    node.attribute(name).map(|value| value.trim().to_string())
}

/// Direct text, with the surrounding whitespace removed and the line breaks
/// inside it kept. A description is shown as the author wrote it.
fn node_text(node: Node) -> String {
    let mut out = String::new();
    for child_node in node.children() {
        if child_node.is_text() {
            if let Some(text) = child_node.text() {
                out.push_str(text);
            }
        }
    }
    out.trim().to_string()
}

fn attr_bool(node: Node, name: &str, default: bool) -> bool {
    match attr(node, name).as_deref() {
        Some(value) if value.eq_ignore_ascii_case("true") || value == "1" => true,
        Some(value) if value.eq_ignore_ascii_case("false") || value == "0" => false,
        _ => default,
    }
}

fn attr_i32(node: Node, name: &str) -> i32 {
    attr(node, name)
        .and_then(|value| value.parse().ok())
        .unwrap_or(0)
}

/// Children named `tag`, in the order the human should see them.
///
/// The schema default is `Ascending` (by name, case-insensitive). `Explicit`
/// keeps document order. Anything else, including a missing attribute, is
/// `Ascending`.
fn ordered_children<'a, 'input: 'a>(
    parent: Node<'a, 'input>,
    tag: &'a str,
) -> Box<dyn Iterator<Item = Node<'a, 'input>> + 'a> {
    let mut items: Vec<Node> = parent
        .children()
        .filter(|node| node.is_element() && node.has_tag_name(tag))
        .collect();
    match attr(parent, "order")
        .as_deref()
        .map(|value| value.to_ascii_lowercase())
        .as_deref()
    {
        Some("explicit") => {}
        Some("descending") => items.sort_by_key(|node| std::cmp::Reverse(name_key(node))),
        _ => items.sort_by_key(name_key),
    }
    Box::new(items.into_iter())
}

fn name_key(node: &Node) -> String {
    attr(*node, "name").unwrap_or_default().to_lowercase()
}

fn collect_files(files_node: Node) -> Vec<FileMapping> {
    files_node
        .children()
        .filter(|node| node.is_element())
        .filter_map(|node| {
            let kind = node.tag_name().name();
            if kind != "file" && kind != "folder" {
                return None;
            }
            let source = attr(node, "source")?;
            let mut destination = attr(node, "destination").unwrap_or_default();
            // An empty file destination keeps the source-relative path
            // (`readme.txt` stays `readme.txt`). An empty folder destination
            // means the folder's children land at the mod root — Engine Fixes
            // ships `<folder source="Required" destination=""/>`.
            if destination.is_empty() && kind != "folder" {
                destination = source.clone();
            }
            Some(FileMapping {
                source: PathBuf::from(source.replace('\\', "/")),
                destination: PathBuf::from(destination.replace('\\', "/")),
                priority: attr_i32(node, "priority"),
                always_install: attr_bool(node, "alwaysInstall", false),
                install_if_usable: attr_bool(node, "installIfUsable", false),
            })
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
              <description>Ligne 1
Ligne 2</description>
              <image path="shots\standard.png"/>
              <files><folder source="Standard" destination=""/></files>
              <typeDescriptor><type name="Optional"/></typeDescriptor>
            </plugin>
            <plugin name="Sombre">
              <description>Plus sombre</description>
              <files><folder source="Sombre" destination=""/></files>
              <typeDescriptor><type name="Recommended"/></typeDescriptor>
            </plugin>
          </plugins>
        </group>
      </optionalFileGroups>
    </installStep>
  </installSteps>
</config>"#;

    fn missing_file(_: &str) -> FileState {
        FileState::Missing
    }

    fn empty_env() -> FomodEnv<'static> {
        FomodEnv {
            file_state: &missing_file,
            game_version: None,
        }
    }

    fn sources(files: &[FileMapping]) -> Vec<String> {
        files
            .iter()
            .map(|file| file.source.to_string_lossy().replace('\\', "/"))
            .collect()
    }

    #[test]
    fn parses_module_name_required_files_image_and_newlines() {
        let module = parse(SIMPLE).unwrap();
        assert_eq!(module.name, "Lux");
        assert_eq!(module.required_files.len(), 1);
        assert_eq!(module.required_files[0].source, PathBuf::from("Core"));
        let image = module.image;
        assert!(image.is_none(), "no moduleImage in this fixture");
        let plugin = &module.steps[0].groups[0].plugins[0];
        assert_eq!(plugin.description, "Ligne 1\nLigne 2");
        assert_eq!(plugin.image.as_deref(), Some("shots\\standard.png"));
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
        let files = resolve(&module, &["s0-g0-p1".to_string()], &empty_env()).unwrap();

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
    fn select_exactly_one_refuses_an_empty_choice() {
        let module = parse(SIMPLE).unwrap();
        let err = resolve(&module, &[], &empty_env()).unwrap_err();
        assert!(err.contains("Intensité"), "{err}");
        assert!(err.contains("choisis une option"), "{err}");
    }

    #[test]
    fn select_at_most_one_allows_nothing_and_at_least_one_does_not() {
        let at_most = parse(&group_xml("SelectAtMostOne")).unwrap();
        let files = resolve(&at_most, &[], &empty_env()).unwrap();
        assert_eq!(sources(&files), vec!["need.txt".to_string()]);

        let at_least = parse(&group_xml("SelectAtLeastOne")).unwrap();
        let err = resolve(&at_least, &[], &empty_env()).unwrap_err();
        assert!(err.contains("au moins"), "{err}");
        let picked = resolve(&at_least, &["s0-g0-p0".to_string()], &empty_env()).unwrap();
        assert!(sources(&picked).contains(&"a.txt".to_string()));
    }

    #[test]
    fn select_any_allows_an_empty_choice() {
        let module = parse(&group_xml("SelectAny")).unwrap();
        let files = resolve(&module, &[], &empty_env()).unwrap();
        assert_eq!(sources(&files), vec!["need.txt".to_string()]);
        let both = resolve(
            &module,
            &["s0-g0-p0".to_string(), "s0-g0-p1".to_string()],
            &empty_env(),
        )
        .unwrap();
        let names = sources(&both);
        assert!(names.contains(&"a.txt".to_string()));
        assert!(names.contains(&"b.txt".to_string()));
    }

    #[test]
    fn select_all_installs_every_usable_plugin_and_skips_not_usable() {
        let xml = r#"<config>
          <requiredInstallFiles><file source="need.txt" destination="need.txt"/></requiredInstallFiles>
          <installSteps order="Explicit"><installStep name="S">
            <optionalFileGroups order="Explicit">
              <group name="Tout" type="SelectAll">
                <plugins order="Explicit">
                  <plugin name="Good"><files><file source="good.txt" destination="good.txt"/></files></plugin>
                  <plugin name="Bad">
                    <files><file source="bad.txt" destination="bad.txt"/></files>
                    <typeDescriptor><type name="NotUsable"/></typeDescriptor>
                  </plugin>
                </plugins>
              </group>
            </optionalFileGroups>
          </installStep></installSteps>
        </config>"#;
        let module = parse(xml).unwrap();
        let files = resolve(&module, &[], &empty_env()).unwrap();
        let names = sources(&files);
        assert!(names.contains(&"good.txt".to_string()), "{names:?}");
        assert!(!names.contains(&"bad.txt".to_string()), "{names:?}");
        let view = evaluate(&module, &[], &empty_env(), SelectionMode::Strict);
        let good = view.plugins.iter().find(|p| p.id == "s0-g0-p0").unwrap();
        let bad = view.plugins.iter().find(|p| p.id == "s0-g0-p1").unwrap();
        assert!(good.selected && good.locked);
        assert!(!bad.selected && bad.locked);
        assert_eq!(bad.reason.as_deref(), Some(NOT_USABLE_REASON));
    }

    #[test]
    fn dependency_type_forces_required_when_a_flag_matches() {
        let module = parse(FLAG_MODULE).unwrap();
        let alone = resolve(&module, &[], &empty_env()).unwrap();
        assert!(!sources(&alone).contains(&"patch.txt".to_string()));

        let files = resolve(&module, &["s0-g0-p0".to_string()], &empty_env()).unwrap();
        let names = sources(&files);
        assert!(names.contains(&"hd.txt".to_string()), "{names:?}");
        assert!(names.contains(&"patch.txt".to_string()), "{names:?}");

        let view = evaluate(
            &module,
            &["s0-g0-p0".to_string()],
            &empty_env(),
            SelectionMode::Strict,
        );
        let patch = view.plugins.iter().find(|p| p.id == "s0-g0-p1").unwrap();
        assert_eq!(patch.type_name, PluginTypeName::Required);
        assert!(patch.selected && patch.locked);
    }

    #[test]
    fn nested_or_dependency_matches_either_branch() {
        let xml = r#"<config><installSteps order="Explicit"><installStep name="S">
          <optionalFileGroups order="Explicit"><group name="G" type="SelectAny">
            <plugins order="Explicit">
              <plugin name="A">
                <conditionFlags><flag name="a">1</flag></conditionFlags>
                <files><file source="a.txt" destination="a.txt"/></files>
              </plugin>
              <plugin name="BC">
                <conditionFlags>
                  <flag name="b">2</flag>
                  <flag name="c">3</flag>
                </conditionFlags>
                <files><file source="bc.txt" destination="bc.txt"/></files>
              </plugin>
              <plugin name="Gate">
                <files><file source="gate.txt" destination="gate.txt"/></files>
                <typeDescriptor><dependencyType>
                  <defaultType name="Optional"/>
                  <patterns><pattern>
                    <dependencies operator="Or">
                      <flagDependency flag="a" value="1"/>
                      <dependencies operator="And">
                        <flagDependency flag="b" value="2"/>
                        <flagDependency flag="c" value="3"/>
                      </dependencies>
                    </dependencies>
                    <type name="Required"/>
                  </pattern></patterns>
                </dependencyType></typeDescriptor>
              </plugin>
            </plugins>
          </group></optionalFileGroups>
        </installStep></installSteps></config>"#;
        let module = parse(xml).unwrap();
        let from_a = resolve(&module, &["s0-g0-p0".to_string()], &empty_env()).unwrap();
        assert!(sources(&from_a).contains(&"gate.txt".to_string()));
        let from_bc = resolve(&module, &["s0-g0-p1".to_string()], &empty_env()).unwrap();
        assert!(sources(&from_bc).contains(&"gate.txt".to_string()));
        let none = resolve(&module, &[], &empty_env()).unwrap();
        assert!(!sources(&none).contains(&"gate.txt".to_string()));
    }

    #[test]
    fn an_invisible_step_is_skipped_until_its_flag_is_set() {
        let module = parse(VISIBLE_MODULE).unwrap();
        let hidden = resolve(&module, &["s1-g0-p0".to_string()], &empty_env()).unwrap();
        assert!(
            !sources(&hidden).contains(&"more.txt".to_string()),
            "a picked plugin on a hidden step is not installed"
        );
        let view = evaluate(
            &module,
            &["s1-g0-p0".to_string()],
            &empty_env(),
            SelectionMode::Strict,
        );
        assert!(!view.steps.iter().find(|s| s.id == "s1").unwrap().visible);

        let err = resolve(&module, &["s0-g0-p0".to_string()], &empty_env()).unwrap_err();
        assert!(err.contains("Extra"), "{err}");

        let shown = resolve(
            &module,
            &["s0-g0-p0".to_string(), "s1-g0-p0".to_string()],
            &empty_env(),
        )
        .unwrap();
        let names = sources(&shown);
        assert!(names.contains(&"yes.txt".to_string()));
        assert!(names.contains(&"more.txt".to_string()));
    }

    #[test]
    fn conditional_file_installs_follow_the_final_flags() {
        let xml = r#"<config>
          <requiredInstallFiles><file source="need.txt" destination="need.txt"/></requiredInstallFiles>
          <installSteps order="Explicit"><installStep name="S">
            <optionalFileGroups order="Explicit"><group name="G" type="SelectAny">
              <plugins order="Explicit">
                <plugin name="Yes">
                  <conditionFlags><flag name="go">yes</flag></conditionFlags>
                  <files><file source="yes.txt" destination="yes.txt"/></files>
                </plugin>
              </plugins>
            </group></optionalFileGroups>
          </installStep></installSteps>
          <conditionalFileInstalls>
            <patterns>
              <pattern>
                <dependencies><flagDependency flag="go" value="yes"/></dependencies>
                <files><file source="bonus.txt" destination="bonus.txt"/></files>
              </pattern>
              <pattern>
                <dependencies><flagDependency flag="go" value="no"/></dependencies>
                <files><file source="nope.txt" destination="nope.txt"/></files>
              </pattern>
            </patterns>
          </conditionalFileInstalls>
        </config>"#;
        let module = parse(xml).unwrap();
        let off = resolve(&module, &[], &empty_env()).unwrap();
        assert_eq!(sources(&off), vec!["need.txt".to_string()]);
        let on = resolve(&module, &["s0-g0-p0".to_string()], &empty_env()).unwrap();
        let names = sources(&on);
        assert!(names.contains(&"bonus.txt".to_string()), "{names:?}");
        assert!(!names.contains(&"nope.txt".to_string()), "{names:?}");
    }

    #[test]
    fn always_install_and_install_if_usable_do_not_wait_for_a_tick() {
        let xml = r#"<config><installSteps order="Explicit"><installStep name="S">
          <optionalFileGroups order="Explicit"><group name="G" type="SelectAny">
            <plugins order="Explicit">
              <plugin name="Always">
                <files><file source="always.txt" destination="always.txt" alwaysInstall="true"/></files>
              </plugin>
              <plugin name="Usable">
                <files><file source="usable.txt" destination="usable.txt" installIfUsable="true"/></files>
              </plugin>
              <plugin name="Plain">
                <files><file source="plain.txt" destination="plain.txt"/></files>
              </plugin>
              <plugin name="Dead">
                <files><file source="dead.txt" destination="dead.txt" installIfUsable="true"/></files>
                <typeDescriptor><type name="NotUsable"/></typeDescriptor>
              </plugin>
            </plugins>
          </group></optionalFileGroups>
        </installStep></installSteps></config>"#;
        let module = parse(xml).unwrap();
        let names = sources(&resolve(&module, &[], &empty_env()).unwrap());
        assert!(names.contains(&"always.txt".to_string()), "{names:?}");
        assert!(names.contains(&"usable.txt".to_string()), "{names:?}");
        assert!(!names.contains(&"plain.txt".to_string()), "{names:?}");
        assert!(!names.contains(&"dead.txt".to_string()), "{names:?}");
    }

    #[test]
    fn higher_priority_wins_a_destination_ignoring_case_and_ties_keep_the_later_file() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("low.txt"), b"low").unwrap();
        fs::write(dir.path().join("high.txt"), b"high").unwrap();
        fs::write(dir.path().join("late.txt"), b"late").unwrap();
        let xml = r#"<config><requiredInstallFiles>
          <file source="low.txt" destination="Out.TXT" priority="1"/>
          <file source="high.txt" destination="out.txt" priority="4"/>
          <file source="late.txt" destination="other.txt" priority="2"/>
          <file source="low.txt" destination="other.txt" priority="2"/>
        </requiredInstallFiles></config>"#;
        let module = parse(xml).unwrap();
        let plan = install_plan(dir.path(), &module, &[], &empty_env()).unwrap();
        let out = plan
            .iter()
            .find(|file| dest_key(&file.destination) == "out.txt")
            .unwrap();
        assert_eq!(out.source, PathBuf::from("high.txt"));
        let other = plan
            .iter()
            .find(|file| dest_key(&file.destination) == "other.txt")
            .unwrap();
        assert_eq!(
            other.source,
            PathBuf::from("low.txt"),
            "same priority: the later entry wins"
        );
    }

    #[test]
    fn a_higher_priority_file_replaces_one_child_of_a_folder() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join("Bundle")).unwrap();
        fs::write(dir.path().join("Bundle").join("a.txt"), b"low").unwrap();
        fs::write(dir.path().join("Bundle").join("b.txt"), b"keep").unwrap();
        fs::write(dir.path().join("winner.txt"), b"high").unwrap();
        let xml = r#"<config><requiredInstallFiles>
          <folder source="Bundle" destination="" priority="0"/>
          <file source="winner.txt" destination="a.txt" priority="5"/>
        </requiredInstallFiles></config>"#;
        let module = parse(xml).unwrap();
        let plan = install_plan(dir.path(), &module, &[], &empty_env()).unwrap();
        let winner = plan
            .iter()
            .find(|file| file.destination == PathBuf::from("a.txt"))
            .unwrap();
        assert_eq!(winner.source, PathBuf::from("winner.txt"));
        assert!(plan.iter().any(|file| {
            file.destination == PathBuf::from("b.txt")
                && file.source == PathBuf::from("Bundle/b.txt")
        }));
    }

    #[test]
    fn a_source_path_that_leaves_the_archive_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let xml = r#"<config><requiredInstallFiles>
          <file source="../secret.txt" destination="secret.txt"/>
        </requiredInstallFiles></config>"#;
        let module = parse(xml).unwrap();
        let err = install_plan(dir.path(), &module, &[], &empty_env()).unwrap_err();
        assert!(err.contains("refusé"), "{err}");
    }

    #[test]
    fn order_defaults_to_ascending_and_explicit_keeps_the_document() {
        let ascending = r#"<config><installSteps><installStep name="S">
          <optionalFileGroups><group name="G" type="SelectAny">
            <plugins>
              <plugin name="Zebra"/>
              <plugin name="Alpha"/>
            </plugins>
          </group></optionalFileGroups>
        </installStep></installSteps></config>"#;
        let module = parse(ascending).unwrap();
        let names: Vec<_> = module.steps[0].groups[0]
            .plugins
            .iter()
            .map(|plugin| plugin.name.as_str())
            .collect();
        assert_eq!(names, vec!["Alpha", "Zebra"]);

        let explicit = r#"<config><installSteps order="Explicit">
          <installStep name="S">
            <optionalFileGroups order="Explicit"><group name="G" type="SelectAny">
              <plugins order="Explicit">
                <plugin name="Zebra"/>
                <plugin name="Alpha"/>
              </plugins>
            </group></optionalFileGroups>
          </installStep>
        </installSteps></config>"#;
        let module = parse(explicit).unwrap();
        let names: Vec<_> = module.steps[0].groups[0]
            .plugins
            .iter()
            .map(|plugin| plugin.name.as_str())
            .collect();
        assert_eq!(names, vec!["Zebra", "Alpha"]);
    }

    #[test]
    fn descending_order_reverses_steps_groups_and_plugins() {
        let xml = r#"<config><installSteps order="Descending">
          <installStep name="Alpha"><optionalFileGroups order="Descending">
            <group name="Bravo" type="SelectAny"><plugins order="Descending">
              <plugin name="Un"/>
              <plugin name="Deux"/>
            </plugins></group>
            <group name="Alpha" type="SelectAny"><plugins><plugin name="X"/></plugins></group>
          </optionalFileGroups></installStep>
          <installStep name="Zulu"><optionalFileGroups/></installStep>
        </installSteps></config>"#;
        let module = parse(xml).unwrap();
        assert_eq!(module.steps[0].name, "Zulu");
        assert_eq!(module.steps[1].name, "Alpha");
        assert_eq!(module.steps[1].groups[0].name, "Bravo");
        assert_eq!(module.steps[1].groups[1].name, "Alpha");
        let names: Vec<_> = module.steps[1].groups[0]
            .plugins
            .iter()
            .map(|plugin| plugin.name.as_str())
            .collect();
        assert_eq!(names, vec!["Un", "Deux"]);
    }

    #[test]
    fn file_dependency_reads_enabled_mods_disabled_mods_and_game_data() {
        let dir = tempfile::tempdir().unwrap();
        let enabled = dir.path().join("on");
        fs::create_dir_all(enabled.join("meshes")).unwrap();
        fs::write(enabled.join("meshes").join("A.nif"), b"nif").unwrap();
        fs::write(enabled.join("Cool.esp"), b"esp").unwrap();
        let disabled = dir.path().join("off");
        fs::create_dir(&disabled).unwrap();
        fs::write(disabled.join("Old.esp"), b"esp").unwrap();
        let data = dir.path().join("Data");
        fs::create_dir(&data).unwrap();
        fs::write(data.join("Skyrim.esm"), b"esm").unwrap();
        let snapshot = BranchSnapshot::new(Some(data), vec![enabled], vec![disabled]);
        assert_eq!(snapshot.state(r"meshes\a.nif"), FileState::Active);
        assert_eq!(snapshot.state("Cool.esp"), FileState::Active);
        assert_eq!(snapshot.state("Data/Skyrim.esm"), FileState::Active);
        assert_eq!(snapshot.state("Old.esp"), FileState::Inactive);
        assert_eq!(snapshot.state("Nope.esp"), FileState::Missing);
        assert_eq!(snapshot.state(r"..\secret.txt"), FileState::Missing);

        let xml = r#"<config><installSteps order="Explicit"><installStep name="S">
          <optionalFileGroups order="Explicit"><group name="G" type="SelectAny">
            <plugins order="Explicit">
              <plugin name="Patch">
                <files><file source="patch.txt" destination="patch.txt"/></files>
                <typeDescriptor><dependencyType>
                  <defaultType name="NotUsable"/>
                  <patterns><pattern>
                    <dependencies>
                      <fileDependency file="Cool.esp" state="Active"/>
                    </dependencies>
                    <type name="Required"/>
                  </pattern></patterns>
                </dependencyType></typeDescriptor>
              </plugin>
            </plugins>
          </group></optionalFileGroups>
        </installStep></installSteps></config>"#;
        let module = parse(xml).unwrap();
        let env = FomodEnv {
            file_state: &|file| snapshot.state(file),
            game_version: None,
        };
        let files = resolve(&module, &[], &env).unwrap();
        assert!(sources(&files).contains(&"patch.txt".to_string()));
    }

    #[test]
    fn game_dependency_is_a_minimum_version_and_fomm_is_always_met() {
        assert!(version_at_least("1.6.1170.0", "1.6.1130"));
        assert!(version_at_least("1.6.1170", "1.6.1170.0"));
        assert!(!version_at_least("1.5.97.0", "1.6"));

        let xml = r#"<config><installSteps order="Explicit"><installStep name="S">
          <optionalFileGroups order="Explicit"><group name="G" type="SelectAny">
            <plugins order="Explicit">
              <plugin name="Game">
                <files><file source="game.txt" destination="game.txt"/></files>
                <typeDescriptor><dependencyType>
                  <defaultType name="Optional"/>
                  <patterns><pattern>
                    <dependencies><gameDependency version="1.6.1130"/></dependencies>
                    <type name="Required"/>
                  </pattern></patterns>
                </dependencyType></typeDescriptor>
              </plugin>
              <plugin name="Fomm">
                <files><file source="fomm.txt" destination="fomm.txt"/></files>
                <typeDescriptor><dependencyType>
                  <defaultType name="Optional"/>
                  <patterns><pattern>
                    <dependencies><fommDependency version="999.0"/></dependencies>
                    <type name="Required"/>
                  </pattern></patterns>
                </dependencyType></typeDescriptor>
              </plugin>
            </plugins>
          </group></optionalFileGroups>
        </installStep></installSteps></config>"#;
        let module = parse(xml).unwrap();
        let new_game = FomodEnv {
            file_state: &missing_file,
            game_version: Some("1.6.1170.0"),
        };
        let files = resolve(&module, &[], &new_game).unwrap();
        let names = sources(&files);
        assert!(names.contains(&"game.txt".to_string()), "{names:?}");
        assert!(names.contains(&"fomm.txt".to_string()), "{names:?}");

        let old_game = FomodEnv {
            file_state: &missing_file,
            game_version: Some("1.5.97.0"),
        };
        let files = resolve(&module, &[], &old_game).unwrap();
        assert!(!sources(&files).contains(&"game.txt".to_string()));
        assert!(sources(&files).contains(&"fomm.txt".to_string()));
    }

    #[test]
    fn recommended_is_prechecked_only_for_the_default_pass() {
        let module = parse(SIMPLE).unwrap();
        let defaults = evaluate(&module, &[], &empty_env(), SelectionMode::Defaults);
        let sombre = defaults
            .plugins
            .iter()
            .find(|plugin| plugin.id == "s0-g0-p1")
            .unwrap();
        assert!(sombre.selected);
        assert_eq!(sombre.type_name, PluginTypeName::Recommended);
        let strict = evaluate(&module, &[], &empty_env(), SelectionMode::Strict);
        assert!(strict.plugins.iter().all(|plugin| !plugin.selected));
        assert!(strict.groups[0].hint.is_some());
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
    fn empty_file_destination_keeps_the_source_relative_path() {
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

    /// Engine Fixes ships `<folder source="Required" destination=""/>`.
    /// The folder name must not survive into the mod: its `SKSE/Plugins`
    /// children belong at the mod root, beside the DLL the other option
    /// installs to `SKSE/Plugins`.
    #[test]
    fn empty_folder_destination_installs_the_folder_contents_at_the_root() {
        let xml = r#"<config>
          <requiredInstallFiles>
            <folder source="Required" destination=""/>
          </requiredInstallFiles>
          <installSteps>
            <installStep name="Main">
              <optionalFileGroups>
                <group name="DLL" type="SelectExactlyOne">
                  <plugins>
                    <plugin name="AE">
                      <files>
                        <folder source="AE/SKSE/Plugins" destination="SKSE/Plugins"/>
                      </files>
                    </plugin>
                  </plugins>
                </group>
              </optionalFileGroups>
            </installStep>
          </installSteps>
        </config>"#;
        let module = parse(xml).unwrap();
        assert!(
            module.required_files[0].destination.as_os_str().is_empty(),
            "Required must not be replayed as a folder name: {:?}",
            module.required_files[0].destination
        );
        let files = resolve(&module, &["s0-g0-p0".to_string()], &empty_env()).unwrap();
        assert_eq!(files[1].destination, PathBuf::from("SKSE/Plugins"));
    }

    #[test]
    fn missing_root_element_is_refused_not_defaulted() {
        let err = parse("<notconfig></notconfig>").unwrap_err();
        assert!(err.contains("config"));
    }

    #[test]
    fn reads_a_real_utf16_module_config_without_garbling_it() {
        let xml = r#"<config><moduleName>Café Mod</moduleName></config>"#;
        let mut bytes = vec![0xFF, 0xFE];
        for unit in xml.encode_utf16() {
            bytes.extend_from_slice(&unit.to_le_bytes());
        }
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("ModuleConfig.xml");
        fs::write(&path, &bytes).unwrap();

        let module = read_module_config(&path).unwrap();
        assert_eq!(module.name, "Café Mod");
    }

    #[test]
    fn reads_a_plain_utf8_module_config_with_no_bom() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("ModuleConfig.xml");
        fs::write(&path, SIMPLE).unwrap();
        assert_eq!(read_module_config(&path).unwrap().name, "Lux");
    }

    #[test]
    fn finds_module_config_regardless_of_casing() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join("FOMOD")).unwrap();
        fs::write(dir.path().join("FOMOD/moduleconfig.xml"), SIMPLE).unwrap();
        let found = find_module_config(dir.path()).unwrap();
        assert_eq!(found, dir.path().join("FOMOD").join("moduleconfig.xml"));
    }

    #[test]
    fn find_module_config_is_none_without_a_fomod_folder() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("readme.txt"), "hi").unwrap();
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

    #[test]
    fn image_path_outside_the_staging_root_is_refused() {
        let parent = tempfile::tempdir().unwrap();
        let staging = parent.path().join("staging");
        fs::create_dir_all(staging.join("shots")).unwrap();
        fs::write(staging.join("shots").join("ok.png"), b"ok").unwrap();
        fs::write(parent.path().join("secret.png"), b"secret").unwrap();
        fs::write(staging.join("note.txt"), b"note").unwrap();

        let err = read_fomod_image(&staging, r"..\secret.png").unwrap_err();
        assert!(err.contains("refusé"), "{err}");
        let err = read_fomod_image(&staging, r"shots\..\..\secret.png").unwrap_err();
        assert!(err.contains("refusé"), "{err}");
        let outside = parent.path().join("secret.png");
        let err = read_fomod_image(&staging, outside.to_str().unwrap()).unwrap_err();
        assert!(err.contains("refusé"), "{err}");
        let err = read_fomod_image(&staging, "note.txt").unwrap_err();
        assert!(err.contains("non pris en charge"), "{err}");

        fs::write(staging.join("big.png"), vec![0_u8; 8]).unwrap();
        let err = read_fomod_image_limited(&staging, "big.png", 4).unwrap_err();
        assert!(err.contains("volumineuse"), "{err}");

        let bytes = read_fomod_image(&staging, r"Shots\OK.PNG").unwrap();
        assert_eq!(bytes, b"ok");
        assert_eq!(
            image_data_url(b"hi", "shot.png"),
            "data:image/png;base64,aGk="
        );
    }

    #[test]
    fn module_image_keeps_show_and_height() {
        let xml = r#"<config>
          <moduleName>Avec image</moduleName>
          <moduleImage path="fomod\header.jpg" showImage="false" height="120"/>
        </config>"#;
        let module = parse(xml).unwrap();
        let image = module.image.unwrap();
        assert_eq!(image.path, "fomod\\header.jpg");
        assert!(!image.show);
        assert_eq!(image.height, Some(120));
    }

    fn group_xml(group_type: &str) -> String {
        format!(
            r#"<config>
              <requiredInstallFiles><file source="need.txt" destination="need.txt"/></requiredInstallFiles>
              <installSteps order="Explicit"><installStep name="S">
                <optionalFileGroups order="Explicit">
                  <group name="G" type="{group_type}">
                    <plugins order="Explicit">
                      <plugin name="A"><files><file source="a.txt" destination="a.txt"/></files></plugin>
                      <plugin name="B"><files><file source="b.txt" destination="b.txt"/></files></plugin>
                    </plugins>
                  </group>
                </optionalFileGroups>
              </installStep></installSteps>
            </config>"#
        )
    }

    const FLAG_MODULE: &str = r#"<config>
      <requiredInstallFiles><file source="need.txt" destination="need.txt"/></requiredInstallFiles>
      <installSteps order="Explicit"><installStep name="S">
        <optionalFileGroups order="Explicit"><group name="Look" type="SelectAny">
          <plugins order="Explicit">
            <plugin name="HD">
              <conditionFlags><flag name="tex">hd</flag></conditionFlags>
              <files><file source="hd.txt" destination="hd.txt"/></files>
            </plugin>
            <plugin name="Patch">
              <files><file source="patch.txt" destination="patch.txt"/></files>
              <typeDescriptor><dependencyType>
                <defaultType name="Optional"/>
                <patterns><pattern>
                  <dependencies operator="And">
                    <flagDependency flag="tex" value="hd"/>
                  </dependencies>
                  <type name="Required"/>
                </pattern></patterns>
              </dependencyType></typeDescriptor>
            </plugin>
          </plugins>
        </group></optionalFileGroups>
      </installStep></installSteps>
    </config>"#;

    const VISIBLE_MODULE: &str = r#"<config>
      <requiredInstallFiles><file source="need.txt" destination="need.txt"/></requiredInstallFiles>
      <installSteps order="Explicit">
        <installStep name="Base">
          <optionalFileGroups order="Explicit"><group name="Go" type="SelectAny">
            <plugins order="Explicit">
              <plugin name="Yes">
                <conditionFlags><flag name="go">yes</flag></conditionFlags>
                <files><file source="yes.txt" destination="yes.txt"/></files>
              </plugin>
            </plugins>
          </group></optionalFileGroups>
        </installStep>
        <installStep name="Extra">
          <visible><dependencies>
            <flagDependency flag="go" value="yes"/>
          </dependencies></visible>
          <optionalFileGroups order="Explicit"><group name="Extra" type="SelectExactlyOne">
            <plugins order="Explicit">
              <plugin name="More">
                <files><file source="more.txt" destination="more.txt"/></files>
              </plugin>
            </plugins>
          </group></optionalFileGroups>
        </installStep>
      </installSteps>
    </config>"#;
}
