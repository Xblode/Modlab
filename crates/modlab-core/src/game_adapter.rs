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
}
