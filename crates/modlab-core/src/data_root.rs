//! The one data root shared by every client (UI, CLI, and later MCP).
//!
//! `MODLAB_DATA_ROOT` wins when it is set. Otherwise the real root
//! `F:\ModLab` when its database is already there, otherwise the
//! `.modlab-dev` sandbox when *that* database is there, otherwise
//! `F:\ModLab` when the directory exists, otherwise `.modlab-dev`.
//! A client must not invent a different order.

use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};

use crate::{Core, Result};

/// Resolve the data root from the process environment.
#[must_use]
pub fn resolve_data_root() -> PathBuf {
    resolve_data_root_from(
        std::env::var_os("MODLAB_DATA_ROOT"),
        Path::new(r"F:\ModLab"),
        Path::new(".modlab-dev"),
    )
}

/// Same rule as [`resolve_data_root`], with the three inputs passed in so
/// tests never depend on this machine's `F:\ModLab` or on the process env.
#[must_use]
pub fn resolve_data_root_from(
    from_env: Option<impl AsRef<OsStr>>,
    real: &Path,
    sandbox: &Path,
) -> PathBuf {
    if let Some(from_env) = from_env {
        return PathBuf::from(from_env.as_ref());
    }
    if database_present(real) {
        return real.to_path_buf();
    }
    if database_present(sandbox) {
        return sandbox.to_path_buf();
    }
    if real.is_dir() {
        return real.to_path_buf();
    }
    sandbox.to_path_buf()
}

fn database_present(root: &Path) -> bool {
    root.join("db").join("modlab.db").is_file() || root.join("modlab.db").is_file()
}

/// Open the engine database under `data_root`, creating the directory when
/// needed. An existing `db/modlab.db` wins; a flat `modlab.db` left by an
/// older CLI is still opened; a brand-new root gets `db/modlab.db`.
pub fn open_at(data_root: &Path) -> Result<Core> {
    fs::create_dir_all(data_root)?;
    let nested = data_root.join("db").join("modlab.db");
    let flat = data_root.join("modlab.db");
    let db_path = if nested.is_file() {
        nested
    } else if flat.is_file() {
        flat
    } else {
        fs::create_dir_all(data_root.join("db"))?;
        nested
    };
    Core::open(db_path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "modlab-root-{tag}-{}-{}",
            std::process::id(),
            unique()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn unique() -> u64 {
        use std::sync::atomic::{AtomicU64, Ordering};
        static N: AtomicU64 = AtomicU64::new(1);
        N.fetch_add(1, Ordering::Relaxed)
    }

    #[test]
    fn env_wins_over_a_real_database() {
        let real = scratch("real");
        fs::create_dir_all(real.join("db")).unwrap();
        fs::write(real.join("db").join("modlab.db"), b"x").unwrap();
        let sandbox = scratch("sandbox");
        let chosen = resolve_data_root_from(Some("D:/custom"), &real, &sandbox);
        assert_eq!(chosen, PathBuf::from("D:/custom"));
        let _ = fs::remove_dir_all(real);
        let _ = fs::remove_dir_all(sandbox);
    }

    #[test]
    fn a_real_database_wins_over_the_sandbox() {
        let real = scratch("real-db");
        fs::write(real.join("modlab.db"), b"x").unwrap();
        let sandbox = scratch("sandbox-db");
        fs::create_dir_all(sandbox.join("db")).unwrap();
        fs::write(sandbox.join("db").join("modlab.db"), b"y").unwrap();
        assert_eq!(resolve_data_root_from(None::<&str>, &real, &sandbox), real);
        let _ = fs::remove_dir_all(real);
        let _ = fs::remove_dir_all(sandbox);
    }

    #[test]
    fn a_sandbox_database_wins_over_an_empty_real_directory() {
        let real = scratch("real-empty");
        let sandbox = scratch("sandbox-only");
        fs::write(sandbox.join("modlab.db"), b"y").unwrap();
        assert_eq!(
            resolve_data_root_from(None::<&str>, &real, &sandbox),
            sandbox
        );
        let _ = fs::remove_dir_all(real);
        let _ = fs::remove_dir_all(sandbox);
    }

    #[test]
    fn an_empty_real_directory_wins_over_a_missing_sandbox() {
        let real = scratch("real-dir");
        let sandbox = real.join("does-not-exist");
        assert_eq!(resolve_data_root_from(None::<&str>, &real, &sandbox), real);
        let _ = fs::remove_dir_all(real);
    }

    #[test]
    fn nothing_present_falls_back_to_the_sandbox_path() {
        let base = scratch("none");
        let real = base.join("real");
        let sandbox = base.join("sandbox");
        assert_eq!(
            resolve_data_root_from(None::<&str>, &real, &sandbox),
            sandbox
        );
        let _ = fs::remove_dir_all(base);
    }

    #[test]
    fn open_at_creates_the_nested_database_and_reopens_a_flat_one() {
        let root = scratch("open");
        let core = open_at(&root).unwrap();
        drop(core);
        assert!(root.join("db").join("modlab.db").is_file());

        let flat_root = scratch("flat");
        fs::write(flat_root.join("modlab.db"), b"").unwrap();
        // An empty file is not a database; open_at must still *choose* it
        // rather than create a second one beside it. Opening may fail.
        let chosen_flat = flat_root.join("modlab.db").is_file()
            && !flat_root.join("db").join("modlab.db").is_file();
        assert!(chosen_flat);
        let _ = open_at(&flat_root);
        assert!(
            !flat_root.join("db").join("modlab.db").is_file(),
            "a pre-existing flat database stays the one that is opened"
        );
        let _ = fs::remove_dir_all(root);
        let _ = fs::remove_dir_all(flat_root);
    }
}
