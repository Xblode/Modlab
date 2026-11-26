//! Plugin load order for Skyrim SE: which `.esp`/`.esm`/`.esl` the game must
//! load, in which order — and the `plugins.txt` that tells it so.
//!
//! Skyrim only loads plugins listed in `%LOCALAPPDATA%\Skyrim Special
//! Edition\Plugins.txt` (Windows-1252, one name per line, a leading `*` means
//! active, line order = load order). ModLab never writes that real file: it
//! writes one `plugins.txt` per instance in its own data root and, at launch,
//! links it onto the game's path through USVFS — exactly what MO2 does (see
//! `usvfs::Vfs::link_file`).
//!
//! # Why `esplugin` and not a hand-written reader
//!
//! The header facts needed here (master/light flags, `MAST` sub-records in
//! Windows-1252, the rule that `.esm`/`.esl` imply the master flag and `.esl`
//! implies light) are exactly what `esplugin` implements — it is the crate
//! LOOT itself is built on, covered by its own tests against real plugins, and
//! it only pulls four small dependencies. A hand-written reader would
//! duplicate those rules with fewer tests. Its licence (GPL-3.0) matches the
//! workspace's. `docs/01-architecture-decidee.md` §4 already names it as
//! sufficient for load-order sorting (not for record-level analysis).
//!
//! # Ordering rules (no LOOT)
//!
//! 1. Official plugins first, always active, in this order: `Skyrim.esm`,
//!    `Update.esm`, `Dawnguard.esm`, `HearthFires.esm`, `Dragonborn.esm`, then
//!    the names in `Skyrim.ccc` (game root) when they exist. They are shown
//!    but *not* written to `plugins.txt` (the game loads them implicitly and
//!    libloadorder omits them too).
//! 2. Masters before non-masters.
//! 3. Every plugin after all its masters (stable topological sort).
//! 4. Ties go to the mod priority (`position`) of the file that wins the
//!    conflict, then to the file name.
//!
//! Problems never abort the computation: a missing master, an unreadable
//! file, a cycle or a limit overflow becomes a [`LoadOrderIssue`], and the
//! plugin concerned is written *inactive* so the game can still start.

use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap, HashSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use encoding_rs::WINDOWS_1252;
use esplugin::{GameId, ParseOptions, Plugin};
use modlab_core::loadout::LoadoutEntryRow;

/// Always loaded first, in this order, whatever `plugins.txt` says.
pub const OFFICIAL_PLUGINS: [&str; 5] = [
    "Skyrim.esm",
    "Update.esm",
    "Dawnguard.esm",
    "HearthFires.esm",
    "Dragonborn.esm",
];

/// Active full (non-light) plugins the engine can hold: indices 0x00..=0xFE.
pub const MAX_FULL_PLUGINS: usize = 255;
/// Active light (ESL) plugins the engine can hold.
pub const MAX_LIGHT_PLUGINS: usize = 4096;

/// Where the game looks for its plugin list, relative to `%LOCALAPPDATA%`
/// (Steam edition).
const GAME_PLUGINS_TXT: [&str; 2] = ["Skyrim Special Edition", "Plugins.txt"];

/// Where a plugin comes from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PluginSource {
    /// An enabled mod of the instance (the one whose file wins a conflict).
    Mod { mod_id: i64, mod_name: String },
    /// A file already sitting in the game's real `Data/` folder.
    GameData,
}

/// One plugin of the computed load order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadOrderEntry {
    /// File name as found on disk (original case).
    pub file_name: String,
    /// Official plugin: loaded first, implicitly active, not written out.
    pub official: bool,
    pub source: PluginSource,
    /// How many layers (mods + game `Data/`) provide this exact file name.
    pub contributors: usize,
    /// Master flag, or `.esm`/`.esl` extension.
    pub master: bool,
    /// Light flag, or `.esl` extension.
    pub light: bool,
    /// Will be written with a `*`. `false` for a plugin that cannot load.
    pub active: bool,
    /// Masters named by the header that no plugin of the instance provides.
    pub missing_masters: Vec<String>,
    /// `false` when the header could not be parsed.
    pub readable: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IssueKind {
    /// `plugin` needs `detail`, which nothing provides.
    MissingMaster,
    /// `plugin` needs `detail`, which is itself inactive.
    InactiveMaster,
    /// `plugin`'s header could not be read (`detail` = why).
    Unreadable,
    /// `plugin` is part of a master cycle: no valid order exists.
    MasterCycle,
    /// `plugin` is a master but had to load after a non-master.
    MasterAfterNonMaster,
    /// More than [`MAX_FULL_PLUGINS`] active full plugins: `plugin` is dropped.
    FullLimit,
    /// More than [`MAX_LIGHT_PLUGINS`] active light plugins: `plugin` is dropped.
    LightLimit,
    /// `plugin`'s name has characters Windows-1252 cannot hold: not writable.
    UnencodableName,
}

impl IssueKind {
    /// Stable identifier for clients (the UI turns it into French text).
    #[must_use]
    pub fn code(self) -> &'static str {
        match self {
            Self::MissingMaster => "missing_master",
            Self::InactiveMaster => "inactive_master",
            Self::Unreadable => "unreadable",
            Self::MasterCycle => "master_cycle",
            Self::MasterAfterNonMaster => "master_after_non_master",
            Self::FullLimit => "full_limit",
            Self::LightLimit => "light_limit",
            Self::UnencodableName => "unencodable_name",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadOrderIssue {
    pub kind: IssueKind,
    pub plugin: String,
    pub detail: Option<String>,
}

/// The computed order plus everything worth telling the human.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LoadOrder {
    /// In load order, officials first.
    pub entries: Vec<LoadOrderEntry>,
    pub issues: Vec<LoadOrderIssue>,
}

impl LoadOrder {
    /// `(active full, active light)` — officials included, as the engine
    /// counts them.
    #[must_use]
    pub fn active_counts(&self) -> (usize, usize) {
        let mut full = 0;
        let mut light = 0;
        for e in self.entries.iter().filter(|e| e.active) {
            if e.light {
                light += 1;
            } else {
                full += 1;
            }
        }
        (full, light)
    }
}

/// What the real game install contributes. Both parts are optional so the
/// order can still be computed (and shown) on a machine without Skyrim.
#[derive(Debug, Clone, Default)]
pub struct GameContext {
    /// Game root — where `Skyrim.ccc` lives.
    pub install_root: Option<PathBuf>,
    /// The real `Data/` folder (base layer under every mod).
    pub data_dir: Option<PathBuf>,
}

/// Facts read from a plugin's TES4 header.
struct Header {
    master: bool,
    light: bool,
    masters: Vec<String>,
}

fn read_header(path: &Path) -> Result<Header, String> {
    let mut plugin = Plugin::new(GameId::SkyrimSE, path);
    plugin
        .parse_file(ParseOptions::header_only())
        .map_err(|e| e.to_string())?;
    let masters = plugin.masters().map_err(|e| e.to_string())?;
    Ok(Header {
        master: plugin.is_master_file(),
        light: plugin.is_light_plugin(),
        masters,
    })
}

fn lower(name: &str) -> String {
    name.to_lowercase()
}

fn extension_is(name: &str, ext: &str) -> bool {
    Path::new(name)
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case(ext))
}

fn is_plugin_name(name: &str) -> bool {
    ["esp", "esm", "esl"].iter().any(|e| extension_is(name, e))
}

/// Plugins directly inside `dir` (the game ignores sub-folders), sorted by
/// lower-cased name. A missing or unreadable folder yields nothing — a mod
/// whose folder vanished simply contributes no plugin.
fn plugin_files(dir: &Path) -> Vec<(String, PathBuf)> {
    let Ok(read) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut out: Vec<(String, PathBuf)> = read
        .filter_map(Result::ok)
        .filter_map(|e| {
            let name = e.file_name().to_str()?.to_string();
            let path = e.path();
            (is_plugin_name(&name) && path.is_file()).then_some((name, path))
        })
        .collect();
    out.sort_by_key(|(name, _)| lower(name));
    out
}

/// The five official names, then the `Skyrim.ccc` ones (Creation Club
/// content), de-duplicated case-insensitively.
fn official_names(install_root: Option<&Path>) -> Vec<String> {
    let mut names: Vec<String> = OFFICIAL_PLUGINS.iter().map(|s| (*s).to_string()).collect();
    if let Some(bytes) = install_root.and_then(|r| fs::read(r.join("Skyrim.ccc")).ok()) {
        let (text, _, _) = WINDOWS_1252.decode(&bytes);
        names.extend(
            text.lines()
                .map(str::trim)
                .filter(|l| !l.is_empty() && !l.starts_with('#'))
                .map(str::to_string),
        );
    }
    let mut seen = HashSet::new();
    names.retain(|n| seen.insert(lower(n)));
    names
}

/// One file name as seen through the layers (game `Data/` < mod 1 < mod 2 …).
struct Candidate {
    file_name: String,
    path: PathBuf,
    source: PluginSource,
    contributors: usize,
    /// 0 = game `Data/`, `i + 1` = i-th enabled mod by priority.
    priority: usize,
}

fn add_candidate(
    map: &mut HashMap<String, Candidate>,
    file_name: String,
    path: PathBuf,
    source: PluginSource,
    priority: usize,
) {
    let key = lower(&file_name);
    let contributors = map.get(&key).map_or(0, |c| c.contributors) + 1;
    map.insert(
        key,
        Candidate {
            file_name,
            path,
            source,
            contributors,
            priority,
        },
    );
}

/// A candidate with its header facts, ready for sorting.
struct Node {
    cand: Candidate,
    master: bool,
    light: bool,
    masters: Vec<String>,
    unreadable: Option<String>,
}

fn load_node(cand: Candidate) -> Node {
    match read_header(&cand.path) {
        Ok(h) => Node {
            master: h.master,
            light: h.light,
            masters: h.masters,
            unreadable: None,
            cand,
        },
        Err(why) => Node {
            // Fall back on what the extension alone implies.
            master: extension_is(&cand.file_name, "esm") || extension_is(&cand.file_name, "esl"),
            light: extension_is(&cand.file_name, "esl"),
            masters: Vec::new(),
            unreadable: Some(why),
            cand,
        },
    }
}

fn encodable(name: &str) -> bool {
    !WINDOWS_1252.encode(name).2
}

/// Compute the load order of an instance.
///
/// `rows` are the instance's mods; disabled ones are ignored, the others are
/// layered by `position` (highest wins a same-named file, like USVFS and
/// `modlab_core::conflicts`). Nothing is written.
#[must_use]
pub fn compute_load_order(rows: &[LoadoutEntryRow], game: &GameContext) -> LoadOrder {
    // 1. Which file wins each plugin name.
    let mut candidates: HashMap<String, Candidate> = HashMap::new();
    if let Some(dir) = &game.data_dir {
        for (name, path) in plugin_files(dir) {
            add_candidate(&mut candidates, name, path, PluginSource::GameData, 0);
        }
    }
    let mut enabled: Vec<&LoadoutEntryRow> = rows.iter().filter(|r| r.enabled).collect();
    enabled.sort_by_key(|r| r.position);
    for (i, row) in enabled.iter().enumerate() {
        for (name, path) in plugin_files(&row.source_path) {
            let source = PluginSource::Mod {
                mod_id: row.mod_id,
                mod_name: row.name.clone(),
            };
            add_candidate(&mut candidates, name, path, source, i + 1);
        }
    }

    let mut order = LoadOrder::default();

    // 2. Official plugins: first, in fixed order, implicitly active.
    // Without a real `Data/` to check, the five base names are assumed
    // present so a mod depending on them is not reported broken.
    let mut early: HashSet<String> = HashSet::new();
    if game.data_dir.is_none() {
        early.extend(OFFICIAL_PLUGINS.iter().map(|n| lower(n)));
    }
    for name in official_names(game.install_root.as_deref()) {
        let Some(cand) = candidates.remove(&lower(&name)) else {
            continue;
        };
        early.insert(lower(&cand.file_name));
        let node = load_node(cand);
        order.entries.push(LoadOrderEntry {
            file_name: node.cand.file_name,
            official: true,
            source: node.cand.source,
            contributors: node.cand.contributors,
            master: node.master,
            light: node.light,
            active: true,
            missing_masters: Vec::new(),
            readable: node.unreadable.is_none(),
        });
    }

    // 3. Everything else, sorted.
    let mut nodes: Vec<Node> = candidates.into_values().map(load_node).collect();
    nodes.sort_by_key(|n| lower(&n.cand.file_name));
    let sorted = sort_nodes(&nodes, &early, &mut order.issues);

    // 4. Activation: drop what cannot load, enforce the engine limits.
    let (mut full, mut light) = order.active_counts();
    let mut inactive: HashSet<String> = HashSet::new();
    for &i in &sorted.indices {
        let node = &nodes[i];
        let name = &node.cand.file_name;
        let missing = &sorted.missing[i];
        let mut active = true;

        for m in missing {
            order.issues.push(LoadOrderIssue {
                kind: IssueKind::MissingMaster,
                plugin: name.clone(),
                detail: Some(m.clone()),
            });
            active = false;
        }
        if let Some(why) = &node.unreadable {
            order.issues.push(LoadOrderIssue {
                kind: IssueKind::Unreadable,
                plugin: name.clone(),
                detail: Some(why.clone()),
            });
            active = false;
        }
        if let Some(m) = node
            .masters
            .iter()
            .find(|m| inactive.contains(&lower(m)))
            .filter(|_| active)
        {
            order.issues.push(LoadOrderIssue {
                kind: IssueKind::InactiveMaster,
                plugin: name.clone(),
                detail: Some(m.clone()),
            });
            active = false;
        }
        if active && !encodable(name) {
            order.issues.push(LoadOrderIssue {
                kind: IssueKind::UnencodableName,
                plugin: name.clone(),
                detail: None,
            });
            active = false;
        }
        if active {
            let (count, limit, kind) = if node.light {
                (&mut light, MAX_LIGHT_PLUGINS, IssueKind::LightLimit)
            } else {
                (&mut full, MAX_FULL_PLUGINS, IssueKind::FullLimit)
            };
            if *count >= limit {
                order.issues.push(LoadOrderIssue {
                    kind,
                    plugin: name.clone(),
                    detail: None,
                });
                active = false;
            } else {
                *count += 1;
            }
        }
        if !active {
            inactive.insert(lower(name));
        }
    }

    // 5. Build the entries and double-check "masters before non-masters".
    let mut seen_non_master = false;
    for &i in &sorted.indices {
        let node = &nodes[i];
        if node.master && seen_non_master {
            order.issues.push(LoadOrderIssue {
                kind: IssueKind::MasterAfterNonMaster,
                plugin: node.cand.file_name.clone(),
                detail: None,
            });
        }
        seen_non_master |= !node.master;
    }
    for &i in &sorted.indices {
        let node = &nodes[i];
        order.entries.push(LoadOrderEntry {
            file_name: node.cand.file_name.clone(),
            official: false,
            source: node.cand.source.clone(),
            contributors: node.cand.contributors,
            master: node.master,
            light: node.light,
            active: !inactive.contains(&lower(&node.cand.file_name)),
            missing_masters: sorted.missing[i].clone(),
            readable: node.unreadable.is_none(),
        });
    }
    order
}

struct Sorted {
    /// Indices into `nodes`, in load order.
    indices: Vec<usize>,
    /// Per node: masters nothing provides.
    missing: Vec<Vec<String>>,
}

/// Stable topological sort (Kahn): among the plugins whose masters are all
/// placed, always take the smallest `(non-master, priority, name)`.
fn sort_nodes(nodes: &[Node], early: &HashSet<String>, issues: &mut Vec<LoadOrderIssue>) -> Sorted {
    let index: HashMap<String, usize> = nodes
        .iter()
        .enumerate()
        .map(|(i, n)| (lower(&n.cand.file_name), i))
        .collect();
    let mut waiting_on = vec![0usize; nodes.len()];
    let mut dependents: Vec<Vec<usize>> = vec![Vec::new(); nodes.len()];
    let mut missing: Vec<Vec<String>> = vec![Vec::new(); nodes.len()];

    for (i, node) in nodes.iter().enumerate() {
        let mut seen = HashSet::new();
        for master in &node.masters {
            let key = lower(master);
            if !seen.insert(key.clone()) || early.contains(&key) {
                continue;
            }
            match index.get(&key) {
                Some(&j) if j == i => {}
                Some(&j) => {
                    waiting_on[i] += 1;
                    dependents[j].push(i);
                }
                None => missing[i].push(master.clone()),
            }
        }
    }

    let key = |i: usize| {
        let n = &nodes[i];
        Reverse((!n.master, n.cand.priority, lower(&n.cand.file_name), i))
    };
    let mut ready: BinaryHeap<_> = (0..nodes.len())
        .filter(|&i| waiting_on[i] == 0)
        .map(key)
        .collect();
    let mut indices = Vec::with_capacity(nodes.len());
    let mut placed = vec![false; nodes.len()];
    while let Some(Reverse((_, _, _, i))) = ready.pop() {
        placed[i] = true;
        indices.push(i);
        for &d in &dependents[i] {
            waiting_on[d] -= 1;
            if waiting_on[d] == 0 {
                ready.push(key(d));
            }
        }
    }

    // Whatever is left is in a cycle (or behind one): no valid order exists,
    // keep them at the end in a deterministic order and say so.
    let mut stuck: Vec<usize> = (0..nodes.len()).filter(|&i| !placed[i]).collect();
    stuck.sort_by_key(|&i| {
        (
            !nodes[i].master,
            nodes[i].cand.priority,
            lower(&nodes[i].cand.file_name),
        )
    });
    for &i in &stuck {
        issues.push(LoadOrderIssue {
            kind: IssueKind::MasterCycle,
            plugin: nodes[i].cand.file_name.clone(),
            detail: None,
        });
    }
    indices.extend(stuck);

    Sorted { indices, missing }
}

/// The text of `plugins.txt` for `order`, in Windows-1252 with CRLF line
/// endings. Official plugins are left out (the game loads them anyway);
/// inactive ones are written without `*`; a name Windows-1252 cannot hold is
/// skipped (it is already inactive and flagged).
#[must_use]
pub fn render_plugins_txt(order: &LoadOrder) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(b"# This file is generated by ModLab at each launch. Do not edit.\r\n");
    for e in order.entries.iter().filter(|e| !e.official) {
        let (bytes, _, had_errors) = WINDOWS_1252.encode(&e.file_name);
        if had_errors {
            continue;
        }
        if e.active {
            out.push(b'*');
        }
        out.extend_from_slice(&bytes);
        out.extend_from_slice(b"\r\n");
    }
    out
}

/// Compute the load order and write the instance's `plugins.txt` at `dest`
/// (parents created, previous file replaced). Returns the order so the caller
/// can journal or show its issues. `dest` must be inside ModLab's own data
/// root — never the game's real AppData folder.
pub fn write_instance_plugins_txt(
    rows: &[LoadoutEntryRow],
    game: &GameContext,
    dest: &Path,
) -> io::Result<LoadOrder> {
    let order = compute_load_order(rows, game);
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = dest.with_extension("txt.tmp");
    fs::write(&tmp, render_plugins_txt(&order))?;
    fs::rename(&tmp, dest)?;
    Ok(order)
}

/// `<local_app_data>\Skyrim Special Edition\Plugins.txt` — the path the game
/// reads, and the target of the USVFS file link. Only a path: nothing is
/// created or touched.
#[must_use]
pub fn game_plugins_txt_path(local_app_data: &Path) -> PathBuf {
    GAME_PLUGINS_TXT
        .iter()
        .fold(local_app_data.to_path_buf(), |p, part| p.join(part))
}

/// [`game_plugins_txt_path`] under the current user's `%LOCALAPPDATA%`.
#[must_use]
pub fn find_game_plugins_txt() -> Option<PathBuf> {
    std::env::var_os("LOCALAPPDATA").map(|dir| game_plugins_txt_path(Path::new(&dir)))
}

/// Masters named by the plugin header.
///
/// `None` when the file cannot be read. The caller must not invent a list.
#[must_use]
pub(crate) fn read_masters(path: &Path) -> Option<Vec<String>> {
    read_header(path).ok().map(|header| header.masters)
}

/// Plugin files directly inside `dir`. Same rule as the load order: the game
/// does not load plugins from a subfolder.
#[must_use]
pub(crate) fn plugins_in(dir: &Path) -> Vec<(String, PathBuf)> {
    plugin_files(dir)
}

/// The five base names, then `Skyrim.ccc` when that file can be read.
#[must_use]
pub(crate) fn official_plugin_names(install_root: Option<&Path>) -> Vec<String> {
    official_names(install_root)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Minimal TES4 header as Skyrim SE writes it: a 24-byte record header,
    /// then `HEDR`, then one `MAST` + `DATA` pair per master (raw bytes, so a
    /// test can use Windows-1252 names).
    fn plugin_bytes(flags: u32, masters: &[&[u8]]) -> Vec<u8> {
        let mut body = Vec::new();
        body.extend_from_slice(b"HEDR");
        body.extend_from_slice(&12u16.to_le_bytes());
        body.extend_from_slice(&1.7f32.to_le_bytes());
        body.extend_from_slice(&0u32.to_le_bytes());
        body.extend_from_slice(&0x800u32.to_le_bytes());
        for m in masters {
            body.extend_from_slice(b"MAST");
            body.extend_from_slice(&u16::try_from(m.len() + 1).unwrap().to_le_bytes());
            body.extend_from_slice(m);
            body.push(0);
            body.extend_from_slice(b"DATA");
            body.extend_from_slice(&8u16.to_le_bytes());
            body.extend_from_slice(&[0; 8]);
        }
        let mut out = Vec::new();
        out.extend_from_slice(b"TES4");
        out.extend_from_slice(&u32::try_from(body.len()).unwrap().to_le_bytes());
        out.extend_from_slice(&flags.to_le_bytes());
        out.extend_from_slice(&0u32.to_le_bytes()); // form id
        out.extend_from_slice(&0u32.to_le_bytes()); // timestamp + version control
        out.extend_from_slice(&44u16.to_le_bytes()); // form version
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&body);
        out
    }

    const MASTER: u32 = 0x1;
    const LIGHT: u32 = 0x200;

    fn put(dir: &Path, name: &str, flags: u32, masters: &[&[u8]]) {
        fs::create_dir_all(dir).unwrap();
        fs::write(dir.join(name), plugin_bytes(flags, masters)).unwrap();
    }

    fn row(name: &str, dir: &Path, position: i64, enabled: bool) -> LoadoutEntryRow {
        LoadoutEntryRow {
            mod_id: position,
            name: name.to_string(),
            source_path: dir.to_path_buf(),
            enabled,
            position,
            version: None,
            status: modlab_core::loadout::STATUS_INSTALLED.to_string(),
            category: None,
            archive_path: None,
            source_host: None,
            page_url: None,
            game_versions: Vec::new(),
            dependencies: Vec::new(),
            loaders: Vec::new(),
            category_raw: None,
            category_nexus_id: None,
            nexus_mod_id: None,
            author: None,
            website: None,
            size_bytes: None,
            installed_at: None,
            downloaded_at: None,
        }
    }

    /// A fake game: root with `Data/` holding the given official plugins.
    fn fake_game(root: &Path, officials: &[&str]) -> GameContext {
        let data = root.join("Data");
        for o in officials {
            put(&data, o, MASTER, &[]);
        }
        GameContext {
            install_root: Some(root.to_path_buf()),
            data_dir: Some(data),
        }
    }

    fn names(order: &LoadOrder) -> Vec<&str> {
        order.entries.iter().map(|e| e.file_name.as_str()).collect()
    }

    fn entry<'a>(order: &'a LoadOrder, name: &str) -> &'a LoadOrderEntry {
        order
            .entries
            .iter()
            .find(|e| e.file_name == name)
            .unwrap_or_else(|| panic!("{name} not in {:?}", names(order)))
    }

    #[test]
    fn an_enabled_installed_mod_is_an_active_plugin_and_a_disabled_one_is_absent() {
        // A download that is a readme plus one folder used to be "unknown",
        // so nothing was installed and enabling the row could not show a
        // plugin. Once the folder is installed, only the enabled flag puts
        // the plugin in the load order — this is not SKSE.
        let staged = tempfile::tempdir().unwrap();
        let esp = staged.path().join("Pack").join("Data").join("Alpha.esp");
        fs::create_dir_all(esp.parent().unwrap()).unwrap();
        fs::write(&esp, plugin_bytes(0, &[b"Skyrim.esm"])).unwrap();
        fs::write(staged.path().join("readme.txt"), b"notes").unwrap();

        let detected = crate::installer::detect_shape(staged.path()).unwrap();
        let out = tempfile::tempdir().unwrap();
        let dest = out.path().join("mod");
        let keep = crate::engine::is_engine_content(&detected.content_root);
        crate::installer::place_content(&detected.content_root, &dest, keep).unwrap();
        assert!(dest.join("Alpha.esp").is_file());

        let game = GameContext {
            install_root: None,
            data_dir: None,
        };
        let on = compute_load_order(&[row("Alpha", &dest, 1, true)], &game);
        assert!(
            entry(&on, "Alpha.esp").active,
            "enabled mod must load: {:?}",
            on.issues
        );

        let off = compute_load_order(&[row("Alpha", &dest, 1, false)], &game);
        assert!(
            off.entries.iter().all(|e| e.file_name != "Alpha.esp"),
            "a disabled mod is not in the load order: {:?}",
            names(&off)
        );
    }

    #[test]
    fn official_plugins_come_first_in_fixed_order_and_are_not_written() {
        let tmp = tempfile::tempdir().unwrap();
        // Created in a deliberately scrambled order; Dawnguard/Update present,
        // HearthFires absent (not owned) — it must simply not appear.
        let game = fake_game(
            &tmp.path().join("game"),
            &[
                "Dragonborn.esm",
                "Skyrim.esm",
                "Dawnguard.esm",
                "Update.esm",
            ],
        );
        let root = game.install_root.clone().unwrap();
        put(
            &root.join("Data"),
            "ccBGSSSE001-Fish.esl",
            MASTER | LIGHT,
            &[],
        );
        fs::write(
            root.join("Skyrim.ccc"),
            b"ccBGSSSE001-Fish.esl\r\nccNotInstalled.esl\r\n",
        )
        .unwrap();

        let mod_a = tmp.path().join("a");
        put(&mod_a, "Alpha.esp", 0, &[b"Skyrim.esm"]);
        let order = compute_load_order(&[row("A", &mod_a, 1, true)], &game);

        assert_eq!(
            names(&order),
            [
                "Skyrim.esm",
                "Update.esm",
                "Dawnguard.esm",
                "Dragonborn.esm",
                "ccBGSSSE001-Fish.esl",
                "Alpha.esp"
            ]
        );
        assert!(order.entries[..5].iter().all(|e| e.official && e.active));
        assert!(order.issues.is_empty(), "{:?}", order.issues);

        let text = render_plugins_txt(&order);
        let text = String::from_utf8(text).unwrap();
        assert!(text.contains("*Alpha.esp\r\n"));
        assert!(!text.contains("Skyrim.esm"), "officials are implicit");
        assert!(!text.contains("Fish"), "ccc plugins are implicit");
    }

    #[test]
    fn master_loads_before_its_dependent_even_against_mod_priority() {
        let tmp = tempfile::tempdir().unwrap();
        let game = fake_game(&tmp.path().join("game"), &["Skyrim.esm"]);
        // The dependent sits in the lowest-priority mod, its master in a
        // higher one: priority alone would load the dependent first.
        let low = tmp.path().join("low");
        let high = tmp.path().join("high");
        put(&low, "Patch.esp", 0, &[b"Library.esp"]);
        put(&high, "Library.esp", 0, &[b"Skyrim.esm"]);
        let order = compute_load_order(
            &[row("Low", &low, 1, true), row("High", &high, 2, true)],
            &game,
        );
        assert_eq!(names(&order), ["Skyrim.esm", "Library.esp", "Patch.esp"]);
        assert!(order.issues.is_empty(), "{:?}", order.issues);
    }

    #[test]
    fn ties_follow_mod_priority_then_name() {
        let tmp = tempfile::tempdir().unwrap();
        let game = fake_game(&tmp.path().join("game"), &["Skyrim.esm"]);
        let first = tmp.path().join("first");
        let second = tmp.path().join("second");
        put(&second, "Aaa.esp", 0, &[]);
        put(&first, "Zzz.esp", 0, &[]);
        put(&first, "Mmm.esp", 0, &[]);
        let order = compute_load_order(
            // Rows deliberately given out of priority order.
            &[
                row("Second", &second, 2, true),
                row("First", &first, 1, true),
            ],
            &game,
        );
        assert_eq!(
            names(&order),
            ["Skyrim.esm", "Mmm.esp", "Zzz.esp", "Aaa.esp"]
        );
    }

    #[test]
    fn esp_with_master_flag_and_esl_load_before_plain_esp() {
        let tmp = tempfile::tempdir().unwrap();
        let game = fake_game(&tmp.path().join("game"), &["Skyrim.esm"]);
        let m = tmp.path().join("m");
        put(&m, "Aaa.esp", 0, &[]);
        put(&m, "Flagged.esp", MASTER, &[]); // .esp that carries the master flag
        put(&m, "Tiny.esl", 0, &[]); // .esl: master + light by extension
        put(&m, "LightEsp.esp", LIGHT, &[]); // light flag on an .esp
        let order = compute_load_order(&[row("M", &m, 1, true)], &game);

        assert_eq!(
            names(&order),
            [
                "Skyrim.esm",
                "Flagged.esp",
                "Tiny.esl",
                "Aaa.esp",
                "LightEsp.esp"
            ]
        );
        let flagged = entry(&order, "Flagged.esp");
        assert!(flagged.master && !flagged.light);
        let tiny = entry(&order, "Tiny.esl");
        assert!(tiny.master && tiny.light);
        let light_esp = entry(&order, "LightEsp.esp");
        assert!(!light_esp.master && light_esp.light);
        assert!(order.issues.is_empty(), "{:?}", order.issues);
    }

    #[test]
    fn missing_master_is_reported_and_deactivates_the_plugin_and_its_dependents() {
        let tmp = tempfile::tempdir().unwrap();
        let game = fake_game(&tmp.path().join("game"), &["Skyrim.esm"]);
        let m = tmp.path().join("m");
        put(&m, "Broken.esp", 0, &[b"Skyrim.esm", b"Gone.esm"]);
        put(&m, "NeedsBroken.esp", 0, &[b"Broken.esp"]);
        put(&m, "Fine.esp", 0, &[b"Skyrim.esm"]);
        let order = compute_load_order(&[row("M", &m, 1, true)], &game);

        let broken = entry(&order, "Broken.esp");
        assert!(!broken.active);
        assert_eq!(broken.missing_masters, ["Gone.esm"]);
        assert!(!entry(&order, "NeedsBroken.esp").active);
        assert!(entry(&order, "Fine.esp").active);
        assert!(order.issues.contains(&LoadOrderIssue {
            kind: IssueKind::MissingMaster,
            plugin: "Broken.esp".into(),
            detail: Some("Gone.esm".into()),
        }));
        assert!(order.issues.contains(&LoadOrderIssue {
            kind: IssueKind::InactiveMaster,
            plugin: "NeedsBroken.esp".into(),
            detail: Some("Broken.esp".into()),
        }));

        let text = String::from_utf8(render_plugins_txt(&order)).unwrap();
        assert!(text.contains("\r\nBroken.esp\r\n"), "inactive: no star");
        assert!(text.contains("*Fine.esp"));
    }

    #[test]
    fn accented_names_round_trip_through_windows_1252() {
        let tmp = tempfile::tempdir().unwrap();
        let game = fake_game(&tmp.path().join("game"), &["Skyrim.esm"]);
        let m = tmp.path().join("m");
        // The master sub-record holds the raw Windows-1252 byte 0xE9 for "é".
        put(&m, "Café.esm", MASTER, &[]);
        put(&m, "Aaa.esp", 0, &[b"Caf\xE9.esm"]);
        let order = compute_load_order(&[row("M", &m, 1, true)], &game);

        // The accented master was decoded properly (else it would be missing).
        assert!(order.issues.is_empty(), "{:?}", order.issues);
        assert_eq!(names(&order), ["Skyrim.esm", "Café.esm", "Aaa.esp"]);

        let bytes = render_plugins_txt(&order);
        assert!(
            bytes.windows(5).any(|w| w == b"*Caf\xE9"),
            "one 0xE9 byte, not UTF-8"
        );
        assert!(
            !bytes.windows(2).any(|w| w == [0xC3, 0xA9]),
            "no UTF-8 sequence must leak into plugins.txt"
        );
    }

    #[test]
    fn plugin_of_a_disabled_mod_is_absent() {
        let tmp = tempfile::tempdir().unwrap();
        let game = fake_game(&tmp.path().join("game"), &["Skyrim.esm"]);
        let on = tmp.path().join("on");
        let off = tmp.path().join("off");
        put(&on, "On.esp", 0, &[]);
        put(&off, "Off.esp", 0, &[]);
        let order = compute_load_order(
            &[row("On", &on, 1, true), row("Off", &off, 2, false)],
            &game,
        );
        assert_eq!(names(&order), ["Skyrim.esm", "On.esp"]);
        let text = String::from_utf8(render_plugins_txt(&order)).unwrap();
        assert!(!text.contains("Off.esp"));
    }

    #[test]
    fn winner_of_a_same_name_conflict_provides_the_file() {
        let tmp = tempfile::tempdir().unwrap();
        let game = fake_game(&tmp.path().join("game"), &["Skyrim.esm"]);
        let a = tmp.path().join("a");
        let b = tmp.path().join("b");
        put(&a, "Shared.esp", 0, &[]);
        // The winner needs a master the loser does not: proves its header is
        // the one read. Different case on purpose.
        put(&b, "SHARED.ESP", 0, &[b"Extra.esm"]);
        put(&b, "Extra.esm", MASTER, &[]);
        let order = compute_load_order(&[row("A", &a, 1, true), row("B", &b, 2, true)], &game);

        let shared = entry(&order, "SHARED.ESP");
        assert_eq!(shared.contributors, 2);
        assert_eq!(
            shared.source,
            PluginSource::Mod {
                mod_id: 2,
                mod_name: "B".into()
            }
        );
        assert_eq!(names(&order), ["Skyrim.esm", "Extra.esm", "SHARED.ESP"]);
    }

    #[test]
    fn only_top_level_plugins_count() {
        let tmp = tempfile::tempdir().unwrap();
        let game = fake_game(&tmp.path().join("game"), &["Skyrim.esm"]);
        let m = tmp.path().join("m");
        put(&m.join("SKSE").join("Plugins"), "Nested.esp", 0, &[]);
        put(&m, "Top.esp", 0, &[]);
        let order = compute_load_order(&[row("M", &m, 1, true)], &game);
        assert_eq!(names(&order), ["Skyrim.esm", "Top.esp"]);
    }

    #[test]
    fn unreadable_plugin_is_flagged_inactive_without_panicking() {
        let tmp = tempfile::tempdir().unwrap();
        let game = fake_game(&tmp.path().join("game"), &["Skyrim.esm"]);
        let m = tmp.path().join("m");
        fs::create_dir_all(&m).unwrap();
        fs::write(m.join("Garbage.esp"), b"not a plugin at all").unwrap();
        put(&m, "Good.esp", 0, &[]);
        let order = compute_load_order(&[row("M", &m, 1, true)], &game);

        let bad = entry(&order, "Garbage.esp");
        assert!(!bad.readable && !bad.active);
        assert!(entry(&order, "Good.esp").active);
        assert!(order
            .issues
            .iter()
            .any(|i| i.kind == IssueKind::Unreadable && i.plugin == "Garbage.esp"));
    }

    #[test]
    fn master_cycle_is_reported_and_does_not_loop() {
        let tmp = tempfile::tempdir().unwrap();
        let game = fake_game(&tmp.path().join("game"), &["Skyrim.esm"]);
        let m = tmp.path().join("m");
        put(&m, "A.esp", 0, &[b"B.esp"]);
        put(&m, "B.esp", 0, &[b"A.esp"]);
        put(&m, "C.esp", 0, &[]);
        let order = compute_load_order(&[row("M", &m, 1, true)], &game);
        assert_eq!(order.entries.len(), 4);
        assert_eq!(names(&order)[1], "C.esp");
        let cyclic: Vec<&str> = order
            .issues
            .iter()
            .filter(|i| i.kind == IssueKind::MasterCycle)
            .map(|i| i.plugin.as_str())
            .collect();
        assert_eq!(cyclic, ["A.esp", "B.esp"]);
    }

    #[test]
    fn engine_limits_drop_the_overflow_but_light_plugins_do_not_count_as_full() {
        let tmp = tempfile::tempdir().unwrap();
        let game = fake_game(&tmp.path().join("game"), &["Skyrim.esm"]);
        let m = tmp.path().join("m");
        // Skyrim.esm already takes one of the 255 slots.
        for i in 0..255 {
            put(&m, &format!("P{i:03}.esp"), 0, &[]);
        }
        for i in 0..10 {
            put(&m, &format!("L{i}.esl"), 0, &[]);
        }
        let order = compute_load_order(&[row("M", &m, 1, true)], &game);

        let dropped: Vec<&LoadOrderIssue> = order
            .issues
            .iter()
            .filter(|i| i.kind == IssueKind::FullLimit)
            .collect();
        assert_eq!(dropped.len(), 1);
        assert_eq!(dropped[0].plugin, "P254.esp");
        assert!(!entry(&order, "P254.esp").active);
        assert!(order.entries.iter().filter(|e| e.light).all(|e| e.active));
        assert_eq!(order.active_counts(), (MAX_FULL_PLUGINS, 10));
    }

    #[test]
    fn without_a_game_folder_official_masters_are_assumed_present() {
        let tmp = tempfile::tempdir().unwrap();
        let m = tmp.path().join("m");
        put(&m, "Needs.esp", 0, &[b"Skyrim.esm", b"Dawnguard.esm"]);
        let order = compute_load_order(&[row("M", &m, 1, true)], &GameContext::default());
        assert_eq!(names(&order), ["Needs.esp"]);
        assert!(order.issues.is_empty(), "{:?}", order.issues);
    }

    #[test]
    fn write_instance_plugins_txt_writes_and_replaces_the_file() {
        let tmp = tempfile::tempdir().unwrap();
        let game = fake_game(&tmp.path().join("game"), &["Skyrim.esm"]);
        let m = tmp.path().join("m");
        put(&m, "One.esp", 0, &[]);
        let dest = tmp
            .path()
            .join("data-root")
            .join("loadouts")
            .join("7")
            .join("plugins.txt");

        write_instance_plugins_txt(&[row("M", &m, 1, true)], &game, &dest).unwrap();
        assert_eq!(
            fs::read(&dest).unwrap(),
            b"# This file is generated by ModLab at each launch. Do not edit.\r\n*One.esp\r\n"
        );

        // Regenerated at every launch: a later call replaces, never appends.
        put(&m, "Two.esp", 0, &[]);
        write_instance_plugins_txt(&[row("M", &m, 1, true)], &game, &dest).unwrap();
        let text = String::from_utf8(fs::read(&dest).unwrap()).unwrap();
        assert_eq!(text.matches("*One.esp").count(), 1);
        assert!(text.contains("*Two.esp"));
        assert!(!dest.with_extension("txt.tmp").exists());
    }

    #[test]
    fn game_plugins_txt_path_is_under_the_given_local_app_data() {
        let p = game_plugins_txt_path(Path::new(r"C:\fake\Local"));
        assert!(p.starts_with(r"C:\fake\Local"));
        assert!(p.ends_with(Path::new("Skyrim Special Edition").join("Plugins.txt")));
    }
}
