//! The `GameAdapter` contract — see docs/01-architecture-decidee.md §3.1.
//!
//! This module defines the shape every game integration follows. It
//! implements none of it: the Core must never know what "Skyrim" is, or
//! what a `.esp` is, or where Steam puts things. A per-game crate (e.g.
//! `modlab-game-skyrim`) provides that. The trait grows only when two
//! adapters share a behaviour; a Skyrim-only rule never lands here.

use std::path::{Path, PathBuf};

/// What's known about one detected game installation.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct GameInstall {
    /// Stable identifier for the game, e.g. `"skyrimse"`.
    pub id: String,
    /// Human-readable name, e.g. `"Skyrim Special Edition"`.
    pub name: String,
    /// The game's install directory.
    pub root: PathBuf,
}

/// Game vocabulary exposed to clients, not a promise that each view is implemented.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum GameSection {
    Dashboard,
    Mods,
    Plugins,
    Archives,
    Directory,
    Saves,
    Health,
    Packs,
    Worlds,
    Servers,
}

/// Whether a path slot names a directory or a single file — purely so a
/// client can pick the right file-picker mode. The Core never validates it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum PathKind {
    Dir,
    File,
}

/// One folder or executable a Game Adapter cares about — e.g. Skyrim's
/// install root (found from its `.exe`) or Minecraft's `.minecraft` folder
/// (found from `%APPDATA%`). `detected` is whatever this adapter's own
/// heuristics find *right now*; a client may let a human override it —
/// persisted by the Core, keyed by `(game id, slot id)` — without this
/// module knowing overrides exist at all. Every game gets to define its own
/// list: nothing here assumes a Skyrim-shaped or Minecraft-shaped install.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PathSlot {
    /// Stable id within this adapter, e.g. `"root"`, `"data_dir"`.
    pub id: &'static str,
    pub label: &'static str,
    pub kind: PathKind,
    pub detected: Option<PathBuf>,
}

/// A relevant game tool. Declaration does not imply installation or execution support.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameTool {
    pub id: &'static str,
    pub name: &'static str,
}

/// Static adapter knowledge. Clients own visual styling, never game capability inference.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameCapabilities {
    pub short_name: &'static str,
    pub sections: Vec<GameSection>,
    pub deployment_label: &'static str,
    pub script_extender: Option<&'static str>,
    pub load_order_tool: Option<GameTool>,
    /// An external mod manager whose instance this game's mods can be
    /// migrated from, as an opaque source id (e.g. `"mo2"`). The Core does
    /// not know what the id means; clients map it to a label and a dialog
    /// the same way they map section ids to tab labels. `None` — the
    /// default for any game without such a migration path — means the UI
    /// shows no import entry point at all.
    pub import_source: Option<&'static str>,
}

/// Where an instance's own files live. The Core stores a path; it does not
/// decide which games need one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstanceRootPolicy {
    /// One shared install. Mods are not kept in a folder named after the
    /// instance (Skyrim).
    SharedInstall,
    /// Each instance owns `<data root>/instances/<dir_name>/.<slug>`.
    /// `subdirs` are empty directories created inside that folder.
    PerInstanceRoot {
        dir_name: &'static str,
        subdirs: &'static [&'static str],
    },
}

/// Slot id of a human override for the per-instance folder itself.
pub const INSTANCE_ROOT_SLOT: &str = "instance_root";

/// What the caller already resolved before asking where mods and config live.
#[derive(Clone, Copy)]
pub struct LocationQuery<'a> {
    /// Resolved game install (override, else detection). Not an instance folder.
    pub install_root: Option<&'a Path>,
    pub data_root: &'a Path,
    /// Stored per-instance folder, when [`InstanceRootPolicy::PerInstanceRoot`]
    /// has assigned one.
    pub instance_root: Option<&'a Path>,
}

/// A local archive the adapter may stage. The Core does not interpret it.
pub struct ArchiveSource<'a> {
    pub name: &'a str,
    pub archive_path: &'a Path,
}

/// An archive staged for install. `shape` is the adapter's own token
/// (`"fomod"`, `"jar"`, …). `refusal` is a French sentence when the file
/// cannot be installed; the caller shows it and stops.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagedMod {
    pub name: String,
    pub shape: String,
    pub needs_wizard: bool,
    pub staged_path: PathBuf,
    pub content_root: PathBuf,
    pub refusal: Option<String>,
}

/// Whether a data-relative file is already present for the branch a guided
/// installer is about to join. The Core does not decide what "active" means
/// for a game — the caller does, from that game's mods and data folder.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BranchFileState {
    /// Nothing on the branch or in the game data folder provides it.
    Missing,
    /// An enabled mod, or the game data folder, provides it.
    Active,
    /// Only a disabled mod provides it.
    Inactive,
}

impl BranchFileState {
    /// Callback for an install that does not look at other files.
    pub fn missing(_: &str) -> Self {
        Self::Missing
    }
}

/// File work for one install. The database lock is not held.
pub struct PrepareInstall<'a> {
    pub data_root: &'a Path,
    pub mod_id: i64,
    pub name: &'a str,
    pub archive_path: &'a Path,
    pub content_root: &'a Path,
    /// Managed mod folder (`<data root>/mods/<id>`). Shared-install games
    /// place the tree here.
    pub dest: &'a Path,
    pub instance_root: Option<&'a Path>,
    pub mods_dir: Option<&'a Path>,
    pub resource_packs: Option<&'a Path>,
    pub shader_packs: Option<&'a Path>,
    pub picked: &'a [String],
    /// Game version the guided installer may compare, when the caller read one.
    pub game_version: Option<&'a str>,
    /// Data-relative files already on the branch. A plain install never calls it.
    pub branch_file: &'a dyn Fn(&str) -> BranchFileState,
}

/// What `prepare_install` left on disk, ready for the transaction.
pub enum PreparedMod {
    /// A directory swap already applied at `dest`.
    Directory {
        dest: PathBuf,
        swap: crate::files::DirSwap,
        adjustments: crate::install::InstallAdjustments,
    },
    /// Files already copied. `primary` is what the mod row points at.
    /// `placed` is everything to move to the trash if the commit fails,
    /// including `primary`.
    Files {
        primary: PathBuf,
        placed: Vec<PathBuf>,
        adjustments: crate::install::InstallAdjustments,
    },
}

/// Where a game keeps the mods ModLab installs or imports.
///
/// The two shapes both current adapters share: a folder ModLab owns under
/// its data root, or a folder derived from the game's install root.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModsLocation {
    /// ModLab owns this directory (`<data root>/mods` for Skyrim). It is not
    /// inside the game install, and a stored path override does not move it.
    Managed(PathBuf),
    /// `<install root>/…`. `None` when the install root itself is unknown.
    InInstall(Option<PathBuf>),
}

/// Where a game keeps its config files.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigLocation {
    /// Detected on its own (Skyrim's My Games folder). A prefs-file override
    /// relocates the folder to that file's parent.
    PrefsParent { detected: Option<PathBuf> },
    /// Derived from the install root. A prefs override does not move it.
    /// `None` when the install root is unknown.
    InInstall(Option<PathBuf>),
}

/// One path after the single precedence rule: a human override wins when
/// one is stored, otherwise the adapter's own detection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FusedPath {
    pub detected: Option<PathBuf>,
    pub override_path: Option<PathBuf>,
    pub effective: Option<PathBuf>,
}

/// Human override when `override_path` is set, otherwise `detected`.
/// Every client that shows or uses a game path goes through this so the
/// precedence cannot be reimplemented differently in each command.
#[must_use]
pub fn fuse_path(detected: Option<PathBuf>, override_path: Option<PathBuf>) -> FusedPath {
    let effective = override_path.clone().or_else(|| detected.clone());
    FusedPath {
        detected,
        override_path,
        effective,
    }
}

/// One mod row an adapter may recognize as a loader. Paths may name a
/// file that is not on disk yet (a download still in flight). The Core
/// does not interpret the strings — each adapter does.
#[derive(Debug, Clone, Copy)]
pub struct ToolClassifyQuery<'a> {
    pub name: &'a str,
    pub source_path: &'a Path,
    pub archive_path: Option<&'a Path>,
    /// Version already stored on the row, when the mod declared one.
    /// Adapters may ignore it. It is not a license to invent a loader
    /// version the file never stated.
    pub stored_version: Option<&'a str>,
    /// Capture URL. Empty when this row has no download ledger entry.
    pub source_url: &'a str,
    /// Capture file name. Empty when unknown.
    pub download_file_name: &'a str,
}

/// A loader the adapter recognized on that row.
///
/// `tool` is a free id (`"skse"`, `"skse-vr"`, `"forge"`, `"fabric"`,
/// `"neoforge"`, …), not a closed enum: the Core does not know which
/// games exist. `version` is set only when the adapter actually read one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClassifiedTool {
    pub tool: String,
    pub version: Option<String>,
}

/// Facts read from a local archive. Every field is optional: an adapter
/// leaves a value unset when the file did not state it. The Core stores
/// the strings and does not interpret them — a game version, a mod id and
/// a category are all just text to this crate.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ArchiveMetadata {
    pub name: Option<String>,
    pub version: Option<String>,
    /// Free-form category text. Prefer [`Self::category_raw`] for new reads;
    /// this stays so an older caller still has a place to put a label.
    pub category: Option<String>,
    /// Category text exactly as the file wrote it.
    pub category_raw: Option<String>,
    /// Numeric category id when the file had one (MO2 stores Nexus ids).
    pub category_nexus_id: Option<String>,
    pub author: Option<String>,
    pub website: Option<String>,
    /// Remote id the file itself stated (`meta.ini` `modid`). Not guessed
    /// from a file name — that parse lives in the Core.
    pub nexus_mod_id: Option<String>,
    /// Page the file pointed at (`meta.ini` `url`, or a website that is
    /// already a mod page).
    pub page_url: Option<String>,
    /// Host token the file stated (`nexus`, …). The Core stores it and
    /// does not decide what it means.
    pub source_host: Option<String>,
    pub game_versions: Vec<String>,
    /// Dependencies the file declared, with kind when it stated one.
    pub dependencies: Vec<DeclaredDependency>,
    /// Loader ids this file stated (`fabric`, `forge`, `neoforge`, `quilt`).
    /// Empty when it stated none. Opaque to the Core.
    pub loaders: Vec<String>,
}

impl ArchiveMetadata {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        fn blank(value: &Option<String>) -> bool {
            value.as_ref().map(|s| s.trim().is_empty()).unwrap_or(true)
        }
        blank(&self.name)
            && blank(&self.version)
            && blank(&self.category)
            && blank(&self.category_raw)
            && blank(&self.category_nexus_id)
            && blank(&self.author)
            && blank(&self.website)
            && blank(&self.nexus_mod_id)
            && blank(&self.page_url)
            && blank(&self.source_host)
            && self.game_versions.is_empty()
            && self.dependencies.is_empty()
            && self.loaders.is_empty()
    }

    /// `self` wins. `fallback` fills only the fields `self` left empty.
    #[must_use]
    pub fn overlay_missing(self, fallback: Self) -> Self {
        fn text(primary: Option<String>, fallback: Option<String>) -> Option<String> {
            primary.filter(|s| !s.trim().is_empty()).or(fallback)
        }
        Self {
            name: text(self.name, fallback.name),
            version: text(self.version, fallback.version),
            category: text(self.category, fallback.category),
            category_raw: text(self.category_raw, fallback.category_raw),
            category_nexus_id: text(self.category_nexus_id, fallback.category_nexus_id),
            author: text(self.author, fallback.author),
            website: text(self.website, fallback.website),
            nexus_mod_id: text(self.nexus_mod_id, fallback.nexus_mod_id),
            page_url: text(self.page_url, fallback.page_url),
            source_host: text(self.source_host, fallback.source_host),
            game_versions: if self.game_versions.is_empty() {
                fallback.game_versions
            } else {
                self.game_versions
            },
            dependencies: if self.dependencies.is_empty() {
                fallback.dependencies
            } else {
                self.dependencies
            },
            loaders: if self.loaders.is_empty() {
                fallback.loaders
            } else {
                self.loaders
            },
        }
    }
}

/// One category this game offers. `id` is what gets stored on the mod.
/// `label` is French, for the UI. `icon` is a name in the shell's icon set.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameCategory {
    pub id: &'static str,
    pub label: &'static str,
    pub icon: &'static str,
}

/// What a client already has about a mod's source category.
///
/// Every field is optional. The adapter decides which ones it understands.
/// The Core does not.
#[derive(Debug, Clone, Copy, Default)]
pub struct CategoryHint<'a> {
    /// Nexus `category_id`, when one was stored. `None` when absent.
    pub nexus_id: Option<i64>,
    /// A label or a number as text: Nexus name, CurseForge class,
    /// Modrinth category, MO2 `category`, or `categoryRaw`.
    pub raw: Option<&'a str>,
    /// Installer group names (FOMOD `<group name>`). Last resort.
    pub groups: &'a [&'a str],
    /// Mod name. Last resort, and only for an unambiguous token.
    pub name: Option<&'a str>,
    /// Installed folder, when the mod has one. An adapter may list it.
    /// It does not extract anything.
    pub folder: Option<&'a Path>,
    /// Packed archive still on disk (zip, jar, 7z). An adapter may list
    /// its entry names or read a small manifest. `None` when there is no
    /// archive. Not extracted.
    pub archive: Option<&'a Path>,
}

/// Fold a source label so "Magic - Spells & Enchantments", `magic` and
/// `Magie` can share one table. Accents, case and punctuation go away.
#[must_use]
pub fn normalize_category_label(raw: &str) -> String {
    let mut out = String::new();
    let mut spaced = true;
    for ch in raw.trim().chars() {
        if ch == '&' {
            push_gap(&mut out, &mut spaced);
            out.push_str("and");
            spaced = false;
            continue;
        }
        let folded = fold_char(ch);
        if folded.is_alphanumeric() {
            out.push(folded);
            spaced = false;
        } else if !spaced {
            out.push(' ');
            spaced = true;
        }
    }
    while out.ends_with(' ') {
        out.pop();
    }
    out
}

fn push_gap(out: &mut String, spaced: &mut bool) {
    if !*spaced {
        out.push(' ');
        *spaced = true;
    }
}

fn fold_char(ch: char) -> char {
    let lower = ch.to_lowercase().next().unwrap_or(ch);
    match lower {
        'é' | 'è' | 'ê' | 'ë' => 'e',
        'à' | 'â' | 'ä' | 'á' => 'a',
        'ù' | 'û' | 'ü' | 'ú' => 'u',
        'ô' | 'ö' | 'ó' => 'o',
        'î' | 'ï' | 'í' => 'i',
        'ç' => 'c',
        'ñ' => 'n',
        other => other,
    }
}

/// `42`, `42,54` or `"-1,"`. `None` when the text is not a list of numbers.
#[must_use]
pub fn parse_category_ids(raw: &str) -> Option<Vec<i64>> {
    let trimmed = raw.trim().trim_matches(|c| c == '"' || c == '\'');
    if trimmed.is_empty() {
        return None;
    }
    let mut ids = Vec::new();
    for part in trimmed.split(|c: char| c == ',' || c == ';' || c.is_whitespace()) {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        let Ok(id) = part.parse::<i64>() else {
            return None;
        };
        ids.push(id);
    }
    if ids.is_empty() {
        None
    } else {
        Some(ids)
    }
}

fn is_unassigned(id: i64) -> bool {
    id == -1 || id == 0
}

enum Read<'a> {
    Missing,
    Hit(&'a str),
    Miss,
}

/// Where [`category_step`] stopped.
pub enum CategoryStep<'a> {
    /// An id, a label, or a group already decided. `None` means the
    /// category stays empty: the name and the files must not fill it.
    Settled(Option<&'a str>),
    /// Nothing explicit was stored. A clear name, or the files, may still match.
    Open,
}

/// Decide from the explicit part of a hint: Nexus id, raw label, groups.
///
/// [`CategoryStep::Open`] is the only case where a name token or a look at
/// the files is allowed. An unknown id or label, or two groups that
/// disagree, settle on empty instead.
#[must_use]
pub fn category_step<'a>(
    hint: &CategoryHint<'_>,
    nexus: &'a [(i64, &'a str)],
    labels: &'a [(&'a str, &'a str)],
) -> CategoryStep<'a> {
    let from_id = read_nexus_id(hint.nexus_id, nexus);
    let from_raw = match hint.raw.map(str::trim).filter(|s| !s.is_empty()) {
        None => Read::Missing,
        Some(raw) => read_raw(raw, nexus, labels),
    };
    match (from_id, from_raw) {
        (Read::Hit(left), Read::Hit(right)) if left != right => {
            return CategoryStep::Settled(None);
        }
        (Read::Hit(id), _) => return CategoryStep::Settled(Some(id)),
        (Read::Miss, Read::Hit(id)) => return CategoryStep::Settled(Some(id)),
        (Read::Miss, _) => return CategoryStep::Settled(None),
        (Read::Missing, Read::Hit(id)) => return CategoryStep::Settled(Some(id)),
        (Read::Missing, Read::Miss) => return CategoryStep::Settled(None),
        (Read::Missing, Read::Missing) => {}
    }
    match group_hits(hint.groups, labels).as_slice() {
        [id] => CategoryStep::Settled(Some(*id)),
        [_, ..] => CategoryStep::Settled(None),
        [] => CategoryStep::Open,
    }
}

/// Turn a [`CategoryHint`] into one id from the adapter's tables.
///
/// An explicit source wins over the mod name. An unknown explicit source
/// stays empty — the name is not consulted, so a guess cannot override a
/// label we simply do not know. Two sources that disagree also stay empty.
#[must_use]
pub fn resolve_category_hint<'a>(
    hint: &CategoryHint<'_>,
    nexus: &'a [(i64, &'a str)],
    labels: &'a [(&'a str, &'a str)],
    name_tokens: &'a [(&'a str, &'a str)],
) -> Option<&'a str> {
    match category_step(hint, nexus, labels) {
        CategoryStep::Settled(value) => value,
        CategoryStep::Open => hint
            .name
            .and_then(|name| match_category_name(name, labels, name_tokens)),
    }
}

fn read_nexus_id<'a>(id: Option<i64>, nexus: &'a [(i64, &'a str)]) -> Read<'a> {
    match id {
        None => Read::Missing,
        Some(id) if is_unassigned(id) => Read::Missing,
        Some(id) => match lookup_nexus(nexus, id) {
            Some(hit) => Read::Hit(hit),
            None => Read::Miss,
        },
    }
}

fn read_raw<'a>(raw: &str, nexus: &'a [(i64, &'a str)], labels: &[(&'a str, &'a str)]) -> Read<'a> {
    if let Some(nums) = parse_category_ids(raw) {
        let assigned: Vec<i64> = nums.into_iter().filter(|id| !is_unassigned(*id)).collect();
        if assigned.is_empty() {
            return Read::Missing;
        }
        let mut hits: Vec<&str> = Vec::new();
        let mut unknown = false;
        for id in assigned {
            match lookup_nexus(nexus, id) {
                Some(hit) if !hits.contains(&hit) => hits.push(hit),
                Some(_) => {}
                None => unknown = true,
            }
        }
        return match (unknown, hits.as_slice()) {
            (false, [id]) => Read::Hit(id),
            _ => Read::Miss,
        };
    }
    match label_id(labels, raw) {
        Some(id) => Read::Hit(id),
        None => Read::Miss,
    }
}

fn lookup_nexus<'a>(nexus: &'a [(i64, &'a str)], id: i64) -> Option<&'a str> {
    nexus
        .iter()
        .find(|(nid, _)| *nid == id)
        .map(|(_, hit)| *hit)
}

/// Exact label, or the longest parent label the text starts with
/// (`"Magic - Spells"` starts with `"magic"`).
fn label_id<'a>(labels: &[(&'a str, &'a str)], raw: &str) -> Option<&'a str> {
    let key = normalize_category_label(raw);
    if key.is_empty() {
        return None;
    }
    if let Some((_, id)) = labels.iter().find(|(label, _)| *label == key) {
        return Some(*id);
    }
    let mut best: Option<&str> = None;
    let mut best_len = 0usize;
    for (label, id) in labels {
        if label.len() < 4 || key.len() <= label.len() {
            continue;
        }
        if key.starts_with(label) && key[label.len()..].starts_with(' ') && label.len() > best_len {
            best = Some(*id);
            best_len = label.len();
        }
    }
    best
}

fn group_hits<'a>(groups: &[&str], labels: &[(&'a str, &'a str)]) -> Vec<&'a str> {
    let mut hits = Vec::new();
    for group in groups {
        if let Some(id) = label_id(labels, group) {
            if !hits.contains(&id) {
                hits.push(id);
            }
        }
    }
    hits
}

/// Whole words and whole phrases in a mod name.
///
/// `SkyUI` is also tried as `sky ui`, and `EngineFixes` as `engine fixes`.
/// A phrase matches only with a word boundary on both sides. Two tokens
/// that point at different ids stay empty.
#[must_use]
pub fn match_category_name<'a>(
    name: &str,
    labels: &[(&'a str, &'a str)],
    tokens: &[(&'a str, &'a str)],
) -> Option<&'a str> {
    let mut found: Option<&str> = None;
    for form in [normalize_category_label(name), loosen_category_name(name)] {
        if form.is_empty() {
            continue;
        }
        let mut hits: Vec<(&str, &str)> = Vec::new();
        if let Some((label, id)) = labels.iter().find(|(label, _)| *label == form) {
            hits.push((label, id));
        }
        for (token, id) in tokens {
            if phrase_in(&form, token) {
                hits.push((token, id));
            }
        }
        // "ferrite core" contains the word "core". The longer phrase wins,
        // so a generic token does not cancel a more specific one.
        let snapshot = hits.clone();
        hits.retain(|(token, _)| {
            !snapshot
                .iter()
                .any(|(other, _)| other.len() > token.len() && phrase_in(other, token))
        });
        for (_, id) in hits {
            if !note_category(&mut found, id) {
                return None;
            }
        }
    }
    found
}

fn note_category<'a>(found: &mut Option<&'a str>, id: &'a str) -> bool {
    match *found {
        Some(prev) if prev != id => false,
        Some(_) => true,
        None => {
            *found = Some(id);
            true
        }
    }
}

fn phrase_in(haystack: &str, phrase: &str) -> bool {
    if phrase.is_empty() || haystack.is_empty() {
        return false;
    }
    let mut padded = String::with_capacity(haystack.len() + 2);
    padded.push(' ');
    padded.push_str(haystack);
    padded.push(' ');
    let mut needle = String::with_capacity(phrase.len() + 2);
    needle.push(' ');
    needle.push_str(phrase);
    needle.push(' ');
    padded.contains(&needle)
}

/// Split CamelCase and the letter/digit boundary, then fold like a label.
fn loosen_category_name(name: &str) -> String {
    let chars: Vec<char> = name.chars().collect();
    let mut spaced = String::new();
    for (index, ch) in chars.iter().copied().enumerate() {
        if index > 0 && camel_boundary(chars[index - 1], ch, chars.get(index + 1).copied()) {
            spaced.push(' ');
        }
        spaced.push(ch);
    }
    normalize_category_label(&spaced)
}

fn camel_boundary(prev: char, current: char, next: Option<char>) -> bool {
    (prev.is_lowercase() && current.is_uppercase())
        || (prev.is_uppercase()
            && current.is_uppercase()
            && next.is_some_and(|ch| ch.is_lowercase()))
        || (prev.is_ascii_alphabetic() && current.is_ascii_digit())
        || (prev.is_ascii_digit() && current.is_ascii_alphabetic())
}

/// One slot after the shell has applied override-else-detection.
/// `id` is the adapter's own slot id. This crate does not interpret it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlotPath {
    pub id: String,
    pub path: PathBuf,
}

/// How a declared or resolved dependency relates to the mod.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DependencyKind {
    /// Hard requirement. Missing or disabled ones are badge problems.
    #[default]
    Required,
    /// Soft suggestion. Shown on Infos; never a badge problem alone.
    Optional,
    /// Must not be present together.
    Incompatible,
    /// Present is allowed but not recommended.
    Discouraged,
    /// Reverse: another mod on the branch depends on this one.
    UsedBy,
}

/// One dependency id a file declared, with the kind it stated.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeclaredDependency {
    pub id: String,
    #[serde(default)]
    pub kind: DependencyKind,
}

impl DeclaredDependency {
    #[must_use]
    pub fn required(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            kind: DependencyKind::Required,
        }
    }

    #[must_use]
    pub fn with_kind(id: impl Into<String>, kind: DependencyKind) -> Self {
        Self {
            id: id.into(),
            kind,
        }
    }
}

/// One mod of the active branch, in terms the Core already stores.
///
/// `tool` is whatever [`GameAdapter::classify_tool`] returned.
/// `dependencies` is the list the file declared. Both stay opaque here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DependencyMod {
    pub mod_id: i64,
    pub name: String,
    pub source_path: PathBuf,
    pub archive_path: Option<PathBuf>,
    pub enabled: bool,
    pub position: i64,
    pub tool: Option<String>,
    pub dependencies: Vec<DeclaredDependency>,
}

/// Whether one dependency is satisfied on this branch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum DependencyState {
    /// An enabled mod provides it, or the adapter recognized a file the
    /// game itself ships.
    Active,
    /// A mod of the branch provides it, and that mod is disabled.
    Disabled,
    /// Nothing on the branch provides it.
    Missing,
}

/// One dependency the adapter could resolve. No entry means it does not
/// know — a client must not turn that silence into a warning.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedDependency {
    pub name: String,
    pub state: DependencyState,
    /// Branch mod that provides it, when one does.
    pub provider_mod_id: Option<i64>,
    /// That mod's name. `None` when there is no provider mod.
    pub provider_name: Option<String>,
    /// Required / optional / … / used-by. Absent in old payloads → required.
    #[serde(default)]
    pub kind: DependencyKind,
}

/// Dependencies of one mod. Mods with an empty list are omitted.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModDependencyReport {
    pub mod_id: i64,
    pub dependencies: Vec<ResolvedDependency>,
}

/// What a Game Adapter must be able to do.
///
/// Deliberately small. Installation is here because two games install an
/// archive differently: the shell calls the instance's adapter instead of
/// a Skyrim function. Diagnosis and launch stay out until a second caller
/// needs them.
pub trait GameAdapter {
    /// Stable identifier for this adapter's game, e.g. `"skyrimse"`.
    fn id(&self) -> &'static str;

    /// Declare game vocabulary and relevant tools independently of detection.
    fn capabilities(&self) -> GameCapabilities;

    /// Try to find a real install of this game on the current machine.
    fn detect(&self) -> Option<GameInstall>;

    /// The named folders/executables this game cares about, each with
    /// whatever this adapter's own detection finds right now. Independent of
    /// `detect()` succeeding — a slot can be reported even when the overall
    /// install wasn't found, so a human can point ModLab at it by hand from
    /// Réglages → Jeux instead.
    fn path_slots(&self) -> Vec<PathSlot>;

    /// Mods folder for this game.
    fn mods_location(&self, query: &LocationQuery<'_>) -> ModsLocation;

    /// Config folder for this game, before a prefs-file override is applied.
    fn config_location(&self, query: &LocationQuery<'_>) -> ConfigLocation;

    /// Whether each instance owns a folder under the data root.
    fn instance_root_policy(&self) -> InstanceRootPolicy {
        InstanceRootPolicy::SharedInstall
    }

    /// Child of a per-instance root for a path slot (`"mods_dir"`,
    /// `"config_dir"`, `"resourcepacks"`, `"shaderpacks"`). `None` when
    /// this game has no such child.
    fn instance_subdir(&self, _slot_id: &str) -> Option<&'static str> {
        None
    }

    /// Canonical config file names, when the game has a fixed set (Skyrim's
    /// three INIs). Empty means "list the `.ini` files that are actually in
    /// the folder".
    fn known_config_files(&self) -> &'static [&'static str] {
        &[]
    }

    /// Path-slot id of the prefs file whose parent is the config folder.
    /// `None` when `config_location` does not track a prefs file.
    fn prefs_file_slot(&self) -> Option<&'static str> {
        None
    }

    /// File name that receives `prefs_file_slot` when no override is set.
    fn prefs_file_name(&self) -> Option<&'static str> {
        None
    }

    /// Whether this row is a loader this game knows. `None` for an ordinary
    /// mod, and for a game that has no loader to recognize — the default.
    /// The shell calls this instead of branching on the game id.
    fn classify_tool(&self, _query: &ToolClassifyQuery<'_>) -> Option<ClassifiedTool> {
        None
    }

    /// Name, version, game versions and dependencies read from a local
    /// archive. `None` when the file is missing, unreadable, or says
    /// nothing this adapter knows. The default reads nothing — a game
    /// without an on-disk manifest stays quiet instead of guessing.
    fn read_archive_metadata(&self, _path: &Path) -> Option<ArchiveMetadata> {
        None
    }

    /// Name, version, game versions and loaders the file name itself states
    /// for this game. `None` when the name is not one this adapter
    /// recognizes — the Core's generic parse still runs. A loader id or a
    /// game version in a jar name belongs here, not in the Core.
    fn filename_metadata(&self, _file_name: &str) -> Option<ArchiveMetadata> {
        None
    }

    /// Page for a remote id this game knows how to address.
    ///
    /// `host` is the opaque token stored on the mod (`nexus`, …). The
    /// default has no page: the Core never invents a domain.
    fn remote_page(&self, _host: &str, _remote_id: &str) -> Option<String> {
        None
    }

    /// This game's fixed category list, including « Autre ». Empty for a
    /// game that has not declared one — the UI then shows no menu.
    fn categories(&self) -> &'static [GameCategory] {
        &[]
    }

    /// Map a source hint onto one id from [`Self::categories`]. `None`
    /// when nothing matched, or when the hint was ambiguous.
    fn resolve_category(&self, _hint: &CategoryHint<'_>) -> Option<&'static str> {
        None
    }

    /// Dependencies of the mods on this branch.
    ///
    /// `paths` are this adapter's slots, already fused by the caller. The
    /// default reports nothing: a game that cannot tell stays quiet instead
    /// of guessing. A disabled mod can still be listed, so a client can show
    /// its own dependencies; the table hides that badge itself.
    fn resolve_dependencies(
        &self,
        _mods: &[DependencyMod],
        _paths: &[SlotPath],
    ) -> Vec<ModDependencyReport> {
        Vec::new()
    }

    /// Unpack and classify an archive. The default refuses: a game with no
    /// installer stays quiet instead of guessing a layout.
    fn stage_archive(
        &self,
        _data_root: &Path,
        _mod_id: i64,
        _source: &ArchiveSource<'_>,
    ) -> std::result::Result<StagedMod, String> {
        Err("Ce jeu ne sait pas installer une archive.".into())
    }

    /// Copy the staged content into place. No database access.
    fn prepare_install(
        &self,
        _job: &PrepareInstall<'_>,
    ) -> std::result::Result<PreparedMod, String> {
        Err("Ce jeu ne sait pas installer une archive.".into())
    }

    /// Guided install (a FOMOD wizard). The default has none.
    fn prepare_guided(
        &self,
        _job: &PrepareInstall<'_>,
    ) -> std::result::Result<PreparedMod, String> {
        Err("Ce jeu n'a pas d'installateur guidé.".into())
    }

    /// Move this mod's staging directory to the trash, if it is still there.
    fn discard_stage(&self, data_root: &Path, mod_id: i64) {
        let staging = data_root.join("staging").join(mod_id.to_string());
        if staging.exists() {
            let _ = crate::files::move_to_trash(data_root, &staging);
        }
    }

    /// Files this adapter placed for `mod_id`, to move to the trash on
    /// uninstall. `None` means the caller only retracts the managed mod
    /// folder — a shared-install game, and an import whose files were never
    /// copied by ModLab.
    fn retract_files(
        &self,
        _data_root: &Path,
        _mod_id: i64,
        _source: &Path,
    ) -> Option<Vec<PathBuf>> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Bare;

    impl GameAdapter for Bare {
        fn id(&self) -> &'static str {
            "bare"
        }

        fn capabilities(&self) -> GameCapabilities {
            GameCapabilities {
                short_name: "Bare",
                sections: Vec::new(),
                deployment_label: "",
                script_extender: None,
                load_order_tool: None,
                import_source: None,
            }
        }

        fn detect(&self) -> Option<GameInstall> {
            None
        }

        fn path_slots(&self) -> Vec<PathSlot> {
            Vec::new()
        }

        fn mods_location(&self, _query: &LocationQuery<'_>) -> ModsLocation {
            ModsLocation::InInstall(None)
        }

        fn config_location(&self, _query: &LocationQuery<'_>) -> ConfigLocation {
            ConfigLocation::InInstall(None)
        }
    }

    #[test]
    fn a_game_without_loaders_classifies_nothing() {
        let name = "fabric-loader-0.16.9.jar";
        let mark = Bare.classify_tool(&ToolClassifyQuery {
            name,
            source_path: Path::new(name),
            archive_path: None,
            stored_version: Some("9.9.9"),
            source_url: "",
            download_file_name: name,
        });
        assert!(mark.is_none());
    }

    #[test]
    fn a_game_without_a_resolver_reports_no_dependency() {
        let mods = [DependencyMod {
            mod_id: 1,
            name: "Example".to_string(),
            source_path: PathBuf::from("example.jar"),
            archive_path: None,
            enabled: true,
            position: 0,
            tool: None,
            dependencies: vec![DeclaredDependency::required("fabric-api")],
        }];
        assert!(Bare.resolve_dependencies(&mods, &[]).is_empty());
    }
}
