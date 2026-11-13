//! ModLab engine core.
//!
//! This crate is the single owner of ModLab's state — see
//! `docs/01-architecture-decidee.md` §3 ("one engine, many clients") before
//! adding anything here. It must never know about a specific game (that's a
//! `GameAdapter`, not implemented yet) or a specific mod platform (that's a
//! `Provider`, not implemented yet).
//!
//! This is a walking-skeleton seed, not a finished domain model: it proves
//! the SQLite + event-log wiring works end to end. Revisions, transactions,
//! the CAS store and the game/provider traits are deliberately not here yet.

use rusqlite::{Connection, OptionalExtension};
use std::path::Path;

pub mod archive;
pub mod conflicts;
pub mod download;
pub mod game_adapter;
pub mod loadout;
pub mod store;
pub mod transaction;

#[derive(Debug, thiserror::Error)]
pub enum CoreError {
    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("filesystem error: {0}")]
    Io(#[from] std::io::Error),
    #[error("failed to serialize event payload: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("{0}")]
    Message(String),
    #[error("{0}")]
    Archive(String),
}

pub type Result<T> = std::result::Result<T, CoreError>;

/// One `loadout_entry` joined with its mod, exactly as storage reads it —
/// `loadout::entries` is what wraps this for callers outside the Core. A
/// struct rather than a tuple because it carries nine fields now: the
/// positional form made every call site an unreadable row of underscores.
pub(crate) struct LoadoutEntryRecord {
    pub mod_id: i64,
    pub name: String,
    pub source_path: String,
    pub enabled: bool,
    pub position: i64,
    pub version: Option<String>,
    /// `"archived"` (captured, nothing extracted yet) or `"installed"`.
    pub status: String,
    pub category: Option<String>,
    /// Where the source archive sits, for anything that came from a capture.
    pub archive_path: Option<String>,
}

/// The engine's single entry point. One `Core` owns one SQLite connection;
/// nothing else in the process should open the database file directly.
pub struct Core {
    db: Connection,
}

impl Core {
    /// Open (or create) the engine database at `path` and apply migrations.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let db = Connection::open(path)?;
        let core = Self { db };
        core.migrate()?;
        Ok(core)
    }

    /// Open an in-memory database — used by tests and by fixtures ("fake
    /// Skyrim" runs), never by real usage.
    pub fn open_in_memory() -> Result<Self> {
        let db = Connection::open_in_memory()?;
        let core = Self { db };
        core.migrate()?;
        Ok(core)
    }

    fn migrate(&self) -> Result<()> {
        // Event log described in docs/01-architecture-decidee.md §5, plus a
        // first slice of the Loadout/Mod/LoadoutEntry domain (§5's data
        // model, simplified: no Archive/Blob/ModFile/LoadoutRevision yet —
        // those need the CAS-backed install pipeline this doesn't have).
        // `mod` isn't a valid bare table name (reserved in some SQL
        // dialects, and shadows Rust's own keyword when read back) — kept
        // as `mod_` throughout.
        self.db.execute_batch(
            "CREATE TABLE IF NOT EXISTS event (
                id          INTEGER PRIMARY KEY AUTOINCREMENT,
                occurred_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
                kind        TEXT NOT NULL,
                payload     TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS loadout (
                id      INTEGER PRIMARY KEY AUTOINCREMENT,
                game_id TEXT NOT NULL,
                name    TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS mod_ (
                id          INTEGER PRIMARY KEY AUTOINCREMENT,
                name        TEXT NOT NULL,
                source_path TEXT NOT NULL,
                version     TEXT
            );
            CREATE TABLE IF NOT EXISTS loadout_entry (
                loadout_id INTEGER NOT NULL REFERENCES loadout(id),
                mod_id     INTEGER NOT NULL REFERENCES mod_(id),
                enabled    INTEGER NOT NULL,
                position   INTEGER NOT NULL,
                PRIMARY KEY (loadout_id, mod_id)
            );
            CREATE TABLE IF NOT EXISTS game_path_override (
                game_id TEXT NOT NULL,
                slot_id TEXT NOT NULL,
                path    TEXT NOT NULL,
                PRIMARY KEY (game_id, slot_id)
            );
            CREATE TABLE IF NOT EXISTS download (
                id          INTEGER PRIMARY KEY AUTOINCREMENT,
                url         TEXT NOT NULL,
                file_name   TEXT NOT NULL,
                path        TEXT NOT NULL,
                size_bytes  INTEGER,
                captured_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
                loadout_id  INTEGER REFERENCES loadout(id),
                mod_id      INTEGER REFERENCES mod_(id)
            );",
        )?;
        self.add_column_if_missing("mod_", "version", "TEXT")?;
        self.add_column_if_missing("loadout", "channel", "TEXT NOT NULL DEFAULT 'stable'")?;
        self.add_column_if_missing("loadout", "deleted_at", "TEXT")?;
        // Everything that predates the embedded browser got into the engine
        // by pointing at a folder that already existed on disk — which is
        // exactly what `installed` means, so the default backfills correctly.
        self.add_column_if_missing("mod_", "status", "TEXT NOT NULL DEFAULT 'installed'")?;
        self.add_column_if_missing("mod_", "archive_path", "TEXT")?;
        self.add_column_if_missing("mod_", "category", "TEXT")?;
        self.add_column_if_missing("mod_", "deleted_at", "TEXT")?;
        Ok(())
    }

    /// Additive schema evolution for a table that already exists in a
    /// database created before the column did. SQLite has no
    /// `ALTER TABLE ... ADD COLUMN IF NOT EXISTS`, so this asks
    /// `pragma_table_info` first — which makes it safe to run on every open,
    /// against a brand-new database (column already in `CREATE TABLE`) or an
    /// upgraded one alike. Additive only, and deliberately so: dropping or
    /// retyping a column would rewrite a table holding a real modlist, and
    /// that needs its own migration with a snapshot behind it, not a
    /// silent one-liner at startup.
    fn add_column_if_missing(&self, table: &str, column: &str, decl_type: &str) -> Result<()> {
        let exists: bool = self.db.query_row(
            "SELECT COUNT(*) > 0 FROM pragma_table_info(?1) WHERE name = ?2",
            (table, column),
            |row| row.get(0),
        )?;
        if !exists {
            // `table`/`column`/`decl_type` are compile-time literals from
            // this module, never user input — an identifier cannot be bound
            // as a parameter in SQLite, hence the format!.
            self.db.execute(
                &format!("ALTER TABLE {table} ADD COLUMN {column} {decl_type}"),
                [],
            )?;
        }
        Ok(())
    }

    /// A human-set override for one of a Game Adapter's `PathSlot`s — takes
    /// precedence over whatever that adapter's own detection finds. `None`
    /// unless a human set one from Réglages → Jeux.
    pub fn game_path_override(&self, game_id: &str, slot_id: &str) -> Result<Option<String>> {
        self.db
            .query_row(
                "SELECT path FROM game_path_override WHERE game_id = ?1 AND slot_id = ?2",
                (game_id, slot_id),
                |row| row.get(0),
            )
            .optional()
            .map_err(CoreError::from)
    }

    /// Every override recorded for one game, as `(slot_id, path)` pairs —
    /// what a client merges with `GameAdapter::path_slots`' live detection.
    pub fn game_path_overrides(&self, game_id: &str) -> Result<Vec<(String, String)>> {
        let mut stmt = self
            .db
            .prepare("SELECT slot_id, path FROM game_path_override WHERE game_id = ?1")?;
        let rows = stmt
            .query_map((game_id,), |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    /// Set (or replace) a human override for one path slot.
    pub fn set_game_path_override(&self, game_id: &str, slot_id: &str, path: &str) -> Result<()> {
        self.db.execute(
            "INSERT INTO game_path_override (game_id, slot_id, path) VALUES (?1, ?2, ?3)
             ON CONFLICT(game_id, slot_id) DO UPDATE SET path = excluded.path",
            (game_id, slot_id, path),
        )?;
        Ok(())
    }

    /// Drop a human override, falling back to the adapter's own detection again.
    pub fn clear_game_path_override(&self, game_id: &str, slot_id: &str) -> Result<()> {
        self.db.execute(
            "DELETE FROM game_path_override WHERE game_id = ?1 AND slot_id = ?2",
            (game_id, slot_id),
        )?;
        Ok(())
    }

    /// Append one event to the journal. `payload` is expected to be a JSON
    /// string once real callers exist; kept as a plain string for now so
    /// this crate doesn't pull in a JSON dependency before it needs one.
    pub fn log_event(&self, kind: &str, payload: &str) -> Result<i64> {
        self.db.execute(
            "INSERT INTO event (kind, payload) VALUES (?1, ?2)",
            (kind, payload),
        )?;
        Ok(self.db.last_insert_rowid())
    }

    /// Number of events recorded so far — mostly useful for tests until a
    /// real query API exists.
    pub fn event_count(&self) -> Result<i64> {
        self.db
            .query_row("SELECT COUNT(*) FROM event", [], |row| row.get(0))
            .map_err(CoreError::from)
    }

    /// Insert a new, empty loadout row. Returns its id.
    pub(crate) fn insert_loadout(&self, game_id: &str, name: &str) -> Result<i64> {
        self.db.execute(
            "INSERT INTO loadout (game_id, name) VALUES (?1, ?2)",
            (game_id, name),
        )?;
        Ok(self.db.last_insert_rowid())
    }

    /// Insert a new mod row. Doesn't dedup against existing mods with the
    /// same name/source — each import creates fresh rows for now; matching
    /// re-imported mods to existing ones is real work for later, not
    /// guessed at here. `version` is whatever the source declared, `None`
    /// when it declared nothing — never a placeholder string.
    pub(crate) fn insert_mod(
        &self,
        name: &str,
        source_path: &str,
        version: Option<&str>,
    ) -> Result<i64> {
        self.db.execute(
            "INSERT INTO mod_ (name, source_path, version) VALUES (?1, ?2, ?3)",
            (name, source_path, version),
        )?;
        Ok(self.db.last_insert_rowid())
    }

    /// Attach a mod to a loadout at a given position, enabled or not.
    pub(crate) fn insert_loadout_entry(
        &self,
        loadout_id: i64,
        mod_id: i64,
        enabled: bool,
        position: i64,
    ) -> Result<()> {
        self.db.execute(
            "INSERT INTO loadout_entry (loadout_id, mod_id, enabled, position) \
             VALUES (?1, ?2, ?3, ?4)",
            (loadout_id, mod_id, enabled, position),
        )?;
        Ok(())
    }

    /// All loadouts, oldest first, excluding soft-deleted ones. Raw tuples
    /// `(id, game_id, name, channel)`.
    pub(crate) fn select_loadouts(&self) -> Result<Vec<(i64, String, String, String)>> {
        let mut stmt = self.db.prepare(
            "SELECT id, game_id, name, channel FROM loadout \
             WHERE deleted_at IS NULL ORDER BY id ASC",
        )?;
        let rows = stmt
            .query_map([], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    /// The mirror of `select_loadouts`: only soft-deleted loadouts, most
    /// recently deleted first — what a trash/restore screen lists.
    pub(crate) fn select_deleted_loadouts(&self) -> Result<Vec<(i64, String, String, String)>> {
        let mut stmt = self.db.prepare(
            "SELECT id, game_id, name, channel FROM loadout \
             WHERE deleted_at IS NOT NULL ORDER BY deleted_at DESC",
        )?;
        let rows = stmt
            .query_map([], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    /// Rename a loadout in place.
    pub(crate) fn set_loadout_name(&self, loadout_id: i64, name: &str) -> Result<()> {
        self.db.execute(
            "UPDATE loadout SET name = ?1 WHERE id = ?2",
            (name, loadout_id),
        )?;
        Ok(())
    }

    /// Set a loadout's channel. Storage only — `loadout::set_channel`
    /// validates the value before it gets here.
    pub(crate) fn set_loadout_channel(&self, loadout_id: i64, channel: &str) -> Result<()> {
        self.db.execute(
            "UPDATE loadout SET channel = ?1 WHERE id = ?2",
            (channel, loadout_id),
        )?;
        Ok(())
    }

    /// Soft-delete a loadout: hidden from `select_loadouts` (and so from
    /// every client-facing list) but its row and every `loadout_entry`
    /// referencing it stay in the database untouched — `restore_loadout`
    /// un-hides it. A loadout is Core-owned metadata, never a path into
    /// F:\ModLab, so this never touches a game file.
    pub(crate) fn soft_delete_loadout(&self, loadout_id: i64) -> Result<()> {
        self.db.execute(
            "UPDATE loadout SET deleted_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ?1",
            [loadout_id],
        )?;
        Ok(())
    }

    /// Insert a mod that exists only as a captured archive so far: nothing
    /// is extracted, so `source_path` deliberately points at the archive
    /// itself until `set_mod_installed` replaces it with the real folder.
    pub(crate) fn insert_archived_mod(
        &self,
        name: &str,
        archive_path: &str,
        version: Option<&str>,
        category: Option<&str>,
    ) -> Result<i64> {
        self.db.execute(
            "INSERT INTO mod_ (name, source_path, version, status, archive_path, category) \
             VALUES (?1, ?2, ?3, 'archived', ?2, ?4)",
            (name, archive_path, version, category),
        )?;
        Ok(self.db.last_insert_rowid())
    }

    /// Promote an archived mod once its archive has actually been extracted.
    pub(crate) fn set_mod_installed(&self, mod_id: i64, source_path: &str) -> Result<()> {
        self.db.execute(
            "UPDATE mod_ SET status = 'installed', source_path = ?1 WHERE id = ?2",
            (source_path, mod_id),
        )?;
        Ok(())
    }

    pub(crate) fn set_mod_category(&self, mod_id: i64, category: Option<&str>) -> Result<()> {
        self.db.execute(
            "UPDATE mod_ SET category = ?1 WHERE id = ?2",
            (category, mod_id),
        )?;
        Ok(())
    }

    /// Rename a mod in place.
    pub(crate) fn set_mod_name(&self, mod_id: i64, name: &str) -> Result<()> {
        self.db
            .execute("UPDATE mod_ SET name = ?1 WHERE id = ?2", (name, mod_id))?;
        Ok(())
    }

    /// Soft-delete a mod: hidden from `select_loadout_entries` (and so from
    /// every client-facing table) but its row, and anything still on real
    /// disk under `source_path`/`archive_path`, stay exactly as they were.
    /// Same "internal trash, never real deletion" rule as
    /// `soft_delete_loadout`, applied one level down.
    pub(crate) fn soft_delete_mod(&self, mod_id: i64) -> Result<()> {
        self.db.execute(
            "UPDATE mod_ SET deleted_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ?1",
            [mod_id],
        )?;
        Ok(())
    }

    /// Revert an installed mod back to `archived`, pointing `source_path` at
    /// the archive again — the mirror of `set_mod_installed`. Only touches
    /// rows that actually have an `archive_path` to fall back to; the caller
    /// (`download::uninstall`) is the one that checks that before calling
    /// this, but the `WHERE` guard keeps this safe to call on its own too.
    pub(crate) fn set_mod_archived(&self, mod_id: i64) -> Result<()> {
        self.db.execute(
            "UPDATE mod_ SET status = 'archived', source_path = archive_path \
             WHERE id = ?1 AND archive_path IS NOT NULL",
            [mod_id],
        )?;
        Ok(())
    }

    /// Every category already in use, alphabetically — the vocabulary the
    /// tag input autocompletes from. Deliberately not per-game: a word worth
    /// typing once is worth reusing everywhere.
    pub(crate) fn select_mod_categories(&self) -> Result<Vec<String>> {
        let mut stmt = self.db.prepare(
            "SELECT DISTINCT category FROM mod_ \
             WHERE category IS NOT NULL AND category <> '' ORDER BY category ASC",
        )?;
        let rows = stmt
            .query_map([], |row| row.get(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    /// One mod's archive and status, for the install step.
    pub(crate) fn select_mod_archive(
        &self,
        mod_id: i64,
    ) -> Result<Option<(String, String, Option<String>)>> {
        self.db
            .query_row(
                "SELECT name, status, archive_path FROM mod_ WHERE id = ?1",
                [mod_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()
            .map_err(CoreError::from)
    }

    /// Move a mod's `loadout_entry` from one loadout to another, appended at
    /// the end of the destination's order. Both statements land together or
    /// not at all — same reasoning as `replace_loadout_entries`: a mod that
    /// briefly belongs to neither loadout, because the process died between
    /// the delete and the insert, is a worse failure than refusing the move.
    pub(crate) fn move_loadout_entry(
        &self,
        mod_id: i64,
        from_loadout_id: i64,
        to_loadout_id: i64,
    ) -> Result<()> {
        let position = self.loadout_entry_count(to_loadout_id)?;
        self.db.execute("BEGIN", [])?;
        let result = (|| -> Result<()> {
            self.db.execute(
                "DELETE FROM loadout_entry WHERE loadout_id = ?1 AND mod_id = ?2",
                (from_loadout_id, mod_id),
            )?;
            self.db.execute(
                "INSERT INTO loadout_entry (loadout_id, mod_id, enabled, position) \
                 VALUES (?1, ?2, 0, ?3)",
                (to_loadout_id, mod_id, position),
            )?;
            Ok(())
        })();
        match result {
            Ok(()) => {
                self.db.execute("COMMIT", [])?;
                Ok(())
            }
            Err(e) => {
                let _ = self.db.execute("ROLLBACK", []);
                Err(e)
            }
        }
    }

    /// Record a file the embedded browser captured. Returns its id.
    pub(crate) fn insert_download(
        &self,
        url: &str,
        file_name: &str,
        path: &str,
        size_bytes: Option<i64>,
    ) -> Result<i64> {
        self.db.execute(
            "INSERT INTO download (url, file_name, path, size_bytes) VALUES (?1, ?2, ?3, ?4)",
            (url, file_name, path, size_bytes),
        )?;
        Ok(self.db.last_insert_rowid())
    }

    /// An existing capture with this exact `file_name` and `size_bytes` —
    /// the dedup key `download::capture` checks before filing a new row, so
    /// installing the same mod+version on a second instance reuses the
    /// archive already on disk instead of piling up another copy of it.
    /// Only matches when both are known: a `None` size never matches
    /// another `None`, since two same-named-but-unsized files are not a
    /// strong enough signal to call them the same download.
    pub(crate) fn find_download_by_name_and_size(
        &self,
        file_name: &str,
        size_bytes: i64,
    ) -> Result<Option<(i64, String)>> {
        self.db
            .query_row(
                "SELECT id, path FROM download WHERE file_name = ?1 AND size_bytes = ?2 \
                 ORDER BY id ASC LIMIT 1",
                (file_name, size_bytes),
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .map_err(CoreError::from)
    }

    /// Every capture, newest first. Raw tuples
    /// `(id, url, file_name, path, size_bytes, captured_at, loadout_id, mod_id)`.
    #[allow(clippy::type_complexity)]
    pub(crate) fn select_downloads(
        &self,
    ) -> Result<
        Vec<(
            i64,
            String,
            String,
            String,
            Option<i64>,
            String,
            Option<i64>,
            Option<i64>,
        )>,
    > {
        let mut stmt = self.db.prepare(
            "SELECT id, url, file_name, path, size_bytes, captured_at, loadout_id, mod_id \
             FROM download ORDER BY id DESC",
        )?;
        let rows = stmt
            .query_map([], |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                    row.get(6)?,
                    row.get(7)?,
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    /// Tie a capture to the loadout it became a mod in.
    pub(crate) fn set_download_target(
        &self,
        download_id: i64,
        loadout_id: i64,
        mod_id: i64,
    ) -> Result<()> {
        self.db.execute(
            "UPDATE download SET loadout_id = ?1, mod_id = ?2 WHERE id = ?3",
            (loadout_id, mod_id, download_id),
        )?;
        Ok(())
    }

    /// One capture by id — `None` when it was never recorded.
    #[allow(clippy::type_complexity)]
    pub(crate) fn select_download(
        &self,
        download_id: i64,
    ) -> Result<Option<(String, String, String)>> {
        self.db
            .query_row(
                "SELECT url, file_name, path FROM download WHERE id = ?1",
                [download_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()
            .map_err(CoreError::from)
    }

    /// Undo `soft_delete_loadout`.
    pub(crate) fn restore_loadout(&self, loadout_id: i64) -> Result<()> {
        self.db.execute(
            "UPDATE loadout SET deleted_at = NULL WHERE id = ?1",
            [loadout_id],
        )?;
        Ok(())
    }

    /// Flip a loadout entry's enabled flag. Returns whether a row was updated.
    pub(crate) fn set_loadout_entry_enabled(
        &self,
        loadout_id: i64,
        mod_id: i64,
        enabled: bool,
    ) -> Result<bool> {
        let n = self.db.execute(
            "UPDATE loadout_entry SET enabled = ?1 WHERE loadout_id = ?2 AND mod_id = ?3",
            (enabled, loadout_id, mod_id),
        )?;
        Ok(n > 0)
    }

    /// Number of mods attached to a loadout — mostly useful for tests until
    /// a real query API exists.
    pub fn loadout_entry_count(&self, loadout_id: i64) -> Result<i64> {
        self.db
            .query_row(
                "SELECT COUNT(*) FROM loadout_entry WHERE loadout_id = ?1",
                [loadout_id],
                |row| row.get(0),
            )
            .map_err(CoreError::from)
    }

    /// Read a loadout's entries back, ordered by `position` ascending
    /// (i.e. the order they were imported/added in). `loadout::entries`
    /// wraps this for callers outside the Core.
    pub(crate) fn select_loadout_entries(
        &self,
        loadout_id: i64,
    ) -> Result<Vec<LoadoutEntryRecord>> {
        let mut stmt = self.db.prepare(
            "SELECT mod_.id, mod_.name, mod_.source_path, loadout_entry.enabled, \
                    loadout_entry.position, mod_.version, mod_.status, \
                    mod_.category, mod_.archive_path \
             FROM loadout_entry \
             JOIN mod_ ON mod_.id = loadout_entry.mod_id \
             WHERE loadout_entry.loadout_id = ?1 AND mod_.deleted_at IS NULL \
             ORDER BY loadout_entry.position ASC",
        )?;
        let rows = stmt
            .query_map([loadout_id], |row| {
                Ok(LoadoutEntryRecord {
                    mod_id: row.get(0)?,
                    name: row.get(1)?,
                    source_path: row.get(2)?,
                    enabled: row.get(3)?,
                    position: row.get(4)?,
                    version: row.get(5)?,
                    status: row.get(6)?,
                    category: row.get(7)?,
                    archive_path: row.get(8)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    /// A loadout's bare state — `(mod_id, enabled, position)` — the minimal
    /// shape a snapshot captures and a rollback restores.
    pub(crate) fn select_loadout_state(&self, loadout_id: i64) -> Result<Vec<(i64, bool, i64)>> {
        let mut stmt = self.db.prepare(
            "SELECT mod_id, enabled, position FROM loadout_entry \
             WHERE loadout_id = ?1 ORDER BY position ASC",
        )?;
        let rows = stmt
            .query_map([loadout_id], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    /// Replace a loadout's whole entry set — rollback's write path. One
    /// statement batch: the delete and the re-inserts land together or not
    /// at all.
    pub(crate) fn replace_loadout_entries(
        &self,
        loadout_id: i64,
        entries: &[(i64, bool, i64)],
    ) -> Result<()> {
        self.db.execute("BEGIN", [])?;
        let result = (|| -> Result<()> {
            self.db.execute(
                "DELETE FROM loadout_entry WHERE loadout_id = ?1",
                [loadout_id],
            )?;
            for (mod_id, enabled, position) in entries {
                self.db.execute(
                    "INSERT INTO loadout_entry (loadout_id, mod_id, enabled, position) \
                     VALUES (?1, ?2, ?3, ?4)",
                    (loadout_id, mod_id, *enabled, *position),
                )?;
            }
            Ok(())
        })();
        match result {
            Ok(()) => {
                self.db.execute("COMMIT", [])?;
                Ok(())
            }
            Err(_) => {
                let _ = self.db.execute("ROLLBACK", []);
                result?;
                Ok(())
            }
        }
    }

    /// Recent events of the given kinds, newest first —
    /// `(id, occurred_at, kind, payload)`.
    pub(crate) fn select_events(
        &self,
        kinds: &[&str],
        limit: usize,
    ) -> Result<Vec<(i64, String, String, String)>> {
        let placeholders = kinds.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let sql = format!(
            "SELECT id, occurred_at, kind, payload FROM event \
             WHERE kind IN ({placeholders}) ORDER BY id DESC LIMIT ?"
        );
        let mut stmt = self.db.prepare(&sql)?;
        let limit_i64 = limit as i64;
        let mut params: Vec<&dyn rusqlite::ToSql> =
            kinds.iter().map(|k| k as &dyn rusqlite::ToSql).collect();
        params.push(&limit_i64);
        let rows = stmt
            .query_map(params.as_slice(), |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opens_in_memory_and_migrates() {
        let core = Core::open_in_memory().expect("should open and migrate cleanly");
        assert_eq!(core.event_count().unwrap(), 0);
    }

    #[test]
    fn logs_and_counts_events() {
        let core = Core::open_in_memory().unwrap();
        core.log_event("engine.started", "{}").unwrap();
        core.log_event("game.detected", "{\"id\":\"skyrimse\"}")
            .unwrap();
        assert_eq!(core.event_count().unwrap(), 2);
    }

    #[test]
    fn persists_across_reopen_on_disk() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("modlab.db");

        {
            let core = Core::open(&db_path).unwrap();
            core.log_event("engine.started", "{}").unwrap();
        }

        let core = Core::open(&db_path).unwrap();
        assert_eq!(core.event_count().unwrap(), 1);
    }

    /// Benjamin's real `F:\ModLab` database predates `mod_.version`. Opening
    /// it must add the column *and keep every row already in there* — this
    /// builds a database with the exact pre-version schema, fills it, then
    /// opens it through `Core` and checks both.
    #[test]
    fn adds_the_version_column_to_a_database_that_predates_it() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("old.db");

        {
            let old = Connection::open(&db_path).unwrap();
            old.execute_batch(
                "CREATE TABLE mod_ (
                    id          INTEGER PRIMARY KEY AUTOINCREMENT,
                    name        TEXT NOT NULL,
                    source_path TEXT NOT NULL
                );
                INSERT INTO mod_ (name, source_path) VALUES ('SkyUI', 'F:/mods/SkyUI');",
            )
            .unwrap();
        }

        let core = Core::open(&db_path).expect("an older database must still open");

        let (name, version): (String, Option<String>) = core
            .db
            .query_row("SELECT name, version FROM mod_", [], |row| {
                Ok((row.get(0)?, row.get(1)?))
            })
            .expect("the pre-existing row survives the migration");
        assert_eq!(name, "SkyUI", "the existing row is untouched");
        assert_eq!(version, None, "the new column is null for old rows");

        // And the migration is idempotent — every open runs it again.
        drop(core);
        Core::open(&db_path).expect("reopening an already-migrated database is a no-op");
    }

    /// Same proof as the `mod_.version` test above, for `loadout.channel` —
    /// added later, needs its own pre-existing-row check, and `NOT NULL
    /// DEFAULT 'stable'` needs confirming: an old row must read back as
    /// `'stable'`, not null.
    #[test]
    fn adds_the_channel_column_to_a_database_that_predates_it() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("old.db");

        {
            let old = Connection::open(&db_path).unwrap();
            old.execute_batch(
                "CREATE TABLE loadout (
                    id      INTEGER PRIMARY KEY AUTOINCREMENT,
                    game_id TEXT NOT NULL,
                    name    TEXT NOT NULL
                );
                INSERT INTO loadout (game_id, name) VALUES ('skyrimse', 'Main 2026');",
            )
            .unwrap();
        }

        let core = Core::open(&db_path).expect("an older database must still open");

        let (name, channel): (String, String) = core
            .db
            .query_row("SELECT name, channel FROM loadout", [], |row| {
                Ok((row.get(0)?, row.get(1)?))
            })
            .expect("the pre-existing row survives the migration");
        assert_eq!(name, "Main 2026");
        assert_eq!(channel, "stable", "NOT NULL DEFAULT backfills old rows");
    }

    #[test]
    fn adds_the_deleted_at_column_to_a_database_that_predates_it() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("old.db");

        {
            let old = Connection::open(&db_path).unwrap();
            old.execute_batch(
                "CREATE TABLE loadout (
                    id      INTEGER PRIMARY KEY AUTOINCREMENT,
                    game_id TEXT NOT NULL,
                    name    TEXT NOT NULL
                );
                INSERT INTO loadout (game_id, name) VALUES ('skyrimse', 'Main 2026');",
            )
            .unwrap();
        }

        let core = Core::open(&db_path).expect("an older database must still open");

        let deleted_at: Option<String> = core
            .db
            .query_row("SELECT deleted_at FROM loadout", [], |row| row.get(0))
            .expect("the pre-existing row survives the migration");
        assert_eq!(
            deleted_at, None,
            "an existing row is never backfilled as deleted"
        );

        // Reopening the already-migrated database must not error either.
        Core::open(&db_path).expect("reopening an already-migrated database must be idempotent");
    }
}
