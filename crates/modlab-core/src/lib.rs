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

use rusqlite::Connection;
use std::path::Path;

pub mod archive;
pub mod conflicts;
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
    Archive(String),
}

pub type Result<T> = std::result::Result<T, CoreError>;

/// `(mod_id, name, source_path, enabled, position)` — see
/// `Core::select_loadout_entries`.
type LoadoutEntryTuple = (i64, String, String, bool, i64);

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
                source_path TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS loadout_entry (
                loadout_id INTEGER NOT NULL REFERENCES loadout(id),
                mod_id     INTEGER NOT NULL REFERENCES mod_(id),
                enabled    INTEGER NOT NULL,
                position   INTEGER NOT NULL,
                PRIMARY KEY (loadout_id, mod_id)
            );",
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
    /// guessed at here.
    pub(crate) fn insert_mod(&self, name: &str, source_path: &str) -> Result<i64> {
        self.db.execute(
            "INSERT INTO mod_ (name, source_path) VALUES (?1, ?2)",
            (name, source_path),
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
    /// (i.e. the order they were imported/added in). Raw tuples
    /// (`mod_id, name, source_path, enabled, position`) — `loadout::entries`
    /// wraps this in a named type for callers outside the Core.
    pub(crate) fn select_loadout_entries(&self, loadout_id: i64) -> Result<Vec<LoadoutEntryTuple>> {
        let mut stmt = self.db.prepare(
            "SELECT mod_.id, mod_.name, mod_.source_path, loadout_entry.enabled, loadout_entry.position \
             FROM loadout_entry \
             JOIN mod_ ON mod_.id = loadout_entry.mod_id \
             WHERE loadout_entry.loadout_id = ?1 \
             ORDER BY loadout_entry.position ASC",
        )?;
        let rows = stmt
            .query_map([loadout_id], |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
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
}
