//! ModLab engine core.
//!
//! This crate is the single owner of ModLab's state — see
//! `docs/01-architecture-decidee.md` §3 ("one engine, many clients") before
//! adding anything here. It must never know about a specific game (that's a
//! `GameAdapter`, implemented by the per-game crates) or a specific mod
//! platform (that's a `Provider`, not implemented yet).
//!
//! The engine owns the SQLite database and its event log, loadouts, the
//! content-addressed store, archive extraction, and the transaction cycle
//! (plan, approval, snapshot, apply, verify, commit or rollback).

use rusqlite::{Connection, OptionalExtension};
use std::path::Path;

pub mod archive;
pub mod conflicts;
pub mod data_root;
pub mod download;
pub mod filename_meta;
pub mod files;
pub mod game_adapter;
pub mod install;
pub mod instance_root;
pub mod loadout;
pub mod mod_config;
pub mod mod_measure;
pub mod mod_meta;
pub mod store;
pub mod transaction;

pub use data_root::{open_at, resolve_data_root};

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

/// Provenance and file facts stored on `mod_`. `deleted` is true after a
/// soft-delete: the row is hidden from loadout lists and the columns stay.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModFacts {
    pub source_host: Option<String>,
    pub page_url: Option<String>,
    pub file_url: Option<String>,
    pub version: Option<String>,
    pub category: Option<String>,
    pub game_versions: Vec<String>,
    pub dependencies: Vec<crate::game_adapter::DeclaredDependency>,
    pub loaders: Vec<String>,
    pub deleted: bool,
    /// Raw category text from the file (`fomod` group, MO2 `category`).
    /// Not the human's tag.
    pub category_raw: Option<String>,
    /// Nexus category number when the file stored one (MO2 `meta.ini`).
    pub category_nexus_id: Option<String>,
    pub nexus_mod_id: Option<String>,
    pub author: Option<String>,
    pub website: Option<String>,
}

/// One mod row's metadata columns, including the provenance JSON.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ModSheet {
    pub name: String,
    pub version: Option<String>,
    pub category: Option<String>,
    pub category_raw: Option<String>,
    pub category_nexus_id: Option<String>,
    pub source_host: Option<String>,
    pub page_url: Option<String>,
    pub file_url: Option<String>,
    pub nexus_mod_id: Option<String>,
    pub author: Option<String>,
    pub website: Option<String>,
    pub game_versions: Vec<String>,
    pub dependencies: Vec<crate::game_adapter::DeclaredDependency>,
    pub loaders: Vec<String>,
    pub field_sources: Option<String>,
    pub status: String,
    pub source_path: String,
    pub archive_path: Option<String>,
    pub deleted: bool,
}

fn nonempty(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|s| !s.is_empty())
}

pub(crate) fn encode_string_list(values: &[String]) -> Result<Option<String>> {
    let cleaned: Vec<&str> = values
        .iter()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();
    if cleaned.is_empty() {
        return Ok(None);
    }
    Ok(Some(serde_json::to_string(&cleaned)?))
}

pub(crate) fn decode_string_list(raw: Option<String>) -> Vec<String> {
    let Some(raw) = raw.filter(|s| !s.trim().is_empty()) else {
        return Vec::new();
    };
    serde_json::from_str::<Vec<String>>(&raw)
        .unwrap_or_default()
        .into_iter()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

/// Soft migration: objects with `id`/`kind`, or a flat string list (all required).
pub(crate) fn encode_dependencies(
    values: &[crate::game_adapter::DeclaredDependency],
) -> Result<Option<String>> {
    let cleaned: Vec<crate::game_adapter::DeclaredDependency> = values
        .iter()
        .filter_map(|dep| {
            let id = dep.id.trim();
            if id.is_empty() {
                None
            } else {
                Some(crate::game_adapter::DeclaredDependency {
                    id: id.to_string(),
                    kind: dep.kind,
                })
            }
        })
        .collect();
    if cleaned.is_empty() {
        return Ok(None);
    }
    Ok(Some(serde_json::to_string(&cleaned)?))
}

pub(crate) fn decode_dependencies(
    raw: Option<String>,
) -> Vec<crate::game_adapter::DeclaredDependency> {
    let Some(raw) = raw.filter(|s| !s.trim().is_empty()) else {
        return Vec::new();
    };
    if let Ok(deps) = serde_json::from_str::<Vec<crate::game_adapter::DeclaredDependency>>(&raw) {
        return deps
            .into_iter()
            .filter_map(|dep| {
                let id = dep.id.trim().to_string();
                if id.is_empty() {
                    None
                } else {
                    Some(crate::game_adapter::DeclaredDependency { id, kind: dep.kind })
                }
            })
            .collect();
    }
    decode_string_list(Some(raw))
        .into_iter()
        .map(crate::game_adapter::DeclaredDependency::required)
        .collect()
}

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
    /// `"downloading"`, `"archived"` (captured, nothing extracted yet) or
    /// `"installed"`.
    pub status: String,
    pub category: Option<String>,
    /// Where the source archive sits, for anything that came from a capture.
    pub archive_path: Option<String>,
    /// Opaque host token a client stored (`nexus`, `curseforge`, …). The
    /// Core does not know what the token means.
    pub source_host: Option<String>,
    /// Page to reopen. Not the file URL.
    pub page_url: Option<String>,
    /// Game versions the file declared, as plain strings. Empty when it
    /// declared none.
    pub game_versions: Vec<String>,
    /// Dependencies the file declared. Not a graph.
    pub dependencies: Vec<crate::game_adapter::DeclaredDependency>,
    /// Loader ids the file or its name stated. Empty when none were stated.
    pub loaders: Vec<String>,
    pub category_raw: Option<String>,
    pub category_nexus_id: Option<String>,
    pub nexus_mod_id: Option<String>,
    pub author: Option<String>,
    pub website: Option<String>,
    /// Byte length of the mod's archive or jar. Stays set after that file goes.
    pub size_bytes: Option<i64>,
    /// When the mod became installed, ISO 8601 UTC. Empty before that.
    pub installed_at: Option<String>,
    /// When the mod was downloaded or otherwise obtained, ISO 8601 UTC.
    /// Empty when nothing recorded it. Stays after the archive is deleted.
    pub downloaded_at: Option<String>,
}

/// One instance row. `root_path` is the folder assigned for a per-instance
/// policy. Empty for a shared install, and until the first assignment.
pub(crate) struct InstanceRow {
    pub name: String,
    pub root_path: Option<String>,
}

/// One branch (`loadout`) joined with the instance that owns it.
pub(crate) struct LoadoutRecord {
    pub id: i64,
    pub game_id: String,
    pub name: String,
    pub channel: String,
    pub instance_id: i64,
    pub instance_name: String,
    pub mc_loader: Option<String>,
    pub mc_game_version: Option<String>,
    pub mc_loader_version: Option<String>,
}

fn read_loadout_record(row: &rusqlite::Row<'_>) -> rusqlite::Result<LoadoutRecord> {
    Ok(LoadoutRecord {
        id: row.get(0)?,
        game_id: row.get(1)?,
        name: row.get(2)?,
        channel: row.get(3)?,
        instance_id: row.get(4)?,
        instance_name: row.get(5)?,
        mc_loader: row.get(6)?,
        mc_game_version: row.get(7)?,
        mc_loader_version: row.get(8)?,
    })
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
            CREATE TABLE IF NOT EXISTS instance (
                id         INTEGER PRIMARY KEY AUTOINCREMENT,
                game_id    TEXT NOT NULL,
                name       TEXT NOT NULL,
                deleted_at TEXT,
                root_path  TEXT
            );
            CREATE TABLE IF NOT EXISTS instance_path_override (
                instance_id INTEGER NOT NULL,
                slot_id     TEXT NOT NULL,
                path        TEXT NOT NULL,
                PRIMARY KEY (instance_id, slot_id)
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
        self.add_column_if_missing("loadout", "mc_loader", "TEXT")?;
        self.add_column_if_missing("loadout", "mc_game_version", "TEXT")?;
        self.add_column_if_missing("loadout", "mc_loader_version", "TEXT")?;
        // Everything that predates the embedded browser got into the engine
        // by pointing at a folder that already existed on disk — which is
        // exactly what `installed` means, so the default backfills correctly.
        self.add_column_if_missing("mod_", "status", "TEXT NOT NULL DEFAULT 'installed'")?;
        self.add_column_if_missing("mod_", "archive_path", "TEXT")?;
        self.add_column_if_missing("mod_", "category", "TEXT")?;
        self.add_column_if_missing("mod_", "deleted_at", "TEXT")?;
        // Provenance and file facts live on the mod row, not on the archive.
        // Deleting or trashing the file does not clear them. Clients write
        // the strings; this crate does not interpret hosts or game versions.
        self.add_column_if_missing("mod_", "source_host", "TEXT")?;
        self.add_column_if_missing("mod_", "page_url", "TEXT")?;
        self.add_column_if_missing("mod_", "file_url", "TEXT")?;
        self.add_column_if_missing("mod_", "game_versions", "TEXT")?;
        self.add_column_if_missing("mod_", "dependencies", "TEXT")?;
        self.add_column_if_missing("mod_", "loaders", "TEXT")?;
        self.add_column_if_missing("download", "source_host", "TEXT")?;
        self.add_column_if_missing("download", "page_url", "TEXT")?;
        // Transfer state machine: queued → progress → archived | failed.
        // Rows captured before this column existed are finished archives.
        self.add_column_if_missing("download", "phase", "TEXT NOT NULL DEFAULT 'archived'")?;
        // Per-field provenance (`{"name":"filename","version":"user",…}`).
        // Empty means the row predates the column; layers treat a raw file
        // label as replaceable and anything else as the human's.
        self.add_column_if_missing("mod_", "field_sources", "TEXT")?;
        self.add_column_if_missing("mod_", "nexus_mod_id", "TEXT")?;
        self.add_column_if_missing("mod_", "category_raw", "TEXT")?;
        self.add_column_if_missing("mod_", "category_nexus_id", "TEXT")?;
        self.add_column_if_missing("mod_", "author", "TEXT")?;
        self.add_column_if_missing("mod_", "website", "TEXT")?;
        // The archive or jar, in bytes. Kept after that file is deleted.
        self.add_column_if_missing("mod_", "size_bytes", "INTEGER")?;
        // Moment the mod became installed, ISO 8601 UTC. Enable/disable
        // does not rewrite it; a later install does.
        self.add_column_if_missing("mod_", "installed_at", "TEXT")?;
        // When the mod was obtained, ISO 8601 UTC. A download writes it at
        // the start (or from the capture, if the row appears later). An
        // import writes the source file's modification time, or the import
        // time when that file has none. Deleting the archive does not clear
        // it. Rows that predate the column stay empty until a branch open
        // fills them (`mod_measure::fill_branch`).
        self.add_column_if_missing("mod_", "downloaded_at", "TEXT")?;
        // Nullable on purpose: rows that predate the column are filled by
        // `backfill_instances` before any client reads them. New inserts
        // always pass an id (`insert_loadout`).
        self.add_column_if_missing("loadout", "instance_id", "INTEGER")?;
        // Assigned once. A later rename of the instance does not rewrite it.
        self.add_column_if_missing("instance", "root_path", "TEXT")?;
        self.db.execute_batch(
            "CREATE TABLE IF NOT EXISTS instance_path_override (
                instance_id INTEGER NOT NULL,
                slot_id     TEXT NOT NULL,
                path        TEXT NOT NULL,
                PRIMARY KEY (instance_id, slot_id)
            );",
        )?;
        self.backfill_instances()?;
        Ok(())
    }

    /// Each loadout that predates instances becomes its own instance.
    ///
    /// Until this column existed, every loadout of a game was a sibling of
    /// the others, and the UI showed that flat list both as "instances" and
    /// as "branches". Snapshot history is keyed by `loadout.id` and must not
    /// be merged, so two loadouts of the same game stay two instances.
    ///
    /// The existing row is kept as that instance's initial branch: same id,
    /// same name, same entries, same snapshots. The new `instance` row copies
    /// the loadout's game, name and `deleted_at` (a loadout already in the
    /// trash stays in the trash). Re-opening is a no-op once `instance_id`
    /// is set.
    ///
    /// A later "create instance" from the UI is different: the instance gets
    /// the name the human typed and the branch is named `principale`
    /// (`loadout::create_instance`). `loadout::create` (CLI, imports, tests)
    /// still makes one instance whose branch keeps the given name, so each
    /// call stays its own instance.
    fn backfill_instances(&self) -> Result<()> {
        let mut stmt = self.db.prepare(
            "SELECT id, game_id, name, deleted_at FROM loadout \
             WHERE instance_id IS NULL ORDER BY id",
        )?;
        let rows = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, Option<String>>(3)?,
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        drop(stmt);
        for (id, game_id, name, deleted_at) in rows {
            self.db.execute(
                "INSERT INTO instance (game_id, name, deleted_at) VALUES (?1, ?2, ?3)",
                (game_id, name, deleted_at),
            )?;
            let instance_id = self.db.last_insert_rowid();
            self.db.execute(
                "UPDATE loadout SET instance_id = ?1 WHERE id = ?2",
                (instance_id, id),
            )?;
        }
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

    /// One instance row. `root_path` stays empty until a per-instance policy
    /// assigns a folder.
    pub(crate) fn instance_by_id(&self, instance_id: i64) -> Result<Option<InstanceRow>> {
        self.db
            .query_row(
                "SELECT name, root_path FROM instance WHERE id = ?1",
                [instance_id],
                |row| {
                    Ok(InstanceRow {
                        name: row.get(0)?,
                        root_path: row.get(1)?,
                    })
                },
            )
            .optional()
            .map_err(CoreError::from)
    }

    /// Every stored per-instance folder, for collision checks. Empty strings
    /// are skipped.
    pub(crate) fn stored_instance_roots(&self) -> Result<Vec<String>> {
        let mut stmt = self
            .db
            .prepare("SELECT root_path FROM instance WHERE root_path IS NOT NULL")?;
        let rows = stmt
            .query_map([], |row| row.get::<_, String>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows.into_iter().filter(|path| !path.is_empty()).collect())
    }

    /// Remember the folder. Callers assign it once; this does not move files.
    pub(crate) fn set_instance_root_path(&self, instance_id: i64, path: &str) -> Result<()> {
        self.db.execute(
            "UPDATE instance SET root_path = ?1 WHERE id = ?2",
            (path, instance_id),
        )?;
        Ok(())
    }

    /// Human override for one slot of one instance. `None` when unset.
    pub fn instance_path_override(
        &self,
        instance_id: i64,
        slot_id: &str,
    ) -> Result<Option<String>> {
        self.db
            .query_row(
                "SELECT path FROM instance_path_override \
                 WHERE instance_id = ?1 AND slot_id = ?2",
                (instance_id, slot_id),
                |row| row.get(0),
            )
            .optional()
            .map_err(CoreError::from)
    }

    /// Every override for one instance, as `(slot_id, path)`.
    pub fn instance_path_overrides(&self, instance_id: i64) -> Result<Vec<(String, String)>> {
        let mut stmt = self
            .db
            .prepare("SELECT slot_id, path FROM instance_path_override WHERE instance_id = ?1")?;
        let rows = stmt
            .query_map([instance_id], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    /// Set (or replace) a human override for one instance slot.
    pub fn set_instance_path_override(
        &self,
        instance_id: i64,
        slot_id: &str,
        path: &str,
    ) -> Result<()> {
        self.db.execute(
            "INSERT INTO instance_path_override (instance_id, slot_id, path) \
             VALUES (?1, ?2, ?3) \
             ON CONFLICT(instance_id, slot_id) DO UPDATE SET path = excluded.path",
            (instance_id, slot_id, path),
        )?;
        Ok(())
    }

    /// Drop a human override for one instance slot.
    pub fn clear_instance_path_override(&self, instance_id: i64, slot_id: &str) -> Result<()> {
        self.db.execute(
            "DELETE FROM instance_path_override WHERE instance_id = ?1 AND slot_id = ?2",
            (instance_id, slot_id),
        )?;
        Ok(())
    }

    /// The live branch that holds `mod_id`, oldest first. `None` when the
    /// mod is not on a live branch.
    pub fn loadout_id_for_mod(&self, mod_id: i64) -> Result<Option<i64>> {
        self.db
            .query_row(
                "SELECT l.id FROM loadout_entry e \
                 JOIN loadout l ON l.id = e.loadout_id \
                 JOIN instance i ON i.id = l.instance_id \
                 WHERE e.mod_id = ?1 AND l.deleted_at IS NULL AND i.deleted_at IS NULL \
                 ORDER BY l.id ASC LIMIT 1",
                [mod_id],
                |row| row.get(0),
            )
            .optional()
            .map_err(CoreError::from)
    }

    /// Insert an instance. Branches are inserted separately and point here.
    pub(crate) fn insert_instance(&self, game_id: &str, name: &str) -> Result<i64> {
        self.db.execute(
            "INSERT INTO instance (game_id, name) VALUES (?1, ?2)",
            (game_id, name),
        )?;
        Ok(self.db.last_insert_rowid())
    }

    /// Insert a branch of `instance_id`. Returns its id. `game_id` is copied
    /// onto the row so existing queries that filter loadouts by game keep
    /// working; it must be the instance's game.
    pub(crate) fn insert_loadout(
        &self,
        game_id: &str,
        name: &str,
        instance_id: i64,
    ) -> Result<i64> {
        self.db.execute(
            "INSERT INTO loadout (game_id, name, instance_id) VALUES (?1, ?2, ?3)",
            (game_id, name, instance_id),
        )?;
        Ok(self.db.last_insert_rowid())
    }

    /// Copy membership, enabled flags and order onto a new branch.
    /// Mod rows stay shared (the files); the new branch gets its own
    /// `loadout_entry` rows and, because snapshots are keyed by loadout id,
    /// its own empty revision history.
    pub(crate) fn copy_loadout_entries(
        &self,
        from_loadout_id: i64,
        to_loadout_id: i64,
    ) -> Result<usize> {
        let n = self.db.execute(
            "INSERT INTO loadout_entry (loadout_id, mod_id, enabled, position) \
             SELECT ?1, mod_id, enabled, position FROM loadout_entry WHERE loadout_id = ?2",
            (to_loadout_id, from_loadout_id),
        )?;
        Ok(n)
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

    /// All live branches, oldest first. A branch whose instance is in the
    /// trash is hidden with it.
    pub(crate) fn select_loadouts(&self) -> Result<Vec<LoadoutRecord>> {
        let mut stmt = self.db.prepare(
            "SELECT l.id, l.game_id, l.name, l.channel, i.id, i.name, \
                    l.mc_loader, l.mc_game_version, l.mc_loader_version \
             FROM loadout l \
             JOIN instance i ON i.id = l.instance_id \
             WHERE l.deleted_at IS NULL AND i.deleted_at IS NULL \
             ORDER BY l.id ASC",
        )?;
        let rows = stmt
            .query_map([], read_loadout_record)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    /// One branch, including one that sits in the trash. `None` when the id
    /// is unknown or has no instance (should not happen after migration).
    pub(crate) fn select_loadout(&self, loadout_id: i64) -> Result<Option<LoadoutRecord>> {
        self.db
            .query_row(
                "SELECT l.id, l.game_id, l.name, l.channel, i.id, i.name, \
                        l.mc_loader, l.mc_game_version, l.mc_loader_version \
                 FROM loadout l \
                 JOIN instance i ON i.id = l.instance_id \
                 WHERE l.id = ?1",
                [loadout_id],
                read_loadout_record,
            )
            .optional()
            .map_err(CoreError::from)
    }

    /// Deleted instances, most recently deleted first. One row per instance
    /// (its oldest branch), named with the instance name — the trash lists
    /// instances, not every branch that went with them.
    pub(crate) fn select_deleted_loadouts(&self) -> Result<Vec<LoadoutRecord>> {
        let mut stmt = self.db.prepare(
            "SELECT l.id, l.game_id, i.name, l.channel, i.id, i.name, \
                    l.mc_loader, l.mc_game_version, l.mc_loader_version \
             FROM instance i \
             JOIN loadout l ON l.id = (
                 SELECT MIN(l2.id) FROM loadout l2 WHERE l2.instance_id = i.id
             ) \
             WHERE i.deleted_at IS NOT NULL \
             ORDER BY i.deleted_at DESC",
        )?;
        let rows = stmt
            .query_map([], read_loadout_record)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    /// Rename an instance. Branches keep their own names.
    pub(crate) fn set_instance_name(&self, instance_id: i64, name: &str) -> Result<()> {
        self.db.execute(
            "UPDATE instance SET name = ?1 WHERE id = ?2",
            (name, instance_id),
        )?;
        Ok(())
    }

    /// Rename a branch in place.
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

    pub(crate) fn set_loadout_mc_loader(&self, loadout_id: i64, value: Option<&str>) -> Result<()> {
        self.db.execute(
            "UPDATE loadout SET mc_loader = ?1 WHERE id = ?2",
            (value, loadout_id),
        )?;
        Ok(())
    }

    pub(crate) fn set_loadout_mc_game_version(
        &self,
        loadout_id: i64,
        value: Option<&str>,
    ) -> Result<()> {
        self.db.execute(
            "UPDATE loadout SET mc_game_version = ?1 WHERE id = ?2",
            (value, loadout_id),
        )?;
        Ok(())
    }

    pub(crate) fn set_loadout_mc_loader_version(
        &self,
        loadout_id: i64,
        value: Option<&str>,
    ) -> Result<()> {
        self.db.execute(
            "UPDATE loadout SET mc_loader_version = ?1 WHERE id = ?2",
            (value, loadout_id),
        )?;
        Ok(())
    }

    /// Soft-delete the instance that owns this branch, and every branch of
    /// that instance. Rows and entries stay; `restore_loadout` un-hides the
    /// whole instance. Nothing here is a path into F:\ModLab.
    pub(crate) fn soft_delete_loadout(&self, loadout_id: i64) -> Result<()> {
        let instance_id: Option<i64> = self
            .db
            .query_row(
                "SELECT instance_id FROM loadout WHERE id = ?1",
                [loadout_id],
                |row| row.get(0),
            )
            .optional()?;
        let Some(instance_id) = instance_id else {
            self.db.execute(
                "UPDATE loadout SET deleted_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ?1",
                [loadout_id],
            )?;
            return Ok(());
        };
        self.db.execute(
            "UPDATE instance SET deleted_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ?1",
            [instance_id],
        )?;
        self.db.execute(
            "UPDATE loadout SET deleted_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') \
             WHERE instance_id = ?1 AND deleted_at IS NULL",
            [instance_id],
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

    /// A mod row for a transfer that has started and not landed yet.
    /// `source_path` points at the destination the file will occupy.
    pub(crate) fn insert_downloading_mod(&self, name: &str, archive_path: &str) -> Result<i64> {
        self.db.execute(
            "INSERT INTO mod_ (name, source_path, version, status, archive_path) \
             VALUES (?1, ?2, NULL, 'downloading', ?2)",
            (name, archive_path),
        )?;
        Ok(self.db.last_insert_rowid())
    }

    /// Download row, mod row and loadout entry, together or not at all.
    /// Returns `(download_id, mod_id)`.
    pub(crate) fn insert_started_download(
        &self,
        url: &str,
        file_name: &str,
        path: &str,
        mod_name: &str,
        loadout_id: i64,
    ) -> Result<(i64, i64)> {
        let position = self.loadout_entry_count(loadout_id)?;
        self.db.execute("BEGIN", [])?;
        let result = (|| -> Result<(i64, i64)> {
            let mod_id = self.insert_downloading_mod(mod_name, path)?;
            self.insert_loadout_entry(loadout_id, mod_id, false, position)?;
            let download_id = self.insert_download(
                url,
                file_name,
                path,
                None,
                crate::download::PHASE_QUEUED,
            )?;
            self.set_download_target(download_id, loadout_id, mod_id)?;
            // The row is on screen as soon as the transfer starts, so the
            // date is this capture's — the same instant, not a later finish.
            self.copy_download_captured_at(download_id, mod_id)?;
            Ok((download_id, mod_id))
        })();
        match result {
            Ok(ids) => {
                self.db.execute("COMMIT", [])?;
                Ok(ids)
            }
            Err(e) => {
                let _ = self.db.execute("ROLLBACK", []);
                Err(e)
            }
        }
    }

    /// Point a still-downloading mod at a different archive (a deduped copy
    /// that was already on disk). No-op once the row has left `downloading`.
    pub(crate) fn retarget_downloading_archive(&self, mod_id: i64, path: &str) -> Result<()> {
        self.db.execute(
            "UPDATE mod_ SET source_path = ?1, archive_path = ?1 \
             WHERE id = ?2 AND status = 'downloading' AND deleted_at IS NULL",
            (path, mod_id),
        )?;
        Ok(())
    }

    /// `downloading` → `archived`. False when the row was not downloading
    /// anymore (already finished, or soft-deleted while the transfer ran).
    pub(crate) fn promote_downloading_mod(&self, mod_id: i64) -> Result<bool> {
        let n = self.db.execute(
            "UPDATE mod_ SET status = 'archived' \
             WHERE id = ?1 AND status = 'downloading' AND deleted_at IS NULL",
            [mod_id],
        )?;
        Ok(n > 0)
    }

    pub(crate) fn set_mod_version(&self, mod_id: i64, version: Option<&str>) -> Result<()> {
        self.db.execute(
            "UPDATE mod_ SET version = ?1 WHERE id = ?2",
            (version, mod_id),
        )?;
        Ok(())
    }

    /// The declared version, or `None` when the row is missing or the column
    /// is null. Callers that learned a version from a file name use this so
    /// they don't overwrite one already stored.
    pub(crate) fn select_mod_version(&self, mod_id: i64) -> Result<Option<String>> {
        let found = self
            .db
            .query_row("SELECT version FROM mod_ WHERE id = ?1", [mod_id], |row| {
                row.get::<_, Option<String>>(0)
            })
            .optional()?;
        Ok(found.flatten())
    }

    /// Promote an archived mod once its archive has actually been extracted.
    /// `installed_at` is the moment of this install, including a reinstall.
    pub(crate) fn set_mod_installed(&self, mod_id: i64, source_path: &str) -> Result<()> {
        self.db.execute(
            "UPDATE mod_ SET status = 'installed', source_path = ?1, \
             installed_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ?2",
            (source_path, mod_id),
        )?;
        Ok(())
    }

    /// Point an already-installed mod at a new on-disk path (e.g. after a
    /// rename). Status and timestamps stay unchanged.
    pub(crate) fn update_mod_source_path(&self, mod_id: i64, source_path: &str) -> Result<()> {
        self.db.execute(
            "UPDATE mod_ SET source_path = ?1 WHERE id = ?2 AND deleted_at IS NULL",
            (source_path, mod_id),
        )?;
        Ok(())
    }

    /// Record the page a client already had. A later call cannot blank a
    /// value, and a filename guess cannot replace it: the page layer
    /// outranks the file name. A value the human set stays.
    pub(crate) fn set_mod_source(
        &self,
        mod_id: i64,
        source_host: Option<&str>,
        page_url: Option<&str>,
        file_url: Option<&str>,
    ) -> Result<()> {
        let patch = crate::mod_meta::Patch {
            source_host: nonempty(source_host).map(str::to_string),
            page_url: nonempty(page_url).map(str::to_string),
            file_url: nonempty(file_url).map(str::to_string),
            ..Default::default()
        };
        crate::mod_meta::apply_layer(
            self,
            mod_id,
            crate::mod_meta::Layer::Page,
            &patch,
            &[],
            false,
        )
    }

    pub(crate) fn set_download_page(
        &self,
        download_id: i64,
        source_host: Option<&str>,
        page_url: Option<&str>,
    ) -> Result<()> {
        if let Some(host) = nonempty(source_host) {
            self.db.execute(
                "UPDATE download SET source_host = ?1 WHERE id = ?2 \
                 AND (source_host IS NULL OR TRIM(source_host) = '')",
                (host, download_id),
            )?;
        }
        if let Some(page) = nonempty(page_url) {
            self.db.execute(
                "UPDATE download SET page_url = ?1 WHERE id = ?2 \
                 AND (page_url IS NULL OR TRIM(page_url) = '')",
                (page, download_id),
            )?;
        }
        Ok(())
    }

    pub(crate) fn select_download_page(
        &self,
        download_id: i64,
    ) -> Result<Option<(Option<String>, Option<String>)>> {
        self.db
            .query_row(
                "SELECT source_host, page_url FROM download WHERE id = ?1",
                [download_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .map_err(CoreError::from)
    }

    /// The fiche, including a soft-deleted one. `None` when the id was never
    /// inserted. File bytes are not consulted.
    pub(crate) fn select_mod_facts(&self, mod_id: i64) -> Result<Option<ModFacts>> {
        Ok(self.select_mod_sheet(mod_id)?.map(|sheet| ModFacts {
            source_host: sheet.source_host,
            page_url: sheet.page_url,
            file_url: sheet.file_url,
            version: sheet.version,
            category: sheet.category,
            game_versions: sheet.game_versions,
            dependencies: sheet.dependencies,
            loaders: sheet.loaders,
            deleted: sheet.deleted,
            category_raw: sheet.category_raw,
            category_nexus_id: sheet.category_nexus_id,
            nexus_mod_id: sheet.nexus_mod_id,
            author: sheet.author,
            website: sheet.website,
        }))
    }

    pub(crate) fn select_mod_sheet(&self, mod_id: i64) -> Result<Option<ModSheet>> {
        self.db
            .query_row(
                "SELECT name, version, category, category_raw, category_nexus_id, \
                        source_host, page_url, file_url, nexus_mod_id, author, website, \
                        game_versions, dependencies, field_sources, status, source_path, \
                        archive_path, deleted_at IS NOT NULL, loaders \
                 FROM mod_ WHERE id = ?1",
                [mod_id],
                |row| {
                    Ok(ModSheet {
                        name: row.get(0)?,
                        version: row.get(1)?,
                        category: row.get(2)?,
                        category_raw: row.get(3)?,
                        category_nexus_id: row.get(4)?,
                        source_host: row.get(5)?,
                        page_url: row.get(6)?,
                        file_url: row.get(7)?,
                        nexus_mod_id: row.get(8)?,
                        author: row.get(9)?,
                        website: row.get(10)?,
                        game_versions: decode_string_list(row.get(11)?),
                        dependencies: decode_dependencies(row.get(12)?),
                        field_sources: row.get(13)?,
                        status: row.get(14)?,
                        source_path: row.get(15)?,
                        archive_path: row.get(16)?,
                        deleted: row.get(17)?,
                        loaders: decode_string_list(row.get(18)?),
                    })
                },
            )
            .optional()
            .map_err(CoreError::from)
    }

    /// Write the metadata columns back. Status, paths and deletion are left
    /// as they were — this is the provenance update, not a lifecycle change.
    pub(crate) fn save_mod_sheet(&self, mod_id: i64, sheet: &ModSheet) -> Result<()> {
        let game_versions = encode_string_list(&sheet.game_versions)?;
        let dependencies = encode_dependencies(&sheet.dependencies)?;
        let loaders = encode_string_list(&sheet.loaders)?;
        self.db.execute(
            "UPDATE mod_ SET name = ?1, version = ?2, category = ?3, category_raw = ?4, \
                    category_nexus_id = ?5, source_host = ?6, page_url = ?7, file_url = ?8, \
                    nexus_mod_id = ?9, author = ?10, website = ?11, game_versions = ?12, \
                    dependencies = ?13, field_sources = ?14, loaders = ?15 \
             WHERE id = ?16",
            (
                &sheet.name,
                &sheet.version,
                &sheet.category,
                &sheet.category_raw,
                &sheet.category_nexus_id,
                &sheet.source_host,
                &sheet.page_url,
                &sheet.file_url,
                &sheet.nexus_mod_id,
                &sheet.author,
                &sheet.website,
                &game_versions,
                &dependencies,
                &sheet.field_sources,
                &loaders,
                mod_id,
            ),
        )?;
        Ok(())
    }

    /// `field_sources` for the given mods. Missing ids are simply absent.
    pub(crate) fn select_field_sources(
        &self,
        mod_ids: &[i64],
    ) -> Result<Vec<(i64, Option<String>)>> {
        if mod_ids.is_empty() {
            return Ok(Vec::new());
        }
        let marks = vec!["?"; mod_ids.len()].join(",");
        let sql = format!("SELECT id, field_sources FROM mod_ WHERE id IN ({marks})");
        let mut stmt = self.db.prepare(&sql)?;
        let rows = stmt
            .query_map(rusqlite::params_from_iter(mod_ids.iter()), |row| {
                Ok((row.get(0)?, row.get(1)?))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    /// Drop recorded provenance. Tests use this to pretend a row was written
    /// before the column existed.
    pub(crate) fn set_field_sources(&self, mod_id: i64, raw: Option<&str>) -> Result<()> {
        self.db.execute(
            "UPDATE mod_ SET field_sources = ?1 WHERE id = ?2",
            (raw, mod_id),
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
    /// `source_path` as stored, so a failed install or uninstall can put the
    /// row back. `None` when the mod does not exist.
    pub(crate) fn select_mod_source_path(&self, mod_id: i64) -> Result<Option<String>> {
        self.db
            .query_row(
                "SELECT source_path FROM mod_ WHERE id = ?1",
                [mod_id],
                |row| row.get(0),
            )
            .optional()
            .map_err(CoreError::from)
    }

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
    /// `phase` is the transfer state (`queued` / `progress` / `archived` / `failed`).
    pub(crate) fn insert_download(
        &self,
        url: &str,
        file_name: &str,
        path: &str,
        size_bytes: Option<i64>,
        phase: &str,
    ) -> Result<i64> {
        self.db.execute(
            "INSERT INTO download (url, file_name, path, size_bytes, phase) \
             VALUES (?1, ?2, ?3, ?4, ?5)",
            (url, file_name, path, size_bytes, phase),
        )?;
        Ok(self.db.last_insert_rowid())
    }

    pub(crate) fn set_download_phase(&self, download_id: i64, phase: &str) -> Result<()> {
        self.db.execute(
            "UPDATE download SET phase = ?1 WHERE id = ?2",
            (phase, download_id),
        )?;
        Ok(())
    }

    pub(crate) fn select_download_phase(&self, download_id: i64) -> Result<Option<String>> {
        self.db
            .query_row(
                "SELECT phase FROM download WHERE id = ?1",
                [download_id],
                |row| row.get(0),
            )
            .optional()
            .map_err(CoreError::from)
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

    /// One capture including the loadout and mod it was filed into.
    /// `(url, file_name, path, loadout_id, mod_id)`.
    #[allow(clippy::type_complexity)]
    pub(crate) fn select_download_binding(
        &self,
        download_id: i64,
    ) -> Result<Option<(String, String, String, Option<i64>, Option<i64>)>> {
        self.db
            .query_row(
                "SELECT url, file_name, path, loadout_id, mod_id FROM download WHERE id = ?1",
                [download_id],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                    ))
                },
            )
            .optional()
            .map_err(CoreError::from)
    }

    /// Newest capture whose stored path is either spelling. Separators differ
    /// between the path we wrote and the path the browser reports back.
    pub(crate) fn find_download_id_by_path(
        &self,
        path: &str,
        alt_path: &str,
    ) -> Result<Option<i64>> {
        self.db
            .query_row(
                "SELECT id FROM download WHERE path = ?1 OR path = ?2 ORDER BY id DESC LIMIT 1",
                (path, alt_path),
                |row| row.get(0),
            )
            .optional()
            .map_err(CoreError::from)
    }

    pub(crate) fn set_download_size(
        &self,
        download_id: i64,
        size_bytes: Option<i64>,
    ) -> Result<()> {
        self.db.execute(
            "UPDATE download SET size_bytes = ?1 WHERE id = ?2",
            (size_bytes, download_id),
        )?;
        Ok(())
    }

    /// Where the browser actually wrote the file, when that path differs from
    /// the destination recorded at `begin`.
    pub(crate) fn set_download_path(&self, download_id: i64, path: &str) -> Result<()> {
        self.db.execute(
            "UPDATE download SET path = ?1 WHERE id = ?2",
            (path, download_id),
        )?;
        Ok(())
    }

    /// Newest ledger row whose `file_name` matches and whose mod is still
    /// `downloading`. Used when the finish event's path/URL no longer match
    /// what `begin` stored (CDN redirect, WebView2 path spelling).
    pub(crate) fn find_in_progress_download_by_file_name(
        &self,
        file_name: &str,
    ) -> Result<Option<i64>> {
        self.db
            .query_row(
                "SELECT d.id FROM download d \
                 INNER JOIN mod_ m ON m.id = d.mod_id \
                 WHERE d.file_name = ?1 \
                   AND m.status = 'downloading' \
                   AND m.deleted_at IS NULL \
                 ORDER BY d.id DESC LIMIT 1",
                [file_name],
                |row| row.get(0),
            )
            .optional()
            .map_err(CoreError::from)
    }

    /// Point the mod's download date at this capture's `captured_at`.
    /// A missing capture does not blank a date already stored. Linking a
    /// different capture later replaces the date with that capture's.
    pub(crate) fn copy_download_captured_at(&self, download_id: i64, mod_id: i64) -> Result<()> {
        self.db.execute(
            "UPDATE mod_ SET downloaded_at = (
                SELECT captured_at FROM download WHERE id = ?1
             ) WHERE id = ?2 AND EXISTS (SELECT 1 FROM download WHERE id = ?1)",
            (download_id, mod_id),
        )?;
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn set_download_captured_at(&self, download_id: i64, at: &str) -> Result<()> {
        self.db.execute(
            "UPDATE download SET captured_at = ?1 WHERE id = ?2",
            (at, download_id),
        )?;
        Ok(())
    }

    /// Drop a placeholder ledger row. Not a mod and not a file: used when a
    /// transfer never finished, or when it turned out to be a duplicate of a
    /// capture already on file.
    pub(crate) fn delete_download(&self, download_id: i64) -> Result<()> {
        self.db
            .execute("DELETE FROM download WHERE id = ?1", [download_id])?;
        Ok(())
    }

    /// Undo `soft_delete_loadout` for the whole instance: the instance and
    /// every one of its branches come back together.
    pub(crate) fn restore_loadout(&self, loadout_id: i64) -> Result<()> {
        let instance_id: Option<i64> = self
            .db
            .query_row(
                "SELECT instance_id FROM loadout WHERE id = ?1",
                [loadout_id],
                |row| row.get(0),
            )
            .optional()?;
        let Some(instance_id) = instance_id else {
            self.db.execute(
                "UPDATE loadout SET deleted_at = NULL WHERE id = ?1",
                [loadout_id],
            )?;
            return Ok(());
        };
        self.db.execute(
            "UPDATE instance SET deleted_at = NULL WHERE id = ?1",
            [instance_id],
        )?;
        self.db.execute(
            "UPDATE loadout SET deleted_at = NULL WHERE instance_id = ?1",
            [instance_id],
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
                    mod_.category, mod_.archive_path, mod_.source_host, mod_.page_url, \
                    mod_.game_versions, mod_.dependencies, mod_.category_raw, \
                    mod_.category_nexus_id, mod_.nexus_mod_id, mod_.author, mod_.website, \
                    mod_.size_bytes, mod_.installed_at, mod_.downloaded_at, mod_.loaders \
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
                    source_host: row.get(9)?,
                    page_url: row.get(10)?,
                    game_versions: decode_string_list(row.get(11)?),
                    dependencies: decode_dependencies(row.get(12)?),
                    category_raw: row.get(13)?,
                    category_nexus_id: row.get(14)?,
                    nexus_mod_id: row.get(15)?,
                    author: row.get(16)?,
                    website: row.get(17)?,
                    size_bytes: row.get(18)?,
                    installed_at: row.get(19)?,
                    downloaded_at: row.get(20)?,
                    loaders: decode_string_list(row.get(21)?),
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

    /// Two loadouts of the same game that predate instances each become
    /// their own instance. Their row ids (and therefore their snapshot
    /// histories) stay put; they are not folded into one shared branch list.
    #[test]
    fn existing_loadouts_of_one_game_become_separate_instances() {
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
                INSERT INTO loadout (game_id, name) VALUES ('skyrimse', 'Main 2026');
                INSERT INTO loadout (game_id, name) VALUES ('skyrimse', 'Labo météo');",
            )
            .unwrap();
        }

        let core = Core::open(&db_path).expect("an older database must still open");
        let rows: Vec<(i64, String, i64, String)> = {
            let mut stmt = core
                .db
                .prepare(
                    "SELECT l.id, l.name, i.id, i.name \
                     FROM loadout l JOIN instance i ON i.id = l.instance_id \
                     ORDER BY l.id",
                )
                .unwrap();
            stmt.query_map([], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
            })
            .unwrap()
            .collect::<rusqlite::Result<Vec<_>>>()
            .unwrap()
        };
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].1, "Main 2026", "the branch keeps the loadout name");
        assert_eq!(rows[1].1, "Labo météo");
        assert_eq!(rows[0].3, "Main 2026", "the instance copies that name");
        assert_eq!(rows[1].3, "Labo météo");
        assert_ne!(
            rows[0].2, rows[1].2,
            "same game, two histories: two instances"
        );
        assert_eq!(rows[0].0, 1);
        assert_eq!(rows[1].0, 2, "loadout ids are not rewritten");
        drop(core);

        let again = Core::open(&db_path).expect("backfill is idempotent");
        let instances: i64 = again
            .db
            .query_row("SELECT COUNT(*) FROM instance", [], |row| row.get(0))
            .unwrap();
        assert_eq!(instances, 2, "reopening must not insert a second pair");
    }
}
