//! Skyrim's own engine mod (SKSE): the one mod that cannot live only in USVFS.
//!
//! Every other mod is deployed into the virtual session USVFS builds at
//! launch (`usvfs::deploy_enabled_mods`) — nothing on the real install ever
//! changes. SKSE needs one deliberate exception, and only for its *root*
//! files: `skse64_loader.exe` and the `skse64_*.dll` files must sit for real,
//! beside `SkyrimSE.exe` — the loader finds its DLL next to itself before any
//! hook could show it anything virtual. Everything else the SKSE archive
//! ships (its `Data\Scripts\*.pex`) goes through USVFS like any other mod,
//! see `virtual_rows`.
//!
//! **What makes a mod "the engine mod" is its content, never its name**: a
//! `skse64_loader.exe` at the top of the mod folder (`is_engine_content`).
//! "Address Library for SKSE Plugins" and every other SKSE plugin carry
//! "SKSE" in their name but are plain Data mods.
//!
//! The copy into the real install root is written to never lose a file
//! (`CLAUDE.md`: everything reversible):
//! - a file already there with identical bytes is left alone, and not
//!   claimed as ours (a hand-made SKSE install is never removed by ModLab);
//! - a file already there with different bytes is first copied aside into
//!   ModLab's own data folder (`<store>/backup`) — only then replaced;
//! - every file placed is listed in a manifest (`<store>/engine-files.tsv`)
//!   with its size and hash, so it can be taken out again later;
//! - taking a file out *moves* it into `<store>/retired` (and moves the
//!   original back from `backup`), it never deletes anything; a file that no
//!   longer matches what ModLab placed (changed by hand) is left untouched.

use modlab_core::loadout::{LoadoutEntryRow, STATUS_INSTALLED};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// SE loader exe. The string is `tool_archive::SE_LOADER_FILE`, the `skse64`
/// row of the same table `recognize` uses. VR and classic loaders are on
/// that table too; only this one is copied beside `SkyrimSE.exe`.
pub const LOADER_FILE: &str = crate::tool_archive::SE_LOADER_FILE;

const MANIFEST_FILE: &str = "engine-files.tsv";
const BACKUP_DIR: &str = "backup";
const RETIRED_DIR: &str = "retired";

/// Does this folder hold SKSE itself — the loader exe at its top level?
/// (A single `stat`: it is asked for every mod of a loadout.)
pub fn is_engine_content(dir: &Path) -> bool {
    dir.join(LOADER_FILE).is_file()
}

/// A top-level file that belongs beside `SkyrimSE.exe`: the loader, the
/// version-bound `skse64_X_Y_Z.dll`, `skse64_steam_loader.dll`. Readmes and
/// the like stay in the mod folder. The prefix is `tool_archive::SE_NAME_PREFIX`.
fn is_engine_root_file(name: &str) -> bool {
    crate::tool_archive::is_se_root_file(name)
}

/// Names of the engine mod's root files — the only ones copied for real.
pub fn engine_root_files(source: &Path) -> io::Result<Vec<String>> {
    let mut out = Vec::new();
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        if !entry.file_type()?.is_file() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        if is_engine_root_file(&name) {
            out.push(name);
        }
    }
    out.sort();
    Ok(out)
}

/// The `Data` folder inside a mod folder, whatever its capitalization.
pub(crate) fn data_subdir(dir: &Path) -> Option<PathBuf> {
    fs::read_dir(dir)
        .ok()?
        .filter_map(|e| e.ok())
        .find(|e| {
            e.file_name().to_string_lossy().eq_ignore_ascii_case("data")
                && e.file_type().is_ok_and(|t| t.is_dir())
        })
        .map(|e| e.path())
}

/// The rows USVFS should layer: every row as is, except the engine mod,
/// which is reduced to its `Data\` part (its root files are copied for real,
/// see `install_root_files`). An engine mod without a `Data\` part has
/// nothing virtual to contribute and is left out. The Plugins tab and
/// `plugins.txt` use the same set, so they show what the game gets.
pub fn virtual_rows(rows: &[LoadoutEntryRow]) -> Vec<LoadoutEntryRow> {
    rows.iter()
        .filter_map(|row| {
            if !is_engine_content(&row.source_path) {
                return Some(row.clone());
            }
            let data = data_subdir(&row.source_path)?;
            let mut engine = row.clone();
            engine.source_path = data;
            Some(engine)
        })
        .collect()
}

/// What `install_root_files` did.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct InstallReport {
    /// Files written into the install root.
    pub copied: Vec<String>,
    /// Files that were already there with other bytes and were set aside
    /// first (a subset of the previous list is replaced after this).
    pub backed_up: Vec<String>,
    /// Files already there with the very same bytes — left alone.
    pub identical: Vec<String>,
}

impl InstallReport {
    fn merge(&mut self, other: InstallReport) {
        self.copied.extend(other.copied);
        self.backed_up.extend(other.backed_up);
        self.identical.extend(other.identical);
    }
}

/// What `uninstall_root_files` did.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct UninstallReport {
    /// Files taken out of the install root (moved into `retired`).
    pub removed: Vec<String>,
    /// Original files moved back from `backup`.
    pub restored: Vec<String>,
    /// Files that no longer match what ModLab placed — left untouched.
    pub left_alone: Vec<String>,
}

/// One file ModLab placed in the install root.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Record {
    name: String,
    size: u64,
    hash: u64,
    /// File name inside `<store>/backup` of the original this one replaced.
    backup: Option<String>,
}

/// FNV-1a, 64 bit. A fixed, dependency-free hash: the manifest outlives any
/// one build, so `std`'s unspecified hasher would not do.
fn fnv1a(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in bytes {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

fn load_manifest(store: &Path) -> io::Result<Vec<Record>> {
    let text = match fs::read_to_string(store.join(MANIFEST_FILE)) {
        Ok(t) => t,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e),
    };
    let bad = |line: &str| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("manifeste SKSE illisible (ligne « {line} ») — rien n'a été modifié"),
        )
    };
    let mut out = Vec::new();
    for line in text.lines().filter(|l| !l.trim().is_empty()) {
        let parts: Vec<&str> = line.split('\t').collect();
        let [name, size, hash, backup] = parts.as_slice() else {
            return Err(bad(line));
        };
        out.push(Record {
            name: (*name).to_string(),
            size: size.parse().map_err(|_| bad(line))?,
            hash: u64::from_str_radix(hash, 16).map_err(|_| bad(line))?,
            backup: (*backup != "-").then(|| (*backup).to_string()),
        });
    }
    Ok(out)
}

fn save_manifest(store: &Path, records: &[Record]) -> io::Result<()> {
    fs::create_dir_all(store)?;
    let mut text = String::new();
    for r in records {
        text.push_str(&format!(
            "{}\t{}\t{:016x}\t{}\n",
            r.name,
            r.size,
            r.hash,
            r.backup.as_deref().unwrap_or("-")
        ));
    }
    let tmp = store.join(format!("{MANIFEST_FILE}.tmp"));
    fs::write(&tmp, text)?;
    fs::rename(&tmp, store.join(MANIFEST_FILE))
}

/// A path under `dir` for `name` that does not exist yet (`name`, `name.1`, ...).
fn unique_path(dir: &Path, name: &str) -> PathBuf {
    let first = dir.join(name);
    if !first.exists() {
        return first;
    }
    (1u32..)
        .map(|n| dir.join(format!("{name}.{n}")))
        .find(|p| !p.exists())
        .expect("an unused numbered name always exists")
}

/// Is the file at `path` exactly the one `record` describes?
fn matches_record(path: &Path, record: &Record) -> io::Result<bool> {
    let bytes = fs::read(path)?;
    Ok(bytes.len() as u64 == record.size && fnv1a(&bytes) == record.hash)
}

/// Move a file without ever deleting it: rename, or — across volumes — copy,
/// check the copy, then drop the original.
fn move_file(from: &Path, to: &Path) -> io::Result<()> {
    if let Some(parent) = to.parent() {
        fs::create_dir_all(parent)?;
    }
    if fs::rename(from, to).is_ok() {
        return Ok(());
    }
    let copied = fs::copy(from, to)?;
    if copied != fs::metadata(from)?.len() {
        return Err(io::Error::other(format!(
            "copie incomplète de {}",
            from.display()
        )));
    }
    fs::remove_file(from)
}

/// Copy `src` over `dest` through a temporary sibling, so a failure halfway
/// never leaves a truncated DLL where the game would load it.
fn replace_file(src: &Path, dest: &Path) -> io::Result<()> {
    let mut tmp_name = dest.file_name().unwrap_or_default().to_os_string();
    tmp_name.push(".modlab-tmp");
    let tmp = dest.with_file_name(tmp_name);
    let result = fs::copy(src, &tmp).and_then(|_| fs::rename(&tmp, dest));
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result
}

/// Put the engine mod's root files (`skse64_loader.exe`, `skse64_*.dll`) into
/// the real install root — safely, see the module doc. `store` is ModLab's
/// own folder for this (backups, retired files, manifest). Idempotent: when
/// everything is already in place it changes nothing.
pub fn install_root_files(
    source: &Path,
    install_root: &Path,
    store: &Path,
) -> io::Result<InstallReport> {
    let mut report = InstallReport::default();
    let mut records = load_manifest(store)?;

    for name in engine_root_files(source)? {
        let bytes = fs::read(source.join(&name))?;
        let (size, hash) = (bytes.len() as u64, fnv1a(&bytes));
        let dest = install_root.join(&name);
        let known = records
            .iter()
            .position(|r| r.name.eq_ignore_ascii_case(&name));
        let mut backup = known.and_then(|i| records[i].backup.clone());

        if dest.exists() {
            if !dest.is_file() {
                return Err(io::Error::other(format!(
                    "{} existe mais n'est pas un fichier",
                    dest.display()
                )));
            }
            let current = fs::read(&dest)?;
            if current == bytes {
                // Same bytes: nothing to do. Only a file ModLab already owns
                // keeps being listed (its hash is refreshed); a hand-made
                // install is never claimed.
                if let Some(i) = known {
                    records[i].size = size;
                    records[i].hash = hash;
                    save_manifest(store, &records)?;
                }
                report.identical.push(name);
                continue;
            }
            let ours = known.is_some_and(|i| {
                current.len() as u64 == records[i].size && fnv1a(&current) == records[i].hash
            });
            if !ours {
                // Somebody else's file: set it aside before anything else.
                let backups = store.join(BACKUP_DIR);
                fs::create_dir_all(&backups)?;
                let target = unique_path(&backups, &name);
                let copied = fs::copy(&dest, &target)?;
                if copied != current.len() as u64 {
                    return Err(io::Error::other(format!(
                        "sauvegarde incomplète de {}",
                        dest.display()
                    )));
                }
                backup = target.file_name().map(|n| n.to_string_lossy().to_string());
                report.backed_up.push(name.clone());
            }
        }

        replace_file(&source.join(&name), &dest)?;
        let record = Record {
            name: name.clone(),
            size,
            hash,
            backup,
        };
        match known {
            Some(i) => records[i] = record,
            None => records.push(record),
        }
        save_manifest(store, &records)?;
        report.copied.push(name);
    }
    Ok(report)
}

/// Take back out what `install_root_files` placed — `only` limits it to those
/// file names (`None` = everything in the manifest). Files are *moved* into
/// `<store>/retired`, never deleted; an original that had been set aside is
/// moved back; a file changed by hand since is left where it is.
pub fn uninstall_root_files(
    install_root: &Path,
    store: &Path,
    only: Option<&[String]>,
) -> io::Result<UninstallReport> {
    let mut report = UninstallReport::default();
    let records = load_manifest(store)?;
    let mut kept = Vec::new();

    for record in records {
        let selected =
            only.is_none_or(|names| names.iter().any(|n| n.eq_ignore_ascii_case(&record.name)));
        if !selected {
            kept.push(record);
            continue;
        }
        let dest = install_root.join(&record.name);
        if !dest.exists() {
            // Already gone: nothing to take out. The set-aside original, if
            // any, stays in `backup` untouched.
            continue;
        }
        if !matches_record(&dest, &record)? {
            report.left_alone.push(record.name.clone());
            continue;
        }
        let retired = store.join(RETIRED_DIR);
        fs::create_dir_all(&retired)?;
        move_file(&dest, &unique_path(&retired, &record.name))?;
        report.removed.push(record.name.clone());
        if let Some(backup) = &record.backup {
            let from = store.join(BACKUP_DIR).join(backup);
            if from.is_file() {
                move_file(&from, &dest)?;
                report.restored.push(record.name.clone());
            }
        }
    }
    save_manifest(store, &kept)?;
    Ok(report)
}

/// Before a launch: make sure every enabled, installed engine mod of `rows`
/// has its root files in the install root — the case a click on the checkbox
/// never covers (an imported or restored loadout whose SKSE is already
/// enabled).
pub fn ensure_engine_installed(
    rows: &[LoadoutEntryRow],
    install_root: &Path,
    store: &Path,
) -> io::Result<InstallReport> {
    let mut total = InstallReport::default();
    for row in enabled_engine_rows(rows) {
        total.merge(install_root_files(&row.source_path, install_root, store)?);
    }
    Ok(total)
}

/// The rows whose root files a launch copies into the install root: enabled,
/// really extracted, and recognised as the engine mod by their content. One
/// definition for the copy and for the checks that must run before it.
pub fn enabled_engine_rows(rows: &[LoadoutEntryRow]) -> impl Iterator<Item = &LoadoutEntryRow> {
    rows.iter().filter(|row| {
        row.enabled && row.status == STATUS_INSTALLED && is_engine_content(&row.source_path)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(root: &Path, rel: &str, bytes: &[u8]) {
        let path = root.join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, bytes).unwrap();
    }

    /// A fake SKSE mod folder as `place_content(.., keep_game_root = true)`
    /// leaves it: root files beside `Data/`.
    fn fake_skse_mod() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "skse64_loader.exe", b"fake loader");
        write(dir.path(), "skse64_1_7_104.dll", b"fake runtime dll");
        write(dir.path(), "skse64_steam_loader.dll", b"fake steam loader");
        write(dir.path(), "skse64_readme.txt", b"readme");
        write(dir.path(), "Data/Scripts/Actor.pex", b"fake pex");
        dir
    }

    /// "Address Library for SKSE Plugins": SKSE in the name, plain Data mod.
    fn fake_address_library() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "SKSE/Plugins/x.dll", b"fake plugin");
        write(dir.path(), "SKSE/Plugins/versionlib.bin", b"fake db");
        dir
    }

    fn row(name: &str, source: &Path, enabled: bool) -> LoadoutEntryRow {
        LoadoutEntryRow {
            mod_id: 1,
            name: name.to_string(),
            source_path: source.to_path_buf(),
            enabled,
            position: 0,
            version: None,
            status: STATUS_INSTALLED.to_string(),
            category: None,
            archive_path: None,
            source_host: None,
            page_url: None,
            game_versions: Vec::new(),
            dependencies: Vec::new(),
            loaders: Vec::new(),
            category_raw: None,
            category_nexus_id: None,
            nexus_mod_id: None,
            author: None,
            website: None,
            size_bytes: None,
            installed_at: None,
            downloaded_at: None,
        }
    }

    #[test]
    fn engine_is_recognised_by_content_not_by_name() {
        let skse = fake_skse_mod();
        let address_library = fake_address_library();
        assert!(is_engine_content(skse.path()));
        assert!(!is_engine_content(address_library.path()));
    }

    #[test]
    fn only_root_files_of_the_engine_are_selected() {
        let skse = fake_skse_mod();
        assert_eq!(
            engine_root_files(skse.path()).unwrap(),
            [
                "skse64_1_7_104.dll",
                "skse64_loader.exe",
                "skse64_steam_loader.dll"
            ]
        );
    }

    #[test]
    fn virtual_rows_keep_skse_named_plugins_and_reduce_the_engine_to_data() {
        let skse = fake_skse_mod();
        let address_library = fake_address_library();
        let rows = vec![
            row("SKSE64", skse.path(), true),
            row(
                "Address Library for SKSE Plugins",
                address_library.path(),
                true,
            ),
        ];

        let virtual_rows = virtual_rows(&rows);

        assert_eq!(virtual_rows.len(), 2);
        assert_eq!(virtual_rows[0].source_path, skse.path().join("Data"));
        assert_eq!(virtual_rows[1].source_path, address_library.path());
    }

    #[test]
    fn plugins_of_skse_named_mods_stay_in_the_load_order() {
        // A mod named after SKSE that ships a plugin file must reach plugins.txt:
        // it is a normal mod, so it goes through USVFS and the Plugins tab.
        let skse = fake_skse_mod();
        let skse_menu = tempfile::tempdir().unwrap();
        write(
            skse_menu.path(),
            "SKSE Menu Framework.esp",
            b"not a real plugin",
        );
        write(skse_menu.path(), "SKSE/Plugins/menu.dll", b"fake plugin");
        let rows = vec![
            row("SKSE64", skse.path(), true),
            row("SKSE Menu Framework", skse_menu.path(), true),
        ];
        let game = crate::plugins::GameContext {
            install_root: None,
            data_dir: None,
        };

        let order = crate::plugins::compute_load_order(&virtual_rows(&rows), &game);

        assert!(order
            .entries
            .iter()
            .any(|e| e.file_name == "SKSE Menu Framework.esp"));
    }

    #[test]
    fn an_engine_without_data_part_is_left_out_of_the_virtual_deployment() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "skse64_loader.exe", b"fake loader");
        assert!(virtual_rows(&[row("SKSE64", dir.path(), true)]).is_empty());
    }

    #[test]
    fn install_copies_root_files_only_and_uninstall_takes_them_out() {
        let skse = fake_skse_mod();
        let game = tempfile::tempdir().unwrap();
        let store = tempfile::tempdir().unwrap();

        let report = install_root_files(skse.path(), game.path(), store.path()).unwrap();

        assert_eq!(report.copied.len(), 3);
        assert!(game.path().join("skse64_loader.exe").is_file());
        assert!(game.path().join("skse64_1_7_104.dll").is_file());
        assert!(!game.path().join("skse64_readme.txt").exists());
        assert!(!game.path().join("Data").exists());

        let report = uninstall_root_files(game.path(), store.path(), None).unwrap();

        assert_eq!(report.removed.len(), 3);
        assert!(!game.path().join("skse64_loader.exe").exists());
        // Nothing deleted for good: the removed files were moved aside, and
        // the mod's own folder is untouched.
        assert!(store.path().join("retired/skse64_loader.exe").is_file());
        assert!(skse.path().join("skse64_loader.exe").is_file());
    }

    #[test]
    fn a_different_existing_file_is_set_aside_then_restored_on_uninstall() {
        let skse = fake_skse_mod();
        let game = tempfile::tempdir().unwrap();
        let store = tempfile::tempdir().unwrap();
        write(game.path(), "skse64_loader.exe", b"the user's own loader");

        let report = install_root_files(skse.path(), game.path(), store.path()).unwrap();

        assert_eq!(report.backed_up, ["skse64_loader.exe"]);
        assert_eq!(
            fs::read(store.path().join("backup/skse64_loader.exe")).unwrap(),
            b"the user's own loader"
        );
        assert_eq!(
            fs::read(game.path().join("skse64_loader.exe")).unwrap(),
            b"fake loader"
        );

        let report = uninstall_root_files(game.path(), store.path(), None).unwrap();

        assert_eq!(report.restored, ["skse64_loader.exe"]);
        assert_eq!(
            fs::read(game.path().join("skse64_loader.exe")).unwrap(),
            b"the user's own loader"
        );
    }

    #[test]
    fn identical_existing_files_are_left_alone_and_never_claimed() {
        let skse = fake_skse_mod();
        let game = tempfile::tempdir().unwrap();
        let store = tempfile::tempdir().unwrap();
        write(game.path(), "skse64_loader.exe", b"fake loader");
        write(game.path(), "skse64_1_7_104.dll", b"fake runtime dll");

        let report = install_root_files(skse.path(), game.path(), store.path()).unwrap();

        assert_eq!(
            report.identical,
            ["skse64_1_7_104.dll", "skse64_loader.exe"]
        );
        assert_eq!(report.copied, ["skse64_steam_loader.dll"]);
        assert!(report.backed_up.is_empty());
        assert!(!store.path().join("backup").exists());

        // A hand-made install is not ModLab's to remove.
        uninstall_root_files(game.path(), store.path(), None).unwrap();
        assert!(game.path().join("skse64_loader.exe").is_file());
        assert!(game.path().join("skse64_1_7_104.dll").is_file());
        assert!(!game.path().join("skse64_steam_loader.dll").exists());
    }

    #[test]
    fn installing_twice_changes_nothing_the_second_time() {
        let skse = fake_skse_mod();
        let game = tempfile::tempdir().unwrap();
        let store = tempfile::tempdir().unwrap();
        install_root_files(skse.path(), game.path(), store.path()).unwrap();

        let report = install_root_files(skse.path(), game.path(), store.path()).unwrap();

        assert!(report.copied.is_empty());
        assert!(report.backed_up.is_empty());
        assert_eq!(report.identical.len(), 3);
    }

    #[test]
    fn updating_the_mod_replaces_our_own_file_without_a_new_backup() {
        let skse = fake_skse_mod();
        let game = tempfile::tempdir().unwrap();
        let store = tempfile::tempdir().unwrap();
        install_root_files(skse.path(), game.path(), store.path()).unwrap();
        write(skse.path(), "skse64_loader.exe", b"newer loader");

        let report = install_root_files(skse.path(), game.path(), store.path()).unwrap();

        assert_eq!(report.copied, ["skse64_loader.exe"]);
        assert!(report.backed_up.is_empty());
        assert_eq!(
            fs::read(game.path().join("skse64_loader.exe")).unwrap(),
            b"newer loader"
        );
    }

    #[test]
    fn a_file_changed_by_hand_since_is_not_removed() {
        let skse = fake_skse_mod();
        let game = tempfile::tempdir().unwrap();
        let store = tempfile::tempdir().unwrap();
        install_root_files(skse.path(), game.path(), store.path()).unwrap();
        write(game.path(), "skse64_loader.exe", b"edited by the user");

        let report = uninstall_root_files(game.path(), store.path(), None).unwrap();

        assert_eq!(report.left_alone, ["skse64_loader.exe"]);
        assert_eq!(
            fs::read(game.path().join("skse64_loader.exe")).unwrap(),
            b"edited by the user"
        );
    }

    #[test]
    fn uninstall_before_any_install_is_not_an_error() {
        let game = tempfile::tempdir().unwrap();
        let store = tempfile::tempdir().unwrap();
        let report = uninstall_root_files(game.path(), store.path(), None).unwrap();
        assert_eq!(report, UninstallReport::default());
    }

    #[test]
    fn an_unreadable_manifest_stops_everything_without_touching_files() {
        let skse = fake_skse_mod();
        let game = tempfile::tempdir().unwrap();
        let store = tempfile::tempdir().unwrap();
        write(store.path(), MANIFEST_FILE, b"garbage without tabs\n");

        assert!(install_root_files(skse.path(), game.path(), store.path()).is_err());
        assert!(!game.path().join("skse64_loader.exe").exists());
    }

    #[test]
    fn ensure_installs_an_already_enabled_engine_and_ignores_the_rest() {
        let skse = fake_skse_mod();
        let address_library = fake_address_library();
        let disabled_skse = fake_skse_mod();
        let game = tempfile::tempdir().unwrap();
        let store = tempfile::tempdir().unwrap();

        let rows = vec![
            row(
                "Address Library for SKSE Plugins",
                address_library.path(),
                true,
            ),
            row("SKSE64", skse.path(), true),
        ];
        let report = ensure_engine_installed(&rows, game.path(), store.path()).unwrap();
        assert_eq!(report.copied.len(), 3);
        // The plugin's own files never reach the install root.
        assert!(!game.path().join("SKSE").exists());

        let game2 = tempfile::tempdir().unwrap();
        let rows = vec![row("SKSE64", disabled_skse.path(), false)];
        let report = ensure_engine_installed(&rows, game2.path(), store.path()).unwrap();
        assert_eq!(report, InstallReport::default());
    }
}
