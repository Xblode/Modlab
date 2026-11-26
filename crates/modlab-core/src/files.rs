//! Reversible directory changes. Nothing a mod owns is deleted: it is
//! renamed into `<data root>/trash`. A replacement is prepared in a
//! sibling directory and renamed into place, so a failure halfway through
//! never leaves a partial tree at the destination.
//!
//! The temporary directory is a sibling of the destination on purpose.
//! `rename` is atomic only when both paths are on the same volume; a temp
//! directory on another drive would turn the "commit" into a copy.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::{CoreError, Result};

/// `<data root>/trash`. Created on the first move.
#[must_use]
pub fn trash_dir(data_root: &Path) -> PathBuf {
    data_root.join("trash")
}

/// Move `path` into the internal trash. The returned path is where the
/// bytes now live. `path` itself is gone.
pub fn move_to_trash(data_root: &Path, path: &Path) -> Result<PathBuf> {
    if !path.exists() {
        return Err(CoreError::Message(format!(
            "rien à mettre à la corbeille : {}",
            path.display()
        )));
    }
    let trash = trash_dir(data_root);
    fs::create_dir_all(&trash)?;
    let name = path
        .file_name()
        .unwrap_or_else(|| std::ffi::OsStr::new("item"));
    let dest = unique_in(&trash, name);
    fs::rename(path, &dest)?;
    Ok(dest)
}

/// A directory swap that puts the previous tree back unless [`DirSwap::commit`]
/// is called. Dropping it without committing is a rollback.
pub struct DirSwap {
    data_root: PathBuf,
    dest: PathBuf,
    /// Previous `dest`, already sitting in the trash. `None` when `dest`
    /// did not exist.
    backup: Option<PathBuf>,
    committed: bool,
    rolled_back: bool,
}

impl DirSwap {
    /// Build the new tree at a sibling of `dest` via `prepare`, then rename
    /// it onto `dest`. `prepare` must create `staged` itself and must not
    /// touch `dest`. If `prepare` fails, `dest` is unchanged and whatever
    /// `prepare` wrote is moved to the trash.
    pub fn replace_with(
        data_root: &Path,
        dest: &Path,
        prepare: impl FnOnce(&Path) -> Result<()>,
    ) -> Result<Self> {
        let parent = dest
            .parent()
            .filter(|dir| !dir.as_os_str().is_empty())
            .ok_or_else(|| {
                CoreError::Message(format!(
                    "dossier de destination sans parent : {}",
                    dest.display()
                ))
            })?;
        fs::create_dir_all(parent)?;
        let staged = unique_in(parent, std::ffi::OsStr::new(".modlab-new"));
        if let Err(err) = prepare(&staged) {
            if staged.exists() {
                let _ = move_to_trash(data_root, &staged);
            }
            return Err(err);
        }
        if !staged.exists() {
            return Err(CoreError::Message(
                "la préparation n'a rien écrit".to_string(),
            ));
        }

        let backup = if dest.exists() {
            match move_to_trash(data_root, dest) {
                Ok(path) => Some(path),
                Err(err) => {
                    let _ = move_to_trash(data_root, &staged);
                    return Err(err);
                }
            }
        } else {
            None
        };
        if let Err(err) = fs::rename(&staged, dest) {
            if let Some(backup) = &backup {
                let _ = fs::rename(backup, dest);
            }
            if staged.exists() {
                let _ = move_to_trash(data_root, &staged);
            }
            return Err(CoreError::Io(err));
        }
        Ok(Self {
            data_root: data_root.to_path_buf(),
            dest: dest.to_path_buf(),
            backup,
            committed: false,
            rolled_back: false,
        })
    }

    /// Keep the new tree. The previous one stays in the trash.
    pub fn commit(mut self) {
        self.committed = true;
    }

    /// Put the previous tree back at `dest`. The new tree goes to the trash.
    pub fn rollback(&mut self) -> Result<()> {
        if self.committed || self.rolled_back {
            return Ok(());
        }
        let aside = if self.dest.exists() {
            let aside = unique_in(
                self.dest.parent().unwrap_or(Path::new(".")),
                std::ffi::OsStr::new(".modlab-rollback"),
            );
            fs::rename(&self.dest, &aside)?;
            Some(aside)
        } else {
            None
        };
        if let Some(backup) = self.backup.clone() {
            if let Err(err) = fs::rename(&backup, &self.dest) {
                if let Some(aside) = &aside {
                    let _ = fs::rename(aside, &self.dest);
                }
                return Err(CoreError::Io(err));
            }
            self.backup = None;
        }
        if let Some(aside) = aside {
            let _ = move_to_trash(&self.data_root, &aside);
        }
        self.rolled_back = true;
        Ok(())
    }
}

impl Drop for DirSwap {
    fn drop(&mut self) {
        if !self.committed && !self.rolled_back {
            let _ = self.rollback();
        }
    }
}

/// Move `path` aside so a later failure can put it back. `None` when
/// `path` is already absent — there is nothing to restore.
pub fn trash_for_rollback(data_root: &Path, path: &Path) -> Result<Option<TrashGuard>> {
    if !path.exists() {
        return Ok(None);
    }
    let trashed = move_to_trash(data_root, path)?;
    Ok(Some(TrashGuard {
        original: path.to_path_buf(),
        trashed,
        committed: false,
    }))
}

/// Holds a mod directory that was moved to the trash until [`TrashGuard::commit`].
pub struct TrashGuard {
    original: PathBuf,
    trashed: PathBuf,
    committed: bool,
}

impl TrashGuard {
    /// Where the bytes sit in the trash. Valid until commit... and after,
    /// since commit leaves them there.
    #[must_use]
    pub fn trashed_path(&self) -> &Path {
        &self.trashed
    }

    /// Leave the directory in the trash for good.
    pub fn commit(mut self) {
        self.committed = true;
    }
}

impl Drop for TrashGuard {
    fn drop(&mut self) {
        if self.committed || !self.trashed.exists() {
            return;
        }
        if let Some(parent) = self.original.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let _ = fs::rename(&self.trashed, &self.original);
    }
}

fn unique_in(dir: &Path, name: &std::ffi::OsStr) -> PathBuf {
    static N: AtomicU64 = AtomicU64::new(1);
    let n = N.fetch_add(1, Ordering::Relaxed);
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    dir.join(format!(
        "{}-{millis}-{n}-{}",
        std::process::id(),
        name.to_string_lossy()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(tag: &str) -> tempfile::TempDir {
        tempfile::Builder::new()
            .prefix(&format!("modlab-files-{tag}-"))
            .tempdir()
            .unwrap()
    }

    fn tree(dir: &Path) -> Vec<(String, Vec<u8>)> {
        let mut out = Vec::new();
        fn walk(base: &Path, dir: &Path, out: &mut Vec<(String, Vec<u8>)>) {
            let Ok(read) = fs::read_dir(dir) else {
                return;
            };
            let mut entries: Vec<_> = read.flatten().collect();
            entries.sort_by_key(|entry| entry.file_name());
            for entry in entries {
                let path = entry.path();
                if path.is_dir() {
                    walk(base, &path, out);
                } else if path.is_file() {
                    let rel = path
                        .strip_prefix(base)
                        .unwrap()
                        .to_string_lossy()
                        .replace('\\', "/");
                    out.push((rel, fs::read(&path).unwrap()));
                }
            }
        }
        walk(dir, dir, &mut out);
        out
    }

    fn trash_contains(data_root: &Path, name: &str, bytes: &[u8]) -> bool {
        fn walk(dir: &Path, name: &str, bytes: &[u8]) -> bool {
            let Ok(read) = fs::read_dir(dir) else {
                return false;
            };
            for entry in read.flatten() {
                let path = entry.path();
                if path.is_file()
                    && path.file_name().and_then(|n| n.to_str()) == Some(name)
                    && fs::read(&path).ok().as_deref() == Some(bytes)
                {
                    return true;
                }
                if path.is_dir() && walk(&path, name, bytes) {
                    return true;
                }
            }
            false
        }
        walk(&trash_dir(data_root), name, bytes)
    }

    #[test]
    fn a_failed_prepare_leaves_the_destination_and_the_game_tree_intact() {
        let root = scratch("fail");
        let game = root.path().join("Game").join("Data");
        fs::create_dir_all(&game).unwrap();
        fs::write(game.join("Skyrim.esm"), b"VANILLA").unwrap();
        let before = tree(&game);

        let dest = root.path().join("mods").join("7");
        fs::create_dir_all(&dest).unwrap();
        fs::write(dest.join("keep.txt"), b"original").unwrap();

        let err = DirSwap::replace_with(root.path(), &dest, |staged| {
            fs::create_dir_all(staged)?;
            fs::write(staged.join("partial.txt"), b"half")?;
            Err(CoreError::Message("extraction interrompue".into()))
        });

        assert!(err.is_err());
        assert_eq!(fs::read(dest.join("keep.txt")).unwrap(), b"original");
        assert!(!dest.join("partial.txt").exists());
        assert_eq!(tree(&game), before, "the game Data folder is untouched");
        assert!(
            trash_contains(root.path(), "partial.txt", b"half"),
            "the partial tree is in the trash, not deleted"
        );
    }

    #[test]
    fn dropping_a_swap_restores_the_previous_tree_including_a_game_folder() {
        let root = scratch("drop");
        let data = root.path().join("Game").join("Data");
        fs::create_dir_all(&data).unwrap();
        fs::write(data.join("Skyrim.esm"), b"VANILLA").unwrap();
        let before = tree(&data);

        {
            let swap = DirSwap::replace_with(root.path(), &data, |staged| {
                fs::create_dir_all(staged)?;
                fs::write(staged.join("Skyrim.esm"), b"CHANGED")?;
                fs::write(staged.join("extra.esp"), b"mod")?;
                Ok(())
            })
            .unwrap();
            assert_eq!(fs::read(data.join("Skyrim.esm")).unwrap(), b"CHANGED");
            assert!(data.join("extra.esp").is_file());
            drop(swap);
        }

        assert_eq!(tree(&data), before, "rollback puts the game tree back");
        assert!(trash_contains(root.path(), "extra.esp", b"mod"));
    }

    #[test]
    fn commit_keeps_the_new_tree_and_the_old_one_in_the_trash() {
        let root = scratch("commit");
        let dest = root.path().join("mods").join("3");
        fs::create_dir_all(&dest).unwrap();
        fs::write(dest.join("old.txt"), b"old").unwrap();

        let swap = DirSwap::replace_with(root.path(), &dest, |staged| {
            fs::create_dir_all(staged)?;
            fs::write(staged.join("new.txt"), b"new")?;
            Ok(())
        })
        .unwrap();
        swap.commit();

        assert_eq!(fs::read(dest.join("new.txt")).unwrap(), b"new");
        assert!(!dest.join("old.txt").exists());
        assert!(trash_contains(root.path(), "old.txt", b"old"));
    }

    #[test]
    fn uninstall_moves_the_mod_folder_to_the_trash_and_can_restore_it() {
        let root = scratch("trash");
        let game = root.path().join("Game").join("Data");
        fs::create_dir_all(&game).unwrap();
        fs::write(game.join("Skyrim.esm"), b"VANILLA").unwrap();
        let before = tree(&game);

        let dest = root.path().join("mods").join("9");
        fs::create_dir_all(dest.join("meshes")).unwrap();
        fs::write(dest.join("meshes").join("a.nif"), b"mesh").unwrap();

        {
            let guard = trash_for_rollback(root.path(), &dest).unwrap().unwrap();
            assert!(!dest.exists());
            assert_eq!(
                fs::read(guard.trashed_path().join("meshes").join("a.nif")).unwrap(),
                b"mesh"
            );
            // Drop without commit: the folder comes back.
        }
        assert_eq!(
            fs::read(dest.join("meshes").join("a.nif")).unwrap(),
            b"mesh"
        );

        let guard = trash_for_rollback(root.path(), &dest).unwrap().unwrap();
        let trashed = guard.trashed_path().to_path_buf();
        guard.commit();
        assert!(!dest.exists());
        assert_eq!(
            fs::read(trashed.join("meshes").join("a.nif")).unwrap(),
            b"mesh"
        );
        assert_eq!(tree(&game), before);
    }
}
