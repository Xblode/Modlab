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
}
