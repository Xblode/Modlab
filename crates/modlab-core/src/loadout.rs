//! Loadouts — a named, versionable mod profile.
//! See docs/01-architecture-decidee.md §5. First slice: creating a named
//! loadout and importing mods into it from some external source (an MO2
//! profile, eventually a Vortex export, ...). Revisions (immutable
//! snapshots) and plugin load order are real domain work for a later pass,
//! once this base is solid.

use crate::transaction::Transaction;
use crate::{Core, Result};
use serde::Serialize;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize)]
pub struct Loadout {
    pub id: i64,
    pub name: String,
}

/// A loadout as listed for UI/CLI clients — includes the game id so a
/// client can group without knowing how the row was stored.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LoadoutSummary {
    pub id: i64,
    pub game_id: String,
    pub name: String,
}

/// List every loadout in the engine database, oldest first.
pub fn list(core: &Core) -> Result<Vec<LoadoutSummary>> {
    Ok(core
        .select_loadouts()?
        .into_iter()
        .map(|(id, game_id, name)| LoadoutSummary { id, game_id, name })
        .collect())
}

/// Create a new, empty loadout and journal it as a transaction.
pub fn create(core: &Core, game_id: &str, name: &str) -> Result<Loadout> {
    let id = core.insert_loadout(game_id, name)?;

    let mut tx = Transaction::new(core, "loadout.create");
    tx.step(format!(
        "created empty loadout '{name}' (id {id}) for game '{game_id}'"
    ));
    tx.commit()?;

    Ok(Loadout {
        id,
        name: name.to_string(),
    })
}

/// One mod ready to be imported into a loadout — deliberately source-agnostic.
/// Whatever knows the external format (an MO2 profile today, something else
/// later) produces these; `import_mods` doesn't need to know which.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportEntry {
    pub name: String,
    pub source_path: PathBuf,
    pub enabled: bool,
}

/// Import a batch of mods into a loadout, preserving the order they were
/// given in. Each becomes a new `mod_` row (no dedup against mods already
/// imported elsewhere — see `Core::insert_mod`) plus a `loadout_entry` row.
/// The whole batch is journaled as one transaction. Returns how many were
/// imported.
pub fn import_mods(core: &Core, loadout_id: i64, entries: &[ImportEntry]) -> Result<usize> {
    let mut tx = Transaction::new(core, "loadout.import_mods");
    for (position, entry) in entries.iter().enumerate() {
        let mod_id = core.insert_mod(&entry.name, &entry.source_path.display().to_string())?;
        core.insert_loadout_entry(
            loadout_id,
            mod_id,
            entry.enabled,
            i64::try_from(position).expect("position fits in i64: no loadout has billions of mods"),
        )?;
        tx.step(format!(
            "imported '{}' ({})",
            entry.name,
            if entry.enabled { "enabled" } else { "disabled" }
        ));
    }
    tx.commit()?;
    Ok(entries.len())
}

/// One mod as attached to a loadout, read back from storage.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LoadoutEntryRow {
    pub mod_id: i64,
    pub name: String,
    pub source_path: PathBuf,
    pub enabled: bool,
    /// Import/add order. Not yet confirmed to correspond to MO2's real
    /// on-screen priority direction — see `modlab_game_skyrim::mo2`'s
    /// module doc. Don't treat a larger `position` as "wins conflicts"
    /// without that confirmation.
    pub position: i64,
}

/// Read a loadout's entries back, in `position` order.
pub fn entries(core: &Core, loadout_id: i64) -> Result<Vec<LoadoutEntryRow>> {
    Ok(core
        .select_loadout_entries(loadout_id)?
        .into_iter()
        .map(
            |(mod_id, name, source_path, enabled, position)| LoadoutEntryRow {
                mod_id,
                name,
                source_path: PathBuf::from(source_path),
                enabled,
                position,
            },
        )
        .collect())
}

/// Enable or disable a mod in a loadout and journal it. Does not snapshot
/// a revision — that cycle is still a later pass (see module docs).
pub fn set_enabled(core: &Core, loadout_id: i64, mod_id: i64, enabled: bool) -> Result<bool> {
    let updated = core.set_loadout_entry_enabled(loadout_id, mod_id, enabled)?;
    if updated {
        let mut tx = Transaction::new(core, "loadout.set_enabled");
        tx.step(format!(
            "mod {mod_id} in loadout {loadout_id} → {}",
            if enabled { "enabled" } else { "disabled" }
        ));
        tx.commit()?;
    }
    Ok(updated)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creating_a_loadout_journals_a_transaction_and_persists() {
        let core = Core::open_in_memory().unwrap();
        let before = core.event_count().unwrap();

        let loadout = create(&core, "skyrimse", "Main 2026").unwrap();

        assert_eq!(loadout.name, "Main 2026");
        assert!(loadout.id > 0);
        assert_eq!(core.event_count().unwrap(), before + 1);
    }

    #[test]
    fn imports_mods_and_preserves_order() {
        let core = Core::open_in_memory().unwrap();
        let loadout = create(&core, "skyrimse", "Main 2026").unwrap();

        let entries = vec![
            ImportEntry {
                name: "SkyUI".into(),
                source_path: "F:/mods/SkyUI".into(),
                enabled: true,
            },
            ImportEntry {
                name: "Disabled Thing".into(),
                source_path: "F:/mods/Disabled Thing".into(),
                enabled: false,
            },
        ];

        let imported = import_mods(&core, loadout.id, &entries).unwrap();

        assert_eq!(imported, 2);
        assert_eq!(core.loadout_entry_count(loadout.id).unwrap(), 2);
    }

    #[test]
    fn importing_into_an_unrelated_loadout_does_not_see_these_entries() {
        let core = Core::open_in_memory().unwrap();
        let loadout_a = create(&core, "skyrimse", "A").unwrap();
        let loadout_b = create(&core, "skyrimse", "B").unwrap();

        import_mods(
            &core,
            loadout_a.id,
            &[ImportEntry {
                name: "SkyUI".into(),
                source_path: "F:/mods/SkyUI".into(),
                enabled: true,
            }],
        )
        .unwrap();

        assert_eq!(core.loadout_entry_count(loadout_a.id).unwrap(), 1);
        assert_eq!(core.loadout_entry_count(loadout_b.id).unwrap(), 0);
    }

    #[test]
    fn reads_entries_back_in_position_order() {
        let core = Core::open_in_memory().unwrap();
        let loadout = create(&core, "skyrimse", "Main 2026").unwrap();
        import_mods(
            &core,
            loadout.id,
            &[
                ImportEntry {
                    name: "First".into(),
                    source_path: "F:/mods/First".into(),
                    enabled: true,
                },
                ImportEntry {
                    name: "Second".into(),
                    source_path: "F:/mods/Second".into(),
                    enabled: false,
                },
            ],
        )
        .unwrap();

        let rows = entries(&core, loadout.id).unwrap();

        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].name, "First");
        assert_eq!(rows[0].position, 0);
        assert!(rows[0].enabled);
        assert_eq!(rows[1].name, "Second");
        assert_eq!(rows[1].position, 1);
        assert!(!rows[1].enabled);
    }

    #[test]
    fn lists_loadouts_and_toggles_enabled() {
        let core = Core::open_in_memory().unwrap();
        let a = create(&core, "skyrimse", "Main 2026").unwrap();
        create(&core, "minecraft", "Survie").unwrap();
        import_mods(
            &core,
            a.id,
            &[ImportEntry {
                name: "SkyUI".into(),
                source_path: "F:/mods/SkyUI".into(),
                enabled: true,
            }],
        )
        .unwrap();

        let listed = list(&core).unwrap();
        assert_eq!(listed.len(), 2);
        assert_eq!(listed[0].name, "Main 2026");
        assert_eq!(listed[1].game_id, "minecraft");

        assert!(set_enabled(&core, a.id, listed_mod_id(&core, a.id), false).unwrap());
        assert!(!entries(&core, a.id).unwrap()[0].enabled);
    }

    fn listed_mod_id(core: &Core, loadout_id: i64) -> i64 {
        entries(core, loadout_id).unwrap()[0].mod_id
    }
}
