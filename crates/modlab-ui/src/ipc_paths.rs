//! Path bounding for Tauri IPC (PowerShell cwd, `reveal_path`, launch
//! path overrides). Same idea as `install_subdir`: canonicalize, then
//! `starts_with` a known root — never trust a raw string from the WebView.

use modlab_core::Core;
use std::path::{Path, PathBuf};

use crate::downloads::{self, DOWNLOADS_DIR_SLOT, SETTINGS_SCOPE};
use crate::{adapters, resolve_slot};

/// French error when a path escapes every allowed root.
pub const OUTSIDE_ALLOWED: &str = "Chemin hors des emplacements autorisés.";

/// Strip Windows `\\?\` / `\\?\UNC\` so `starts_with` compares equal forms.
pub(crate) fn strip_verbatim(path: &Path) -> PathBuf {
    let s = path.to_string_lossy();
    if let Some(rest) = s.strip_prefix(r"\\?\UNC\") {
        PathBuf::from(format!(r"\\{rest}"))
    } else if let Some(rest) = s.strip_prefix(r"\\?\") {
        PathBuf::from(rest)
    } else {
        path.to_path_buf()
    }
}

/// Canonicalize when the path exists; otherwise absolutize + strip verbatim.
pub(crate) fn canonicalize_path(path: &Path) -> Result<PathBuf, String> {
    if path.as_os_str().is_empty() || path.to_string_lossy().contains('\0') {
        return Err(OUTSIDE_ALLOWED.into());
    }
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|e| format!("Répertoire courant illisible : {e}"))?
            .join(path)
    };
    let canon = absolute.canonicalize().unwrap_or_else(|_| absolute.clone());
    Ok(strip_verbatim(&canon))
}

pub(crate) fn is_under_root(path: &Path, root: &Path) -> bool {
    let path = strip_verbatim(path);
    let root = strip_verbatim(root);
    path == root || path.starts_with(&root)
}

fn push_existing_root(roots: &mut Vec<PathBuf>, path: &Path) {
    let Ok(canon) = canonicalize_path(path) else {
        return;
    };
    let root = if canon.is_dir() {
        canon
    } else if let Some(parent) = canon.parent() {
        parent.to_path_buf()
    } else {
        return;
    };
    if !roots.iter().any(|r| r == &root) {
        roots.push(root);
    }
}

/// Roots a WebView-supplied path may sit under: ModLab data, the user
/// profile, LocalAppData, detected game installs, human root overrides,
/// and the downloads root (default or overridden).
pub(crate) fn known_roots(data_root: &Path, core: Option<&Core>) -> Vec<PathBuf> {
    let mut roots = Vec::new();
    push_existing_root(&mut roots, data_root);
    if let Some(profile) = std::env::var_os("USERPROFILE") {
        push_existing_root(&mut roots, Path::new(&profile));
    }
    if let Some(local) = std::env::var_os("LOCALAPPDATA") {
        push_existing_root(&mut roots, Path::new(&local));
    }
    for adapter in adapters() {
        if let Some(install) = adapter.detect() {
            push_existing_root(&mut roots, &install.root);
        }
        if let Some(core) = core {
            if let Ok(Some(over)) = core.game_path_override(adapter.id(), "root") {
                push_existing_root(&mut roots, Path::new(&over));
            }
            // Effective launch slots (file → parent dir) so revealing a
            // prefs INI or usvfs DLL next to MO2 stays allowed once set.
            for slot_id in [
                "exe",
                "data_dir",
                "usvfs_dll",
                "prefs_ini",
                "saves_dir",
                "launcher_exe",
                "mods_dir",
            ] {
                if let Some(path) = resolve_slot(core, adapter, slot_id) {
                    push_existing_root(&mut roots, &path);
                }
            }
        }
    }
    if let Some(core) = core {
        let downloads = downloads::resolve_downloads_root(core, data_root);
        push_existing_root(&mut roots, &downloads);
        if let Ok(Some(over)) = core.game_path_override(SETTINGS_SCOPE, DOWNLOADS_DIR_SLOT) {
            push_existing_root(&mut roots, Path::new(&over));
        }
    } else {
        push_existing_root(&mut roots, &downloads::default_downloads_root(data_root));
    }
    roots
}

/// `path` must canonicalize under at least one of `roots`.
pub(crate) fn ensure_under_roots(path: &Path, roots: &[PathBuf]) -> Result<PathBuf, String> {
    let canon = canonicalize_path(path)?;
    if roots.iter().any(|root| is_under_root(&canon, root)) {
        return Ok(canon);
    }
    Err(OUTSIDE_ALLOWED.into())
}

fn windir_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    for key in ["WINDIR", "SYSTEMROOT"] {
        if let Some(val) = std::env::var_os(key) {
            push_existing_root(&mut roots, Path::new(&val));
        }
    }
    roots
}

fn refuses_system_dir(path: &Path) -> Result<(), String> {
    let canon = canonicalize_path(path)?;
    for root in windir_roots() {
        if is_under_root(&canon, &root) {
            return Err("Chemin système Windows refusé.".into());
        }
    }
    Ok(())
}

/// Effective install `root` for `game_id` (override wins, else detection).
fn game_install_root(core: &Core, game_id: &str) -> Option<PathBuf> {
    let adapter = adapters().into_iter().find(|a| a.id() == game_id)?;
    resolve_slot(core, adapter, "root")
}

/// Validate a Réglages → Jeux override before it is stored. Launch-critical
/// child slots must stay under the install root; `usvfs_dll` must keep the
/// expected file name.
pub(crate) fn validate_game_path_override(
    core: &Core,
    data_root: &Path,
    game_id: &str,
    slot_id: &str,
    path: &Path,
) -> Result<(), String> {
    if path.as_os_str().is_empty() || path.to_string_lossy().contains('\0') {
        return Err("Le chemin ne peut pas être vide.".into());
    }
    refuses_system_dir(path)?;

    if slot_id == "usvfs_dll" {
        modlab_game_skyrim::usvfs::check_usvfs_dll_file_name(path)
            .map_err(|_| "Le fichier doit s'appeler usvfs_x64.dll.".to_string())?;
        if !path.is_file() {
            return Err(format!("Fichier introuvable : {}", path.display()));
        }
        return Ok(());
    }

    match slot_id {
        "root" => {
            let canon = canonicalize_path(path)?;
            if !canon.is_dir() {
                return Err(format!("Pas un dossier : {}", path.display()));
            }
            Ok(())
        }
        "exe" | "data_dir" | "launcher_exe" | "mods_dir" => {
            let Some(root) = game_install_root(core, game_id) else {
                return Err(
                    "Renseigne d'abord le dossier d'installation du jeu (slot root).".into(),
                );
            };
            let root_canon = canonicalize_path(&root)?;
            let target = canonicalize_path(path)?;
            if !is_under_root(&target, &root_canon) {
                return Err("Hors du dossier d'installation.".into());
            }
            Ok(())
        }
        "prefs_ini" | "saves_dir" => {
            let roots = known_roots(data_root, Some(core));
            ensure_under_roots(path, &roots)?;
            Ok(())
        }
        _ => {
            // Unknown slot: still refuse Windows system dirs; existence is
            // the adapter's problem at use time.
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn scratch(tag: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("modlab-ipc-paths-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn ensure_under_roots_accepts_child_and_rejects_sibling() {
        let root = scratch("roots-ok");
        let child = root.join("mods");
        fs::create_dir_all(&child).unwrap();
        let outside = scratch("roots-out");
        fs::write(outside.join("x.txt"), b"x").unwrap();

        let roots = vec![canonicalize_path(&root).unwrap()];
        assert!(ensure_under_roots(&child, &roots).is_ok());
        let err = ensure_under_roots(&outside, &roots).unwrap_err();
        assert!(err.contains("autorisés"), "{err}");

        let _ = fs::remove_dir_all(&root);
        let _ = fs::remove_dir_all(&outside);
    }

    #[test]
    fn validate_usvfs_override_requires_expected_file_name() {
        let core = Core::open_in_memory().unwrap();
        let data = scratch("usvfs-over");
        let good = data.join("usvfs_x64.dll");
        fs::write(&good, b"x").unwrap();
        let bad = data.join("evil.dll");
        fs::write(&bad, b"x").unwrap();

        assert!(validate_game_path_override(&core, &data, "skyrimse", "usvfs_dll", &good).is_ok());
        let err =
            validate_game_path_override(&core, &data, "skyrimse", "usvfs_dll", &bad).unwrap_err();
        assert!(err.contains("usvfs_x64.dll"), "{err}");

        let _ = fs::remove_dir_all(&data);
    }

    #[test]
    fn validate_data_dir_must_stay_under_root() {
        let core = Core::open_in_memory().unwrap();
        let data = scratch("data-over");
        let game = scratch("game-root");
        let data_dir = game.join("Data");
        fs::create_dir_all(&data_dir).unwrap();
        let outside = scratch("other-data");
        fs::create_dir_all(&outside).unwrap();

        core.set_game_path_override("skyrimse", "root", &game.display().to_string())
            .unwrap();
        assert!(
            validate_game_path_override(&core, &data, "skyrimse", "data_dir", &data_dir).is_ok()
        );
        let err = validate_game_path_override(&core, &data, "skyrimse", "data_dir", &outside)
            .unwrap_err();
        assert!(err.contains("Hors du dossier"), "{err}");

        let _ = fs::remove_dir_all(&data);
        let _ = fs::remove_dir_all(&game);
        let _ = fs::remove_dir_all(&outside);
    }
}
