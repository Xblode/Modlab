//! A folder owned by one instance, when the game's adapter asks for one.
//!
//! The Core does not know which game that is. The caller passes an
//! [`InstanceRootPolicy`]. The path is stored on the instance row the first
//! time it is assigned and is never rebuilt from the name, so a rename does
//! not move the folder. Empty directories are a journaled mutation. Nothing
//! already on disk is moved here.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use crate::game_adapter::InstanceRootPolicy;
use crate::transaction::Transaction;
use crate::{Core, CoreError, Result};

/// Stable folder name for an instance title.
///
/// Lower case, accents folded, Windows-forbidden characters removed.
/// Whitespace becomes a single hyphen. An empty result becomes `instance`.
#[must_use]
pub fn instance_slug(name: &str) -> String {
    let mut out = String::new();
    let mut hyphen = false;
    for ch in name.chars().flat_map(|ch| ch.to_lowercase()) {
        let ch = fold_char(ch);
        if ch.is_ascii_alphanumeric() {
            out.push(ch);
            hyphen = false;
        } else if ch.is_whitespace() || ch == '-' || ch == '_' {
            if !out.is_empty() && !hyphen {
                out.push('-');
                hyphen = true;
            }
        } else if is_forbidden(ch) || ch.is_control() || ch == '.' {
            // Dropped. A dot would nest the folder; the caller adds the
            // leading dot that hides the directory.
        } else if ch.is_alphanumeric() {
            out.push(ch);
            hyphen = false;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    if out.is_empty() {
        "instance".to_string()
    } else {
        out
    }
}

fn fold_char(ch: char) -> char {
    match ch {
        'é' | 'è' | 'ê' | 'ë' => 'e',
        'à' | 'â' | 'ä' | 'á' => 'a',
        'ù' | 'û' | 'ü' | 'ú' => 'u',
        'ô' | 'ö' | 'ó' => 'o',
        'î' | 'ï' | 'í' => 'i',
        'ç' => 'c',
        'ñ' => 'n',
        other => other,
    }
}

fn is_forbidden(ch: char) -> bool {
    matches!(ch, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*')
}

/// The folder already stored for this instance. Does not assign one.
pub fn stored_instance_root(core: &Core, instance_id: i64) -> Result<Option<PathBuf>> {
    Ok(core
        .instance_by_id(instance_id)?
        .and_then(|row| row.root_path)
        .filter(|path| !path.is_empty())
        .map(PathBuf::from))
}

/// The stored folder for `instance_id`, assigning one when the policy asks
/// for a per-instance root and none is stored yet.
///
/// `Ok(None)` for [`InstanceRootPolicy::SharedInstall`]. Existing files are
/// not moved. A name collision, or a directory that is already there, gets
/// the next free `-<n>` suffix starting at 2.
pub fn ensure_instance_root(
    core: &Core,
    data_root: &Path,
    instance_id: i64,
    policy: InstanceRootPolicy,
) -> Result<Option<PathBuf>> {
    let InstanceRootPolicy::PerInstanceRoot { dir_name, subdirs } = policy else {
        return Ok(None);
    };
    let Some(row) = core.instance_by_id(instance_id)? else {
        return Err(CoreError::Message(format!(
            "instance {instance_id} introuvable"
        )));
    };
    if let Some(existing) = row.root_path {
        let path = PathBuf::from(existing);
        create_layout(&path, subdirs)?;
        return Ok(Some(path));
    }

    let parent = data_root.join("instances").join(dir_name);
    let taken = core.stored_instance_roots()?;
    let path = allocate_folder(&parent, &instance_slug(&row.name), &taken);
    create_layout(&path, subdirs)?;
    if let Err(err) = core.set_instance_root_path(instance_id, &path.display().to_string()) {
        let _ = crate::files::move_to_trash(data_root, &path);
        return Err(err);
    }
    let mut tx = Transaction::new(core, "instance.root");
    tx.step(format!(
        "dossier de l'instance {instance_id} : {}",
        path.display()
    ));
    tx.commit()?;
    Ok(Some(path))
}

fn create_layout(root: &Path, subdirs: &[&str]) -> Result<()> {
    fs::create_dir_all(root)?;
    for name in subdirs {
        if name.is_empty() || name.contains(['/', '\\']) {
            continue;
        }
        fs::create_dir_all(root.join(name))?;
    }
    Ok(())
}

fn allocate_folder(parent: &Path, slug: &str, taken: &[String]) -> PathBuf {
    let taken: HashSet<PathBuf> = taken.iter().map(PathBuf::from).collect();
    for n in 1..10_000 {
        let folder = if n == 1 {
            format!(".{slug}")
        } else {
            format!(".{slug}-{n}")
        };
        let path = parent.join(folder);
        let used = path.exists() || taken.iter().any(|other| same_path(other, &path));
        if !used {
            return path;
        }
    }
    parent.join(format!(".{slug}-overflow"))
}

fn same_path(left: &Path, right: &Path) -> bool {
    left.components().eq(right.components())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::loadout;

    #[test]
    fn slug_folds_accents_and_drops_forbidden_characters() {
        assert_eq!(instance_slug("Décube"), "decube");
        assert_eq!(instance_slug("  Décube  "), "decube");
        assert_eq!(instance_slug("Foo Bar"), "foo-bar");
        assert_eq!(instance_slug("a/b:c*"), "abc");
        assert_eq!(instance_slug("***"), "instance");
    }

    #[test]
    fn a_second_instance_with_the_same_name_gets_a_suffix_and_a_rename_does_not_move_it() {
        let core = Core::open_in_memory().unwrap();
        let data = tempfile::tempdir().unwrap();
        let policy = InstanceRootPolicy::PerInstanceRoot {
            dir_name: "minecraft",
            subdirs: &["mods", "config"],
        };
        let first = loadout::create_instance(&core, "minecraft", "Décube").unwrap();
        let second = loadout::create_instance(&core, "minecraft", "decube").unwrap();

        let root_a = ensure_instance_root(&core, data.path(), first.instance_id, policy)
            .unwrap()
            .unwrap();
        let root_b = ensure_instance_root(&core, data.path(), second.instance_id, policy)
            .unwrap()
            .unwrap();

        assert_eq!(
            root_a,
            data.path()
                .join("instances")
                .join("minecraft")
                .join(".decube")
        );
        assert_eq!(
            root_b,
            data.path()
                .join("instances")
                .join("minecraft")
                .join(".decube-2")
        );
        assert!(root_a.join("mods").is_dir());
        assert!(root_b.join("config").is_dir());

        loadout::rename_instance_of_loadout(&core, first.id, "Autre nom").unwrap();
        let again = ensure_instance_root(&core, data.path(), first.instance_id, policy)
            .unwrap()
            .unwrap();
        assert_eq!(again, root_a, "a rename must not move the stored folder");
    }

    #[test]
    fn shared_install_assigns_nothing() {
        let core = Core::open_in_memory().unwrap();
        let data = tempfile::tempdir().unwrap();
        let created = loadout::create(&core, "skyrimse", "Test").unwrap();
        let assigned = ensure_instance_root(
            &core,
            data.path(),
            created.instance_id,
            InstanceRootPolicy::SharedInstall,
        )
        .unwrap();
        assert!(assigned.is_none());
        assert!(!data.path().join("instances").exists());
    }
}
