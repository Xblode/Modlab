//! File conflicts — the same relative path provided by more than one mod.
//! See docs/01-architecture-decidee.md §5 ("indicateurs de conflit par
//! fichier"). Generic, not Skyrim-specific: any game whose mods are
//! folders of files has this same concept.
//!
//! Deliberately conservative about "who wins": this module computes
//! winners as *last in the sequence you give it wins*, full stop. It does
//! **not** decide what that sequence should be — `modlab_game_skyrim::mo2`
//! found a real, still-open question (which direction `modlist.txt`'s line
//! order maps to MO2's actual on-screen priority) and this module must not
//! quietly bake in a guessed answer to that. Callers pass mods already in
//! the priority order they're confident in.

use crate::loadout::LoadoutEntryRow;
use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};

/// One mod's contribution to a conflict check: an id, a name (for
/// readable output), and the relative file paths it provides.
pub struct ModFiles {
    pub mod_id: i64,
    pub name: String,
    pub files: Vec<PathBuf>,
}

/// One relative path that more than one mod provides.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileConflict {
    pub relative_path: PathBuf,
    /// `(mod_id, name)` pairs, in the priority order they were given —
    /// the last entry is the winner (the file the game actually sees).
    pub contributors: Vec<(i64, String)>,
}

impl FileConflict {
    #[must_use]
    pub fn winner(&self) -> &(i64, String) {
        self.contributors
            .last()
            .expect("a FileConflict always has at least 2 contributors")
    }
}

/// Find every relative path provided by more than one mod. `mods` must
/// already be in priority order (lowest priority first) — see the module
/// doc for why this function won't determine that order itself.
#[must_use]
pub fn find_conflicts(mods: &[ModFiles]) -> Vec<FileConflict> {
    let mut by_path: HashMap<&Path, Vec<(i64, String)>> = HashMap::new();
    for m in mods {
        for file in &m.files {
            by_path
                .entry(file.as_path())
                .or_default()
                .push((m.mod_id, m.name.clone()));
        }
    }

    let mut conflicts: Vec<FileConflict> = by_path
        .into_iter()
        .filter(|(_, contributors)| contributors.len() > 1)
        .map(|(path, contributors)| FileConflict {
            relative_path: path.to_path_buf(),
            contributors,
        })
        .collect();
    conflicts.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));
    conflicts
}

/// One mod's tally of the conflicts it takes part in.
///
/// This is the shape a UI row needs: enough to badge a mod without shipping
/// every conflicting path over IPC. `wins` and `loses` count *distinct paths*
/// this mod provides that another enabled mod also provides — a mod with
/// `wins = 0, loses = 0` takes part in no conflict at all.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModConflictCounts {
    pub mod_id: i64,
    pub name: String,
    /// Paths where this mod is the winner (last in the priority order given).
    pub wins: usize,
    /// Paths where another mod, later in that order, wins over this one.
    pub loses: usize,
}

/// A conflict scan over one loadout, ready to be serialized to a client.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ConflictReport {
    pub conflicts: Vec<FileConflict>,
    /// Per-mod tallies, in the same order as the input entries.
    pub mods: Vec<ModConflictCounts>,
    /// `(mod_id, why)` for enabled mods whose folder could not be walked.
    ///
    /// Kept apart from an empty `conflicts` list on purpose: a mod folder
    /// that could not be read means the scan *does not know*, which is not
    /// the same fact as "these mods share no file". A client that hides the
    /// difference would report "no conflict" about a mod it never looked at.
    pub unreadable: Vec<(i64, String)>,
}

/// Fold `find_conflicts`' output into per-mod tallies.
///
/// `mods` is the same slice that was passed to `find_conflicts`, so the
/// result keeps the caller's priority order — and includes mods with no
/// conflict at all, which a UI still needs to tell "no conflicts" from
/// "not scanned".
#[must_use]
pub fn tally(conflicts: &[FileConflict], mods: &[ModFiles]) -> Vec<ModConflictCounts> {
    let mut wins: HashMap<i64, usize> = HashMap::new();
    let mut loses: HashMap<i64, usize> = HashMap::new();
    for conflict in conflicts {
        let winner_id = conflict.winner().0;
        for (mod_id, _) in &conflict.contributors {
            if *mod_id == winner_id {
                *wins.entry(*mod_id).or_default() += 1;
            } else {
                *loses.entry(*mod_id).or_default() += 1;
            }
        }
    }

    mods.iter()
        .map(|m| ModConflictCounts {
            mod_id: m.mod_id,
            name: m.name.clone(),
            wins: wins.get(&m.mod_id).copied().unwrap_or(0),
            loses: loses.get(&m.mod_id).copied().unwrap_or(0),
        })
        .collect()
}

/// Filenames that are mod-manager bookkeeping, never real mod content, and
/// so must never be treated as "this file conflicts." Found for real, not
/// guessed: importing Benjamin's actual MO2 instance flagged `meta.ini` —
/// MO2 writes one into *every* mod folder it manages (install source,
/// Nexus id, install date) — as a "conflict" between every pair of
/// MO2-managed mods, which is noise, not a real conflict. Not exhaustive;
/// extend if another tool's bookkeeping file shows up the same way.
const IGNORED_FILENAMES: &[&str] = &["meta.ini", "thumbs.db", "desktop.ini"];

/// List every real (non-bookkeeping) file under `root`, as paths relative
/// to `root`. Symlinks are followed (Windows junctions inside a mods
/// folder are not unusual); nothing is hashed or read, just enumerated.
pub fn list_files_relative(root: &Path) -> io::Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    walk(root, root, &mut out)?;
    Ok(out)
}

fn walk(root: &Path, dir: &Path, out: &mut Vec<PathBuf>) -> io::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if entry.file_type()?.is_dir() {
            walk(root, &path, out)?;
        } else {
            let is_ignored = path
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|name| {
                    IGNORED_FILENAMES
                        .iter()
                        .any(|i| name.eq_ignore_ascii_case(i))
                });
            if is_ignored {
                continue;
            }
            let relative = path
                .strip_prefix(root)
                .expect("walked path is always under root")
                .to_path_buf();
            out.push(relative);
        }
    }
    Ok(())
}

/// Scan a loadout's entries: enabled mods only, in the order given — which the
/// caller must already have in priority order (see the module doc; today that
/// order is the loadout's stored import order).
///
/// This is the single entry point both the CLI and the UI go through, so
/// "what the UI badges" and "what the CLI prints" cannot drift apart.
/// Unreadable folders land in `unreadable` instead of failing the whole scan:
/// one broken folder must not hide every other mod's conflicts, and it must
/// not turn into a silent "no conflict" either.
#[must_use]
pub fn report_for_entries(entries: &[LoadoutEntryRow]) -> ConflictReport {
    let mut mods = Vec::new();
    let mut unreadable = Vec::new();
    for row in entries.iter().filter(|r| r.enabled) {
        match list_files_relative(&row.source_path) {
            Ok(files) => mods.push(ModFiles {
                mod_id: row.mod_id,
                name: row.name.clone(),
                files,
            }),
            Err(e) => unreadable.push((row.mod_id, format!("{}: {e}", row.source_path.display()))),
        }
    }

    let conflicts = find_conflicts(&mods);
    let mods = tally(&conflicts, &mods);
    ConflictReport {
        conflicts,
        mods,
        unreadable,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn finds_a_conflict_and_picks_the_last_mod_as_winner() {
        let mods = vec![
            ModFiles {
                mod_id: 1,
                name: "Base Textures".into(),
                files: vec![PathBuf::from("textures/armor/steel.dds")],
            },
            ModFiles {
                mod_id: 2,
                name: "Retexture Overhaul".into(),
                files: vec![PathBuf::from("textures/armor/steel.dds")],
            },
        ];

        let conflicts = find_conflicts(&mods);

        assert_eq!(conflicts.len(), 1);
        assert_eq!(
            conflicts[0].relative_path,
            PathBuf::from("textures/armor/steel.dds")
        );
        assert_eq!(
            conflicts[0].winner(),
            &(2, "Retexture Overhaul".to_string())
        );
    }

    #[test]
    fn files_provided_by_only_one_mod_are_not_conflicts() {
        let mods = vec![
            ModFiles {
                mod_id: 1,
                name: "A".into(),
                files: vec![PathBuf::from("meshes/a.nif")],
            },
            ModFiles {
                mod_id: 2,
                name: "B".into(),
                files: vec![PathBuf::from("meshes/b.nif")],
            },
        ];

        assert!(find_conflicts(&mods).is_empty());
    }

    #[test]
    fn three_way_conflict_keeps_all_contributors_in_order() {
        let path = PathBuf::from("scripts/source/thing.psc");
        let mods = vec![
            ModFiles {
                mod_id: 1,
                name: "A".into(),
                files: vec![path.clone()],
            },
            ModFiles {
                mod_id: 2,
                name: "B".into(),
                files: vec![path.clone()],
            },
            ModFiles {
                mod_id: 3,
                name: "C".into(),
                files: vec![path.clone()],
            },
        ];

        let conflicts = find_conflicts(&mods);

        assert_eq!(conflicts.len(), 1);
        assert_eq!(
            conflicts[0].contributors,
            vec![
                (1, "A".to_string()),
                (2, "B".to_string()),
                (3, "C".to_string())
            ]
        );
        assert_eq!(conflicts[0].winner(), &(3, "C".to_string()));
    }

    #[test]
    fn list_files_relative_walks_nested_directories() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join("meshes/armor")).unwrap();
        fs::write(dir.path().join("meshes/armor/steel.nif"), b"x").unwrap();
        fs::write(dir.path().join("readme.txt"), b"x").unwrap();

        let mut files = list_files_relative(dir.path()).unwrap();
        files.sort();

        assert_eq!(
            files,
            vec![
                PathBuf::from("meshes/armor/steel.nif"),
                PathBuf::from("readme.txt"),
            ],
        );
    }

    #[test]
    fn ignores_mod_manager_bookkeeping_files() {
        // Regression test for the real false-positive found importing
        // Benjamin's actual MO2 instance: every MO2-managed mod folder has
        // its own meta.ini, which must never look like a file conflict.
        let dir_a = tempfile::tempdir().unwrap();
        fs::write(dir_a.path().join("meta.ini"), b"[General]\ninstalled=true").unwrap();
        fs::write(dir_a.path().join("plugin_a.esp"), b"x").unwrap();

        let dir_b = tempfile::tempdir().unwrap();
        fs::write(dir_b.path().join("meta.ini"), b"[General]\ninstalled=true").unwrap();
        fs::write(dir_b.path().join("plugin_b.esp"), b"x").unwrap();

        let files_a = list_files_relative(dir_a.path()).unwrap();
        let files_b = list_files_relative(dir_b.path()).unwrap();

        assert_eq!(files_a, vec![PathBuf::from("plugin_a.esp")]);
        assert_eq!(files_b, vec![PathBuf::from("plugin_b.esp")]);

        let mods = vec![
            ModFiles {
                mod_id: 1,
                name: "A".into(),
                files: files_a,
            },
            ModFiles {
                mod_id: 2,
                name: "B".into(),
                files: files_b,
            },
        ];
        assert!(
            find_conflicts(&mods).is_empty(),
            "meta.ini must not be treated as a shared/conflicting file"
        );
    }

    fn row(
        mod_id: i64,
        name: &str,
        source_path: PathBuf,
        enabled: bool,
        position: i64,
    ) -> LoadoutEntryRow {
        LoadoutEntryRow {
            mod_id,
            name: name.to_string(),
            source_path,
            enabled,
            position,
        }
    }

    #[test]
    fn tally_counts_wins_and_loses_per_mod() {
        let shared = PathBuf::from("interface/config.txt");
        let mods = vec![
            ModFiles {
                mod_id: 1,
                name: "Lux".into(),
                files: vec![shared.clone(), PathBuf::from("only/one.nif")],
            },
            ModFiles {
                mod_id: 2,
                name: "JK's Skyrim".into(),
                files: vec![shared],
            },
        ];

        let tallies = tally(&find_conflicts(&mods), &mods);

        assert_eq!(
            tallies,
            vec![
                ModConflictCounts {
                    mod_id: 1,
                    name: "Lux".into(),
                    wins: 0,
                    loses: 1,
                },
                ModConflictCounts {
                    mod_id: 2,
                    name: "JK's Skyrim".into(),
                    wins: 1,
                    loses: 0,
                },
            ],
            "the later mod wins the shared path, the earlier one loses it"
        );
    }

    #[test]
    fn a_three_way_conflict_is_one_win_and_two_losses() {
        let shared = PathBuf::from("meshes/architecture/riften.nif");
        let mods = vec![
            ModFiles {
                mod_id: 1,
                name: "A".into(),
                files: vec![shared.clone()],
            },
            ModFiles {
                mod_id: 2,
                name: "B".into(),
                files: vec![shared.clone()],
            },
            ModFiles {
                mod_id: 3,
                name: "C".into(),
                files: vec![shared],
            },
        ];

        let tallies = tally(&find_conflicts(&mods), &mods);

        assert_eq!(tallies[0].loses, 1);
        assert_eq!(tallies[1].loses, 1);
        assert_eq!(tallies[2].wins, 1);
        assert!(
            tallies.iter().all(|t| t.wins + t.loses <= 1),
            "one shared path is counted once per mod, not once per other mod"
        );
    }

    #[test]
    fn tally_keeps_mods_with_no_conflict_at_zero() {
        let mods = vec![
            ModFiles {
                mod_id: 1,
                name: "Lonely".into(),
                files: vec![PathBuf::from("meshes/a.nif")],
            },
            ModFiles {
                mod_id: 2,
                name: "Empty".into(),
                files: Vec::new(),
            },
        ];

        let tallies = tally(&find_conflicts(&mods), &mods);

        assert_eq!(tallies.len(), 2, "mods are never dropped from the tally");
        assert!(tallies.iter().all(|t| t.wins == 0 && t.loses == 0));
    }

    #[test]
    fn report_for_entries_walks_real_folders_and_finds_the_conflict() {
        let dir_a = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir_a.path().join("interface")).unwrap();
        fs::write(dir_a.path().join("interface/config.txt"), b"a").unwrap();

        let dir_b = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir_b.path().join("interface")).unwrap();
        fs::write(dir_b.path().join("interface/config.txt"), b"b").unwrap();

        let report = report_for_entries(&[
            row(1, "Lux", dir_a.path().to_path_buf(), true, 0),
            row(2, "SkyUI", dir_b.path().to_path_buf(), true, 1),
        ]);

        assert_eq!(report.conflicts.len(), 1);
        assert_eq!(
            report.conflicts[0].relative_path,
            PathBuf::from("interface/config.txt")
        );
        assert_eq!(report.conflicts[0].winner().0, 2);
        assert_eq!(report.mods.len(), 2);
        assert_eq!((report.mods[0].wins, report.mods[0].loses), (0, 1));
        assert_eq!((report.mods[1].wins, report.mods[1].loses), (1, 0));
        assert!(report.unreadable.is_empty());
    }

    #[test]
    fn report_for_entries_skips_disabled_mods() {
        let dir_a = tempfile::tempdir().unwrap();
        fs::write(dir_a.path().join("shared.txt"), b"a").unwrap();
        let dir_b = tempfile::tempdir().unwrap();
        fs::write(dir_b.path().join("shared.txt"), b"b").unwrap();

        let report = report_for_entries(&[
            row(1, "Enabled", dir_a.path().to_path_buf(), true, 0),
            row(2, "Disabled", dir_b.path().to_path_buf(), false, 1),
        ]);

        assert!(
            report.conflicts.is_empty(),
            "a disabled mod must not contribute files to the scan"
        );
        assert_eq!(report.mods.len(), 1);
        assert_eq!(report.mods[0].mod_id, 1);
    }

    #[test]
    fn report_for_entries_reports_an_unreadable_folder_instead_of_failing() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("shared.txt"), b"x").unwrap();

        let report = report_for_entries(&[
            row(1, "Readable", dir.path().to_path_buf(), true, 0),
            row(2, "Gone", dir.path().join("no-such-mod-folder"), true, 1),
        ]);

        assert_eq!(report.unreadable.len(), 1, "the missing folder is reported");
        assert_eq!(report.unreadable[0].0, 2);
        assert!(
            report.unreadable[0].1.contains("no-such-mod-folder"),
            "the message names the folder that failed"
        );
        assert_eq!(
            report.mods.len(),
            1,
            "the readable mod is still scanned and badgeable"
        );
        assert_eq!(report.mods[0].mod_id, 1);
    }
}
