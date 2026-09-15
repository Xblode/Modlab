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

    /// Try to find a real install of this game on the current machine.
    fn detect(&self) -> Option<GameInstall>;
}
