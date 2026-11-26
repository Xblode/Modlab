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
    /// The instance this branch belongs to. Branches are not shared across
    /// instances.
    pub instance_id: i64,
    pub instance_name: String,
}

/// Name of the first branch when the UI creates an instance. French, because
/// it is a label the human reads. Existing loadouts migrated in place keep
/// their own name instead of being renamed to this.
pub const INITIAL_BRANCH_NAME: &str = "principale";

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
    /// Parent instance. Every branch of one instance shares this id, and no
    /// branch of another instance does.
    pub instance_id: i64,
    pub instance_name: String,
    pub mc_loader: Option<String>,
    pub mc_game_version: Option<String>,
    pub mc_loader_version: Option<String>,
}

fn summary_from(record: crate::LoadoutRecord) -> LoadoutSummary {
    LoadoutSummary {
        id: record.id,
        game_id: record.game_id,
        name: record.name,
        channel: record.channel,
        instance_id: record.instance_id,
        instance_name: record.instance_name,
        mc_loader: record.mc_loader,
        mc_game_version: record.mc_game_version,
        mc_loader_version: record.mc_loader_version,
    }
}

/// List every live branch, oldest first.
pub fn list(core: &Core) -> Result<Vec<LoadoutSummary>> {
    Ok(core
        .select_loadouts()?
        .into_iter()
        .map(summary_from)
        .collect())
}

fn validate_name(name: &str) -> Result<String> {
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
    Ok(name.to_string())
}

/// Whether another live instance of `game_id` already uses this name
/// (case-insensitive). `except_instance_id` skips one instance (rename).
fn instance_name_taken(
    core: &Core,
    game_id: &str,
    name: &str,
    except_instance_id: Option<i64>,
) -> Result<bool> {
    let needle = name.trim().to_lowercase();
    let mut seen = std::collections::HashSet::new();
    for summary in list(core)? {
        if summary.game_id != game_id {
            continue;
        }
        if !seen.insert(summary.instance_id) {
            continue;
        }
        if except_instance_id == Some(summary.instance_id) {
            continue;
        }
        if summary.instance_name.to_lowercase() == needle {
            return Ok(true);
        }
    }
    Ok(false)
}

fn loadout_from(id: i64, name: &str, instance_id: i64, instance_name: &str) -> Loadout {
    Loadout {
        id,
        name: name.to_string(),
        instance_id,
        instance_name: instance_name.to_string(),
    }
}

/// Create an independent instance whose initial branch keeps `name`.
///
/// CLI, MO2 import, Minecraft import and tests call this. Each call is its
/// own instance, so two calls for the same game never share branches or
/// snapshot history. The UI's "Créer une instance" uses `create_instance`
/// instead, which names the first branch [`INITIAL_BRANCH_NAME`].
pub fn create(core: &Core, game_id: &str, name: &str) -> Result<Loadout> {
    let instance_id = core.insert_instance(game_id, name)?;
    let id = core.insert_loadout(game_id, name, instance_id)?;

    let mut tx = Transaction::new(core, "loadout.create");
    tx.step(format!(
        "created empty loadout '{name}' (id {id}) for game '{game_id}'"
    ));
    tx.commit()?;

    Ok(loadout_from(id, name, instance_id, name))
}

/// Create an independent instance named `name`, with an empty initial branch
/// called [`INITIAL_BRANCH_NAME`]. Does not attach that branch to any
/// instance that already exists.
pub fn create_instance(core: &Core, game_id: &str, name: &str) -> Result<Loadout> {
    let instance_name = validate_name(name)?;
    if instance_name_taken(core, game_id, &instance_name, None)? {
        return Err(CoreError::Message(
            "une instance porte déjà ce nom pour ce jeu".to_string(),
        ));
    }
    let instance_id = core.insert_instance(game_id, &instance_name)?;
    let id = core.insert_loadout(game_id, INITIAL_BRANCH_NAME, instance_id)?;

    let mut tx = Transaction::new(core, "loadout.create");
    tx.step(format!(
        "created instance '{instance_name}' with branch '{INITIAL_BRANCH_NAME}' (id {id}) for game '{game_id}'"
    ));
    tx.commit()?;

    Ok(loadout_from(
        id,
        INITIAL_BRANCH_NAME,
        instance_id,
        &instance_name,
    ))
}

/// Create a branch of the instance that owns `from_loadout_id`.
///
/// Copies that branch's mod list (membership, enabled, order) onto a new
/// loadout row. Snapshots are not copied: the new branch starts at revision
/// 0, and Enregistrer / Annuler on the source branch keep using the source
/// id. The new branch is invisible to every other instance.
pub fn create_branch(core: &Core, from_loadout_id: i64, name: &str) -> Result<Loadout> {
    let name = validate_name(name)?;
    let source = core
        .select_loadout(from_loadout_id)?
        .ok_or_else(|| CoreError::Message(format!("branche {from_loadout_id} introuvable")))?;
    let taken = list(core)?
        .into_iter()
        .any(|branch| branch.instance_id == source.instance_id && branch.name == name);
    if taken {
        return Err(CoreError::Message(
            "une branche porte déjà ce nom dans cette instance".to_string(),
        ));
    }
    let id = core.insert_loadout(&source.game_id, &name, source.instance_id)?;
    core.copy_loadout_entries(from_loadout_id, id)?;
    if source.channel != "stable" {
        core.set_loadout_channel(id, &source.channel)?;
    }

    let mut tx = Transaction::new(core, "loadout.create_branch");
    tx.step(format!(
        "created branch '{name}' (id {id}) of instance {} from loadout {from_loadout_id}",
        source.instance_id
    ));
    tx.commit()?;

    Ok(loadout_from(
        id,
        &name,
        source.instance_id,
        &source.instance_name,
    ))
}

/// Rename a loadout. A metadata edit, not a configuration change — nothing
/// about which mods are enabled is at stake, so this journals like `create`
/// rather than going through the snapshot/verify cycle `set_enabled` does.
pub fn rename(core: &Core, loadout_id: i64, name: &str) -> Result<()> {
    let name = validate_name(name)?;
    core.set_loadout_name(loadout_id, &name)?;
    let mut tx = Transaction::new(core, "loadout.rename");
    tx.step(format!("loadout {loadout_id} renommé en « {name} »"));
    tx.commit()?;
    Ok(())
}

/// Rename the instance that owns `loadout_id`. The branch's own name stays.
/// The sidebar renames instances; the branch selector shows branch names.
pub fn rename_instance_of_loadout(core: &Core, loadout_id: i64, name: &str) -> Result<()> {
    let name = validate_name(name)?;
    let record = core.select_loadout(loadout_id)?.ok_or_else(|| {
        CoreError::Message(format!("instance de la branche {loadout_id} introuvable"))
    })?;
    if instance_name_taken(core, &record.game_id, &name, Some(record.instance_id))? {
        return Err(CoreError::Message(
            "une instance porte déjà ce nom pour ce jeu".to_string(),
        ));
    }
    core.set_instance_name(record.instance_id, &name)?;
    let mut tx = Transaction::new(core, "instance.rename");
    tx.step(format!(
        "instance {} renommée en « {name} »",
        record.instance_id
    ));
    tx.commit()?;
    Ok(())
}

/// The three channels a loadout can be on. Not a `GameAdapter` concern — a
/// loadout's maturity ("does this build still move?") is generic across
/// every game, same as the transaction cycle itself.
pub const CHANNELS: [&str; 3] = ["stable", "beta", "alpha"];

/// Minecraft loader ids the UI may store on a branch.
pub const MC_LOADERS: [&str; 5] = ["vanilla", "fabric", "forge", "neoforge", "quilt"];

fn normalize_mc_loader(loader: &str) -> Result<String> {
    let loader = loader.trim().to_lowercase();
    if loader.is_empty() {
        return Err(CoreError::Message("loader requis".to_string()));
    }
    if MC_LOADERS.iter().any(|id| *id == loader) {
        Ok(loader)
    } else {
        Err(CoreError::Message(format!("loader inconnu : {loader}")))
    }
}

fn validate_mc_version(value: &str) -> Result<String> {
    let value = value.trim();
    if value.is_empty() {
        return Err(CoreError::Message("version Minecraft requise".to_string()));
    }
    if value.chars().count() > 32 {
        return Err(CoreError::Message(
            "version Minecraft trop longue".to_string(),
        ));
    }
    Ok(value.to_string())
}

/// Set which Minecraft loader this branch targets.
pub fn set_mc_loader(core: &Core, loadout_id: i64, loader: &str) -> Result<()> {
    let loader = normalize_mc_loader(loader)?;
    core.set_loadout_mc_loader(loadout_id, Some(&loader))?;
    core.set_loadout_mc_loader_version(loadout_id, None)?;
    let mut tx = Transaction::new(core, "loadout.set_mc_loader");
    tx.step(format!("loadout {loadout_id} → loader {loader}"));
    tx.commit()?;
    Ok(())
}

/// Set the Minecraft game version for this branch.
///
/// Changing the game version clears the loader choice and its version — the
/// human must pick again (except vanilla, which needs no loader version).
pub fn set_mc_game_version(core: &Core, loadout_id: i64, version: &str) -> Result<()> {
    let version = validate_mc_version(version)?;
    let record = core
        .select_loadout(loadout_id)?
        .ok_or_else(|| CoreError::Message("branche introuvable".to_string()))?;
    let version_changed = record.mc_game_version.as_deref() != Some(version.as_str());
    core.set_loadout_mc_game_version(loadout_id, Some(&version))?;
    if version_changed {
        core.set_loadout_mc_loader(loadout_id, None)?;
        core.set_loadout_mc_loader_version(loadout_id, None)?;
    }
    let mut tx = Transaction::new(core, "loadout.set_mc_game_version");
    tx.step(format!("loadout {loadout_id} → Minecraft {version}"));
    tx.commit()?;
    Ok(())
}

/// Set the mod-loader version (Fabric, Forge, …). Cleared when loader is vanilla.
pub fn set_mc_loader_version(core: &Core, loadout_id: i64, version: &str) -> Result<()> {
    let version = version.trim();
    if version.is_empty() {
        return Err(CoreError::Message("version du loader requise".to_string()));
    }
    let record = core
        .select_loadout(loadout_id)?
        .ok_or_else(|| CoreError::Message("branche introuvable".to_string()))?;
    if record.mc_loader.as_deref() == Some("vanilla") {
        return Err(CoreError::Message(
            "Vanilla n'a pas de version de loader".to_string(),
        ));
    }
    core.set_loadout_mc_loader_version(loadout_id, Some(version))?;
    let mut tx = Transaction::new(core, "loadout.set_mc_loader_version");
    tx.step(format!("loadout {loadout_id} → loader {version}"));
    tx.commit()?;
    Ok(())
}

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
        .map(summary_from)
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
/// imported elsewhere — see `Core::insert_mod`) plus a `loadout_entry` row
/// appended after any entries already present. The whole batch is journaled
/// as one transaction. Returns how many were imported.
pub fn import_mods(core: &Core, loadout_id: i64, entries: &[ImportEntry]) -> Result<usize> {
    let mut tx = Transaction::new(core, "loadout.import_mods");
    let base = core.loadout_entry_count(loadout_id)?;
    for (offset, entry) in entries.iter().enumerate() {
        let mod_id = core.insert_mod(
            &entry.name,
            &entry.source_path.display().to_string(),
            entry.version.as_deref(),
        )?;
        // An import lands the mod already installed. The clock is the import
        // time when the source has no older install date to offer. A file
        // (a Minecraft jar) also records its length; a folder does not.
        crate::mod_measure::note_imported(core, mod_id, &entry.source_path)?;
        let position = base
            + i64::try_from(offset).expect("position fits in i64: no loadout has billions of mods");
        core.insert_loadout_entry(loadout_id, mod_id, entry.enabled, position)?;
        tx.step(format!(
            "imported '{}' ({})",
            entry.name,
            if entry.enabled { "enabled" } else { "disabled" }
        ));
    }
    tx.commit()?;
    Ok(entries.len())
}

/// Host token for a mod the human added from disk (file picker or a jar
/// already sitting in `mods/`), with no platform page.
pub const SOURCE_HOST_MANUAL: &str = "manual";

/// Mark one mod as a manual disk add. Does not blank a host or page already
/// stored by a higher layer.
pub fn mark_manual_source(core: &Core, mod_id: i64) -> Result<()> {
    core.set_mod_source(mod_id, Some(SOURCE_HOST_MANUAL), None, None)
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
    /// `"downloading"` — the browser has started the transfer, the file is
    /// not here yet — `"archived"` once it has landed and not been extracted,
    /// or `"installed"`. A downloading or archived mod cannot be enabled.
    pub status: String,
    /// A free tag the human typed, `None` until they do.
    pub category: Option<String>,
    /// Where the source archive sits, for anything that came from a capture.
    pub archive_path: Option<PathBuf>,
    /// Opaque host token stored at download (`nexus`, `curseforge`, …).
    pub source_host: Option<String>,
    /// Page to reopen in the embedded browser. Not the file URL.
    pub page_url: Option<String>,
    /// Game versions the file declared. Empty when it declared none.
    pub game_versions: Vec<String>,
    /// Dependencies the file declared.
    pub dependencies: Vec<crate::game_adapter::DeclaredDependency>,
    /// Loader ids the file or its name stated. Empty when none were stated.
    pub loaders: Vec<String>,
    /// Category text as the file wrote it, before any game list translates it.
    pub category_raw: Option<String>,
    /// Nexus category number when the file had one.
    pub category_nexus_id: Option<String>,
    /// Remote mod id read from the file name or from `meta.ini`.
    pub nexus_mod_id: Option<String>,
    pub author: Option<String>,
    pub website: Option<String>,
    /// Byte length of the mod file (archive or jar). `None` until known.
    pub size_bytes: Option<i64>,
    /// When the mod became installed, ISO 8601 UTC. `None` before that.
    pub installed_at: Option<String>,
    /// When the mod was downloaded or otherwise obtained, ISO 8601 UTC.
    /// `None` when it could not be recovered. Stays after the archive goes.
    pub downloaded_at: Option<String>,
}

/// States a mod row can be in. Stored as text so another state doesn't need
/// a migration of existing rows.
pub const STATUS_DOWNLOADING: &str = "downloading";
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
            source_host: r.source_host,
            page_url: r.page_url,
            game_versions: r.game_versions,
            dependencies: r.dependencies,
            loaders: r.loaders,
            category_raw: r.category_raw,
            category_nexus_id: r.category_nexus_id,
            nexus_mod_id: r.nexus_mod_id,
            author: r.author,
            website: r.website,
            size_bytes: r.size_bytes,
            installed_at: r.installed_at,
            downloaded_at: r.downloaded_at,
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
    fn import_appends_after_existing_entries() {
        let core = Core::open_in_memory().unwrap();
        let loadout = create(&core, "skyrimse", "Main 2026").unwrap();
        import_mods(
            &core,
            loadout.id,
            &[ImportEntry {
                name: "First".into(),
                source_path: "F:/mods/First".into(),
                enabled: true,
                version: None,
            }],
        )
        .unwrap();
        import_mods(
            &core,
            loadout.id,
            &[ImportEntry {
                name: "Second".into(),
                source_path: "F:/mods/Second".into(),
                enabled: true,
                version: None,
            }],
        )
        .unwrap();
        let rows = entries(&core, loadout.id).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].name, "First");
        assert_eq!(rows[0].position, 0);
        assert_eq!(rows[1].name, "Second");
        assert_eq!(rows[1].position, 1);
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

    #[test]
    fn create_instance_is_independent_and_starts_on_principale() {
        let core = Core::open_in_memory().unwrap();
        let first = create(&core, "skyrimse", "Déjà là").unwrap();
        let created = create_instance(&core, "skyrimse", "Labo météo").unwrap();

        assert_eq!(created.name, INITIAL_BRANCH_NAME);
        assert_eq!(created.instance_name, "Labo météo");
        assert_ne!(created.instance_id, first.instance_id);

        let branches = list(&core).unwrap();
        let own: Vec<_> = branches
            .iter()
            .filter(|b| b.instance_id == created.instance_id)
            .collect();
        assert_eq!(own.len(), 1);
        assert_eq!(own[0].name, "principale");
    }

    #[test]
    fn create_instance_rejects_duplicate_name_same_game() {
        let core = Core::open_in_memory().unwrap();
        create_instance(&core, "minecraft", "Nouvelle instance").unwrap();
        let err = create_instance(&core, "minecraft", "nouvelle instance").unwrap_err();
        assert!(
            err.to_string().contains("déjà ce nom"),
            "expected duplicate name error, got {err}"
        );
        // Same label is fine on another game.
        create_instance(&core, "skyrimse", "Nouvelle instance").unwrap();
    }

    #[test]
    fn changing_mc_game_version_clears_loader_choice() {
        let core = Core::open_in_memory().unwrap();
        let created = create_instance(&core, "minecraft", "Labo").unwrap();
        set_mc_game_version(&core, created.id, "1.21.1").unwrap();
        set_mc_loader(&core, created.id, "fabric").unwrap();
        set_mc_loader_version(&core, created.id, "0.16.0").unwrap();

        set_mc_game_version(&core, created.id, "1.20.1").unwrap();
        let row = list(&core)
            .unwrap()
            .into_iter()
            .find(|b| b.id == created.id)
            .unwrap();
        assert_eq!(row.mc_game_version.as_deref(), Some("1.20.1"));
        assert!(row.mc_loader.is_none());
        assert!(row.mc_loader_version.is_none());
    }

    #[test]
    fn a_branch_stays_inside_its_instance_and_keeps_histories_apart() {
        let core = Core::open_in_memory().unwrap();
        let origin = create(&core, "skyrimse", "Main").unwrap();
        let other = create(&core, "skyrimse", "Autre").unwrap();
        import_mods(
            &core,
            origin.id,
            &[ImportEntry {
                name: "SkyUI".into(),
                source_path: "F:/mods/SkyUI".into(),
                enabled: true,
                version: None,
            }],
        )
        .unwrap();

        let branched = create_branch(&core, origin.id, "essai").unwrap();

        assert_eq!(branched.instance_id, origin.instance_id);
        assert_ne!(branched.instance_id, other.instance_id);
        assert_eq!(entries(&core, branched.id).unwrap().len(), 1);
        assert_eq!(entries(&core, other.id).unwrap().len(), 0);

        crate::transaction::snapshot_loadout(&core, origin.id).unwrap();
        assert_eq!(
            crate::transaction::revision_count(&core, origin.id).unwrap(),
            1
        );
        assert_eq!(
            crate::transaction::revision_count(&core, branched.id).unwrap(),
            0,
            "the new branch does not inherit the source snapshots"
        );

        set_enabled(
            &core,
            branched.id,
            entries(&core, branched.id).unwrap()[0].mod_id,
            false,
        )
        .unwrap();
        assert!(
            entries(&core, origin.id).unwrap()[0].enabled,
            "toggling the branch does not change the source branch"
        );

        delete(&core, origin.id).unwrap();
        let left = list(&core).unwrap();
        assert!(left.iter().all(|b| b.instance_id == other.instance_id));
        assert_eq!(
            left.len(),
            1,
            "both branches of the deleted instance are gone"
        );
    }

    #[test]
    fn refuses_a_second_branch_with_the_same_name() {
        let core = Core::open_in_memory().unwrap();
        let origin = create(&core, "skyrimse", "Main").unwrap();
        let err = create_branch(&core, origin.id, "Main").unwrap_err();
        assert!(err.to_string().contains("déjà ce nom"));
    }
}
