//! Byte size and install time stored on `mod_`.
//!
//! The size is the mod file (downloaded archive or Minecraft jar), not the
//! extracted tree. It is written when that file is known and kept after the
//! file is removed. A folder sum is only a fallback for a mod that never had
//! an archive, and only when one mod is refreshed — never while listing a
//! branch.
//!
//! `installed_at` is the moment the mod became installed. Turning it off and
//! on does not change that moment. Installing it again does.
//!
//! `downloaded_at` is when the mod was obtained. A download writes it when
//! the row appears; an import writes the source file's modification time,
//! or the import time when that file has none. It stays after the archive
//! is deleted. A capture attached later replaces it. Rows that predate the
//! column are filled here: the linked capture, else the archive's
//! modification time, else `installed_at`.

use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::OptionalExtension;
use serde::Deserialize;

use crate::loadout::STATUS_INSTALLED;
use crate::{Core, Result};

/// One mod on a branch that is still missing a size, an install time, a
/// download time, or some of those.
struct MeasureGap {
    mod_id: i64,
    status: String,
    source_path: String,
    archive_path: Option<String>,
    size_bytes: Option<i64>,
    installed_at: Option<String>,
    downloaded_at: Option<String>,
}

struct ModMeasure {
    status: String,
    source_path: String,
    archive_path: Option<String>,
    size_bytes: Option<i64>,
    installed_at: Option<String>,
    downloaded_at: Option<String>,
}

#[derive(Deserialize)]
struct Committed {
    kind: String,
    #[serde(default)]
    steps: Vec<String>,
}

impl Core {
    pub(crate) fn set_mod_size(&self, mod_id: i64, size_bytes: i64) -> Result<()> {
        self.db.execute(
            "UPDATE mod_ SET size_bytes = ?1 WHERE id = ?2",
            (size_bytes, mod_id),
        )?;
        Ok(())
    }

    pub(crate) fn set_mod_size_if_empty(&self, mod_id: i64, size_bytes: i64) -> Result<()> {
        self.db.execute(
            "UPDATE mod_ SET size_bytes = ?1 WHERE id = ?2 AND size_bytes IS NULL",
            (size_bytes, mod_id),
        )?;
        Ok(())
    }

    fn set_mod_size_for_source(&self, source_path: &str, size_bytes: i64) -> Result<()> {
        self.db.execute(
            "UPDATE mod_ SET size_bytes = ?1 \
             WHERE source_path = ?2 AND size_bytes IS NULL AND deleted_at IS NULL",
            (size_bytes, source_path),
        )?;
        Ok(())
    }

    pub(crate) fn touch_installed_at(&self, mod_id: i64) -> Result<()> {
        self.db.execute(
            "UPDATE mod_ SET installed_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') \
             WHERE id = ?1",
            [mod_id],
        )?;
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn set_installed_at(&self, mod_id: i64, at: Option<&str>) -> Result<()> {
        self.db.execute(
            "UPDATE mod_ SET installed_at = ?1 WHERE id = ?2",
            (at, mod_id),
        )?;
        Ok(())
    }

    fn set_installed_at_if_empty(&self, mod_id: i64, at: &str) -> Result<()> {
        self.db.execute(
            "UPDATE mod_ SET installed_at = ?1 WHERE id = ?2 AND installed_at IS NULL",
            (at, mod_id),
        )?;
        Ok(())
    }

    pub(crate) fn set_downloaded_at(&self, mod_id: i64, at: &str) -> Result<()> {
        self.db.execute(
            "UPDATE mod_ SET downloaded_at = ?1 WHERE id = ?2",
            (at, mod_id),
        )?;
        Ok(())
    }

    pub(crate) fn touch_downloaded_at(&self, mod_id: i64) -> Result<()> {
        self.db.execute(
            "UPDATE mod_ SET downloaded_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') \
             WHERE id = ?1",
            [mod_id],
        )?;
        Ok(())
    }

    fn set_downloaded_at_if_empty(&self, mod_id: i64, at: &str) -> Result<()> {
        self.db.execute(
            "UPDATE mod_ SET downloaded_at = ?1 WHERE id = ?2 AND downloaded_at IS NULL",
            (at, mod_id),
        )?;
        Ok(())
    }

    fn select_linked_download_captured_at(&self, mod_id: i64) -> Result<Option<String>> {
        let found = self
            .db
            .query_row(
                "SELECT captured_at FROM download \
                 WHERE mod_id = ?1 AND captured_at IS NOT NULL \
                 ORDER BY id DESC LIMIT 1",
                [mod_id],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        Ok(found)
    }

    pub(crate) fn select_download_size(&self, download_id: i64) -> Result<Option<i64>> {
        let found = self
            .db
            .query_row(
                "SELECT size_bytes FROM download WHERE id = ?1",
                [download_id],
                |row| row.get::<_, Option<i64>>(0),
            )
            .optional()?;
        Ok(found.flatten())
    }

    fn select_linked_download_size(&self, mod_id: i64) -> Result<Option<i64>> {
        let found = self
            .db
            .query_row(
                "SELECT size_bytes FROM download \
                 WHERE mod_id = ?1 AND size_bytes IS NOT NULL \
                 ORDER BY id DESC LIMIT 1",
                [mod_id],
                |row| row.get::<_, Option<i64>>(0),
            )
            .optional()?;
        Ok(found.flatten())
    }

    fn select_measure_gaps(&self, loadout_id: i64) -> Result<Vec<MeasureGap>> {
        let mut stmt = self.db.prepare(
            "SELECT mod_.id, mod_.status, mod_.source_path, mod_.archive_path, \
                    mod_.size_bytes, mod_.installed_at, mod_.downloaded_at \
             FROM loadout_entry \
             JOIN mod_ ON mod_.id = loadout_entry.mod_id \
             WHERE loadout_entry.loadout_id = ?1 AND mod_.deleted_at IS NULL \
               AND (mod_.size_bytes IS NULL \
                    OR mod_.downloaded_at IS NULL \
                    OR (mod_.status = 'installed' AND mod_.installed_at IS NULL))",
        )?;
        let rows = stmt
            .query_map([loadout_id], |row| {
                Ok(MeasureGap {
                    mod_id: row.get(0)?,
                    status: row.get(1)?,
                    source_path: row.get(2)?,
                    archive_path: row.get(3)?,
                    size_bytes: row.get(4)?,
                    installed_at: row.get(5)?,
                    downloaded_at: row.get(6)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    fn select_mod_measure(&self, mod_id: i64) -> Result<Option<ModMeasure>> {
        self.db
            .query_row(
                "SELECT status, source_path, archive_path, size_bytes, installed_at, \
                        downloaded_at \
                 FROM mod_ WHERE id = ?1",
                [mod_id],
                |row| {
                    Ok(ModMeasure {
                        status: row.get(0)?,
                        source_path: row.get(1)?,
                        archive_path: row.get(2)?,
                        size_bytes: row.get(3)?,
                        installed_at: row.get(4)?,
                        downloaded_at: row.get(5)?,
                    })
                },
            )
            .optional()
            .map_err(Into::into)
    }

    fn select_install_payloads(&self) -> Result<Vec<(String, String)>> {
        let mut stmt = self.db.prepare(
            "SELECT occurred_at, payload FROM event \
             WHERE kind = 'transaction.committed' AND payload LIKE '%extrait%' \
             ORDER BY id DESC",
        )?;
        let rows = stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }
}

/// Length of a regular file. A directory, a missing path, or a length that
/// does not fit in `i64` is `None` — callers must not walk a folder here.
pub(crate) fn file_len(path: &Path) -> Option<i64> {
    if !path.is_file() {
        return None;
    }
    let len = fs::metadata(path).ok()?.len();
    i64::try_from(len).ok()
}

/// An import creates an already-installed row. Stamp that moment, and the
/// file length when `source` itself is the mod file (a jar).
///
/// `downloaded_at` is that file's modification time. A folder (an MO2 mod)
/// or a path that is not on disk uses the import time instead.
pub(crate) fn note_imported(core: &Core, mod_id: i64, source: &Path) -> Result<()> {
    core.touch_installed_at(mod_id)?;
    if let Some(at) = file_mtime(source) {
        core.set_downloaded_at(mod_id, &at)?;
    } else {
        core.touch_downloaded_at(mod_id)?;
    }
    if let Some(len) = file_len(source) {
        core.set_mod_size(mod_id, len)?;
    }
    Ok(())
}

/// Record `size_bytes` on the mod whose `source_path` is `source`, when that
/// column is still empty. Used when an importer knows an archive that is not
/// the installed folder itself.
pub fn set_size_for_source(core: &Core, source: &Path, size_bytes: i64) -> Result<()> {
    if size_bytes < 0 {
        return Ok(());
    }
    core.set_mod_size_for_source(&source.display().to_string(), size_bytes)
}

/// Cheap pass for one branch: `metadata().len()` of archive files (and of a
/// mod whose source is itself a file). No directory walk. Install times that
/// are still empty are filled from the install transaction, else the
/// installed folder's modification time. Download times that are still empty
/// are filled from the linked capture, else the archive's modification time,
/// else the install time.
pub fn fill_branch(core: &Core, loadout_id: i64) -> Result<()> {
    let gaps = core.select_measure_gaps(loadout_id)?;
    if gaps.is_empty() {
        return Ok(());
    }
    let needs_date = gaps
        .iter()
        .any(|gap| gap.status == STATUS_INSTALLED && gap.installed_at.is_none());
    let dates = if needs_date {
        install_dates(core)?
    } else {
        HashMap::new()
    };
    for gap in gaps {
        if gap.size_bytes.is_none() {
            if let Some(len) = listed_file_len(gap.archive_path.as_deref(), &gap.source_path) {
                core.set_mod_size_if_empty(gap.mod_id, len)?;
            }
        }
        let mut installed = gap.installed_at.clone();
        if gap.status == STATUS_INSTALLED && installed.is_none() {
            installed = dates
                .get(&gap.mod_id)
                .cloned()
                .or_else(|| modified_iso(Path::new(&gap.source_path)));
            if let Some(at) = &installed {
                core.set_installed_at_if_empty(gap.mod_id, at)?;
            }
        }
        if gap.downloaded_at.is_none() {
            if let Some(at) = recovered_download_date(
                core,
                gap.mod_id,
                gap.archive_path.as_deref(),
                &gap.source_path,
                installed,
            )? {
                core.set_downloaded_at_if_empty(gap.mod_id, &at)?;
            }
        }
    }
    Ok(())
}

/// Full fill for one mod, including the linked download's stored size and,
/// when the mod never had an archive, the sum of the installed folder.
/// Listing a branch does not call this.
pub fn refresh_one(core: &Core, mod_id: i64) -> Result<()> {
    let Some(row) = core.select_mod_measure(mod_id)? else {
        return Ok(());
    };
    if row.size_bytes.is_none() {
        if let Some(len) =
            missing_size(core, mod_id, row.archive_path.as_deref(), &row.source_path)?
        {
            core.set_mod_size_if_empty(mod_id, len)?;
        }
    }
    let mut installed = row.installed_at.clone();
    if row.status == STATUS_INSTALLED && installed.is_none() {
        installed = install_dates(core)?
            .get(&mod_id)
            .cloned()
            .or_else(|| modified_iso(Path::new(&row.source_path)));
        if let Some(at) = &installed {
            core.set_installed_at_if_empty(mod_id, at)?;
        }
    }
    if row.downloaded_at.is_none() {
        if let Some(at) = recovered_download_date(
            core,
            mod_id,
            row.archive_path.as_deref(),
            &row.source_path,
            installed,
        )? {
            core.set_downloaded_at_if_empty(mod_id, &at)?;
        }
    }
    Ok(())
}

/// Capture date, else the archive (or jar) modification time, else the
/// install date already known. `None` when none of those exist.
fn recovered_download_date(
    core: &Core,
    mod_id: i64,
    archive: Option<&str>,
    source: &str,
    installed_at: Option<String>,
) -> Result<Option<String>> {
    if let Some(at) = nonempty(core.select_linked_download_captured_at(mod_id)?) {
        return Ok(Some(at));
    }
    if let Some(at) = file_mtime_str(archive).or_else(|| file_mtime_str(Some(source))) {
        return Ok(Some(at));
    }
    Ok(nonempty(installed_at))
}

fn nonempty(value: Option<String>) -> Option<String> {
    value.filter(|text| !text.trim().is_empty())
}

fn listed_file_len(archive: Option<&str>, source: &str) -> Option<i64> {
    file_len_str(archive).or_else(|| file_len_str(Some(source)))
}

fn file_len_str(path: Option<&str>) -> Option<i64> {
    file_len(Path::new(path?))
}

fn missing_size(
    core: &Core,
    mod_id: i64,
    archive: Option<&str>,
    source: &str,
) -> Result<Option<i64>> {
    if let Some(len) = file_len_str(archive) {
        return Ok(Some(len));
    }
    if let Some(len) = core.select_linked_download_size(mod_id)? {
        return Ok(Some(len));
    }
    let source_path = Path::new(source);
    if let Some(len) = file_len(source_path) {
        return Ok(Some(len));
    }
    // A recorded archive that is no longer on disk is not replaced by the
    // extracted tree. The folder sum is only for a mod that never had one.
    let had_archive = archive.is_some_and(|path| !path.trim().is_empty());
    if had_archive {
        return Ok(None);
    }
    Ok(dir_size(source_path))
}

fn dir_size(root: &Path) -> Option<i64> {
    if !root.is_dir() {
        return None;
    }
    let mut total: u64 = 0;
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            if kind.is_symlink() {
                continue;
            }
            if kind.is_dir() {
                stack.push(entry.path());
            } else if kind.is_file() {
                if let Ok(meta) = entry.metadata() {
                    total = total.saturating_add(meta.len());
                }
            }
        }
    }
    i64::try_from(total).ok()
}

fn install_dates(core: &Core) -> Result<HashMap<i64, String>> {
    let mut dates = HashMap::new();
    for (occurred_at, payload) in core.select_install_payloads()? {
        let Ok(parsed) = serde_json::from_str::<Committed>(&payload) else {
            continue;
        };
        if parsed.kind != "download.installed" {
            continue;
        }
        for step in &parsed.steps {
            let Some(id) = installed_mod_id(step) else {
                continue;
            };
            dates.entry(id).or_insert_with(|| occurred_at.clone());
        }
    }
    Ok(dates)
}

fn installed_mod_id(step: &str) -> Option<i64> {
    let rest = step.trim().strip_prefix("mod ")?;
    let (id, tail) = rest.split_once(' ')?;
    if tail.starts_with("extrait") {
        id.parse().ok()
    } else {
        None
    }
}

/// Modification time of a regular file, ISO 8601 UTC. A directory is not
/// an archive or a jar, so it does not count.
fn file_mtime(path: &Path) -> Option<String> {
    if !path.is_file() {
        return None;
    }
    modified_iso(path)
}

fn file_mtime_str(path: Option<&str>) -> Option<String> {
    file_mtime(Path::new(path?))
}

fn modified_iso(path: &Path) -> Option<String> {
    let modified = fs::metadata(path).ok()?.modified().ok()?;
    system_time_iso(modified)
}

fn system_time_iso(time: SystemTime) -> Option<String> {
    let dur = time.duration_since(UNIX_EPOCH).ok()?;
    let secs = dur.as_secs();
    let millis = u64::from(dur.subsec_millis());
    let days = i64::try_from(secs / 86_400).ok()?;
    let tod = secs % 86_400;
    let hour = tod / 3600;
    let min = (tod % 3600) / 60;
    let sec = tod % 60;
    let (year, month, day) = civil_from_days(days)?;
    Some(format!(
        "{year:04}-{month:02}-{day:02}T{hour:02}:{min:02}:{sec:02}.{millis:03}Z"
    ))
}

/// Days since the Unix epoch to a civil year, month, day.
/// Howard Hinnant's `civil_from_days`, public domain.
fn civil_from_days(days: i64) -> Option<(i32, u32, u32)> {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = u64::try_from(z.rem_euclid(146_097)).ok()?;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let mut year = i64::try_from(yoe).ok()? + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    if month <= 2 {
        year += 1;
    }
    let year = i32::try_from(year).ok()?;
    let month = u32::try_from(month).ok()?;
    let day = u32::try_from(day).ok()?;
    Some((year, month, day))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::download::{self, mark_installed};
    use crate::loadout;
    use std::io::Write;

    #[test]
    fn unix_epoch_and_a_known_day_format_as_utc() {
        let epoch = system_time_iso(UNIX_EPOCH).unwrap();
        assert_eq!(epoch, "1970-01-01T00:00:00.000Z");
        let known = UNIX_EPOCH + std::time::Duration::from_secs(1_577_836_800);
        assert_eq!(
            system_time_iso(known).as_deref(),
            Some("2020-01-01T00:00:00.000Z")
        );
    }

    #[test]
    fn branch_load_stats_an_archive_and_leaves_a_folder_alone() {
        let core = Core::open_in_memory().unwrap();
        let lo = loadout::create(&core, "skyrimse", "Main").unwrap();
        let root = tempfile::tempdir().unwrap();
        let archive = root.path().join("lux.7z");
        fs::write(&archive, b"0123456789").unwrap();
        let folder = root.path().join("SkyUI");
        fs::create_dir(&folder).unwrap();
        fs::write(folder.join("mesh.nif"), b"much-longer-than-the-archive").unwrap();

        let archived = core
            .insert_archived_mod("lux", &archive.display().to_string(), None, None)
            .unwrap();
        core.insert_loadout_entry(lo.id, archived, false, 0)
            .unwrap();
        loadout::import_mods(
            &core,
            lo.id,
            &[loadout::ImportEntry {
                name: "SkyUI".into(),
                source_path: folder.clone(),
                enabled: true,
                version: None,
            }],
        )
        .unwrap();

        fill_branch(&core, lo.id).unwrap();
        let rows = loadout::entries(&core, lo.id).unwrap();
        let archive_row = rows.iter().find(|row| row.mod_id == archived).unwrap();
        assert_eq!(archive_row.size_bytes, Some(10));
        let folder_row = rows.iter().find(|row| row.name == "SkyUI").unwrap();
        assert_eq!(
            folder_row.size_bytes, None,
            "a branch load must not walk the installed folder"
        );

        refresh_one(&core, folder_row.mod_id).unwrap();
        let again = loadout::entries(&core, lo.id).unwrap();
        let folder_row = again.iter().find(|row| row.name == "SkyUI").unwrap();
        let expected = fs::metadata(folder.join("mesh.nif")).unwrap().len() as i64;
        assert_eq!(folder_row.size_bytes, Some(expected));
    }

    #[test]
    fn a_stored_size_survives_deleting_the_archive_and_a_refresh() {
        let core = Core::open_in_memory().unwrap();
        let lo = loadout::create(&core, "skyrimse", "Main").unwrap();
        let root = tempfile::tempdir().unwrap();
        let archive = root.path().join("lux.7z");
        fs::write(&archive, b"0123456789").unwrap();
        let id = download::capture(
            &core,
            "https://example.test/lux.7z",
            "lux.7z",
            &archive,
            Some(10),
        )
        .unwrap()
        .id();
        let mod_id = download::attach_to_loadout(&core, id, lo.id, None).unwrap();
        assert_eq!(
            loadout::entries(&core, lo.id).unwrap()[0].size_bytes,
            Some(10)
        );

        fs::remove_file(&archive).unwrap();
        core.delete_download(id).unwrap();
        refresh_one(&core, mod_id).unwrap();
        assert_eq!(
            loadout::entries(&core, lo.id).unwrap()[0].size_bytes,
            Some(10),
            "the recorded length stays after the file and the download row are gone"
        );
    }

    #[test]
    fn install_time_comes_back_from_the_journal_and_a_reinstall_replaces_it() {
        let core = Core::open_in_memory().unwrap();
        let lo = loadout::create(&core, "skyrimse", "Main").unwrap();
        let root = tempfile::tempdir().unwrap();
        let archive = root.path().join("lux.7z");
        let mut file = fs::File::create(&archive).unwrap();
        file.write_all(b"zip").unwrap();
        let id = download::capture(
            &core,
            "https://example.test/lux.7z",
            "lux.7z",
            &archive,
            Some(3),
        )
        .unwrap()
        .id();
        let mod_id = download::attach_to_loadout(&core, id, lo.id, None).unwrap();
        mark_installed(&core, mod_id, &root.path().join("mods").join("lux")).unwrap();

        assert!(
            loadout::entries(&core, lo.id).unwrap()[0]
                .installed_at
                .is_some(),
            "install writes installed_at"
        );
        let journal = core.select_install_payloads().unwrap()[0].0.clone();
        core.set_installed_at(mod_id, None).unwrap();
        fill_branch(&core, lo.id).unwrap();
        let restored = loadout::entries(&core, lo.id).unwrap()[0]
            .installed_at
            .clone()
            .unwrap();
        assert_eq!(restored, journal, "the journal date is reused");

        core.set_installed_at(mod_id, Some("1999-01-01T00:00:00.000Z"))
            .unwrap();
        mark_installed(&core, mod_id, &root.path().join("mods").join("lux")).unwrap();
        let replaced = loadout::entries(&core, lo.id).unwrap()[0]
            .installed_at
            .clone()
            .unwrap();
        assert_ne!(replaced, "1999-01-01T00:00:00.000Z");
    }

    #[test]
    fn a_mod_with_no_install_transaction_uses_the_folder_mtime() {
        let core = Core::open_in_memory().unwrap();
        let lo = loadout::create(&core, "skyrimse", "Main").unwrap();
        let root = tempfile::tempdir().unwrap();
        let folder = root.path().join("SkyUI");
        fs::create_dir(&folder).unwrap();
        fs::write(folder.join("a.txt"), b"a").unwrap();
        let mod_id = core
            .insert_mod("SkyUI", &folder.display().to_string(), None)
            .unwrap();
        core.insert_loadout_entry(lo.id, mod_id, true, 0).unwrap();

        fill_branch(&core, lo.id).unwrap();
        let stamped = loadout::entries(&core, lo.id).unwrap()[0]
            .installed_at
            .clone()
            .unwrap();
        assert_eq!(stamped, modified_iso(&folder).unwrap());
    }

    #[test]
    fn disabling_keeps_the_install_time() {
        let core = Core::open_in_memory().unwrap();
        let lo = loadout::create(&core, "skyrimse", "Main").unwrap();
        let root = tempfile::tempdir().unwrap();
        let archive = root.path().join("lux.7z");
        fs::write(&archive, b"zip").unwrap();
        let id = download::capture(
            &core,
            "https://example.test/lux.7z",
            "lux.7z",
            &archive,
            Some(3),
        )
        .unwrap()
        .id();
        let mod_id = download::attach_to_loadout(&core, id, lo.id, None).unwrap();
        mark_installed(&core, mod_id, &root.path().join("installed")).unwrap();
        let stamped = loadout::entries(&core, lo.id).unwrap()[0]
            .installed_at
            .clone()
            .unwrap();

        assert!(loadout::set_enabled(&core, lo.id, mod_id, false).unwrap());
        let off = loadout::entries(&core, lo.id).unwrap();
        assert!(!off[0].enabled);
        assert_eq!(off[0].installed_at.as_deref(), Some(stamped.as_str()));

        assert!(loadout::set_enabled(&core, lo.id, mod_id, true).unwrap());
        let on = loadout::entries(&core, lo.id).unwrap();
        assert!(on[0].enabled);
        assert_eq!(on[0].installed_at.as_deref(), Some(stamped.as_str()));
    }

    #[test]
    fn an_empty_download_date_comes_back_from_the_linked_capture() {
        let core = Core::open_in_memory().unwrap();
        let lo = loadout::create(&core, "skyrimse", "Main").unwrap();
        let root = tempfile::tempdir().unwrap();

        let from_capture = core.insert_mod("Lux", "F:/missing/Lux", None).unwrap();
        core.insert_loadout_entry(lo.id, from_capture, true, 0)
            .unwrap();
        let download_id = core
            .insert_download(
                "https://example.test/lux.7z",
                "lux.7z",
                "F:/missing/lux.7z",
                Some(3),
                crate::download::PHASE_ARCHIVED,
            )
            .unwrap();
        core.set_download_target(download_id, lo.id, from_capture)
            .unwrap();
        core.set_download_captured_at(download_id, "2018-06-15T08:30:00.000Z")
            .unwrap();
        core.set_installed_at(from_capture, Some("2020-01-01T00:00:00.000Z"))
            .unwrap();

        let archive = root.path().join("enb.7z");
        fs::write(&archive, b"0123456789").unwrap();
        let file = fs::File::options().write(true).open(&archive).unwrap();
        file.set_modified(UNIX_EPOCH + std::time::Duration::from_secs(1_577_836_800))
            .unwrap();
        drop(file);
        let from_file = core
            .insert_archived_mod("ENB", &archive.display().to_string(), None, None)
            .unwrap();
        core.insert_loadout_entry(lo.id, from_file, false, 1)
            .unwrap();

        let from_install = core.insert_mod("SkyUI", "F:/missing/SkyUI", None).unwrap();
        core.insert_loadout_entry(lo.id, from_install, true, 2)
            .unwrap();
        core.set_installed_at(from_install, Some("2021-03-04T05:06:07.000Z"))
            .unwrap();

        fill_branch(&core, lo.id).unwrap();
        let rows = loadout::entries(&core, lo.id).unwrap();
        let capture_row = rows.iter().find(|row| row.mod_id == from_capture).unwrap();
        assert_eq!(
            capture_row.downloaded_at.as_deref(),
            Some("2018-06-15T08:30:00.000Z"),
            "the linked capture wins over the install date"
        );
        let file_row = rows.iter().find(|row| row.mod_id == from_file).unwrap();
        assert_eq!(
            file_row.downloaded_at.as_deref(),
            modified_iso(&archive).as_deref(),
            "an archive with no capture uses its modification time"
        );
        let install_row = rows.iter().find(|row| row.mod_id == from_install).unwrap();
        assert_eq!(
            install_row.downloaded_at.as_deref(),
            Some("2021-03-04T05:06:07.000Z"),
            "with no capture and no archive, the install date stands in"
        );
    }

    #[test]
    fn importing_a_file_keeps_its_mtime_and_a_folder_uses_the_import_time() {
        let core = Core::open_in_memory().unwrap();
        let lo = loadout::create(&core, "minecraft", "Survie").unwrap();
        let root = tempfile::tempdir().unwrap();
        let jar = root.path().join("sodium.jar");
        fs::write(&jar, b"jar").unwrap();
        let past = UNIX_EPOCH + std::time::Duration::from_secs(1_577_836_800);
        let file = fs::File::options().write(true).open(&jar).unwrap();
        file.set_modified(past).unwrap();
        drop(file);

        let folder = root.path().join("SkyUI");
        fs::create_dir(&folder).unwrap();
        fs::write(folder.join("a.txt"), b"a").unwrap();
        let folder_mtime_forced = fs::File::open(&folder)
            .and_then(|dir| dir.set_modified(past))
            .is_ok();

        loadout::import_mods(
            &core,
            lo.id,
            &[
                loadout::ImportEntry {
                    name: "Sodium".into(),
                    source_path: jar.clone(),
                    enabled: true,
                    version: None,
                },
                loadout::ImportEntry {
                    name: "SkyUI".into(),
                    source_path: folder.clone(),
                    enabled: true,
                    version: None,
                },
            ],
        )
        .unwrap();

        let rows = loadout::entries(&core, lo.id).unwrap();
        let sodium = rows.iter().find(|row| row.name == "Sodium").unwrap();
        assert_eq!(
            sodium.downloaded_at.as_deref(),
            modified_iso(&jar).as_deref()
        );
        let sky = rows.iter().find(|row| row.name == "SkyUI").unwrap();
        assert!(
            sky.downloaded_at.is_some(),
            "an import always records a date"
        );
        if folder_mtime_forced {
            assert_ne!(
                sky.downloaded_at.as_deref(),
                modified_iso(&folder).as_deref(),
                "a folder is not an archive; the import time stands in"
            );
        }
    }
}
