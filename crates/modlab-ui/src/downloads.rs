//! Download capture for the embedded browser.
//!
//! The rule this module exists to enforce: ModLab never *initiates* a
//! download. It observes the ones the human started by clicking a link in the
//! browser tab — with their own session — and gives them a place to land plus
//! a row in the Core. Nothing here scripts, scrapes or batches anything, per
//! `CLAUDE.md`'s standing Nexus constraint.
//!
//! Captured files land in `<data root>/downloads/`, never in the OS Downloads
//! folder: the point of the feature is that a file is tracked from the moment
//! it arrives, and a file in a shared folder with everything else is not.

use modlab_core::{download, Core};
use std::path::{Path, PathBuf};
use tauri::{Emitter, Manager, Webview};

use crate::AppState;

/// Not a real game id — the key the downloads-root override is filed under
/// in the same `game_path_override` table a real game's path overrides use
/// (generic `(game_id, slot_id) -> path`, reused rather than duplicated for
/// one more setting).
pub const SETTINGS_SCOPE: &str = "_app";
pub const DOWNLOADS_DIR_SLOT: &str = "downloads_dir";

/// Where captures land absent any override.
pub fn default_downloads_root(data_root: &Path) -> PathBuf {
    data_root.join("downloads")
}

/// The downloads root a human chose from Réglages, or the default.
pub fn resolve_downloads_root(core: &Core, data_root: &Path) -> PathBuf {
    core.game_path_override(SETTINGS_SCOPE, DOWNLOADS_DIR_SLOT)
        .ok()
        .flatten()
        .map(PathBuf::from)
        .unwrap_or_else(|| default_downloads_root(data_root))
}

/// Emitted the moment a capture lands, so the shell can put the mod in the
/// table without waiting for anything. Carries the Core's own row id — the
/// download exists in the Core whether or not the UI was listening.
#[derive(serde::Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CapturedDownload {
    pub download_id: i64,
    pub file_name: String,
    pub url: String,
    pub path: String,
}

/// Where captures for `game_id` land under `root` (itself already resolved —
/// the override from Réglages, or `default_downloads_root`) — ranged by game
/// so a shared downloads folder doesn't become one flat pile. `None`/empty
/// `game_id` (no active game when the browser was opened) files under
/// `_non-classes` rather than the root itself, so it's still obviously a
/// bucket to sort later rather than looking like every other game's files.
///
/// Absolute, with native separators, both non-negotiable: WebView2 refuses a
/// relative destination outright and is unhappy with the mixed `C:/a\b` form
/// that comes out of joining onto a forward-slash root. `resolve_data_root`
/// can legitimately return a relative `.modlab-dev`, so this cannot assume
/// it was handed an absolute path.
pub fn downloads_dir(root: &Path, game_id: Option<&str>) -> PathBuf {
    let sub = game_id.filter(|g| !g.is_empty()).unwrap_or("_non-classes");
    let dir = root.join(sub);
    let absolute = if dir.is_absolute() {
        dir
    } else {
        std::env::current_dir()
            .map(|cwd| cwd.join(&dir))
            .unwrap_or(dir)
    };
    native_separators(absolute)
}

#[cfg(windows)]
fn native_separators(path: PathBuf) -> PathBuf {
    PathBuf::from(path.to_string_lossy().replace('/', "\\"))
}

#[cfg(not(windows))]
fn native_separators(path: PathBuf) -> PathBuf {
    path
}

/// Pick the name the file gets on disk, avoiding collisions by suffixing
/// rather than overwriting — a re-download of the same mod must never
/// silently replace the archive an existing entry still points at.
fn unique_destination(dir: &Path, file_name: &str) -> PathBuf {
    let candidate = dir.join(file_name);
    if !candidate.exists() {
        return candidate;
    }
    let stem = Path::new(file_name)
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| file_name.to_string());
    let ext = Path::new(file_name)
        .extension()
        .map(|s| format!(".{}", s.to_string_lossy()))
        .unwrap_or_default();
    for n in 2..10_000 {
        let candidate = dir.join(format!("{stem} ({n}){ext}"));
        if !candidate.exists() {
            return candidate;
        }
    }
    dir.join(file_name)
}

/// Redirect the browser's own download into ModLab's downloads folder.
pub fn on_requested(webview: &Webview, url: &str, destination: &mut PathBuf) {
    let Some(state) = webview.try_state::<AppState>() else {
        return;
    };
    let Ok(core) = state.core.lock() else {
        return;
    };
    let root = resolve_downloads_root(&core, &state.data_root);
    drop(core);
    let game_id = state.active_game.lock().ok().and_then(|g| g.clone());
    let dir = downloads_dir(&root, game_id.as_deref());
    if std::fs::create_dir_all(&dir).is_err() {
        // Leave the browser's own destination alone rather than pointing it
        // at a folder that could not be created.
        return;
    }
    let file_name = destination
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .or_else(|| {
            url.rsplit('/')
                .next()
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string())
        })
        .unwrap_or_else(|| "telechargement".to_string());

    *destination = unique_destination(&dir, &file_name);
}

/// Record the finished file in the Core and tell the shell about it.
pub fn on_finished(webview: &Webview, url: &str, path: Option<PathBuf>, success: bool) {
    if !success {
        // Worth a line: a silent failure here looks exactly like "the site
        // never started a download", and the two need different fixes.
        eprintln!("[dl] échec du téléchargement de {url}");
        return;
    }
    let Some(path) = path else { return };
    let Some(state) = webview.try_state::<AppState>() else {
        return;
    };
    let Ok(core) = state.core.lock() else { return };

    let file_name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "telechargement".to_string());
    let size = std::fs::metadata(&path).map(|m| m.len() as i64).ok();

    let Ok(outcome) = download::capture(&core, url, &file_name, &path, size) else {
        return;
    };
    drop(core);

    // A dedup means `path` (what the browser just wrote) is now a redundant
    // copy of the existing capture's own file — clean it up and report the
    // existing one instead, so the shell shows one entry, not two.
    let (download_id, reported_path) = match outcome {
        download::CaptureOutcome::New(id) => (id, path),
        download::CaptureOutcome::Deduped {
            existing_id,
            existing_path,
        } => {
            if existing_path != path {
                let _ = std::fs::remove_file(&path);
            }
            (existing_id, existing_path)
        }
    };
    let path = reported_path;

    let _ = webview.app_handle().emit(
        "download://captured",
        CapturedDownload {
            download_id,
            file_name,
            url: url.to_string(),
            path: path.display().to_string(),
        },
    );
}
