//! Captured downloads — the ledger behind the embedded browser's downloads
//! folder.
//!
//! A capture is deliberately *not* a mod yet. It is a file that arrived, with
//! the URL it came from, sitting in `<data root>/downloads/`. Turning one into
//! a mod row is `attach_to_loadout`, and actually unpacking it is
//! `install_archived` — three separate steps, because the human is allowed to
//! stop after any of them.
//!
//! The Core never downloads anything itself: every row here was created by a
//! client reporting a file the human's own click produced.

use crate::transaction::Transaction;
use crate::{Core, CoreError, Result};
use serde::Serialize;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Download {
    pub id: i64,
    pub url: String,
    pub file_name: String,
    pub path: PathBuf,
    pub size_bytes: Option<i64>,
    pub captured_at: String,
    /// Which loadout it was filed into, `None` while still unfiled.
    pub loadout_id: Option<i64>,
    /// The mod row it became, `None` while still unfiled.
    pub mod_id: Option<i64>,
}

/// What `capture` actually did — a plain new row, or a reuse of one already
/// on file. Callers that redirected a real download onto disk (the embedded
/// browser) need to know which: a dedup means the file they just fetched is
/// now a redundant duplicate of `existing_path` and should be cleaned up.
pub enum CaptureOutcome {
    New(i64),
    Deduped { existing_id: i64, existing_path: PathBuf },
}

impl CaptureOutcome {
    #[must_use]
    pub fn id(&self) -> i64 {
        match self {
            Self::New(id) | Self::Deduped { existing_id: id, .. } => *id,
        }
    }
}

/// Record a file the human downloaded through the embedded browser — unless
/// a capture with the exact same file name and size already exists and its
/// file is still on disk, in which case that existing row is reused rather
/// than filing a near-certain duplicate (the same mod+version, downloaded
/// again for a second instance). The browser still has to fetch the bytes
/// either way — this only prevents ModLab from keeping two copies and two
/// database rows for what is, for bookkeeping purposes, the same file.
pub fn capture(
    core: &Core,
    url: &str,
    file_name: &str,
    path: &Path,
    size_bytes: Option<i64>,
) -> Result<CaptureOutcome> {
    if let Some(size) = size_bytes {
        if let Some((existing_id, existing_path)) =
            core.find_download_by_name_and_size(file_name, size)?
        {
            let existing_path = PathBuf::from(existing_path);
            if existing_path.is_file() {
                return Ok(CaptureOutcome::Deduped {
                    existing_id,
                    existing_path,
                });
            }
        }
    }

    let id = core.insert_download(url, file_name, &path.display().to_string(), size_bytes)?;
    let mut tx = Transaction::new(core, "download.captured");
    tx.step(format!("« {file_name} » capturé depuis {url}"));
    tx.commit()?;
    Ok(CaptureOutcome::New(id))
}

/// Every capture, newest first.
pub fn list(core: &Core) -> Result<Vec<Download>> {
    Ok(core
        .select_downloads()?
        .into_iter()
        .map(
            |(id, url, file_name, path, size_bytes, captured_at, loadout_id, mod_id)| Download {
                id,
                url,
                file_name,
                path: PathBuf::from(path),
                size_bytes,
                captured_at,
                loadout_id,
                mod_id,
            },
        )
        .collect())
}

/// File a capture into a loadout as an *archived* mod: it shows up in the
/// mod list immediately, disabled and not extracted. `name` defaults to the
/// file's own stem — the human renames it later if they care.
pub fn attach_to_loadout(
    core: &Core,
    download_id: i64,
    loadout_id: i64,
    category: Option<&str>,
) -> Result<i64> {
    let (_, file_name, path) = core
        .select_download(download_id)?
        .ok_or_else(|| CoreError::Message(format!("téléchargement {download_id} introuvable")))?;

    let name = Path::new(&file_name)
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| file_name.clone());

    let mod_id = core.insert_archived_mod(&name, &path, None, category)?;
    let position = core.loadout_entry_count(loadout_id)?;
    // Disabled on arrival, always: an archive that has not been unpacked
    // cannot deploy anything, so enabling it would be a lie.
    core.insert_loadout_entry(loadout_id, mod_id, false, position)?;
    core.set_download_target(download_id, loadout_id, mod_id)?;

    let mut tx = Transaction::new(core, "download.attached");
    tx.step(format!(
        "« {name} » ajouté au loadout {loadout_id} (archivé, désactivé)"
    ));
    tx.commit()?;
    Ok(mod_id)
}

/// Move an archived mod to a different loadout. Only ever archived: nothing
/// has been extracted yet, so nothing about the mod's files changes — only
/// which instance's mod list it shows up in. An installed mod's files live
/// under a real deployed path a specific loadout's table already reflects;
/// moving that is a different, bigger feature than this.
pub fn reassign(core: &Core, mod_id: i64, from_loadout_id: i64, to_loadout_id: i64) -> Result<()> {
    let (name, status, _) = core
        .select_mod_archive(mod_id)?
        .ok_or_else(|| CoreError::Message(format!("mod {mod_id} introuvable")))?;
    if status != crate::loadout::STATUS_ARCHIVED {
        return Err(CoreError::Message(format!(
            "« {name} » est déjà installé — impossible de le réassigner à une autre instance"
        )));
    }
    core.move_loadout_entry(mod_id, from_loadout_id, to_loadout_id)?;
    let mut tx = Transaction::new(core, "download.reassigned");
    tx.step(format!(
        "« {name} » déplacé du loadout {from_loadout_id} vers {to_loadout_id}"
    ));
    tx.commit()?;
    Ok(())
}

/// Re-tag a mod. `None` clears the tag.
pub fn set_category(core: &Core, mod_id: i64, category: Option<&str>) -> Result<()> {
    let cleaned = category.map(str::trim).filter(|c| !c.is_empty());
    core.set_mod_category(mod_id, cleaned)?;
    Ok(())
}

/// Every tag already in use — what the category input autocompletes from.
pub fn categories(core: &Core) -> Result<Vec<String>> {
    core.select_mod_categories()
}

/// What `install_archived` needs to know before unpacking.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArchivedMod {
    pub name: String,
    pub archive_path: PathBuf,
}

/// Read back an archived mod, refusing anything that is not actually
/// archived — installing twice would extract over a live folder.
pub fn archived_mod(core: &Core, mod_id: i64) -> Result<ArchivedMod> {
    let (name, status, archive_path) = core
        .select_mod_archive(mod_id)?
        .ok_or_else(|| CoreError::Message(format!("mod {mod_id} introuvable")))?;
    if status != crate::loadout::STATUS_ARCHIVED {
        return Err(CoreError::Message(format!(
            "« {name} » n'est pas une archive en attente (statut : {status})"
        )));
    }
    let archive_path = archive_path.ok_or_else(|| {
        CoreError::Message(format!("« {name} » n'a pas d'archive source enregistrée"))
    })?;
    Ok(ArchivedMod {
        name,
        archive_path: PathBuf::from(archive_path),
    })
}

/// Mark an archived mod as installed, now that `source_path` holds its
/// extracted files. The extraction itself belongs to whoever knows the game's
/// layout — the Core only records that it happened.
pub fn mark_installed(core: &Core, mod_id: i64, source_path: &Path) -> Result<()> {
    core.set_mod_installed(mod_id, &source_path.display().to_string())?;
    let mut tx = Transaction::new(core, "download.installed");
    tx.step(format!(
        "mod {mod_id} extrait dans {}",
        source_path.display()
    ));
    tx.commit()?;
    Ok(())
}

/// Rename a mod.
pub fn rename(core: &Core, mod_id: i64, name: &str) -> Result<()> {
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
    core.set_mod_name(mod_id, name)?;
    let mut tx = Transaction::new(core, "mod.rename");
    tx.step(format!("mod {mod_id} renommé en « {name} »"));
    tx.commit()?;
    Ok(())
}

/// Soft-delete a mod: it disappears from every loadout table it's in, but
/// nothing on disk (extracted files, the archive) or in its own history is
/// destroyed — per the project's "internal trash, never real deletion" rule.
/// There is no restore path exposed yet (unlike a loadout's trash), only the
/// safety guarantee that this never throws bytes away.
pub fn remove(core: &Core, mod_id: i64) -> Result<()> {
    let (name, ..) = core
        .select_mod_archive(mod_id)?
        .ok_or_else(|| CoreError::Message(format!("mod {mod_id} introuvable")))?;
    core.soft_delete_mod(mod_id)?;
    let mut tx = Transaction::new(core, "mod.remove");
    tx.step(format!("« {name} » retiré de la liste (récupérable)"));
    tx.commit()?;
    Ok(())
}

/// Revert an installed mod back to `archived`, ready to be re-extracted —
/// the caller is responsible for actually clearing the extracted copy on
/// disk first (a plain derived folder, not the mod's real asset; the archive
/// is). Refuses anything not currently installed, and anything with no
/// archive to fall back to — reverting would otherwise leave a mod with no
/// usable source at all.
pub fn uninstall(core: &Core, mod_id: i64) -> Result<()> {
    let (name, status, archive_path) = core
        .select_mod_archive(mod_id)?
        .ok_or_else(|| CoreError::Message(format!("mod {mod_id} introuvable")))?;
    if status != crate::loadout::STATUS_INSTALLED {
        return Err(CoreError::Message(format!(
            "« {name} » n'est pas installé (statut : {status})"
        )));
    }
    if archive_path.is_none() {
        return Err(CoreError::Message(format!(
            "« {name} » n'a pas d'archive source enregistrée — rien à quoi revenir"
        )));
    }
    core.set_mod_archived(mod_id)?;
    let mut tx = Transaction::new(core, "mod.uninstall");
    tx.step(format!("« {name} » désinstallé, revenu à l'état d'archive"));
    tx.commit()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::loadout;

    #[test]
    fn captures_a_download_and_lists_it_unfiled() {
        let core = Core::open_in_memory().unwrap();

        let id = capture(
            &core,
            "https://example.test/lux.7z",
            "lux.7z",
            Path::new("F:/ModLab/downloads/lux.7z"),
            Some(1234),
        )
        .unwrap()
        .id();

        let all = list(&core).unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].id, id);
        assert_eq!(all[0].file_name, "lux.7z");
        assert_eq!(all[0].size_bytes, Some(1234));
        assert_eq!(all[0].loadout_id, None, "unfiled until attached");
        assert_eq!(all[0].mod_id, None);
    }

    #[test]
    fn recapturing_the_same_name_and_size_dedupes_to_the_existing_row() {
        let core = Core::open_in_memory().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let first_path = dir.path().join("lux.7z");
        std::fs::write(&first_path, b"seven-bytes").unwrap();

        let first = capture(&core, "https://example.test/x", "lux.7z", &first_path, Some(11)).unwrap();
        assert!(matches!(first, CaptureOutcome::New(_)));

        // A second instance's browser downloads the same file again, to a
        // different path.
        let second_path = dir.path().join("lux (2).7z");
        std::fs::write(&second_path, b"seven-bytes").unwrap();
        let second =
            capture(&core, "https://example.test/x", "lux.7z", &second_path, Some(11)).unwrap();

        match second {
            CaptureOutcome::Deduped { existing_id, existing_path } => {
                assert_eq!(existing_id, first.id());
                assert_eq!(existing_path, first_path);
            }
            CaptureOutcome::New(_) => panic!("expected a dedup, got a new row"),
        }
        assert_eq!(list(&core).unwrap().len(), 1, "no second row was filed");
    }

    #[test]
    fn recapturing_after_the_original_file_is_gone_files_a_new_row() {
        let core = Core::open_in_memory().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let first_path = dir.path().join("lux.7z");
        std::fs::write(&first_path, b"seven-bytes").unwrap();
        capture(&core, "https://example.test/x", "lux.7z", &first_path, Some(11)).unwrap();
        std::fs::remove_file(&first_path).unwrap();

        let second_path = dir.path().join("lux (2).7z");
        std::fs::write(&second_path, b"seven-bytes").unwrap();
        let second =
            capture(&core, "https://example.test/x", "lux.7z", &second_path, Some(11)).unwrap();

        assert!(
            matches!(second, CaptureOutcome::New(_)),
            "the original file is gone, so there's nothing to dedup against"
        );
        assert_eq!(list(&core).unwrap().len(), 2);
    }

    #[test]
    fn captures_with_no_known_size_never_dedup() {
        let core = Core::open_in_memory().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("lux.7z");
        std::fs::write(&path, b"x").unwrap();

        capture(&core, "https://example.test/x", "lux.7z", &path, None).unwrap();
        let second = capture(&core, "https://example.test/x", "lux.7z", &path, None).unwrap();

        assert!(matches!(second, CaptureOutcome::New(_)));
        assert_eq!(list(&core).unwrap().len(), 2);
    }

    #[test]
    fn attaching_shows_the_mod_archived_and_disabled() {
        let core = Core::open_in_memory().unwrap();
        let lo = loadout::create(&core, "skyrimse", "Main").unwrap();
        let dl = capture(
            &core,
            "https://example.test/lux.7z",
            "lux.7z",
            Path::new("F:/ModLab/downloads/lux.7z"),
            None,
        )
        .unwrap()
        .id();

        let mod_id = attach_to_loadout(&core, dl, lo.id, Some("Éclairage")).unwrap();

        let rows = loadout::entries(&core, lo.id).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].mod_id, mod_id);
        assert_eq!(rows[0].name, "lux", "named after the file stem");
        assert_eq!(rows[0].status, loadout::STATUS_ARCHIVED);
        assert!(!rows[0].enabled, "an unextracted archive is never enabled");
        assert_eq!(rows[0].category.as_deref(), Some("Éclairage"));

        let filed = &list(&core).unwrap()[0];
        assert_eq!(filed.loadout_id, Some(lo.id));
        assert_eq!(filed.mod_id, Some(mod_id));
    }

    #[test]
    fn installing_flips_the_status_and_repoints_the_source() {
        let core = Core::open_in_memory().unwrap();
        let lo = loadout::create(&core, "skyrimse", "Main").unwrap();
        let dl = capture(
            &core,
            "https://example.test/lux.7z",
            "lux.7z",
            Path::new("F:/ModLab/downloads/lux.7z"),
            None,
        )
        .unwrap()
        .id();
        let mod_id = attach_to_loadout(&core, dl, lo.id, None).unwrap();

        let pending = archived_mod(&core, mod_id).unwrap();
        assert_eq!(pending.archive_path, PathBuf::from("F:/ModLab/downloads/lux.7z"));

        mark_installed(&core, mod_id, Path::new("F:/ModLab/mods/lux")).unwrap();

        let rows = loadout::entries(&core, lo.id).unwrap();
        assert_eq!(rows[0].status, loadout::STATUS_INSTALLED);
        assert_eq!(rows[0].source_path, PathBuf::from("F:/ModLab/mods/lux"));
    }

    #[test]
    fn installing_an_already_installed_mod_is_refused() {
        let core = Core::open_in_memory().unwrap();
        let lo = loadout::create(&core, "skyrimse", "Main").unwrap();
        let dl = capture(
            &core,
            "https://example.test/lux.7z",
            "lux.7z",
            Path::new("F:/ModLab/downloads/lux.7z"),
            None,
        )
        .unwrap()
        .id();
        let mod_id = attach_to_loadout(&core, dl, lo.id, None).unwrap();
        mark_installed(&core, mod_id, Path::new("F:/ModLab/mods/lux")).unwrap();

        let err = archived_mod(&core, mod_id).unwrap_err();

        assert!(err.to_string().contains("installed"), "{err}");
    }

    #[test]
    fn reassigning_an_archived_mod_moves_it_to_the_other_loadout() {
        let core = Core::open_in_memory().unwrap();
        let a = loadout::create(&core, "skyrimse", "A").unwrap();
        let b = loadout::create(&core, "skyrimse", "B").unwrap();
        let dl = capture(&core, "https://example.test/x", "lux.7z", Path::new("lux.7z"), None).unwrap().id();
        let mod_id = attach_to_loadout(&core, dl, a.id, None).unwrap();

        reassign(&core, mod_id, a.id, b.id).unwrap();

        assert_eq!(loadout::entries(&core, a.id).unwrap().len(), 0, "gone from A");
        let in_b = loadout::entries(&core, b.id).unwrap();
        assert_eq!(in_b.len(), 1, "landed in B");
        assert_eq!(in_b[0].mod_id, mod_id);
        assert!(!in_b[0].enabled, "still disabled — still not extracted");
    }

    #[test]
    fn reassigning_an_installed_mod_is_refused() {
        let core = Core::open_in_memory().unwrap();
        let a = loadout::create(&core, "skyrimse", "A").unwrap();
        let b = loadout::create(&core, "skyrimse", "B").unwrap();
        let dl = capture(&core, "https://example.test/x", "lux.7z", Path::new("lux.7z"), None).unwrap().id();
        let mod_id = attach_to_loadout(&core, dl, a.id, None).unwrap();
        mark_installed(&core, mod_id, Path::new("F:/ModLab/mods/lux")).unwrap();

        let err = reassign(&core, mod_id, a.id, b.id).unwrap_err();

        assert!(err.to_string().contains("installé"), "{err}");
        assert_eq!(loadout::entries(&core, a.id).unwrap().len(), 1, "untouched");
    }

    #[test]
    fn renaming_a_mod_updates_its_entry() {
        let core = Core::open_in_memory().unwrap();
        let lo = loadout::create(&core, "skyrimse", "Main").unwrap();
        let dl = capture(&core, "https://example.test/x", "lux.7z", Path::new("lux.7z"), None).unwrap().id();
        let mod_id = attach_to_loadout(&core, dl, lo.id, None).unwrap();

        rename(&core, mod_id, "  Lux Orbis  ").unwrap();

        assert_eq!(loadout::entries(&core, lo.id).unwrap()[0].name, "Lux Orbis");
    }

    #[test]
    fn renaming_to_an_empty_name_is_refused() {
        let core = Core::open_in_memory().unwrap();
        let lo = loadout::create(&core, "skyrimse", "Main").unwrap();
        let dl = capture(&core, "https://example.test/x", "lux.7z", Path::new("lux.7z"), None).unwrap().id();
        let mod_id = attach_to_loadout(&core, dl, lo.id, None).unwrap();

        let err = rename(&core, mod_id, "   ").unwrap_err();

        assert!(err.to_string().contains("vide"), "{err}");
        assert_eq!(loadout::entries(&core, lo.id).unwrap()[0].name, "lux");
    }

    #[test]
    fn removing_a_mod_hides_it_but_keeps_its_row() {
        let core = Core::open_in_memory().unwrap();
        let lo = loadout::create(&core, "skyrimse", "Main").unwrap();
        let dl = capture(&core, "https://example.test/x", "lux.7z", Path::new("lux.7z"), None).unwrap().id();
        let mod_id = attach_to_loadout(&core, dl, lo.id, None).unwrap();

        remove(&core, mod_id).unwrap();

        assert_eq!(loadout::entries(&core, lo.id).unwrap().len(), 0, "hidden from the table");
        // Not actually gone: the archive lookup (which ignores deleted_at)
        // still finds the row.
        assert!(core.select_mod_archive(mod_id).unwrap().is_some(), "row still exists");
    }

    #[test]
    fn uninstalling_reverts_to_archived_and_repoints_the_source() {
        let core = Core::open_in_memory().unwrap();
        let lo = loadout::create(&core, "skyrimse", "Main").unwrap();
        let dl = capture(
            &core,
            "https://example.test/x",
            "lux.7z",
            Path::new("F:/ModLab/downloads/lux.7z"),
            None,
        )
        .unwrap()
        .id();
        let mod_id = attach_to_loadout(&core, dl, lo.id, None).unwrap();
        mark_installed(&core, mod_id, Path::new("F:/ModLab/mods/lux")).unwrap();

        uninstall(&core, mod_id).unwrap();

        let rows = loadout::entries(&core, lo.id).unwrap();
        assert_eq!(rows[0].status, loadout::STATUS_ARCHIVED);
        assert_eq!(rows[0].source_path, PathBuf::from("F:/ModLab/downloads/lux.7z"));
    }

    #[test]
    fn uninstalling_an_archived_mod_is_refused() {
        let core = Core::open_in_memory().unwrap();
        let lo = loadout::create(&core, "skyrimse", "Main").unwrap();
        let dl = capture(&core, "https://example.test/x", "lux.7z", Path::new("lux.7z"), None).unwrap().id();
        let mod_id = attach_to_loadout(&core, dl, lo.id, None).unwrap();

        let err = uninstall(&core, mod_id).unwrap_err();

        assert!(err.to_string().contains("installé"), "{err}");
    }

    #[test]
    fn categories_are_deduplicated_and_shared_across_games() {
        let core = Core::open_in_memory().unwrap();
        let sky = loadout::create(&core, "skyrimse", "Main").unwrap();
        let mc = loadout::create(&core, "minecraft", "Survie").unwrap();
        for (name, lo, cat) in [
            ("a.7z", sky.id, Some("Visuel")),
            ("b.7z", mc.id, Some("Visuel")),
            ("c.7z", sky.id, Some("Gameplay")),
            ("d.7z", sky.id, None),
        ] {
            let dl = capture(&core, "https://example.test/x", name, Path::new(name), None).unwrap().id();
            attach_to_loadout(&core, dl, lo, cat).unwrap();
        }

        assert_eq!(categories(&core).unwrap(), vec!["Gameplay", "Visuel"]);
    }
}
