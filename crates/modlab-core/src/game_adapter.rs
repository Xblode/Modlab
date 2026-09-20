//! The `GameAdapter` contract — see docs/01-architecture-decidee.md §3.1.
//!
//! This module defines the shape every game integration follows. It
//! implements none of it: the Core must never know what "Skyrim" is, or
//! what a `.esp` is, or where Steam puts things. A per-game crate (e.g.
//! `modlab-game-skyrim`) provides that. Adding a second game must never
//! require touching this trait — if it does, the trait was wrong.

use std::path::PathBuf;

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

/// What a Game Adapter must be able to do.
///
/// Deliberately small right now — only what's actually implemented and
/// exercised (detection). The fuller contract from §3.1 (structure,
/// installation, diagnosis, launch, declared tools) gets added to this
/// trait as `modlab-game-skyrim` actually implements each piece, not
/// speculatively all at once before there's a second adapter to check the
/// abstraction against.
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
}
