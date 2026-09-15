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

pub mod detect;
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
}

pub type Result<T> = std::result::Result<T, CoreError>;

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
        // First slice of the event log described in
        // docs/01-architecture-decidee.md §5. Transactions, revisions and
        // the CAS store land on top of this in later work.
        self.db.execute_batch(
            "CREATE TABLE IF NOT EXISTS event (
                id          INTEGER PRIMARY KEY AUTOINCREMENT,
                occurred_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
                kind        TEXT NOT NULL,
                payload     TEXT NOT NULL
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
