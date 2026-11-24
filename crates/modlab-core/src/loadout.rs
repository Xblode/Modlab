//! Loadouts — a named, versionable mod profile.
//! See docs/01-architecture-decidee.md §5. First slice: creating a named
//! loadout and importing mods into it from some external source (an MO2
//! profile, eventually a Vortex export, ...). Revisions (immutable
//! snapshots) and plugin load order are real domain work for a later pass,
//! once this base is solid.

use crate::transaction::{Plan, Transaction};
use crate::{Core, CoreError, Result};
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
    /// `"stable"`, `"beta"` or `"alpha"` — see `set_channel`. New loadouts
    /// start `"stable"` (the column's own `DEFAULT`), not a claim that a
    /// fresh, empty loadout is actually battle-tested — just this module's
    /// starting point until a human says otherwise.
    pub channel: String,
}

/// List every loadout in the engine database, oldest first.
pub fn list(core: &Core) -> Result<Vec<LoadoutSummary>> {
    Ok(core
        .select_loadouts()?
        .into_iter()
        .map(|(id, game_id, name, channel)| LoadoutSummary {
            id,
            game_id,
            name,
            channel,
        })
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

/// Rename a loadout. A metadata edit, not a configuration change — nothing
/// about which mods are enabled is at stake, so this journals like `create`
/// rather than going through the snapshot/verify cycle `set_enabled` does.
pub fn rename(core: &Core, loadout_id: i64, name: &str) -> Result<()> {
    let name = name.trim();
    if name.is_empty() {
        return Err(CoreError::Message(
            "le nom ne peut pas être vide".to_string(),
        ));
    }
    if name.chars().count() > 100 {
        return Err(CoreError::Message(
            "le nom ne peut pas dépasser 100 caractères".to_string(),
        ));
    }
    core.set_loadout_name(loadout_id, name)?;
    let mut tx = Transaction::new(core, "loadout.rename");
    tx.step(format!("loadout {loadout_id} renommé en « {name} »"));
    tx.commit()?;
    Ok(())
}

/// The three channels a loadout can be on. Not a `GameAdapter` concern — a
/// loadout's maturity ("does this build still move?") is generic across
/// every game, same as the transaction cycle itself.
pub const CHANNELS: [&str; 3] = ["stable", "beta", "alpha"];

/// Set a loadout's channel. Refuses anything outside `CHANNELS` rather than
/// storing it: the UI's channel picker and the `.chan.*`/`ChannelBadge`
/// styling both hardcode exactly these three, so a fourth value stored here
/// would just render unstyled.
pub fn set_channel(core: &Core, loadout_id: i64, channel: &str) -> Result<()> {
    if !CHANNELS.contains(&channel) {
        return Err(CoreError::Message(format!("canal inconnu : {channel}")));
    }
    core.set_loadout_channel(loadout_id, channel)?;
    let mut tx = Transaction::new(core, "loadout.set_channel");
    tx.step(format!("loadout {loadout_id} → canal {channel}"));
    tx.commit()?;
    Ok(())
}

/// The mirror of `list`: every soft-deleted loadout, most recently deleted
/// first — what a trash/restore screen shows.
pub fn list_trash(core: &Core) -> Result<Vec<LoadoutSummary>> {
    Ok(core
        .select_deleted_loadouts()?
        .into_iter()
        .map(|(id, game_id, name, channel)| LoadoutSummary {
            id,
            game_id,
            name,
            channel,
        })
        .collect())
}

/// Soft-delete a loadout: it disappears from `list` (and so from every
/// client), but nothing about it is destroyed — its row and its
/// `loadout_entry`/`event` history stay exactly as they were, and `restore`
/// undoes this. Per the project's "internal trash, never real deletion"
/// rule (`docs/01-architecture-decidee.md`), just applied to Core metadata
/// rather than a game file.
pub fn delete(core: &Core, loadout_id: i64) -> Result<()> {
    core.soft_delete_loadout(loadout_id)?;
    let mut tx = Transaction::new(core, "loadout.delete");
    tx.step(format!("loadout {loadout_id} supprimé (récupérable)"));
    tx.commit()?;
    Ok(())
}

/// Undo `delete`.
pub fn restore(core: &Core, loadout_id: i64) -> Result<()> {
    core.restore_loadout(loadout_id)?;
    let mut tx = Transaction::new(core, "loadout.restore");
    tx.step(format!("loadout {loadout_id} restauré"));
    tx.commit()?;
    Ok(())
}

/// One mod ready to be imported into a loadout — deliberately source-agnostic.
/// Whatever knows the external format (an MO2 profile today, something else
/// later) produces these; `import_mods` doesn't need to know which.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportEntry {
    pub name: String,
    pub source_path: PathBuf,
    pub enabled: bool,
    /// Whatever version the source declared, if any — `None` when the
    /// source carries no version metadata (MO2's `modlist.txt` never does)
    /// rather than a guess.
    pub version: Option<String>,
}

/// Import a batch of mods into a loadout, preserving the order they were
/// given in. Each becomes a new `mod_` row (no dedup against mods already
/// imported elsewhere — see `Core::insert_mod`) plus a `loadout_entry` row.
/// The whole batch is journaled as one transaction. Returns how many were
/// imported.
pub fn import_mods(core: &Core, loadout_id: i64, entries: &[ImportEntry]) -> Result<usize> {
    let mut tx = Transaction::new(core, "loadout.import_mods");
    for (position, entry) in entries.iter().enumerate() {
        let mod_id = core.insert_mod(
            &entry.name,
            &entry.source_path.display().to_string(),
            entry.version.as_deref(),
        )?;
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
    /// Priority order: the larger `position` wins file conflicts (ADR 0002).
    /// Contiguous from 0 after any `move_mods`. Importers must hand mods
    /// over lowest priority first (`modlab_game_skyrim::mo2` reverses
    /// `modlist.txt`, whose first line is the winner).
    pub position: i64,
    /// Whatever version the mod was imported/installed with, if its source
    /// declared one — `None` when it didn't (most Skyrim imports today).
    pub version: Option<String>,
    /// `"archived"` — captured by the browser, archive not extracted yet, so
    /// the mod cannot be enabled — or `"installed"`. See `ModStatus`.
    pub status: String,
    /// A free tag the human typed, `None` until they do.
    pub category: Option<String>,
    /// Where the source archive sits, for anything that came from a capture.
    pub archive_path: Option<PathBuf>,
}

/// The two states a mod row can be in. Stored as text so a future third
/// state doesn't need a migration of existing rows.
pub const STATUS_ARCHIVED: &str = "archived";
pub const STATUS_INSTALLED: &str = "installed";

/// Read a loadout's entries back, in `position` order.
pub fn entries(core: &Core, loadout_id: i64) -> Result<Vec<LoadoutEntryRow>> {
    Ok(core
        .select_loadout_entries(loadout_id)?
        .into_iter()
        .map(|r| LoadoutEntryRow {
            mod_id: r.mod_id,
            name: r.name,
            source_path: PathBuf::from(r.source_path),
            enabled: r.enabled,
            position: r.position,
            version: r.version,
            status: r.status,
            category: r.category,
            archive_path: r.archive_path.map(PathBuf::from),
        })
        .collect())
}

/// Enable or disable a mod in a loadout, through the full §5 cycle: a
/// snapshot is taken right before the flip, the flip is re-read back to
/// confirm it actually stuck, and the loadout is rolled back to that
/// snapshot automatically if either step fails. Nothing is journaled as
/// committed unless both succeeded.
pub fn set_enabled(core: &Core, loadout_id: i64, mod_id: i64, enabled: bool) -> Result<bool> {
    let mut plan = Plan::new(core, loadout_id, "loadout.set_enabled");
    plan.step(format!(
        "mod {mod_id} in loadout {loadout_id} → {}",
        if enabled { "enabled" } else { "disabled" }
    ));
    plan.approve().apply(
        |core| core.set_loadout_entry_enabled(loadout_id, mod_id, enabled),
        |core, updated| {
            // No row matched at all — not a mutation that could have
            // half-applied, so there is nothing to verify.
            if !*updated {
                return Ok(());
            }
            let now_enabled = core
                .select_loadout_entries(loadout_id)?
                .into_iter()
                .find(|r| r.mod_id == mod_id)
                .map(|r| r.enabled);
            if now_enabled == Some(enabled) {
                Ok(())
            } else {
                Err(CoreError::Message(format!(
                    "vérification échouée : le mod {mod_id} n'est pas repassé à {enabled} après écriture"
                )))
            }
        },
    )
}

/// Move one or several mods to a new place in a loadout's priority order,
/// through the full §5 cycle (plan → approval → snapshot → apply → verify →
/// automatic rollback), journaled as `loadout.move_mods`.
///
/// Order is `position` ascending, so the *last* mod wins conflicts. `to` is
/// the index, in the final list of visible mods, where the moved block lands
/// (clamped to the valid range, so `usize::MAX` means "top priority" and `0`
/// "lowest"). The moved mods keep their relative order. Positions are
/// renumbered contiguously from 0 afterwards. Returns `false` (nothing
/// written, nothing journaled) when the order would not change.
pub fn move_mods(core: &Core, loadout_id: i64, mod_ids: &[i64], to: usize) -> Result<bool> {
    move_mods_with(core, loadout_id, mod_ids, to, |_| Ok(()))
}

/// `move_mods` with a hook run between the write and its verification — the
/// only way a test can make the verification fail on purpose.
fn move_mods_with(
    core: &Core,
    loadout_id: i64,
    mod_ids: &[i64],
    to: usize,
    after_write: impl FnOnce(&Core) -> Result<()>,
) -> Result<bool> {
    let visible: Vec<i64> = core
        .select_loadout_entries(loadout_id)?
        .into_iter()
        .map(|r| r.mod_id)
        .collect();
    let mut moved: Vec<i64> = Vec::new();
    for id in mod_ids {
        if !visible.contains(id) {
            return Err(CoreError::Message(format!(
                "le mod {id} n'appartient pas à l'instance {loadout_id}"
            )));
        }
        if !moved.contains(id) {
            moved.push(*id);
        }
    }
    // Keep the moved mods in their current relative order.
    moved.sort_by_key(|id| visible.iter().position(|v| v == id));
    let mut rest: Vec<i64> = visible
        .iter()
        .copied()
        .filter(|id| !moved.contains(id))
        .collect();
    let at = to.min(rest.len());
    rest.splice(at..at, moved.iter().copied());
    let expected = rest;
    if moved.is_empty() || expected == visible {
        return Ok(false);
    }

    let mut plan = Plan::new(core, loadout_id, "loadout.move_mods");
    plan.step(format!(
        "{} mod(s) déplacé(s) à la place {} dans l'instance {loadout_id}",
        moved.len(),
        at + 1
    ));
    plan.approve().apply(
        |core| {
            // Hidden (soft-deleted) mods keep their slots; the visible ones
            // are re-dealt into the slots visible mods used to occupy.
            let state = core.select_loadout_state(loadout_id)?;
            let enabled: std::collections::HashMap<i64, bool> =
                state.iter().map(|(m, e, _)| (*m, *e)).collect();
            let mut slots = state
                .iter()
                .filter(|(m, _, _)| visible.contains(m))
                .map(|(_, _, p)| *p);
            let mut merged: Vec<(i64, bool, i64)> = state
                .iter()
                .filter(|(m, _, _)| !visible.contains(m))
                .copied()
                .collect();
            for id in &expected {
                let slot = slots.next().ok_or_else(|| {
                    CoreError::Message("positions incohérentes dans l'instance".into())
                })?;
                merged.push((*id, enabled[id], slot));
            }
            merged.sort_by_key(|(_, _, p)| *p);
            let renumbered: Vec<(i64, bool, i64)> = merged
                .into_iter()
                .enumerate()
                .map(|(i, (m, e, _))| (m, e, i as i64))
                .collect();
            core.replace_loadout_entries(loadout_id, &renumbered)?;
            after_write(core)?;
            Ok(())
        },
        |core, ()| {
            let state = core.select_loadout_state(loadout_id)?;
            let contiguous = state
                .iter()
                .enumerate()
                .all(|(i, (_, _, p))| i64::try_from(i) == Ok(*p));
            let order: Vec<i64> = core
                .select_loadout_entries(loadout_id)?
                .into_iter()
                .map(|r| r.mod_id)
                .collect();
            if contiguous && order == expected {
                Ok(())
            } else {
                Err(CoreError::Message(
                    "vérification échouée : l'ordre relu ne correspond pas au déplacement demandé"
                        .into(),
                ))
            }
        },
    )?;
    Ok(true)
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
                version: None,
            },
            ImportEntry {
                name: "Disabled Thing".into(),
                source_path: "F:/mods/Disabled Thing".into(),
                enabled: false,
                version: None,
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
                version: None,
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
                    version: None,
                },
                ImportEntry {
                    name: "Second".into(),
                    source_path: "F:/mods/Second".into(),
                    enabled: false,
                    version: None,
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
                version: None,
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

    #[test]
    fn new_loadouts_start_on_the_stable_channel() {
        let core = Core::open_in_memory().unwrap();
        create(&core, "skyrimse", "Main 2026").unwrap();
        assert_eq!(list(&core).unwrap()[0].channel, "stable");
    }

    #[test]
    fn renames_a_loadout_and_journals_it() {
        let core = Core::open_in_memory().unwrap();
        let lo = create(&core, "skyrimse", "Main 2026").unwrap();
        let before = core.event_count().unwrap();

        rename(&core, lo.id, "  Modlist définitive  ").unwrap();

        assert_eq!(
            list(&core).unwrap()[0].name,
            "Modlist définitive",
            "trimmed"
        );
        assert_eq!(core.event_count().unwrap(), before + 1);
    }

    #[test]
    fn rename_refuses_an_empty_name() {
        let core = Core::open_in_memory().unwrap();
        let lo = create(&core, "skyrimse", "Main 2026").unwrap();
        let err = rename(&core, lo.id, "   ").unwrap_err();
        assert!(err.to_string().contains("vide"));
        assert_eq!(list(&core).unwrap()[0].name, "Main 2026", "untouched");
    }

    #[test]
    fn sets_a_valid_channel_and_journals_it() {
        let core = Core::open_in_memory().unwrap();
        let lo = create(&core, "skyrimse", "Main 2026").unwrap();

        set_channel(&core, lo.id, "beta").unwrap();

        assert_eq!(list(&core).unwrap()[0].channel, "beta");
    }

    #[test]
    fn set_channel_refuses_anything_outside_the_three_known_channels() {
        let core = Core::open_in_memory().unwrap();
        let lo = create(&core, "skyrimse", "Main 2026").unwrap();

        let err = set_channel(&core, lo.id, "nightly").unwrap_err();

        assert!(err.to_string().contains("nightly"));
        assert_eq!(
            list(&core).unwrap()[0].channel,
            "stable",
            "an unknown channel must never overwrite the real one"
        );
    }

    #[test]
    fn deletes_a_loadout_and_it_disappears_from_the_list() {
        let core = Core::open_in_memory().unwrap();
        let a = create(&core, "skyrimse", "Main 2026").unwrap();
        create(&core, "skyrimse", "Labo météo").unwrap();

        delete(&core, a.id).unwrap();

        let remaining = list(&core).unwrap();
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].name, "Labo météo");
    }

    #[test]
    fn delete_journals_a_transaction() {
        let core = Core::open_in_memory().unwrap();
        let lo = create(&core, "skyrimse", "Main 2026").unwrap();
        let before = core.event_count().unwrap();

        delete(&core, lo.id).unwrap();

        assert_eq!(core.event_count().unwrap(), before + 1);
    }

    #[test]
    fn a_deleted_loadout_appears_in_the_trash_and_leaves_it_on_restore() {
        let core = Core::open_in_memory().unwrap();
        let a = create(&core, "skyrimse", "Main 2026").unwrap();
        create(&core, "skyrimse", "Labo météo").unwrap();

        assert_eq!(list_trash(&core).unwrap().len(), 0, "nothing deleted yet");

        delete(&core, a.id).unwrap();
        let trash = list_trash(&core).unwrap();
        assert_eq!(trash.len(), 1);
        assert_eq!(trash[0].name, "Main 2026");

        restore(&core, a.id).unwrap();
        assert_eq!(
            list_trash(&core).unwrap().len(),
            0,
            "restored, so no longer in the trash"
        );
        assert_eq!(list(&core).unwrap().len(), 2, "and back in the live list");
    }

    #[test]
    fn restoring_a_deleted_loadout_brings_it_back_with_its_data_intact() {
        let core = Core::open_in_memory().unwrap();
        let lo = create(&core, "skyrimse", "Main 2026").unwrap();
        import_mods(
            &core,
            lo.id,
            &[ImportEntry {
                name: "SkyUI".into(),
                source_path: "F:/mods/SkyUI".into(),
                enabled: true,
                version: None,
            }],
        )
        .unwrap();

        delete(&core, lo.id).unwrap();
        assert_eq!(list(&core).unwrap().len(), 0, "hidden while deleted");

        restore(&core, lo.id).unwrap();

        let restored = list(&core).unwrap();
        assert_eq!(restored.len(), 1);
        assert_eq!(restored[0].name, "Main 2026");
        assert_eq!(
            core.loadout_entry_count(lo.id).unwrap(),
            1,
            "entries were never touched by the soft delete"
        );
    }

    /// Five mods A..E at positions 0..4; returns the loadout id and mod ids.
    fn five_mods(core: &Core) -> (i64, Vec<i64>) {
        let lo = create(core, "skyrimse", "Order").unwrap();
        let entries: Vec<ImportEntry> = ["A", "B", "C", "D", "E"]
            .iter()
            .map(|n| ImportEntry {
                name: (*n).into(),
                source_path: format!("F:/mods/{n}").into(),
                enabled: true,
                version: None,
            })
            .collect();
        import_mods(core, lo.id, &entries).unwrap();
        let ids = entries_ids(core, lo.id);
        (lo.id, ids)
    }

    fn entries_ids(core: &Core, loadout_id: i64) -> Vec<i64> {
        entries(core, loadout_id)
            .unwrap()
            .iter()
            .map(|r| r.mod_id)
            .collect()
    }

    fn names(core: &Core, loadout_id: i64) -> String {
        entries(core, loadout_id)
            .unwrap()
            .iter()
            .map(|r| r.name.as_str())
            .collect()
    }

    fn assert_contiguous(core: &Core, loadout_id: i64) {
        let positions: Vec<i64> = entries(core, loadout_id)
            .unwrap()
            .iter()
            .map(|r| r.position)
            .collect();
        assert_eq!(positions, (0..positions.len() as i64).collect::<Vec<_>>());
    }

    #[test]
    fn moves_one_mod_and_journals_it() {
        let core = Core::open_in_memory().unwrap();
        let (lo, ids) = five_mods(&core);
        let before = core.event_count().unwrap();

        assert!(move_mods(&core, lo, &[ids[0]], 3).unwrap());

        assert_eq!(names(&core, lo), "BCDAE");
        assert_contiguous(&core, lo);
        assert_eq!(core.event_count().unwrap(), before + 1);
        let last = &crate::transaction::list(&core, 1).unwrap()[0];
        assert_eq!(last.kind, "loadout.move_mods");
    }

    #[test]
    fn moves_several_mods_as_a_block_keeping_their_relative_order() {
        let core = Core::open_in_memory().unwrap();
        let (lo, ids) = five_mods(&core);

        // Given out of order on purpose: E then B.
        move_mods(&core, lo, &[ids[4], ids[1]], 0).unwrap();
        assert_eq!(names(&core, lo), "BEACD");
        assert_contiguous(&core, lo);

        let (lo2, ids2) = five_mods(&core);
        move_mods(&core, lo2, &[ids2[0], ids2[1]], 3).unwrap();
        assert_eq!(names(&core, lo2), "CDEAB");
    }

    #[test]
    fn target_is_clamped_to_the_bounds() {
        let core = Core::open_in_memory().unwrap();
        let (lo, ids) = five_mods(&core);

        move_mods(&core, lo, &[ids[0]], usize::MAX).unwrap();
        assert_eq!(names(&core, lo), "BCDEA");
        move_mods(&core, lo, &[ids[0]], 0).unwrap();
        assert_eq!(names(&core, lo), "ABCDE");
        assert_contiguous(&core, lo);
    }

    #[test]
    fn a_move_that_changes_nothing_writes_and_journals_nothing() {
        let core = Core::open_in_memory().unwrap();
        let (lo, ids) = five_mods(&core);
        let before = core.event_count().unwrap();

        assert!(!move_mods(&core, lo, &[ids[1]], 1).unwrap());
        assert!(!move_mods(&core, lo, &[], 2).unwrap());
        assert_eq!(core.event_count().unwrap(), before);
    }

    #[test]
    fn refuses_a_mod_that_is_not_in_the_loadout() {
        let core = Core::open_in_memory().unwrap();
        let (lo, _) = five_mods(&core);
        assert!(move_mods(&core, lo, &[9999], 0).is_err());
        assert_eq!(names(&core, lo), "ABCDE");
    }

    #[test]
    fn a_failed_verification_restores_the_previous_order() {
        let core = Core::open_in_memory().unwrap();
        let (lo, ids) = five_mods(&core);
        let before = core.event_count().unwrap();

        // Corrupt the write after it happens: a gap in the positions.
        let result = move_mods_with(&core, lo, &[ids[0]], 4, |core| {
            let state: Vec<(i64, bool, i64)> = core
                .select_loadout_state(lo)?
                .into_iter()
                .map(|(m, e, p)| (m, e, p * 2))
                .collect();
            core.replace_loadout_entries(lo, &state)
        });

        assert!(result.is_err());
        assert_eq!(names(&core, lo), "ABCDE");
        assert_contiguous(&core, lo);
        assert_eq!(core.event_count().unwrap(), before, "nothing journaled");
    }

    #[test]
    fn a_move_survives_the_explicit_revision_rollback() {
        let core = Core::open_in_memory().unwrap();
        let (lo, ids) = five_mods(&core);
        crate::transaction::snapshot_loadout(&core, lo).unwrap();

        move_mods(&core, lo, &[ids[0]], 4).unwrap();
        assert_eq!(names(&core, lo), "BCDEA");

        crate::transaction::rollback_loadout(&core, lo).unwrap();
        assert_eq!(names(&core, lo), "ABCDE");
    }
}
