//! Install and uninstall a mod's files, through the transaction cycle.
//!
//! The Core does not know which game the files belong to. The caller
//! prepares the tree (an adapter knows the layout). This module swaps that
//! tree into place, records the row, and puts both back if verification
//! fails. Removed files go to the internal trash — they are not deleted.

use std::path::{Path, PathBuf};

use crate::files::{self, DirSwap};
use crate::loadout::STATUS_ARCHIVED;
use crate::loadout::STATUS_INSTALLED;
use crate::transaction::Plan;
use crate::{Core, CoreError, Result};

/// Optional label written in the same transaction as the install, so a
/// failure puts the previous name and version back with the files.
#[derive(Debug, Clone, Default)]
pub struct InstallAdjustments {
    /// New display name. `None`, empty, or equal to the current name: no rename.
    pub rename_to: Option<String>,
    /// Stored only when the row has no version yet.
    pub version_if_empty: Option<String>,
}

/// Extract/copy into a sibling directory, rename it onto `dest`, then mark
/// the mod installed. A failure at any point restores `dest` and the row.
pub fn commit_install(
    core: &Core,
    data_root: &Path,
    loadout_id: i64,
    mod_id: i64,
    dest: &Path,
    adjustments: InstallAdjustments,
    prepare: impl FnOnce(&Path) -> Result<()>,
) -> Result<()> {
    commit_install_with(
        core,
        data_root,
        loadout_id,
        mod_id,
        dest,
        adjustments,
        prepare,
        |_| Ok(()),
    )
}

/// `after_write` runs after the row is marked installed and before
/// verification. Tests use it to force the failure path; production passes
/// a no-op via [`commit_install`].
struct ArchivedSnapshot {
    name: String,
    version: Option<String>,
    field_sources: Option<String>,
}

fn read_archived_snapshot(core: &Core, mod_id: i64) -> Result<ArchivedSnapshot> {
    let (previous_name, status, _) = core
        .select_mod_archive(mod_id)?
        .ok_or_else(|| CoreError::Message(format!("mod {mod_id} introuvable")))?;
    if status != STATUS_ARCHIVED {
        return Err(CoreError::Message(format!(
            "« {previous_name} » n'est pas une archive en attente (statut : {status})"
        )));
    }
    let field_sources = core
        .select_mod_sheet(mod_id)?
        .and_then(|sheet| sheet.field_sources);
    Ok(ArchivedSnapshot {
        name: previous_name,
        version: core.select_mod_version(mod_id)?,
        field_sources,
    })
}

#[allow(clippy::too_many_arguments)]
fn commit_install_with(
    core: &Core,
    data_root: &Path,
    loadout_id: i64,
    mod_id: i64,
    dest: &Path,
    adjustments: InstallAdjustments,
    prepare: impl FnOnce(&Path) -> Result<()>,
    after_write: impl FnOnce(&Core) -> Result<()>,
) -> Result<()> {
    let snapshot = read_archived_snapshot(core, mod_id)?;
    let swap = DirSwap::replace_with(data_root, dest, prepare)?;
    let outcome = apply_install(
        core,
        loadout_id,
        mod_id,
        dest,
        &snapshot.name,
        &adjustments,
        after_write,
        false,
    );
    finish_swap(core, mod_id, &snapshot, swap, outcome)
}

/// Record an install whose tree is already in place (`swap` comes from
/// [`crate::files::DirSwap::replace_with`]). The status is read again here,
/// so a caller can build the tree without holding the database lock and
/// still refuse the commit if the mod is no longer an archive. A refused
/// commit drops `swap`, which puts the previous folder back.
pub fn commit_prepared_install(
    core: &Core,
    loadout_id: i64,
    mod_id: i64,
    dest: &Path,
    adjustments: InstallAdjustments,
    swap: DirSwap,
) -> Result<()> {
    let snapshot = match read_archived_snapshot(core, mod_id) {
        Ok(snapshot) => snapshot,
        Err(err) => {
            drop(swap);
            return Err(err);
        }
    };
    let outcome = apply_install(
        core,
        loadout_id,
        mod_id,
        dest,
        &snapshot.name,
        &adjustments,
        |_| Ok(()),
        false,
    );
    finish_swap(core, mod_id, &snapshot, swap, outcome)
}

/// Record an install whose file is already in place. `dest` may be a file
/// (a jar dropped into a mods folder) or a directory. A failed verification
/// puts the row back; the caller moves the new file to the trash.
pub fn commit_placed_path(
    core: &Core,
    loadout_id: i64,
    mod_id: i64,
    dest: &Path,
    adjustments: InstallAdjustments,
) -> Result<()> {
    let snapshot = read_archived_snapshot(core, mod_id)?;
    let outcome = apply_install(
        core,
        loadout_id,
        mod_id,
        dest,
        &snapshot.name,
        &adjustments,
        |_| Ok(()),
        true,
    );
    match outcome {
        Ok(()) => Ok(()),
        Err(err) => {
            let _ = core.set_mod_name(mod_id, &snapshot.name);
            let _ = core.set_mod_version(mod_id, snapshot.version.as_deref());
            let _ = core.set_field_sources(mod_id, snapshot.field_sources.as_deref());
            let _ = core.set_mod_archived(mod_id);
            Err(err)
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn apply_install(
    core: &Core,
    loadout_id: i64,
    mod_id: i64,
    dest: &Path,
    previous_name: &str,
    adjustments: &InstallAdjustments,
    after_write: impl FnOnce(&Core) -> Result<()>,
    allow_file: bool,
) -> Result<()> {
    let mut plan = Plan::new(core, loadout_id, "download.installed");
    plan.step(format!("mod {mod_id} extrait dans {}", dest.display()));
    if let Some(name) = adjustments.rename_to.as_deref().map(str::trim) {
        if !name.is_empty() && name != previous_name {
            plan.step(format!("mod {mod_id} renommé en « {name} »"));
        }
    }
    if adjustments.version_if_empty.as_deref().is_some() {
        plan.step(format!(
            "version du mod {mod_id} complétée si elle était vide"
        ));
    }
    let readable = plan.describe();
    if readable.is_empty() || readable.iter().any(|step| step.trim().is_empty()) {
        return Err(CoreError::Message(
            "plan d'installation illisible".to_string(),
        ));
    }

    let dest_str = dest.display().to_string();
    let rename_to = adjustments.rename_to.clone();
    let version_if_empty = adjustments.version_if_empty.clone();
    let previous = previous_name.to_string();
    plan.approve().apply(
        |core| {
            if let Some(name) = rename_to.as_deref().map(str::trim) {
                if !name.is_empty() && name != previous {
                    if name.chars().count() > 100 {
                        return Err(CoreError::Message(
                            "le nom ne peut pas dépasser 100 caractères".to_string(),
                        ));
                    }
                    crate::download::lock_recognized_name(core, mod_id, name)?;
                }
            }
            if let Some(version) = version_if_empty.as_deref() {
                crate::download::set_tool_version_if_empty(core, mod_id, version)?;
            }
            core.set_mod_installed(mod_id, &dest_str)?;
            after_write(core)?;
            Ok(())
        },
        |core, _| verify_installed(core, mod_id, dest, allow_file),
    )
}

fn verify_installed(core: &Core, mod_id: i64, dest: &Path, allow_file: bool) -> Result<()> {
    let on_disk = if allow_file {
        dest.exists()
    } else {
        dest.is_dir()
    };
    let row = core.select_mod_archive(mod_id)?;
    match row {
        Some((_, status, _)) if status == STATUS_INSTALLED && on_disk => Ok(()),
        _ => Err(CoreError::Message(
            "vérification échouée : le mod n'est pas installé sur le disque et dans la base"
                .to_string(),
        )),
    }
}

fn finish_swap(
    core: &Core,
    mod_id: i64,
    previous: &ArchivedSnapshot,
    swap: DirSwap,
    outcome: Result<()>,
) -> Result<()> {
    match outcome {
        Ok(()) => {
            swap.commit();
            Ok(())
        }
        Err(err) => {
            // Drop rolls the files back before the row is restored, so a
            // crash between the two still leaves the old tree on disk.
            drop(swap);
            let _ = core.set_mod_name(mod_id, &previous.name);
            let _ = core.set_mod_version(mod_id, previous.version.as_deref());
            let _ = core.set_field_sources(mod_id, previous.field_sources.as_deref());
            let _ = core.set_mod_archived(mod_id);
            Err(err)
        }
    }
}

/// Move `dest` to the trash and mark the mod archived again. If the database
/// update or its verification fails, the folder is renamed back.
/// Returns the trash path when a folder was actually moved.
pub fn uninstall_mod(
    core: &Core,
    data_root: &Path,
    loadout_id: i64,
    mod_id: i64,
    dest: &Path,
) -> Result<Option<PathBuf>> {
    let (name, status, archive_path) = core
        .select_mod_archive(mod_id)?
        .ok_or_else(|| CoreError::Message(format!("mod {mod_id} introuvable")))?;
    if status != STATUS_INSTALLED {
        return Err(CoreError::Message(format!(
            "« {name} » n'est pas installé (statut : {status})"
        )));
    }
    if archive_path.is_none() {
        return Err(CoreError::Message(format!(
            "« {name} » n'a pas d'archive source enregistrée — rien à quoi revenir"
        )));
    }
    let previous_source = core.select_mod_source_path(mod_id)?;

    let guard = files::trash_for_rollback(data_root, dest)?;
    let trashed = guard.as_ref().map(|g| g.trashed_path().to_path_buf());

    let mut plan = Plan::new(core, loadout_id, "mod.uninstall");
    plan.step(format!("« {name} » désinstallé, revenu à l'état d'archive"));
    if trashed.is_some() {
        plan.step(format!(
            "dossier {} déplacé dans la corbeille",
            dest.display()
        ));
    }
    let readable = plan.describe();
    if readable.is_empty() {
        drop(guard);
        return Err(CoreError::Message(
            "plan de désinstallation illisible".to_string(),
        ));
    }

    let outcome = plan.approve().apply(
        |core| {
            core.set_mod_archived(mod_id)?;
            Ok(())
        },
        |core, _| {
            let row = core.select_mod_archive(mod_id)?;
            match row {
                Some((_, status, _)) if status == STATUS_ARCHIVED => Ok(()),
                _ => Err(CoreError::Message(
                    "vérification échouée : le mod n'est pas revenu à l'état d'archive".to_string(),
                )),
            }
        },
    );

    match outcome {
        Ok(()) => {
            if let Some(guard) = guard {
                guard.commit();
            }
            Ok(trashed)
        }
        Err(err) => {
            drop(guard);
            if let Some(source) = previous_source.as_deref() {
                let _ = core.set_mod_installed(mod_id, source);
            }
            Err(err)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::download::{attach_to_loadout, capture};
    use crate::loadout;
    use std::fs;

    fn scratch(tag: &str) -> tempfile::TempDir {
        tempfile::Builder::new()
            .prefix(&format!("modlab-install-{tag}-"))
            .tempdir()
            .unwrap()
    }

    fn archived_mod(core: &Core, loadout_id: i64, archive: &Path) -> i64 {
        let dl = capture(
            core,
            "https://example.test/mod.zip",
            "mod.zip",
            archive,
            None,
        )
        .unwrap()
        .id();
        attach_to_loadout(core, dl, loadout_id, None).unwrap()
    }

    fn game_fixture(root: &Path) -> PathBuf {
        let data = root.join("Game").join("Data");
        fs::create_dir_all(&data).unwrap();
        fs::write(data.join("Skyrim.esm"), b"VANILLA").unwrap();
        data
    }

    #[test]
    fn a_failed_install_leaves_no_partial_mod_and_an_unchanged_game_tree() {
        let root = scratch("fail");
        let data = game_fixture(root.path());
        let vanilla = fs::read(data.join("Skyrim.esm")).unwrap();
        let core = Core::open_in_memory().unwrap();
        let lo = loadout::create(&core, "fixture", "Test").unwrap();
        let archive = root.path().join("mod.zip");
        fs::write(&archive, b"zip").unwrap();
        let mod_id = archived_mod(&core, lo.id, &archive);
        let dest = root.path().join("mods").join(mod_id.to_string());

        let err = commit_install_with(
            &core,
            root.path(),
            lo.id,
            mod_id,
            &dest,
            InstallAdjustments::default(),
            |staged| {
                fs::create_dir_all(staged)?;
                fs::write(staged.join("half.txt"), b"partial")?;
                Err(CoreError::Message("coupé au milieu".into()))
            },
            |_| Ok(()),
        );

        assert!(err.is_err());
        assert!(!dest.join("half.txt").exists());
        assert!(
            !dest.exists() || fs::read_dir(&dest).unwrap().next().is_none(),
            "the destination holds no partial install"
        );
        let row = loadout::entries(&core, lo.id).unwrap();
        assert_eq!(row[0].status, STATUS_ARCHIVED);
        assert_eq!(fs::read(data.join("Skyrim.esm")).unwrap(), vanilla);
    }

    #[test]
    fn a_refused_verification_restores_files_and_the_archived_status() {
        let root = scratch("verify");
        let data = game_fixture(root.path());
        let core = Core::open_in_memory().unwrap();
        let lo = loadout::create(&core, "fixture", "Test").unwrap();
        let archive = root.path().join("mod.zip");
        fs::write(&archive, b"zip").unwrap();
        let mod_id = archived_mod(&core, lo.id, &archive);
        let dest = root.path().join("mods").join(mod_id.to_string());

        let err = commit_install_with(
            &core,
            root.path(),
            lo.id,
            mod_id,
            &dest,
            InstallAdjustments::default(),
            |staged| {
                fs::create_dir_all(staged)?;
                fs::write(staged.join("meshes.txt"), b"ok")?;
                Ok(())
            },
            |_| Err(CoreError::Message("vérification refusée".into())),
        );

        assert!(err.is_err());
        assert!(
            !dest.exists(),
            "the installed tree is rolled back, not left half-applied"
        );
        assert_eq!(
            loadout::entries(&core, lo.id).unwrap()[0].status,
            STATUS_ARCHIVED
        );
        assert_eq!(fs::read(data.join("Skyrim.esm")).unwrap(), b"VANILLA");
    }

    #[test]
    fn uninstall_keeps_the_files_in_the_trash_and_the_game_tree() {
        let root = scratch("uninstall");
        let data = game_fixture(root.path());
        let core = Core::open_in_memory().unwrap();
        let lo = loadout::create(&core, "fixture", "Test").unwrap();
        let archive = root.path().join("mod.zip");
        fs::write(&archive, b"zip").unwrap();
        let mod_id = archived_mod(&core, lo.id, &archive);
        let dest = root.path().join("mods").join(mod_id.to_string());

        commit_install(
            &core,
            root.path(),
            lo.id,
            mod_id,
            &dest,
            InstallAdjustments::default(),
            |staged| {
                fs::create_dir_all(staged.join("meshes"))?;
                fs::write(staged.join("meshes").join("a.nif"), b"mesh")?;
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(
            fs::read(dest.join("meshes").join("a.nif")).unwrap(),
            b"mesh"
        );

        let trashed = uninstall_mod(&core, root.path(), lo.id, mod_id, &dest)
            .unwrap()
            .expect("the folder was moved");
        assert!(!dest.exists());
        assert_eq!(
            fs::read(trashed.join("meshes").join("a.nif")).unwrap(),
            b"mesh"
        );
        assert_eq!(
            loadout::entries(&core, lo.id).unwrap()[0].status,
            STATUS_ARCHIVED
        );
        assert_eq!(fs::read(data.join("Skyrim.esm")).unwrap(), b"VANILLA");
    }

    #[test]
    fn a_prepared_tree_is_put_back_when_the_mod_is_no_longer_archived() {
        let root = scratch("prepared-stale");
        let core = Core::open_in_memory().unwrap();
        let lo = loadout::create(&core, "fixture", "Test").unwrap();
        let archive = root.path().join("mod.zip");
        fs::write(&archive, b"zip").unwrap();
        let mod_id = archived_mod(&core, lo.id, &archive);
        let dest = root.path().join("mods").join(mod_id.to_string());
        fs::create_dir_all(&dest).unwrap();
        fs::write(dest.join("old.txt"), b"old").unwrap();

        let swap = DirSwap::replace_with(root.path(), &dest, |staged| {
            fs::create_dir_all(staged)?;
            fs::write(staged.join("new.txt"), b"new")?;
            Ok(())
        })
        .unwrap();
        assert_eq!(fs::read(dest.join("new.txt")).unwrap(), b"new");
        core.set_mod_installed(mod_id, &dest.display().to_string())
            .unwrap();

        let err = commit_prepared_install(
            &core,
            lo.id,
            mod_id,
            &dest,
            InstallAdjustments::default(),
            swap,
        );
        assert!(err.is_err());
        assert_eq!(fs::read(dest.join("old.txt")).unwrap(), b"old");
        assert!(!dest.join("new.txt").exists());
        assert_eq!(
            loadout::entries(&core, lo.id).unwrap()[0].status,
            STATUS_INSTALLED
        );
    }
}
