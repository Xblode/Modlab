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
//! the human's click starts it, and a file in a shared folder with everything
//! else is not.

use modlab_core::{download, Core};
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Emitter, Manager, Webview};

use crate::AppState;

/// How far a finish hook is allowed to go when nothing is still in flight.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FinishMode {
    /// Tauri's `DownloadEvent::Finished`: file an unfiled capture when needed.
    Full,
    /// WebView2 `StateChanged` backup: only promote a row that is still
    /// `downloading`. Skips capture so a second finish cannot duplicate.
    TrackedOnly,
}

/// What [`finalize_by_id`] did — the shell uses this when progress hit 100 %
/// but `download://captured` never arrived.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum FinalizeOutcome {
    /// Mod promoted (or was already archived); `download://captured` emitted.
    Completed,
    /// Still `downloading` and the file is not on disk yet.
    NotReady,
    /// Unknown id, or the row is no longer downloading and needs no event.
    AlreadyDone,
    /// File missing after the shell asked to give up; row abandoned + failed event.
    Abandoned,
}

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

/// Emitted the moment a transfer is filed into a loadout, before the file
/// has arrived. The Mods table listens and refreshes.
#[derive(serde::Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DownloadStarted {
    pub download_id: i64,
    pub mod_id: i64,
    pub loadout_id: i64,
    pub file_name: String,
    pub display_name: String,
    /// Mod / listing page the browser was on when the click started — the
    /// shell restores it so the human is not left on a CDN blank.
    pub page_url: Option<String>,
}

/// Emitted when a transfer fails or is cancelled. The in-progress mod row,
/// if one was filed, has already been hidden.
#[derive(serde::Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DownloadFailed {
    pub file_name: String,
    pub loadout_id: Option<i64>,
    /// French sentence the shell can show; empty when the browser cancelled.
    pub message: String,
}

/// Emitted the moment a capture lands, so the shell can put the mod in the
/// table without waiting for anything. Carries the Core's own row id — the
/// download exists in the Core whether or not the UI was listening.
///
/// `already_filed` means `begin` already put a row in the loadout and
/// `complete` just promoted it. The shell refreshes; it must not attach again.
#[derive(serde::Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CapturedDownload {
    pub download_id: i64,
    pub file_name: String,
    pub url: String,
    pub path: String,
    pub already_filed: bool,
    pub loadout_id: Option<i64>,
    pub display_name: String,
    /// Set when `begin` already filed a mod — shell can flip État to Archive
    /// without waiting for a full snapshot.
    pub mod_id: Option<i64>,
}

/// Page and host captured when a transfer starts. The finish event uses
/// this when the download was not filed into a loadout at the start.
#[derive(Clone)]
pub struct PendingOrigin {
    pub file_url: String,
    pub source_host: String,
    pub page_url: Option<String>,
}

/// One transfer `begin` filed, remembered so the finish event can find it
/// even when the path string the browser reports doesn't match byte for byte.
#[derive(Clone)]
pub struct InFlight {
    pub download_id: i64,
    pub loadout_id: i64,
    pub file_name: String,
    pub url: String,
    pub path: PathBuf,
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

/// Redirect the browser's own download into ModLab's downloads folder, and
/// when an instance is on screen file a `downloading` mod into it immediately.
pub fn on_requested(webview: &Webview, url: &str, destination: &mut PathBuf) {
    let Some(state) = webview.try_state::<AppState>() else {
        return;
    };
    // Same URL already in flight (double DownloadStarting / redirect echo):
    // reuse that row and path — a second `begin` would leave a stuck twin.
    if let Some(existing) = peek_in_flight_by_url(&state, url) {
        *destination = existing.path;
        return;
    }
    let Ok(core) = state.core.lock() else {
        return;
    };
    let root = resolve_downloads_root(&core, &state.data_root);
    drop(core);
    let game_id = state.active_game.lock().ok().and_then(|g| g.clone());
    let loadout_id = state.active_loadout.lock().ok().and_then(|id| *id);
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
    let final_name = destination
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or(file_name);
    // Same file name already downloading in this loadout: do not insert again,
    // but keep an in-flight handle so finish/progress can still find the row
    // when the CDN URL no longer matches the first begin.
    if let Some(loadout_id) = loadout_id {
        let Ok(core) = state.core.lock() else {
            return;
        };
        if let Ok(Some((download_id, path))) =
            download::in_progress_named_in_loadout(&core, &final_name, loadout_id)
        {
            *destination = path.clone();
            drop(core);
            remember_in_flight(
                &state,
                InFlight {
                    download_id,
                    loadout_id,
                    file_name: final_name,
                    url: url.to_string(),
                    path,
                },
            );
            return;
        }
        drop(core);
    }
    let page = state
        .browser_page
        .lock()
        .ok()
        .and_then(|guard| guard.clone());
    let parsed = crate::source_page::capture_origin(page.as_deref(), url);
    let origin = download::DownloadOrigin {
        source_host: parsed.source_host.to_string(),
        page_url: parsed.page_url.clone(),
    };
    remember_pending(
        &state,
        PendingOrigin {
            file_url: url.to_string(),
            source_host: origin.source_host.clone(),
            page_url: origin.page_url.clone(),
        },
    );
    let Some(loadout_id) = loadout_id else {
        // No instance on screen: the file still lands in the downloads
        // folder, and `on_finished` files an unfiled capture as before.
        // The page snapshot stays in `pending_origins` until then.
        return;
    };
    let started = {
        let Ok(core) = state.core.lock() else {
            return;
        };
        download::begin_with_origin(&core, url, &final_name, destination, loadout_id, &origin)
    };
    match started {
        Ok(started) => {
            remember_in_flight(
                &state,
                InFlight {
                    download_id: started.download_id,
                    loadout_id: started.loadout_id,
                    file_name: started.file_name.clone(),
                    url: url.to_string(),
                    path: destination.clone(),
                },
            );
            let _ = webview.app_handle().emit(
                "download://started",
                DownloadStarted {
                    download_id: started.download_id,
                    mod_id: started.mod_id,
                    loadout_id: started.loadout_id,
                    file_name: started.file_name,
                    display_name: started.name,
                    page_url: origin.page_url.clone(),
                },
            );
        }
        Err(e) => {
            // The browser download still proceeds. `on_finished` will file
            // an unfiled capture, which the shell attaches the old way.
            eprintln!("[dl] pas de ligne au démarrage de {url} : {e}");
        }
    }
}

/// Record the finished file in the Core and tell the shell about it.
///
/// A transfer that was filed at start is promoted in place (archived, and
/// renamed when the Skyrim adapter recognises it). Anything else — no
/// instance was on screen when it started — stays the unfiled capture the
/// shell attaches itself.
///
/// Matching is deliberately loose: CDN redirects and WebView2 path spelling
/// often mean the finish URL/path differ from `begin`. Falling through to a
/// fresh `capture` while a `downloading` row still exists is what produced
/// duplicates and rows stuck on « téléchargement ».
///
/// Byte counts are not part of this hook. `download_progress` emits
/// `download://progress` from WebView2 while the transfer is in flight.
pub fn on_finished(webview: &Webview, url: &str, path: Option<PathBuf>, success: bool) {
    // WebView2 download callbacks run on the UI thread. Promote/SQLite must
    // not block it — that froze the shell for the whole of `complete`.
    let app = webview.app_handle().clone();
    let url = url.to_string();
    std::thread::spawn(move || {
        finish(&app, &url, path, success, FinishMode::Full);
    });
}

/// WebView2 `StateChanged` reached a terminal state. Same promote path as
/// Tauri's Finished, but never files a fresh capture — safe when both hooks
/// fire for one transfer (progress already showed 100 %).
pub fn on_operation_terminal(app: &AppHandle, url: &str, path: Option<PathBuf>, success: bool) {
    let app = app.clone();
    let url = url.to_string();
    std::thread::spawn(move || {
        finish(&app, &url, path, success, FinishMode::TrackedOnly);
    });
}

/// Shell safety net when WebView2 never emits Finished / `download://captured`
/// (Mouse Tweaks / Modrinth CDN typical). Reconciles disk → Core `complete`
/// (archived); on `give_up` with no file → Core `abandon` (failed) + FR toast.
///
/// Browser-first: never starts a second HTTP download — only observes the
/// file WebView2 was told to write.
pub fn finalize_by_id(app: &AppHandle, download_id: i64, give_up: bool) -> FinalizeOutcome {
    let Some(state) = app.try_state::<AppState>() else {
        return FinalizeOutcome::AlreadyDone;
    };

    let Ok(core) = state.core.lock() else {
        return FinalizeOutcome::NotReady;
    };
    let Some(row) = download::progress_row(&core, download_id).ok().flatten() else {
        drop(core);
        let _ = take_in_flight_by_id(&state, download_id);
        return FinalizeOutcome::AlreadyDone;
    };
    if row.status != modlab_core::loadout::STATUS_DOWNLOADING {
        drop(core);
        let _ = take_in_flight_by_id(&state, download_id);
        return FinalizeOutcome::AlreadyDone;
    }
    let path = row.path;
    let file_name = row.file_name;
    let loadout_id = row.loadout_id;
    let url = row.url;
    drop(core);

    if !file_ready(&path) {
        if !give_up {
            // Keep in-flight so a late Tauri Finished / StateChanged can still match.
            eprintln!(
                "[dl] finalize #{download_id} : fichier pas prêt ({})",
                path.display()
            );
            return FinalizeOutcome::NotReady;
        }
        eprintln!(
            "[dl] abandon #{download_id} : fichier introuvable ({})",
            path.display()
        );
        let _ = take_in_flight_by_id(&state, download_id);
        let abandoned =
            abandon_in_progress(&state, None, Some(path.as_path()), Some(file_name.as_str()));
        let _ = app.emit(
            "download://failed",
            DownloadFailed {
                file_name,
                loadout_id: abandoned.or(loadout_id),
                message: "Téléchargement abandonné : le fichier n’est jamais arrivé.".into(),
            },
        );
        return FinalizeOutcome::Abandoned;
    }

    // Completing: drop in-flight so a duplicate Finished is a no-op.
    let _ = take_in_flight_by_id(&state, download_id);
    eprintln!("[dl] reconcile disque #{download_id} → {}", path.display());
    match promote_tracked(app, &state, download_id, &url, &path, &file_name, None) {
        PromoteResult::Emitted => FinalizeOutcome::Completed,
        PromoteResult::Failed => FinalizeOutcome::Abandoned,
    }
}

/// Non-empty file on disk — WebView2 may create a 0-byte stub before writing.
fn file_ready(path: &Path) -> bool {
    std::fs::metadata(path)
        .map(|meta| meta.is_file() && meta.len() > 0)
        .unwrap_or(false)
}

fn finish(app: &AppHandle, url: &str, path: Option<PathBuf>, success: bool, mode: FinishMode) {
    let Some(state) = app.try_state::<AppState>() else {
        return;
    };
    let pending = match mode {
        FinishMode::Full => take_pending(&state, url),
        FinishMode::TrackedOnly => None,
    };
    let file_hint = path
        .as_ref()
        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()));
    let tracked = take_in_flight(&state, path.as_deref(), url, file_hint.as_deref());

    if !success {
        // Worth a line: a silent failure here looks exactly like "the site
        // never started a download", and the two need different fixes.
        eprintln!("[dl] échec du téléchargement de {url}");
        let file_name = event_file_name(path.as_deref(), tracked.as_ref(), url);
        let loadout_id = abandon_in_progress(
            &state,
            tracked.as_ref(),
            path.as_deref(),
            Some(file_name.as_str()),
        );
        let _ = app.emit(
            "download://failed",
            DownloadFailed {
                file_name,
                loadout_id,
                message: "Téléchargement interrompu ou annulé.".into(),
            },
        );
        return;
    }

    let Some(path) = path.or_else(|| tracked.as_ref().map(|item| item.path.clone())) else {
        if tracked.is_some() {
            eprintln!("[dl] fin sans chemin pour {url}");
            let file_name = event_file_name(None, tracked.as_ref(), url);
            let loadout_id = abandon_in_progress(&state, tracked.as_ref(), None, Some(&file_name));
            let _ = app.emit(
                "download://failed",
                DownloadFailed {
                    file_name,
                    loadout_id,
                    message: "Téléchargement terminé sans fichier.".into(),
                },
            );
        }
        return;
    };
    let file_name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "telechargement".to_string());

    let Ok(core) = state.core.lock() else { return };
    let in_progress = tracked
        .as_ref()
        .map(|item| item.download_id)
        .or_else(|| download::in_progress_at(&core, &path).ok().flatten())
        .or_else(|| {
            download::in_progress_named(&core, &file_name)
                .ok()
                .flatten()
        });

    if let Some(download_id) = in_progress {
        drop(core);
        let _ = promote_tracked(
            app,
            &state,
            download_id,
            url,
            &path,
            &file_name,
            tracked.as_ref(),
        );
        return;
    }

    if mode == FinishMode::TrackedOnly {
        // Tauri Finished already promoted, or this was never begun.
        return;
    }

    // Already archived at this path (second finish after a successful first).
    if download_already_filed(&core, &path, &file_name) {
        return;
    }

    let size = std::fs::metadata(&path).map(|m| m.len() as i64).ok();
    let filed_origin = pending.map(|item| download::DownloadOrigin {
        source_host: item.source_host,
        page_url: item.page_url,
    });
    let Ok(outcome) =
        download::capture_with_origin(&core, url, &file_name, &path, size, filed_origin.as_ref())
    else {
        eprintln!("[dl] capture impossible pour {url}");
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
            if !same_file(&existing_path, &path) {
                let _ = std::fs::remove_file(&path);
            }
            (existing_id, existing_path)
        }
    };

    let _ = app.emit(
        "download://captured",
        CapturedDownload {
            download_id,
            file_name: file_name.clone(),
            url: url.to_string(),
            path: reported_path.display().to_string(),
            already_filed: false,
            loadout_id: None,
            display_name: file_name,
            mod_id: None,
        },
    );
}

enum PromoteResult {
    Emitted,
    Failed,
}

fn promote_tracked(
    app: &AppHandle,
    state: &AppState,
    download_id: i64,
    url: &str,
    path: &Path,
    file_name: &str,
    tracked: Option<&InFlight>,
) -> PromoteResult {
    let size = std::fs::metadata(path).map(|m| m.len() as i64).ok();
    let Ok(core) = state.core.lock() else {
        return PromoteResult::Failed;
    };
    if let Err(err) = download::retarget_path(&core, download_id, path) {
        eprintln!("[dl] chemin non mis à jour ({url}) : {err}");
    }
    let recognized = modlab_game_skyrim::tool_archive::recognize(file_name, url);
    let version = recognized.as_ref().and_then(|tool| tool.version.clone());
    let name = recognized.as_ref().map(|tool| tool.display_name);
    match download::complete(&core, download_id, size, name, version.as_deref()) {
        Ok(finished) => {
            let loadout_id = finished.loadout_id;
            let mod_id = finished.mod_id;
            drop(core);
            if let Some(extra) = &finished.redundant_path {
                if !same_file(extra, &finished.archive_path) {
                    let _ = std::fs::remove_file(extra);
                }
            }
            // Emit before metadata reads so the UI leaves « téléchargement »
            // even when jar/archive parsing is slow (Minecraft freeze).
            let _ = app.emit(
                "download://captured",
                CapturedDownload {
                    download_id: finished.download_id,
                    file_name: finished.file_name,
                    url: url.to_string(),
                    path: finished.archive_path.display().to_string(),
                    already_filed: true,
                    loadout_id,
                    display_name: finished.display_name,
                    mod_id: Some(mod_id),
                },
            );
            if let Some(loadout_id) = loadout_id {
                let app = app.clone();
                std::thread::spawn(move || {
                    let Some(state) = app.try_state::<AppState>() else {
                        return;
                    };
                    let Ok(core) = state.core.lock() else {
                        return;
                    };
                    if let Err(err) = crate::enrich_mod_file(&core, loadout_id, mod_id) {
                        eprintln!("[dl] métadonnées non lues : {err}");
                    }
                });
            }
            PromoteResult::Emitted
        }
        Err(e) => {
            eprintln!("[dl] fin du téléchargement non enregistrée ({url}) : {e}");
            let loadout_id = tracked.map(|item| item.loadout_id);
            drop(core);
            let abandoned = abandon_in_progress(state, tracked, Some(path), Some(file_name));
            let _ = app.emit(
                "download://failed",
                DownloadFailed {
                    file_name: file_name.to_string(),
                    loadout_id: abandoned.or(loadout_id),
                    message: format!("Le téléchargement n’a pas pu être enregistré : {e}"),
                },
            );
            PromoteResult::Failed
        }
    }
}

fn download_already_filed(core: &Core, path: &Path, file_name: &str) -> bool {
    if download::in_progress_at(core, path)
        .ok()
        .flatten()
        .is_some()
    {
        return false;
    }
    if download::in_progress_named(core, file_name)
        .ok()
        .flatten()
        .is_some()
    {
        return false;
    }
    // A ledger row at this path means a prior finish already won — do not
    // capture again (second Finished after WebView2 backup, etc.).
    download::ledger_has_path(core, path).unwrap_or(false)
}

/// Record the page the embedded browser just finished loading, or the
/// address the shell asked it to open. `about:blank` is ignored.
pub fn remember_browser_page(state: &AppState, url: &str) {
    let url = url.trim();
    if url.is_empty() || url.eq_ignore_ascii_case("about:blank") {
        return;
    }
    if let Ok(mut guard) = state.browser_page.lock() {
        *guard = Some(url.to_string());
    }
}

fn remember_pending(state: &AppState, item: PendingOrigin) {
    if let Ok(mut guard) = state.pending_origins.lock() {
        guard.push(item);
    }
}

fn take_pending(state: &AppState, url: &str) -> Option<PendingOrigin> {
    let mut guard = state.pending_origins.lock().ok()?;
    let idx = guard
        .iter()
        .position(|item| urls_match(&item.file_url, url))?;
    Some(guard.remove(idx))
}

fn remember_in_flight(state: &AppState, item: InFlight) {
    if let Ok(mut guard) = state.in_flight.lock() {
        guard.push(item);
    }
}

fn peek_in_flight_by_url(state: &AppState, url: &str) -> Option<InFlight> {
    let guard = state.in_flight.lock().ok()?;
    guard
        .iter()
        .rev()
        .find(|item| urls_match(&item.url, url))
        .cloned()
}

fn take_in_flight(
    state: &AppState,
    path: Option<&Path>,
    url: &str,
    file_name: Option<&str>,
) -> Option<InFlight> {
    let mut guard = state.in_flight.lock().ok()?;
    let by_path =
        path.and_then(|wanted| guard.iter().position(|item| same_file(&item.path, wanted)));
    let idx = by_path
        .or_else(|| {
            let matches: Vec<usize> = guard
                .iter()
                .enumerate()
                .filter(|(_, item)| urls_match(&item.url, url))
                .map(|(idx, _)| idx)
                .collect();
            if matches.len() == 1 {
                Some(matches[0])
            } else {
                matches.last().copied()
            }
        })
        .or_else(|| {
            let name = file_name?;
            let matches: Vec<usize> = guard
                .iter()
                .enumerate()
                .filter(|(_, item)| item.file_name.eq_ignore_ascii_case(name))
                .map(|(idx, _)| idx)
                .collect();
            if matches.len() == 1 {
                Some(matches[0])
            } else {
                matches.last().copied()
            }
        });
    idx.map(|idx| guard.remove(idx))
}

fn take_in_flight_by_id(state: &AppState, download_id: i64) -> Option<InFlight> {
    let mut guard = state.in_flight.lock().ok()?;
    let idx = guard
        .iter()
        .position(|item| item.download_id == download_id)?;
    Some(guard.remove(idx))
}

fn abandon_in_progress(
    state: &AppState,
    tracked: Option<&InFlight>,
    path: Option<&Path>,
    file_name: Option<&str>,
) -> Option<i64> {
    let Ok(core) = state.core.lock() else {
        return tracked.map(|item| item.loadout_id);
    };
    let id = tracked
        .map(|item| item.download_id)
        .or_else(|| path.and_then(|p| download::in_progress_at(&core, p).ok().flatten()))
        .or_else(|| {
            file_name.and_then(|name| download::in_progress_named(&core, name).ok().flatten())
        })?;
    match download::abandon(&core, id) {
        Ok(Some(loadout_id)) => Some(loadout_id),
        Ok(None) => tracked.map(|item| item.loadout_id),
        Err(e) => {
            eprintln!("[dl] abandon impossible : {e}");
            tracked.map(|item| item.loadout_id)
        }
    }
}

fn urls_match(stored: &str, reported: &str) -> bool {
    if stored == reported {
        return true;
    }
    match (url::Url::parse(stored), url::Url::parse(reported)) {
        (Ok(left), Ok(right)) => {
            // CDN finish URLs often rotate query tokens; path+host is enough.
            left.scheme() == right.scheme()
                && left.host_str() == right.host_str()
                && left.port_or_known_default() == right.port_or_known_default()
                && left.path() == right.path()
        }
        _ => false,
    }
}

fn event_file_name(path: Option<&Path>, tracked: Option<&InFlight>, url: &str) -> String {
    path.and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
        .or_else(|| tracked.map(|item| item.file_name.clone()))
        .or_else(|| {
            url.rsplit('/')
                .next()
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string())
        })
        .unwrap_or_else(|| "téléchargement".to_string())
}

fn same_file(a: &Path, b: &Path) -> bool {
    path_key(a) == path_key(b)
}

fn path_key(path: &Path) -> String {
    let mut s = path
        .to_string_lossy()
        .replace('\\', "/")
        .to_ascii_lowercase();
    // WebView2 sometimes reports `\\?\F:\...` while begin stored `F:\...`.
    if let Some(rest) = s.strip_prefix("//?/") {
        s = rest.to_string();
    }
    s
}

#[cfg(test)]
mod matching_tests {
    use super::{path_key, same_file, urls_match};
    use std::path::Path;

    #[test]
    fn same_file_ignores_slash_style_and_extended_prefix() {
        assert!(same_file(
            Path::new(r"F:\ModLab\downloads\minecraft\sodium.jar"),
            Path::new("F:/ModLab/downloads/minecraft/sodium.jar"),
        ));
        assert!(same_file(
            Path::new(r"\\?\F:\ModLab\downloads\minecraft\sodium.jar"),
            Path::new(r"F:\ModLab\downloads\minecraft\sodium.jar"),
        ));
        assert_eq!(
            path_key(Path::new(r"\\?\F:\a\b.jar")),
            path_key(Path::new(r"F:\a\b.jar"))
        );
    }

    #[test]
    fn urls_match_ignores_cdn_query_tokens() {
        let begin = "https://cdn.modrinth.com/data/AANobbMI/versions/x/sodium.jar";
        let finish = "https://cdn.modrinth.com/data/AANobbMI/versions/x/sodium.jar?token=abc";
        assert!(urls_match(begin, finish));
        assert!(!urls_match(
            begin,
            "https://cdn.modrinth.com/data/other/sodium.jar"
        ));
    }
}
