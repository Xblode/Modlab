//! Captured downloads — the ledger behind the embedded browser's downloads
//! folder.
//!
//! # Transfer state machine (Core is source of truth)
//!
//! ```text
//! queued  →  progress  →  archived
//!    │            │
//!    └────────────┴──→  failed
//! ```
//!
//! - [`PHASE_QUEUED`]: `begin` just filed the row (mod status `downloading`).
//! - [`PHASE_PROGRESS`]: bytes are arriving (`note_progress`).
//! - [`PHASE_ARCHIVED`]: [`complete`] promoted the same mod to `archived`.
//! - [`PHASE_FAILED`]: [`abandon`] soft-deleted the mod and dropped the ledger row.
//!
//! One ledger id / one mod row per transfer — never a second capture while
//! still `queued`/`progress`. The UI only displays Core status; it must not
//! invent a lasting local « downloading » after the Core has archived.
//!
//! `begin` files a download into a loadout the moment it starts: a disabled
//! mod in the `downloading` state, so the Mods table can show it before any
//! byte has arrived. `complete` turns that same row into an archived mod —
//! the state `attach_to_loadout` produces, including a name the client
//! already decided (SKSE recognition stays in the client). `abandon` hides
//! the row when the transfer fails or is cancelled: the mod is soft-deleted,
//! same as a removed row, and the placeholder ledger entry is dropped. A
//! partial file the browser may have left on disk is not deleted.
//!
//! A transfer that was never begun (no instance was on screen) stays an
//! unfiled capture until `attach_to_loadout` or `attach_recognized`. Those
//! two still insert a new archived mod for an unfiled capture. Called on a
//! download that is already `downloading` in that same loadout, they promote
//! the existing row instead of inserting a second one.
//!
//! The Core never downloads anything itself: every row here was created by a
//! client reporting a file the human's own click produced (WebView2).

use crate::transaction::Transaction;
use crate::{Core, CoreError, Result};
use serde::Serialize;
use std::path::{Path, PathBuf};

/// Ledger phase right after [`begin`] — no bytes confirmed yet.
pub const PHASE_QUEUED: &str = "queued";
/// Bytes are arriving (or progress was observed).
pub const PHASE_PROGRESS: &str = "progress";
/// [`complete`] succeeded — mod is `archived`.
pub const PHASE_ARCHIVED: &str = "archived";
/// [`abandon`] — transfer failed; ledger row is then dropped.
pub const PHASE_FAILED: &str = "failed";

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Download {
    pub id: i64,
    pub url: String,
    pub file_name: String,
    pub path: PathBuf,
    pub size_bytes: Option<i64>,
    pub captured_at: String,
    /// Which loadout it was filed into, `None` while still unfiled.
    pub loadout_id: Option<i64>,
    /// The mod row it became, `None` while still unfiled.
    pub mod_id: Option<i64>,
}

/// What `capture` actually did — a plain new row, or a reuse of one already
/// on file. Callers that redirected a real download onto disk (the embedded
/// browser) need to know which: a dedup means the file they just fetched is
/// now a redundant duplicate of `existing_path` and should be cleaned up.
pub enum CaptureOutcome {
    New(i64),
    Deduped {
        existing_id: i64,
        existing_path: PathBuf,
    },
}

impl CaptureOutcome {
    #[must_use]
    pub fn id(&self) -> i64 {
        match self {
            Self::New(id)
            | Self::Deduped {
                existing_id: id, ..
            } => *id,
        }
    }
}

/// Record a file the human downloaded through the embedded browser — unless
/// a capture with the exact same file name and size already exists and its
/// file is still on disk, in which case that existing row is reused rather
/// than filing a near-certain duplicate (the same mod+version, downloaded
/// again for a second instance). The browser still has to fetch the bytes
/// either way — this only prevents ModLab from keeping two copies and two
/// database rows for what is, for bookkeeping purposes, the same file.
pub fn capture(
    core: &Core,
    url: &str,
    file_name: &str,
    path: &Path,
    size_bytes: Option<i64>,
) -> Result<CaptureOutcome> {
    capture_with_origin(core, url, file_name, path, size_bytes, None)
}

/// [`capture`], keeping a page and host the client already had. On a dedup
/// the existing row is filled only where it is still empty.
pub fn capture_with_origin(
    core: &Core,
    url: &str,
    file_name: &str,
    path: &Path,
    size_bytes: Option<i64>,
    origin: Option<&DownloadOrigin>,
) -> Result<CaptureOutcome> {
    if let Some(size) = size_bytes {
        if let Some((existing_id, existing_path)) =
            core.find_download_by_name_and_size(file_name, size)?
        {
            let existing_path = PathBuf::from(existing_path);
            if existing_path.is_file() {
                if let Some(origin) = origin {
                    remember_origin_download(core, existing_id, url, origin)?;
                }
                return Ok(CaptureOutcome::Deduped {
                    existing_id,
                    existing_path,
                });
            }
        }
    }

    let id = core.insert_download(
        url,
        file_name,
        &path.display().to_string(),
        size_bytes,
        PHASE_ARCHIVED,
    )?;
    if let Some(origin) = origin {
        core.set_download_page(
            id,
            Some(origin.source_host.as_str()),
            origin.page_url.as_deref(),
        )?;
    }
    let mut tx = Transaction::new(core, "download.captured");
    tx.step(format!("« {file_name} » capturé depuis {url}"));
    tx.commit()?;
    Ok(CaptureOutcome::New(id))
}

/// Current ledger phase, if the row still exists.
pub fn phase(core: &Core, download_id: i64) -> Result<Option<String>> {
    core.select_download_phase(download_id)
}

/// Mark that bytes are flowing: `queued` → `progress`. Idempotent once in
/// progress or already finished. No-op when the id is unknown.
pub fn note_progress(core: &Core, download_id: i64) -> Result<()> {
    let Some(current) = core.select_download_phase(download_id)? else {
        return Ok(());
    };
    if current == PHASE_QUEUED {
        core.set_download_phase(download_id, PHASE_PROGRESS)?;
    }
    Ok(())
}

/// What [`reconcile`] decided for a transfer stuck without a browser Finished.
#[derive(Debug)]
pub enum ReconcileOutcome {
    /// File on disk → [`complete`] ran.
    Completed(FinishedDownload),
    /// Still downloading and the file is missing or empty.
    NotReady,
    /// [`abandon`] ran (give-up with no file).
    Failed {
        loadout_id: Option<i64>,
        file_name: String,
    },
    /// Unknown id, or already archived / not downloading.
    AlreadyDone,
}

/// Disk safety net: if the destination file is present and non-empty, promote
/// to archived; if `give_up` and still missing, fail the transfer. Never
/// inserts a second capture.
pub fn reconcile(core: &Core, download_id: i64, give_up: bool) -> Result<ReconcileOutcome> {
    let Some(row) = progress_row(core, download_id)? else {
        return Ok(ReconcileOutcome::AlreadyDone);
    };
    if row.status != crate::loadout::STATUS_DOWNLOADING {
        return Ok(ReconcileOutcome::AlreadyDone);
    }
    if file_ready(&row.path) {
        let size = std::fs::metadata(&row.path).ok().map(|m| m.len() as i64);
        let finished = complete(core, download_id, size, None, None)?;
        return Ok(ReconcileOutcome::Completed(finished));
    }
    if !give_up {
        return Ok(ReconcileOutcome::NotReady);
    }
    let file_name = row.file_name.clone();
    let loadout_id = abandon(core, download_id)?;
    Ok(ReconcileOutcome::Failed {
        loadout_id: loadout_id.or(row.loadout_id),
        file_name,
    })
}

fn file_ready(path: &Path) -> bool {
    std::fs::metadata(path)
        .map(|meta| meta.is_file() && meta.len() > 0)
        .unwrap_or(false)
}

/// Every capture, newest first.
pub fn list(core: &Core) -> Result<Vec<Download>> {
    Ok(core
        .select_downloads()?
        .into_iter()
        .map(
            |(id, url, file_name, path, size_bytes, captured_at, loadout_id, mod_id)| Download {
                id,
                url,
                file_name,
                path: PathBuf::from(path),
                size_bytes,
                captured_at,
                loadout_id,
                mod_id,
            },
        )
        .collect())
}

/// The url and file name of one capture — what a client needs to decide
/// whether the archive is a known tool before filing it. The Core does not
/// interpret either string.
pub struct CapturedFile {
    pub url: String,
    pub file_name: String,
}

/// Read one capture's url and file name. `Ok(None)` when that id was never
/// recorded.
pub fn captured_file(core: &Core, download_id: i64) -> Result<Option<CapturedFile>> {
    Ok(core
        .select_download(download_id)?
        .map(|(url, file_name, _path)| CapturedFile { url, file_name }))
}

/// File a capture into a loadout as an *archived* mod: it shows up in the
/// mod list immediately, disabled and not extracted. The name defaults to
/// the file's own stem — the human renames it later if they care.
pub fn attach_to_loadout(
    core: &Core,
    download_id: i64,
    loadout_id: i64,
    category: Option<&str>,
) -> Result<i64> {
    file_into(core, download_id, loadout_id, category, None, None)
}

/// Same filing as `attach_to_loadout`, with a display name and version the
/// client already decided (a game adapter recognised the archive). An empty
/// name falls back to the file stem. The archive path is unchanged: install
/// still unpacks the captured file.
pub fn attach_recognized(
    core: &Core,
    download_id: i64,
    loadout_id: i64,
    category: Option<&str>,
    name: &str,
    version: Option<&str>,
) -> Result<i64> {
    file_into(core, download_id, loadout_id, category, Some(name), version)
}

/// A transfer just filed into a loadout, before the file exists.
pub struct StartedDownload {
    pub download_id: i64,
    pub mod_id: i64,
    pub loadout_id: i64,
    pub file_name: String,
    /// The name the Mods table shows (the file stem, until recognition).
    pub name: String,
}

/// What `complete` did to the in-progress row.
#[derive(Debug)]
pub struct FinishedDownload {
    /// The ledger row that remains. On a dedup this is the capture that was
    /// already on file, not the placeholder created at start.
    pub download_id: i64,
    pub mod_id: i64,
    pub loadout_id: Option<i64>,
    pub file_name: String,
    pub display_name: String,
    pub archive_path: PathBuf,
    /// A file the caller should delete: the bytes just written duplicate
    /// `archive_path`. `None` when this transfer is the only copy.
    pub redundant_path: Option<PathBuf>,
}

/// Page and host a client already had when the download started. The Core
/// stores the strings and does not decide what a host token means.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DownloadOrigin {
    pub source_host: String,
    pub page_url: Option<String>,
}

/// File a download into `loadout_id` the moment it starts. The mod is
/// disabled and `downloading`. The destination file does not need to exist
/// yet — `path` is where the browser was told to write it.
pub fn begin(
    core: &Core,
    url: &str,
    file_name: &str,
    path: &Path,
    loadout_id: i64,
) -> Result<StartedDownload> {
    start(core, url, file_name, path, loadout_id, None)
}

/// [`begin`], plus the page and host the client already parsed from the
/// addresses it had. A version in the file name is stored when the row
/// does not have one yet.
pub fn begin_with_origin(
    core: &Core,
    url: &str,
    file_name: &str,
    path: &Path,
    loadout_id: i64,
    origin: &DownloadOrigin,
) -> Result<StartedDownload> {
    start(core, url, file_name, path, loadout_id, Some(origin))
}

fn start(
    core: &Core,
    url: &str,
    file_name: &str,
    path: &Path,
    loadout_id: i64,
    origin: Option<&DownloadOrigin>,
) -> Result<StartedDownload> {
    let facts = crate::filename_meta::parse_file_name(file_name);
    let name = facts.name.clone();
    let (download_id, mod_id) = core.insert_started_download(
        url,
        file_name,
        &path.display().to_string(),
        &name,
        loadout_id,
    )?;
    // The row was just inserted, so stamping the file-name layer records
    // who wrote the name. A later archive read can replace it; a rename
    // cannot be replaced.
    crate::mod_meta::apply_layer(
        core,
        mod_id,
        crate::mod_meta::Layer::Filename,
        &crate::mod_meta::patch_from_facts(&facts),
        &[file_name.to_string()],
        true,
    )?;
    if let Some(origin) = origin {
        remember_origin(core, download_id, mod_id, url, origin)?;
    }
    let mut tx = Transaction::new(core, "download.started");
    tx.step(format!("« {name} » en cours de téléchargement"));
    tx.commit()?;
    Ok(StartedDownload {
        download_id,
        mod_id,
        loadout_id,
        file_name: file_name.to_string(),
        name,
    })
}

/// The in-progress capture stored at `path`, if the mod is still
/// `downloading`. A finished capture at the same path is `Ok(None)` so a
/// later transfer can be filed on its own.
pub fn in_progress_at(core: &Core, path: &Path) -> Result<Option<i64>> {
    let Some(id) = id_for_path(core, path)? else {
        return Ok(None);
    };
    if download_still_downloading(core, id)? {
        Ok(Some(id))
    } else {
        Ok(None)
    }
}

/// Newest in-progress capture with this exact `file_name`. Fallback when the
/// finish event's path or URL no longer matches what `begin` recorded.
pub fn in_progress_named(core: &Core, file_name: &str) -> Result<Option<i64>> {
    let Some(id) = core.find_in_progress_download_by_file_name(file_name)? else {
        return Ok(None);
    };
    Ok(Some(id))
}

/// Like [`in_progress_named`], but only when that row is already filed into
/// `loadout_id`. Returns the ledger id and the path `begin` stored.
pub fn in_progress_named_in_loadout(
    core: &Core,
    file_name: &str,
    loadout_id: i64,
) -> Result<Option<(i64, PathBuf)>> {
    let Some(id) = in_progress_named(core, file_name)? else {
        return Ok(None);
    };
    let Some((_, _, path, bound, _)) = core.select_download_binding(id)? else {
        return Ok(None);
    };
    if bound != Some(loadout_id) {
        return Ok(None);
    }
    Ok(Some((id, PathBuf::from(path))))
}

/// Snapshot used by the shell to finish a transfer that reached 100 % without
/// a browser Finished event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DownloadProgressRow {
    pub url: String,
    pub file_name: String,
    pub path: PathBuf,
    pub loadout_id: Option<i64>,
    pub mod_id: Option<i64>,
    /// Mod status when `mod_id` is set; empty when the capture is still unfiled.
    pub status: String,
}

/// Ledger row + mod status for `download_id`, if any.
pub fn progress_row(core: &Core, download_id: i64) -> Result<Option<DownloadProgressRow>> {
    let Some((url, file_name, path, loadout_id, mod_id)) =
        core.select_download_binding(download_id)?
    else {
        return Ok(None);
    };
    let status = match mod_id {
        Some(mod_id) => core
            .select_mod_archive(mod_id)?
            .map(|(_, status, _)| status)
            .unwrap_or_default(),
        None => String::new(),
    };
    Ok(Some(DownloadProgressRow {
        url,
        file_name,
        path: PathBuf::from(path),
        loadout_id,
        mod_id,
        status,
    }))
}

/// Whether any ledger row already points at `path` (either slash style).
pub fn ledger_has_path(core: &Core, path: &Path) -> Result<bool> {
    Ok(id_for_path(core, path)?.is_some())
}

/// Point the ledger (and the downloading mod) at the path the browser
/// actually wrote, when it differs from the destination set at start.
pub fn retarget_path(core: &Core, download_id: i64, path: &Path) -> Result<()> {
    let path_str = path.display().to_string();
    core.set_download_path(download_id, &path_str)?;
    if let Some((_, _, _, _, Some(mod_id))) = core.select_download_binding(download_id)? {
        if let Some((_, status, _)) = core.select_mod_archive(mod_id)? {
            if status == crate::loadout::STATUS_DOWNLOADING {
                core.retarget_downloading_archive(mod_id, &path_str)?;
            }
        }
    }
    Ok(())
}

fn download_still_downloading(core: &Core, download_id: i64) -> Result<bool> {
    let Some((_, _, _, _, mod_id)) = core.select_download_binding(download_id)? else {
        return Ok(false);
    };
    let Some(mod_id) = mod_id else {
        return Ok(false);
    };
    let Some((_, status, _)) = core.select_mod_archive(mod_id)? else {
        return Ok(false);
    };
    Ok(status == crate::loadout::STATUS_DOWNLOADING)
}

/// The bytes landed. The in-progress mod becomes archived — same end state
/// as `attach_to_loadout`. `name_override` / `version` are what a client
/// decided (recognition); `None` keeps the name from `begin`.
///
/// A second call is not an error: the row stays archived and the size is
/// recorded. A name the human already has on an archived row is not
/// overwritten by that second call.
///
/// When `size_bytes` matches a capture already on disk, the mod is pointed
/// at that file, the placeholder ledger row is dropped, and the surviving
/// capture is retargeted at this mod — the same bookkeeping `capture`'s
/// dedup plus `attach_to_loadout` does for a transfer that was never begun.
pub fn complete(
    core: &Core,
    download_id: i64,
    size_bytes: Option<i64>,
    name_override: Option<&str>,
    version: Option<&str>,
) -> Result<FinishedDownload> {
    let (_, file_name, path, loadout_id, mod_id) = core
        .select_download_binding(download_id)?
        .ok_or_else(|| CoreError::Message(format!("téléchargement {download_id} introuvable")))?;
    let mod_id = mod_id.ok_or_else(|| {
        CoreError::Message(format!(
            "téléchargement {download_id} n'est pas encore classé"
        ))
    })?;
    let (name, status, _) = core
        .select_mod_archive(mod_id)?
        .ok_or_else(|| CoreError::Message(format!("mod {mod_id} introuvable")))?;

    if status != crate::loadout::STATUS_DOWNLOADING {
        if status == crate::loadout::STATUS_ARCHIVED || status == crate::loadout::STATUS_INSTALLED {
            let _ = core.set_download_phase(download_id, PHASE_ARCHIVED);
            if let Some(size) = size_bytes {
                core.set_download_size(download_id, Some(size))?;
                store_mod_size(core, mod_id, Some(size))?;
            }
            return Ok(FinishedDownload {
                download_id,
                mod_id,
                loadout_id,
                file_name,
                display_name: name,
                archive_path: PathBuf::from(path),
                redundant_path: None,
            });
        }
        return Err(CoreError::Message(format!(
            "« {name} » ne peut pas être terminé (statut : {status})"
        )));
    }

    if let Some(size) = size_bytes {
        if let Some((existing_id, existing_path)) =
            core.find_download_by_name_and_size(&file_name, size)?
        {
            if existing_id != download_id {
                let existing_buf = PathBuf::from(&existing_path);
                let own_buf = PathBuf::from(&path);
                if existing_buf.is_file() && !paths_equal(&existing_buf, &own_buf) {
                    core.retarget_downloading_archive(mod_id, &existing_path)?;
                    store_mod_size(core, mod_id, size_bytes)?;
                    let display =
                        promote_started(core, mod_id, &file_name, name_override, version)?;
                    // The bytes we keep belong to the capture already on
                    // file. The date follows that capture, not this transfer.
                    core.copy_download_captured_at(existing_id, mod_id)?;
                    if let Some(loadout_id) = loadout_id {
                        core.set_download_target(existing_id, loadout_id, mod_id)?;
                    }
                    core.set_download_phase(existing_id, PHASE_ARCHIVED)?;
                    core.delete_download(download_id)?;
                    return Ok(FinishedDownload {
                        download_id: existing_id,
                        mod_id,
                        loadout_id,
                        file_name,
                        display_name: display,
                        archive_path: existing_buf,
                        redundant_path: Some(own_buf),
                    });
                }
            }
        }
    }

    if let Some(size) = size_bytes {
        core.set_download_size(download_id, Some(size))?;
    }
    store_mod_size(core, mod_id, size_bytes)?;
    let display = promote_started(core, mod_id, &file_name, name_override, version)?;
    core.set_download_phase(download_id, PHASE_ARCHIVED)?;
    Ok(FinishedDownload {
        download_id,
        mod_id,
        loadout_id,
        file_name,
        display_name: display,
        archive_path: PathBuf::from(path),
        redundant_path: None,
    })
}

/// The transfer failed or was cancelled. The mod disappears from the table
/// (soft-delete, same as removing a row — there is no separate error
/// status, because a half-written file cannot be installed). The placeholder
/// ledger row is dropped. The partial file, if the browser left one, stays
/// on disk.
///
/// `Ok(None)` when there was nothing in progress to remove (unknown id, or
/// the row already reached `archived` / `installed`). `Ok(Some(loadout))`
/// when a row was hidden.
pub fn abandon(core: &Core, download_id: i64) -> Result<Option<i64>> {
    let Some((_, _, _, loadout_id, mod_id)) = core.select_download_binding(download_id)? else {
        return Ok(None);
    };
    let Some(mod_id) = mod_id else {
        return Ok(None);
    };
    let Some((name, status, _)) = core.select_mod_archive(mod_id)? else {
        core.delete_download(download_id)?;
        return Ok(loadout_id);
    };
    if status != crate::loadout::STATUS_DOWNLOADING {
        return Ok(None);
    }
    let _ = core.set_download_phase(download_id, PHASE_FAILED);
    core.soft_delete_mod(mod_id)?;
    core.delete_download(download_id)?;
    let mut tx = Transaction::new(core, "download.abandoned");
    tx.step(format!(
        "« {name} » téléchargement interrompu, retiré de la liste"
    ));
    tx.commit()?;
    Ok(loadout_id)
}

fn remember_origin(
    core: &Core,
    download_id: i64,
    mod_id: i64,
    file_url: &str,
    origin: &DownloadOrigin,
) -> Result<()> {
    core.set_download_page(
        download_id,
        Some(origin.source_host.as_str()),
        origin.page_url.as_deref(),
    )?;
    core.set_mod_source(
        mod_id,
        Some(origin.source_host.as_str()),
        origin.page_url.as_deref(),
        Some(file_url),
    )?;
    Ok(())
}

fn remember_origin_download(
    core: &Core,
    download_id: i64,
    file_url: &str,
    origin: &DownloadOrigin,
) -> Result<()> {
    core.set_download_page(
        download_id,
        Some(origin.source_host.as_str()),
        origin.page_url.as_deref(),
    )?;
    if let Some((_, _, _, _, Some(mod_id))) = core.select_download_binding(download_id)? {
        core.set_mod_source(
            mod_id,
            Some(origin.source_host.as_str()),
            origin.page_url.as_deref(),
            Some(file_url),
        )?;
    }
    Ok(())
}

/// Copy the ledger's page and host onto the mod, and a version from the
/// file name when the mod still has none. Empty values do not clear
/// anything already stored.
fn note_origin_from_download(core: &Core, download_id: i64, mod_id: i64) -> Result<()> {
    let Some((url, file_name, _)) = core.select_download(download_id)? else {
        return Ok(());
    };
    let (host, page) = core
        .select_download_page(download_id)?
        .unwrap_or((None, None));
    core.set_mod_source(mod_id, host.as_deref(), page.as_deref(), Some(&url))?;
    if let Some(version) = version_from_file_name(&file_name) {
        set_version_if_empty(core, mod_id, &version)?;
    }
    Ok(())
}

/// The archive path stored on the mod, whether or not the file is still
/// on disk. `Ok(None)` when the row has none.
pub fn stored_archive(core: &Core, mod_id: i64) -> Result<Option<PathBuf>> {
    let Some((_, _, archive)) = core.select_mod_archive(mod_id)? else {
        return Ok(None);
    };
    Ok(archive.map(PathBuf::from))
}

/// The fiche, including after a soft-delete. Does not read the file.
pub fn mod_facts(core: &Core, mod_id: i64) -> Result<Option<crate::ModFacts>> {
    core.select_mod_facts(mod_id)
}

/// Write facts an adapter read from a local file. A missing value does not
/// clear the column. A declared version replaces a filename guess; an empty
/// one does not. The display name is updated only while it is still the
/// file stem, so a name already chosen (SKSE, a rename) stays.
pub fn apply_archive_metadata(
    core: &Core,
    mod_id: i64,
    meta: &crate::game_adapter::ArchiveMetadata,
) -> Result<()> {
    let sheet = core
        .select_mod_sheet(mod_id)?
        .ok_or_else(|| CoreError::Message(format!("mod {mod_id} introuvable")))?;
    let mut hints = Vec::new();
    if let Some(path) = &sheet.archive_path {
        hints.push(path.clone());
    }
    hints.push(sheet.source_path);
    let mut patch = crate::mod_meta::patch_from_archive(meta);
    // The human's tag is not the raw category. A raw label lands in
    // `category_raw`; translation into the game's list is a separate step.
    patch.category = None;
    patch.clear_category = false;
    if sheet
        .source_host
        .as_deref()
        .is_some_and(|host| !host.trim().is_empty())
    {
        patch.source_host = None;
    }
    crate::mod_meta::apply_layer(
        core,
        mod_id,
        crate::mod_meta::Layer::Archive,
        &patch,
        &hints,
        false,
    )
}

/// Version stated by a file name. Nexus-shaped names yield `6.11` from
/// `6-11`. A dotted token yields `2.2.6` or `5.2SE`. Underscore builds like
/// `skse64_2_02_06` are not a version — that pattern belongs to the adapter
/// that already knows it.
#[must_use]
pub fn version_from_file_name(file_name: &str) -> Option<String> {
    crate::filename_meta::parse_file_name(file_name).version
}

fn resolved_name(file_name: &str, name_override: Option<&str>) -> Result<String> {
    let stem = Path::new(file_name)
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| file_name.to_string());
    match name_override {
        Some(raw) => {
            let trimmed = raw.trim();
            if trimmed.is_empty() {
                Ok(stem)
            } else if trimmed.chars().count() > 100 {
                Err(CoreError::Message(
                    "le nom ne peut pas dépasser 100 caractères".to_string(),
                ))
            } else {
                Ok(trimmed.to_string())
            }
        }
        None => Ok(stem),
    }
}

fn promote_started(
    core: &Core,
    mod_id: i64,
    file_name: &str,
    name_override: Option<&str>,
    version: Option<&str>,
) -> Result<String> {
    let (_, status, _) = core
        .select_mod_archive(mod_id)?
        .ok_or_else(|| CoreError::Message(format!("mod {mod_id} introuvable")))?;
    if status != crate::loadout::STATUS_DOWNLOADING {
        return Err(CoreError::Message(format!(
            "mod {mod_id} n'est pas en cours de téléchargement (statut : {status})"
        )));
    }
    if name_override.is_some() {
        let name = resolved_name(file_name, name_override)?;
        // Recognition (SKSE) outranks the file name and the archive, same
        // as a name the human typed: later layers must not put the long
        // Nexus title back.
        stamp_user(core, mod_id, Some(&name), None, None, false)?;
    }
    if let Some(version) = version.map(str::trim).filter(|v| !v.is_empty()) {
        stamp_user(core, mod_id, None, Some(version), None, false)?;
    }
    if !core.promote_downloading_mod(mod_id)? {
        return Err(CoreError::Message(format!(
            "mod {mod_id} n'est plus en cours de téléchargement"
        )));
    }
    let (display, _, _) = core
        .select_mod_archive(mod_id)?
        .ok_or_else(|| CoreError::Message(format!("mod {mod_id} introuvable")))?;
    let mut tx = Transaction::new(core, "download.completed");
    tx.step(format!(
        "« {display} » téléchargement terminé (archivé, désactivé)"
    ));
    tx.commit()?;
    Ok(display)
}

fn downloading_mod_in(core: &Core, download_id: i64, loadout_id: i64) -> Result<Option<i64>> {
    let Some((_, _, _, bound, mod_id)) = core.select_download_binding(download_id)? else {
        return Ok(None);
    };
    if bound != Some(loadout_id) {
        return Ok(None);
    }
    let Some(mod_id) = mod_id else {
        return Ok(None);
    };
    let Some((_, status, _)) = core.select_mod_archive(mod_id)? else {
        return Ok(None);
    };
    if status == crate::loadout::STATUS_DOWNLOADING {
        Ok(Some(mod_id))
    } else {
        Ok(None)
    }
}

fn id_for_path(core: &Core, path: &Path) -> Result<Option<i64>> {
    let primary = path.display().to_string();
    let alt = if primary.contains('\\') {
        primary.replace('\\', "/")
    } else {
        primary.replace('/', "\\")
    };
    if let Some(id) = core.find_download_id_by_path(&primary, &alt)? {
        return Ok(Some(id));
    }
    // WebView2 may report `\\?\F:\...` while begin stored `F:\...`.
    let stripped = primary
        .strip_prefix(r"\\?\")
        .or_else(|| primary.strip_prefix("//?/"));
    if let Some(rest) = stripped {
        let rest_alt = if rest.contains('\\') {
            rest.replace('\\', "/")
        } else {
            rest.replace('/', "\\")
        };
        return core.find_download_id_by_path(rest, &rest_alt);
    }
    Ok(None)
}

fn store_mod_size(core: &Core, mod_id: i64, size_bytes: Option<i64>) -> Result<()> {
    if let Some(size) = size_bytes.filter(|n| *n >= 0) {
        core.set_mod_size(mod_id, size)?;
    }
    Ok(())
}

fn copy_download_size(core: &Core, download_id: i64, mod_id: i64) -> Result<()> {
    store_mod_size(core, mod_id, core.select_download_size(download_id)?)
}

fn paths_equal(a: &Path, b: &Path) -> bool {
    path_key(a) == path_key(b)
}

fn path_key(path: &Path) -> String {
    let mut s = path
        .to_string_lossy()
        .replace('\\', "/")
        .to_ascii_lowercase();
    if let Some(rest) = s.strip_prefix("//?/") {
        s = rest.to_string();
    }
    s
}

fn file_into(
    core: &Core,
    download_id: i64,
    loadout_id: i64,
    category: Option<&str>,
    name_override: Option<&str>,
    version: Option<&str>,
) -> Result<i64> {
    let (_, file_name, path) = core
        .select_download(download_id)?
        .ok_or_else(|| CoreError::Message(format!("téléchargement {download_id} introuvable")))?;

    // Already on screen as a downloading row in this loadout: finishing it
    // is the whole job. Inserting again would duplicate the mod.
    if let Some(mod_id) = downloading_mod_in(core, download_id, loadout_id)? {
        if let Some(category) = category.map(str::trim).filter(|c| !c.is_empty()) {
            stamp_user(core, mod_id, None, None, Some(category), false)?;
        }
        promote_started(core, mod_id, &file_name, name_override, version)?;
        copy_download_size(core, download_id, mod_id)?;
        // Same capture as `begin`: copying again does not move the date.
        core.copy_download_captured_at(download_id, mod_id)?;
        note_origin_from_download(core, download_id, mod_id)?;
        core.set_download_phase(download_id, PHASE_ARCHIVED)?;
        return Ok(mod_id);
    }

    let facts = crate::filename_meta::parse_file_name(&file_name);
    let name = if name_override.is_some() {
        resolved_name(&file_name, name_override)?
    } else {
        facts.name.clone()
    };

    let mod_id = core.insert_archived_mod(&name, &path, None, None)?;
    let position = core.loadout_entry_count(loadout_id)?;
    // Disabled on arrival, always: an archive that has not been unpacked
    // cannot deploy anything, so enabling it would be a lie.
    core.insert_loadout_entry(loadout_id, mod_id, false, position)?;
    core.set_download_target(download_id, loadout_id, mod_id)?;
    // The row did not exist when the file was captured. The date is the
    // capture's, not the moment the human files it into a loadout.
    core.copy_download_captured_at(download_id, mod_id)?;
    let mut filename = crate::mod_meta::patch_from_facts(&facts);
    if name_override.is_some() {
        filename.name = None;
    }
    crate::mod_meta::apply_layer(
        core,
        mod_id,
        crate::mod_meta::Layer::Filename,
        &filename,
        &[file_name],
        true,
    )?;
    if name_override.is_some() {
        stamp_user(core, mod_id, Some(&name), None, None, false)?;
    }
    if let Some(version) = version.map(str::trim).filter(|v| !v.is_empty()) {
        stamp_user(core, mod_id, None, Some(version), None, false)?;
    }
    if let Some(category) = category.map(str::trim).filter(|c| !c.is_empty()) {
        stamp_user(core, mod_id, None, None, Some(category), false)?;
    }
    note_origin_from_download(core, download_id, mod_id)?;
    copy_download_size(core, download_id, mod_id)?;

    let mut tx = Transaction::new(core, "download.attached");
    tx.step(format!(
        "« {name} » ajouté au loadout {loadout_id} (archivé, désactivé)"
    ));
    tx.commit()?;
    Ok(mod_id)
}

/// Move an archived mod to a different loadout. Only ever archived: nothing
/// has been extracted yet, so nothing about the mod's files changes — only
/// which instance's mod list it shows up in. An installed mod's files live
/// under a real deployed path a specific loadout's table already reflects;
/// moving that is a different, bigger feature than this.
pub fn reassign(core: &Core, mod_id: i64, from_loadout_id: i64, to_loadout_id: i64) -> Result<()> {
    let (name, status, _) = core
        .select_mod_archive(mod_id)?
        .ok_or_else(|| CoreError::Message(format!("mod {mod_id} introuvable")))?;
    if status == crate::loadout::STATUS_DOWNLOADING {
        return Err(CoreError::Message(format!(
            "« {name} » est encore en cours de téléchargement"
        )));
    }
    if status != crate::loadout::STATUS_ARCHIVED {
        return Err(CoreError::Message(format!(
            "« {name} » est déjà installé — impossible de le réassigner à une autre instance"
        )));
    }
    core.move_loadout_entry(mod_id, from_loadout_id, to_loadout_id)?;
    let mut tx = Transaction::new(core, "download.reassigned");
    tx.step(format!(
        "« {name} » déplacé du loadout {from_loadout_id} vers {to_loadout_id}"
    ));
    tx.commit()?;
    Ok(())
}

/// A name, version or tag chosen by the human — or by loader recognition,
/// which must survive the same way. Later layers do not overwrite it.
fn stamp_user(
    core: &Core,
    mod_id: i64,
    name: Option<&str>,
    version: Option<&str>,
    category: Option<&str>,
    clear_category: bool,
) -> Result<()> {
    let patch = crate::mod_meta::Patch {
        name: name.map(str::to_string),
        version: version.map(str::to_string),
        category: category.map(str::to_string),
        clear_category,
        ..Default::default()
    };
    crate::mod_meta::apply_layer(
        core,
        mod_id,
        crate::mod_meta::Layer::User,
        &patch,
        &[],
        false,
    )
}

/// Keep a loader's display name (`SKSE`) ahead of every metadata layer.
pub fn lock_recognized_name(core: &Core, mod_id: i64, name: &str) -> Result<()> {
    stamp_user(core, mod_id, Some(name), None, None, false)
}

/// Version a loader recognition produced, stored only when the row has none.
/// Recorded as the human's so an archive manifest cannot replace SKSE's.
pub fn set_tool_version_if_empty(core: &Core, mod_id: i64, version: &str) -> Result<()> {
    let version = version.trim();
    if version.is_empty() {
        return Ok(());
    }
    let current = core.select_mod_version(mod_id)?;
    if current
        .as_deref()
        .is_some_and(|existing| !existing.trim().is_empty())
    {
        return Ok(());
    }
    stamp_user(core, mod_id, None, Some(version), None, false)
}

/// Re-tag a mod. `None` clears the tag. Either way the tag is the human's:
/// a later layer does not put a file category back.
pub fn set_category(core: &Core, mod_id: i64, category: Option<&str>) -> Result<()> {
    let cleaned = category.map(str::trim).filter(|c| !c.is_empty());
    stamp_user(core, mod_id, None, None, cleaned, cleaned.is_none())
}

/// Every tag already in use — what the category input autocompletes from.
pub fn categories(core: &Core) -> Result<Vec<String>> {
    core.select_mod_categories()
}

/// What `install_archived` needs to know before unpacking.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArchivedMod {
    pub name: String,
    pub archive_path: PathBuf,
}

/// Read back an archived mod, refusing anything that is not actually
/// archived — installing twice would extract over a live folder.
pub fn archived_mod(core: &Core, mod_id: i64) -> Result<ArchivedMod> {
    let (name, status, archive_path) = core
        .select_mod_archive(mod_id)?
        .ok_or_else(|| CoreError::Message(format!("mod {mod_id} introuvable")))?;
    if status != crate::loadout::STATUS_ARCHIVED {
        return Err(CoreError::Message(format!(
            "« {name} » n'est pas une archive en attente (statut : {status})"
        )));
    }
    let archive_path = archive_path.ok_or_else(|| {
        CoreError::Message(format!("« {name} » n'a pas d'archive source enregistrée"))
    })?;
    Ok(ArchivedMod {
        name,
        archive_path: PathBuf::from(archive_path),
    })
}

/// Mark an archived mod as installed, now that `source_path` holds its
/// extracted files. The extraction itself belongs to whoever knows the game's
/// layout — the Core only records that it happened.
pub fn mark_installed(core: &Core, mod_id: i64, source_path: &Path) -> Result<()> {
    core.set_mod_installed(mod_id, &source_path.display().to_string())?;
    let mut tx = Transaction::new(core, "download.installed");
    tx.step(format!(
        "mod {mod_id} extrait dans {}",
        source_path.display()
    ));
    tx.commit()?;
    Ok(())
}

/// Point an installed mod at a new on-disk location after a rename the
/// adapter already performed. Does not change status or `installed_at`.
pub fn relocate_source(core: &Core, mod_id: i64, source_path: &Path) -> Result<()> {
    core.update_mod_source_path(mod_id, &source_path.display().to_string())?;
    Ok(())
}

/// Store a version parsed from a file name, but leave one that is already
/// set. An empty string is ignored. A missing row updates nothing.
pub fn set_version_if_empty(core: &Core, mod_id: i64, version: &str) -> Result<()> {
    let version = version.trim();
    if version.is_empty() {
        return Ok(());
    }
    let current = core.select_mod_version(mod_id)?;
    if current
        .as_deref()
        .is_some_and(|existing| !existing.trim().is_empty())
    {
        return Ok(());
    }
    let patch = crate::mod_meta::Patch {
        version: Some(version.to_string()),
        ..Default::default()
    };
    crate::mod_meta::apply_layer(
        core,
        mod_id,
        crate::mod_meta::Layer::Filename,
        &patch,
        &[],
        false,
    )
}

/// Rename a mod.
pub fn rename(core: &Core, mod_id: i64, name: &str) -> Result<()> {
    let name = name.trim();
    if name.is_empty() {
        return Err(CoreError::Message(
            "le nom ne peut pas être vide".to_string(),
        ));
    }
    if name.chars().count() > 100 {
        return Err(CoreError::Message(
            "le nom ne peut pas dépasser 100 caractères".to_string(),
        ));
    }
    stamp_user(core, mod_id, Some(name), None, None, false)?;
    let mut tx = Transaction::new(core, "mod.rename");
    tx.step(format!("mod {mod_id} renommé en « {name} »"));
    tx.commit()?;
    Ok(())
}

/// Soft-delete a mod: it disappears from every loadout table it's in, but
/// nothing on disk (extracted files, the archive) or in its own history is
/// destroyed — per the project's "internal trash, never real deletion" rule.
/// There is no restore path exposed yet (unlike a loadout's trash), only the
/// safety guarantee that this never throws bytes away.
pub fn remove(core: &Core, mod_id: i64) -> Result<()> {
    let (name, ..) = core
        .select_mod_archive(mod_id)?
        .ok_or_else(|| CoreError::Message(format!("mod {mod_id} introuvable")))?;
    core.soft_delete_mod(mod_id)?;
    let mut tx = Transaction::new(core, "mod.remove");
    tx.step(format!("« {name} » retiré de la liste (récupérable)"));
    tx.commit()?;
    Ok(())
}

/// Revert an installed mod back to `archived`, ready to be re-extracted —
/// the caller is responsible for actually clearing the extracted copy on
/// disk first (a plain derived folder, not the mod's real asset; the archive
/// is). Refuses anything not currently installed, and anything with no
/// archive to fall back to — reverting would otherwise leave a mod with no
/// usable source at all.
pub fn uninstall(core: &Core, mod_id: i64) -> Result<()> {
    let (name, status, archive_path) = core
        .select_mod_archive(mod_id)?
        .ok_or_else(|| CoreError::Message(format!("mod {mod_id} introuvable")))?;
    if status != crate::loadout::STATUS_INSTALLED {
        return Err(CoreError::Message(format!(
            "« {name} » n'est pas installé (statut : {status})"
        )));
    }
    if archive_path.is_none() {
        return Err(CoreError::Message(format!(
            "« {name} » n'a pas d'archive source enregistrée — rien à quoi revenir"
        )));
    }
    core.set_mod_archived(mod_id)?;
    let mut tx = Transaction::new(core, "mod.uninstall");
    tx.step(format!("« {name} » désinstallé, revenu à l'état d'archive"));
    tx.commit()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::loadout;

    #[test]
    fn captures_a_download_and_lists_it_unfiled() {
        let core = Core::open_in_memory().unwrap();

        let id = capture(
            &core,
            "https://example.test/lux.7z",
            "lux.7z",
            Path::new("F:/ModLab/downloads/lux.7z"),
            Some(1234),
        )
        .unwrap()
        .id();

        let all = list(&core).unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].id, id);
        assert_eq!(all[0].file_name, "lux.7z");
        assert_eq!(all[0].size_bytes, Some(1234));
        assert_eq!(all[0].loadout_id, None, "unfiled until attached");
        assert_eq!(all[0].mod_id, None);
    }

    #[test]
    fn recapturing_the_same_name_and_size_dedupes_to_the_existing_row() {
        let core = Core::open_in_memory().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let first_path = dir.path().join("lux.7z");
        std::fs::write(&first_path, b"seven-bytes").unwrap();

        let first = capture(
            &core,
            "https://example.test/x",
            "lux.7z",
            &first_path,
            Some(11),
        )
        .unwrap();
        assert!(matches!(first, CaptureOutcome::New(_)));

        // A second instance's browser downloads the same file again, to a
        // different path.
        let second_path = dir.path().join("lux (2).7z");
        std::fs::write(&second_path, b"seven-bytes").unwrap();
        let second = capture(
            &core,
            "https://example.test/x",
            "lux.7z",
            &second_path,
            Some(11),
        )
        .unwrap();

        match second {
            CaptureOutcome::Deduped {
                existing_id,
                existing_path,
            } => {
                assert_eq!(existing_id, first.id());
                assert_eq!(existing_path, first_path);
            }
            CaptureOutcome::New(_) => panic!("expected a dedup, got a new row"),
        }
        assert_eq!(list(&core).unwrap().len(), 1, "no second row was filed");
    }

    #[test]
    fn recapturing_after_the_original_file_is_gone_files_a_new_row() {
        let core = Core::open_in_memory().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let first_path = dir.path().join("lux.7z");
        std::fs::write(&first_path, b"seven-bytes").unwrap();
        capture(
            &core,
            "https://example.test/x",
            "lux.7z",
            &first_path,
            Some(11),
        )
        .unwrap();
        std::fs::remove_file(&first_path).unwrap();

        let second_path = dir.path().join("lux (2).7z");
        std::fs::write(&second_path, b"seven-bytes").unwrap();
        let second = capture(
            &core,
            "https://example.test/x",
            "lux.7z",
            &second_path,
            Some(11),
        )
        .unwrap();

        assert!(
            matches!(second, CaptureOutcome::New(_)),
            "the original file is gone, so there's nothing to dedup against"
        );
        assert_eq!(list(&core).unwrap().len(), 2);
    }

    #[test]
    fn captures_with_no_known_size_never_dedup() {
        let core = Core::open_in_memory().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("lux.7z");
        std::fs::write(&path, b"x").unwrap();

        capture(&core, "https://example.test/x", "lux.7z", &path, None).unwrap();
        let second = capture(&core, "https://example.test/x", "lux.7z", &path, None).unwrap();

        assert!(matches!(second, CaptureOutcome::New(_)));
        assert_eq!(list(&core).unwrap().len(), 2);
    }

    #[test]
    fn attaching_shows_the_mod_archived_and_disabled() {
        let core = Core::open_in_memory().unwrap();
        let lo = loadout::create(&core, "skyrimse", "Main").unwrap();
        let dl = capture(
            &core,
            "https://example.test/lux.7z",
            "lux.7z",
            Path::new("F:/ModLab/downloads/lux.7z"),
            None,
        )
        .unwrap()
        .id();

        let mod_id = attach_to_loadout(&core, dl, lo.id, Some("Éclairage")).unwrap();

        let rows = loadout::entries(&core, lo.id).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].mod_id, mod_id);
        assert_eq!(rows[0].name, "lux", "named after the file stem");
        assert_eq!(rows[0].status, loadout::STATUS_ARCHIVED);
        assert!(!rows[0].enabled, "an unextracted archive is never enabled");
        assert_eq!(rows[0].category.as_deref(), Some("Éclairage"));
        assert_eq!(rows[0].version, None);

        let filed = &list(&core).unwrap()[0];
        assert_eq!(filed.loadout_id, Some(lo.id));
        assert_eq!(filed.mod_id, Some(mod_id));
    }

    #[test]
    fn attaching_a_recognized_archive_keeps_the_given_name_and_version() {
        let core = Core::open_in_memory().unwrap();
        let lo = loadout::create(&core, "skyrimse", "Main").unwrap();
        let dl = capture(
            &core,
            "https://skse.silverlock.org/skse64_2_02_06.7z",
            "skse64_2_02_06.7z",
            Path::new("F:/ModLab/downloads/skse64_2_02_06.7z"),
            None,
        )
        .unwrap()
        .id();

        let mod_id = attach_recognized(&core, dl, lo.id, None, "SKSE", Some("2.2.6")).unwrap();

        let rows = loadout::entries(&core, lo.id).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].mod_id, mod_id);
        assert_eq!(rows[0].name, "SKSE");
        assert_eq!(rows[0].version.as_deref(), Some("2.2.6"));
        assert_eq!(rows[0].status, loadout::STATUS_ARCHIVED);
        assert!(!rows[0].enabled);
        assert_eq!(
            rows[0].archive_path.as_deref(),
            Some(Path::new("F:/ModLab/downloads/skse64_2_02_06.7z"))
        );
    }

    #[test]
    fn installing_flips_the_status_and_repoints_the_source() {
        let core = Core::open_in_memory().unwrap();
        let lo = loadout::create(&core, "skyrimse", "Main").unwrap();
        let dl = capture(
            &core,
            "https://example.test/lux.7z",
            "lux.7z",
            Path::new("F:/ModLab/downloads/lux.7z"),
            None,
        )
        .unwrap()
        .id();
        let mod_id = attach_to_loadout(&core, dl, lo.id, None).unwrap();

        let pending = archived_mod(&core, mod_id).unwrap();
        assert_eq!(
            pending.archive_path,
            PathBuf::from("F:/ModLab/downloads/lux.7z")
        );

        mark_installed(&core, mod_id, Path::new("F:/ModLab/mods/lux")).unwrap();

        let rows = loadout::entries(&core, lo.id).unwrap();
        assert_eq!(rows[0].status, loadout::STATUS_INSTALLED);
        assert_eq!(rows[0].source_path, PathBuf::from("F:/ModLab/mods/lux"));
    }

    #[test]
    fn installing_an_already_installed_mod_is_refused() {
        let core = Core::open_in_memory().unwrap();
        let lo = loadout::create(&core, "skyrimse", "Main").unwrap();
        let dl = capture(
            &core,
            "https://example.test/lux.7z",
            "lux.7z",
            Path::new("F:/ModLab/downloads/lux.7z"),
            None,
        )
        .unwrap()
        .id();
        let mod_id = attach_to_loadout(&core, dl, lo.id, None).unwrap();
        mark_installed(&core, mod_id, Path::new("F:/ModLab/mods/lux")).unwrap();

        let err = archived_mod(&core, mod_id).unwrap_err();

        assert!(err.to_string().contains("installed"), "{err}");
    }

    #[test]
    fn reassigning_an_archived_mod_moves_it_to_the_other_loadout() {
        let core = Core::open_in_memory().unwrap();
        let a = loadout::create(&core, "skyrimse", "A").unwrap();
        let b = loadout::create(&core, "skyrimse", "B").unwrap();
        let dl = capture(
            &core,
            "https://example.test/x",
            "lux.7z",
            Path::new("lux.7z"),
            None,
        )
        .unwrap()
        .id();
        let mod_id = attach_to_loadout(&core, dl, a.id, None).unwrap();

        reassign(&core, mod_id, a.id, b.id).unwrap();

        assert_eq!(
            loadout::entries(&core, a.id).unwrap().len(),
            0,
            "gone from A"
        );
        let in_b = loadout::entries(&core, b.id).unwrap();
        assert_eq!(in_b.len(), 1, "landed in B");
        assert_eq!(in_b[0].mod_id, mod_id);
        assert!(!in_b[0].enabled, "still disabled — still not extracted");
    }

    #[test]
    fn reassigning_an_installed_mod_is_refused() {
        let core = Core::open_in_memory().unwrap();
        let a = loadout::create(&core, "skyrimse", "A").unwrap();
        let b = loadout::create(&core, "skyrimse", "B").unwrap();
        let dl = capture(
            &core,
            "https://example.test/x",
            "lux.7z",
            Path::new("lux.7z"),
            None,
        )
        .unwrap()
        .id();
        let mod_id = attach_to_loadout(&core, dl, a.id, None).unwrap();
        mark_installed(&core, mod_id, Path::new("F:/ModLab/mods/lux")).unwrap();

        let err = reassign(&core, mod_id, a.id, b.id).unwrap_err();

        assert!(err.to_string().contains("installé"), "{err}");
        assert_eq!(loadout::entries(&core, a.id).unwrap().len(), 1, "untouched");
    }

    #[test]
    fn renaming_a_mod_updates_its_entry() {
        let core = Core::open_in_memory().unwrap();
        let lo = loadout::create(&core, "skyrimse", "Main").unwrap();
        let dl = capture(
            &core,
            "https://example.test/x",
            "lux.7z",
            Path::new("lux.7z"),
            None,
        )
        .unwrap()
        .id();
        let mod_id = attach_to_loadout(&core, dl, lo.id, None).unwrap();

        rename(&core, mod_id, "  Lux Orbis  ").unwrap();

        assert_eq!(loadout::entries(&core, lo.id).unwrap()[0].name, "Lux Orbis");
    }

    #[test]
    fn renaming_to_an_empty_name_is_refused() {
        let core = Core::open_in_memory().unwrap();
        let lo = loadout::create(&core, "skyrimse", "Main").unwrap();
        let dl = capture(
            &core,
            "https://example.test/x",
            "lux.7z",
            Path::new("lux.7z"),
            None,
        )
        .unwrap()
        .id();
        let mod_id = attach_to_loadout(&core, dl, lo.id, None).unwrap();

        let err = rename(&core, mod_id, "   ").unwrap_err();

        assert!(err.to_string().contains("vide"), "{err}");
        assert_eq!(loadout::entries(&core, lo.id).unwrap()[0].name, "lux");
    }

    #[test]
    fn removing_a_mod_hides_it_but_keeps_its_row() {
        let core = Core::open_in_memory().unwrap();
        let lo = loadout::create(&core, "skyrimse", "Main").unwrap();
        let dl = capture(
            &core,
            "https://example.test/x",
            "lux.7z",
            Path::new("lux.7z"),
            None,
        )
        .unwrap()
        .id();
        let mod_id = attach_to_loadout(&core, dl, lo.id, None).unwrap();

        remove(&core, mod_id).unwrap();

        assert_eq!(
            loadout::entries(&core, lo.id).unwrap().len(),
            0,
            "hidden from the table"
        );
        // Not actually gone: the archive lookup (which ignores deleted_at)
        // still finds the row.
        assert!(
            core.select_mod_archive(mod_id).unwrap().is_some(),
            "row still exists"
        );
    }

    #[test]
    fn uninstalling_reverts_to_archived_and_repoints_the_source() {
        let core = Core::open_in_memory().unwrap();
        let lo = loadout::create(&core, "skyrimse", "Main").unwrap();
        let dl = capture(
            &core,
            "https://example.test/x",
            "lux.7z",
            Path::new("F:/ModLab/downloads/lux.7z"),
            None,
        )
        .unwrap()
        .id();
        let mod_id = attach_to_loadout(&core, dl, lo.id, None).unwrap();
        mark_installed(&core, mod_id, Path::new("F:/ModLab/mods/lux")).unwrap();

        uninstall(&core, mod_id).unwrap();

        let rows = loadout::entries(&core, lo.id).unwrap();
        assert_eq!(rows[0].status, loadout::STATUS_ARCHIVED);
        assert_eq!(
            rows[0].source_path,
            PathBuf::from("F:/ModLab/downloads/lux.7z")
        );
    }

    #[test]
    fn uninstalling_an_archived_mod_is_refused() {
        let core = Core::open_in_memory().unwrap();
        let lo = loadout::create(&core, "skyrimse", "Main").unwrap();
        let dl = capture(
            &core,
            "https://example.test/x",
            "lux.7z",
            Path::new("lux.7z"),
            None,
        )
        .unwrap()
        .id();
        let mod_id = attach_to_loadout(&core, dl, lo.id, None).unwrap();

        let err = uninstall(&core, mod_id).unwrap_err();

        assert!(err.to_string().contains("installé"), "{err}");
    }

    #[test]
    fn categories_are_deduplicated_and_shared_across_games() {
        let core = Core::open_in_memory().unwrap();
        let sky = loadout::create(&core, "skyrimse", "Main").unwrap();
        let mc = loadout::create(&core, "minecraft", "Survie").unwrap();
        for (name, lo, cat) in [
            ("a.7z", sky.id, Some("Visuel")),
            ("b.7z", mc.id, Some("Visuel")),
            ("c.7z", sky.id, Some("Gameplay")),
            ("d.7z", sky.id, None),
        ] {
            let dl = capture(&core, "https://example.test/x", name, Path::new(name), None)
                .unwrap()
                .id();
            attach_to_loadout(&core, dl, lo, cat).unwrap();
        }

        assert_eq!(categories(&core).unwrap(), vec!["Gameplay", "Visuel"]);
    }

    #[test]
    fn beginning_a_download_lists_a_disabled_downloading_mod() {
        let core = Core::open_in_memory().unwrap();
        let lo = loadout::create(&core, "skyrimse", "Main").unwrap();

        let started = begin(
            &core,
            "https://example.test/lux.7z",
            "lux.7z",
            Path::new("F:/ModLab/downloads/lux.7z"),
            lo.id,
        )
        .unwrap();

        let rows = loadout::entries(&core, lo.id).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].mod_id, started.mod_id);
        assert_eq!(rows[0].name, "lux");
        assert_eq!(rows[0].status, loadout::STATUS_DOWNLOADING);
        assert!(!rows[0].enabled, "nothing has arrived, so it cannot deploy");
        assert!(
            archived_mod(&core, started.mod_id).is_err(),
            "a transfer still in flight is not an archive to unpack"
        );
        assert_eq!(
            phase(&core, started.download_id).unwrap().as_deref(),
            Some(PHASE_QUEUED)
        );

        let listed = list(&core).unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].id, started.download_id);
        assert_eq!(listed[0].loadout_id, Some(lo.id));
        assert_eq!(listed[0].mod_id, Some(started.mod_id));
        assert_eq!(listed[0].size_bytes, None);

        // The browser may report the other slash direction.
        let found = in_progress_at(&core, Path::new("F:\\ModLab\\downloads\\lux.7z")).unwrap();
        assert_eq!(found, Some(started.download_id));
    }

    #[test]
    fn transfer_phase_moves_queued_progress_archived_and_reconcile_uses_disk() {
        let core = Core::open_in_memory().unwrap();
        let lo = loadout::create(&core, "minecraft", "Survival").unwrap();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("MouseTweaks.jar");

        let started = begin(
            &core,
            "https://cdn.modrinth.com/data/mouse/MouseTweaks.jar",
            "MouseTweaks.jar",
            &path,
            lo.id,
        )
        .unwrap();
        assert_eq!(
            phase(&core, started.download_id).unwrap().as_deref(),
            Some(PHASE_QUEUED)
        );

        note_progress(&core, started.download_id).unwrap();
        assert_eq!(
            phase(&core, started.download_id).unwrap().as_deref(),
            Some(PHASE_PROGRESS)
        );
        note_progress(&core, started.download_id).unwrap();
        assert_eq!(
            phase(&core, started.download_id).unwrap().as_deref(),
            Some(PHASE_PROGRESS),
            "idempotent"
        );

        assert!(matches!(
            reconcile(&core, started.download_id, false).unwrap(),
            ReconcileOutcome::NotReady
        ));

        std::fs::write(&path, b"jar-bytes").unwrap();
        match reconcile(&core, started.download_id, false).unwrap() {
            ReconcileOutcome::Completed(finished) => {
                assert_eq!(finished.mod_id, started.mod_id);
                assert_eq!(
                    loadout::entries(&core, lo.id).unwrap()[0].status,
                    loadout::STATUS_ARCHIVED
                );
            }
            other => panic!("expected Completed, got {other:?}"),
        }
        assert_eq!(
            phase(&core, started.download_id).unwrap().as_deref(),
            Some(PHASE_ARCHIVED)
        );
        assert_eq!(
            loadout::entries(&core, lo.id).unwrap()[0].status,
            loadout::STATUS_ARCHIVED
        );
        assert!(matches!(
            reconcile(&core, started.download_id, true).unwrap(),
            ReconcileOutcome::AlreadyDone
        ));
    }

    #[test]
    fn reconcile_give_up_without_file_fails_the_transfer() {
        let core = Core::open_in_memory().unwrap();
        let lo = loadout::create(&core, "minecraft", "Survival").unwrap();
        let started = begin(
            &core,
            "https://cdn.modrinth.com/data/x/missing.jar",
            "missing.jar",
            Path::new("F:/ModLab/downloads/minecraft/missing.jar"),
            lo.id,
        )
        .unwrap();

        match reconcile(&core, started.download_id, true).unwrap() {
            ReconcileOutcome::Failed {
                loadout_id,
                file_name,
            } => {
                assert_eq!(loadout_id, Some(lo.id));
                assert_eq!(file_name, "missing.jar");
            }
            other => panic!("expected Failed, got {other:?}"),
        }
        assert!(loadout::entries(&core, lo.id).unwrap().is_empty());
        assert!(list(&core).unwrap().is_empty());
        assert!(phase(&core, started.download_id).unwrap().is_none());
    }

    #[test]
    fn completing_a_download_promotes_the_same_row_to_archived() {
        let core = Core::open_in_memory().unwrap();
        let lo = loadout::create(&core, "skyrimse", "Main").unwrap();
        let started = begin(
            &core,
            "https://example.test/lux.7z",
            "lux.7z",
            Path::new("F:/ModLab/downloads/lux.7z"),
            lo.id,
        )
        .unwrap();

        let finished = complete(&core, started.download_id, Some(99), None, None).unwrap();

        assert_eq!(finished.mod_id, started.mod_id);
        assert_eq!(finished.display_name, "lux");
        assert!(finished.redundant_path.is_none());
        let rows = loadout::entries(&core, lo.id).unwrap();
        assert_eq!(rows.len(), 1, "completion must not insert a second mod");
        assert_eq!(rows[0].mod_id, started.mod_id);
        assert_eq!(rows[0].status, loadout::STATUS_ARCHIVED);
        assert!(!rows[0].enabled);
        assert_eq!(rows[0].name, "lux");
        assert_eq!(rows[0].size_bytes, Some(99));
        assert_eq!(list(&core).unwrap()[0].size_bytes, Some(99));

        // A repeated finish records the size again and still does not duplicate.
        complete(
            &core,
            started.download_id,
            Some(100),
            Some("renamed"),
            Some("9"),
        )
        .unwrap();
        let rows = loadout::entries(&core, lo.id).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].status, loadout::STATUS_ARCHIVED);
        assert_eq!(rows[0].name, "lux", "a second finish does not rename");
        assert_eq!(rows[0].size_bytes, Some(100));
        assert_eq!(list(&core).unwrap()[0].size_bytes, Some(100));
    }

    #[test]
    fn in_progress_named_finds_a_row_when_the_finish_path_differs() {
        let core = Core::open_in_memory().unwrap();
        let lo = loadout::create(&core, "minecraft", "Survival").unwrap();
        let started = begin(
            &core,
            "https://cdn.modrinth.com/data/sodium.jar",
            "sodium.jar",
            Path::new("F:/ModLab/downloads/minecraft/sodium.jar"),
            lo.id,
        )
        .unwrap();

        assert_eq!(
            in_progress_named(&core, "sodium.jar").unwrap(),
            Some(started.download_id)
        );
        // Finish reported a different spelling — retarget then complete.
        let actual = Path::new("F:\\ModLab\\downloads\\minecraft\\sodium.jar");
        retarget_path(&core, started.download_id, actual).unwrap();
        let finished = complete(&core, started.download_id, Some(12), None, None).unwrap();
        assert_eq!(finished.mod_id, started.mod_id);
        assert_eq!(
            loadout::entries(&core, lo.id).unwrap()[0].status,
            loadout::STATUS_ARCHIVED
        );
        assert!(in_progress_named(&core, "sodium.jar").unwrap().is_none());
    }

    #[test]
    fn progress_row_and_extended_path_still_find_a_downloading_mod() {
        let core = Core::open_in_memory().unwrap();
        let lo = loadout::create(&core, "minecraft", "Survival").unwrap();
        let path = Path::new("F:/ModLab/downloads/minecraft/iris.jar");
        let started = begin(
            &core,
            "https://cdn.modrinth.com/data/iris.jar?token=1",
            "iris.jar",
            path,
            lo.id,
        )
        .unwrap();
        let row = progress_row(&core, started.download_id).unwrap().unwrap();
        assert_eq!(row.status, loadout::STATUS_DOWNLOADING);
        assert_eq!(row.file_name, "iris.jar");
        assert!(ledger_has_path(&core, path).unwrap());
        let extended = Path::new(r"\\?\F:\ModLab\downloads\minecraft\iris.jar");
        assert_eq!(
            in_progress_at(&core, extended).unwrap(),
            Some(started.download_id)
        );
        complete(&core, started.download_id, Some(8), None, None).unwrap();
        let done = progress_row(&core, started.download_id).unwrap().unwrap();
        assert_eq!(done.status, loadout::STATUS_ARCHIVED);
        assert!(in_progress_at(&core, extended).unwrap().is_none());
        assert!(ledger_has_path(&core, extended).unwrap());
    }

    #[test]
    fn completing_with_a_recognized_name_renames_the_same_row() {
        let core = Core::open_in_memory().unwrap();
        let lo = loadout::create(&core, "skyrimse", "Main").unwrap();
        let started = begin(
            &core,
            "https://skse.silverlock.org/skse64_2_02_06.7z",
            "skse64_2_02_06.7z",
            Path::new("F:/ModLab/downloads/skse64_2_02_06.7z"),
            lo.id,
        )
        .unwrap();
        assert_eq!(
            loadout::entries(&core, lo.id).unwrap()[0].name,
            "skse64 2 02 06",
            "underscores become spaces until recognition runs"
        );

        complete(
            &core,
            started.download_id,
            Some(50),
            Some("SKSE"),
            Some("2.2.6"),
        )
        .unwrap();

        let rows = loadout::entries(&core, lo.id).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].mod_id, started.mod_id);
        assert_eq!(rows[0].name, "SKSE");
        assert_eq!(rows[0].version.as_deref(), Some("2.2.6"));
        assert_eq!(rows[0].status, loadout::STATUS_ARCHIVED);
        assert!(!rows[0].enabled);
    }

    #[test]
    fn attaching_while_downloading_promotes_instead_of_inserting() {
        let core = Core::open_in_memory().unwrap();
        let lo = loadout::create(&core, "skyrimse", "Main").unwrap();
        let started = begin(
            &core,
            "https://skse.silverlock.org/skse64_2_02_06.7z",
            "skse64_2_02_06.7z",
            Path::new("F:/ModLab/downloads/skse64_2_02_06.7z"),
            lo.id,
        )
        .unwrap();

        let mod_id = attach_recognized(
            &core,
            started.download_id,
            lo.id,
            None,
            "SKSE",
            Some("2.2.6"),
        )
        .unwrap();

        assert_eq!(mod_id, started.mod_id);
        let rows = loadout::entries(&core, lo.id).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].name, "SKSE");
        assert_eq!(rows[0].version.as_deref(), Some("2.2.6"));
        assert_eq!(rows[0].status, loadout::STATUS_ARCHIVED);
    }

    #[test]
    fn abandoning_a_download_hides_the_mod_and_drops_the_ledger_row() {
        let core = Core::open_in_memory().unwrap();
        let lo = loadout::create(&core, "skyrimse", "Main").unwrap();
        let started = begin(
            &core,
            "https://example.test/lux.7z",
            "lux.7z",
            Path::new("F:/ModLab/downloads/lux.7z"),
            lo.id,
        )
        .unwrap();

        let hidden = abandon(&core, started.download_id).unwrap();

        assert_eq!(hidden, Some(lo.id));
        assert!(
            loadout::entries(&core, lo.id).unwrap().is_empty(),
            "a failed transfer leaves no row to install"
        );
        assert!(list(&core).unwrap().is_empty());
        // Still in the database, same rule as any removed mod.
        assert!(core.select_mod_archive(started.mod_id).unwrap().is_some());
    }

    #[test]
    fn abandoning_after_completion_leaves_the_archived_mod() {
        let core = Core::open_in_memory().unwrap();
        let lo = loadout::create(&core, "skyrimse", "Main").unwrap();
        let started = begin(
            &core,
            "https://example.test/lux.7z",
            "lux.7z",
            Path::new("F:/ModLab/downloads/lux.7z"),
            lo.id,
        )
        .unwrap();
        complete(&core, started.download_id, Some(4), None, None).unwrap();

        assert_eq!(abandon(&core, started.download_id).unwrap(), None);
        let rows = loadout::entries(&core, lo.id).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].status, loadout::STATUS_ARCHIVED);
        assert_eq!(list(&core).unwrap().len(), 1);
    }

    #[test]
    fn completing_a_duplicate_reuses_the_existing_archive() {
        let core = Core::open_in_memory().unwrap();
        let lo = loadout::create(&core, "skyrimse", "Main").unwrap();
        let dir = tempfile::tempdir().unwrap();
        let existing_path = dir.path().join("lux.7z");
        std::fs::write(&existing_path, b"seven-bytes").unwrap();
        let existing_id = capture(
            &core,
            "https://example.test/x",
            "lux.7z",
            &existing_path,
            Some(11),
        )
        .unwrap()
        .id();

        let second = dir.path().join("again").join("lux.7z");
        std::fs::create_dir_all(second.parent().unwrap()).unwrap();
        std::fs::write(&second, b"seven-bytes").unwrap();
        let started = begin(&core, "https://example.test/x", "lux.7z", &second, lo.id).unwrap();

        let finished = complete(&core, started.download_id, Some(11), None, None).unwrap();

        assert_eq!(finished.download_id, existing_id);
        assert_eq!(finished.mod_id, started.mod_id);
        assert_eq!(finished.redundant_path.as_deref(), Some(second.as_path()));
        assert_eq!(finished.archive_path, existing_path);

        let rows = loadout::entries(&core, lo.id).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].status, loadout::STATUS_ARCHIVED);
        assert_eq!(
            rows[0].archive_path.as_deref(),
            Some(existing_path.as_path())
        );

        let all = list(&core).unwrap();
        assert_eq!(all.len(), 1, "the placeholder ledger row is gone");
        assert_eq!(all[0].id, existing_id);
        assert_eq!(all[0].mod_id, Some(started.mod_id));
        assert_eq!(all[0].loadout_id, Some(lo.id));
        assert_eq!(all[0].path, existing_path);
    }

    #[test]
    fn a_download_keeps_its_page_and_version_after_the_file_is_gone() {
        let core = Core::open_in_memory().unwrap();
        let lo = loadout::create(&core, "skyrimse", "Main").unwrap();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("Lux-2.2.6.7z");
        std::fs::write(&path, b"not-a-real-archive").unwrap();

        let started = begin_with_origin(
            &core,
            "https://cf-files.nexusmods.com/cdn/lux.7z",
            "Lux-2.2.6.7z",
            &path,
            lo.id,
            &DownloadOrigin {
                source_host: "nexus".to_string(),
                page_url: Some(
                    "https://www.nexusmods.com/skyrimspecialedition/mods/123".to_string(),
                ),
            },
        )
        .unwrap();

        std::fs::remove_file(&path).unwrap();
        assert!(!path.exists(), "the archive file is gone");

        let facts = mod_facts(&core, started.mod_id).unwrap().unwrap();
        assert_eq!(facts.source_host.as_deref(), Some("nexus"));
        assert_eq!(
            facts.page_url.as_deref(),
            Some("https://www.nexusmods.com/skyrimspecialedition/mods/123")
        );
        assert_eq!(
            facts.file_url.as_deref(),
            Some("https://cf-files.nexusmods.com/cdn/lux.7z")
        );
        assert_eq!(facts.version.as_deref(), Some("2.2.6"));
        assert!(!facts.deleted);

        let rows = loadout::entries(&core, lo.id).unwrap();
        assert_eq!(rows.len(), 1, "the fiche stays in the list");
        assert_eq!(rows[0].source_host.as_deref(), Some("nexus"));
        assert_eq!(rows[0].version.as_deref(), Some("2.2.6"));

        apply_archive_metadata(
            &core,
            started.mod_id,
            &crate::game_adapter::ArchiveMetadata::default(),
        )
        .unwrap();
        let facts = mod_facts(&core, started.mod_id).unwrap().unwrap();
        assert_eq!(
            facts.version.as_deref(),
            Some("2.2.6"),
            "an empty read must not wipe a version"
        );

        remove(&core, started.mod_id).unwrap();
        assert!(loadout::entries(&core, lo.id).unwrap().is_empty());
        let facts = mod_facts(&core, started.mod_id).unwrap().unwrap();
        assert!(
            facts.deleted,
            "soft-delete hides the row, it does not drop it"
        );
        assert_eq!(facts.source_host.as_deref(), Some("nexus"));
        assert_eq!(
            facts.page_url.as_deref(),
            Some("https://www.nexusmods.com/skyrimspecialedition/mods/123")
        );
        assert_eq!(facts.version.as_deref(), Some("2.2.6"));
    }

    #[test]
    fn two_mods_keep_distinct_game_versions_without_their_files() {
        let core = Core::open_in_memory().unwrap();
        let lo = loadout::create(&core, "minecraft", "Survie").unwrap();
        let dir = tempfile::tempdir().unwrap();
        let old = dir.path().join("alpha-1.12.2.jar");
        let new = dir.path().join("beta-1.16.5.jar");
        std::fs::write(&old, b"a").unwrap();
        std::fs::write(&new, b"b").unwrap();

        let first = begin(
            &core,
            "https://example.test/a",
            "alpha-1.12.2.jar",
            &old,
            lo.id,
        )
        .unwrap();
        let second = begin(
            &core,
            "https://example.test/b",
            "beta-1.16.5.jar",
            &new,
            lo.id,
        )
        .unwrap();
        apply_archive_metadata(
            &core,
            first.mod_id,
            &crate::game_adapter::ArchiveMetadata {
                game_versions: vec!["1.12.2".to_string()],
                dependencies: vec![crate::game_adapter::DeclaredDependency::required("jei")],
                ..crate::game_adapter::ArchiveMetadata::default()
            },
        )
        .unwrap();
        apply_archive_metadata(
            &core,
            second.mod_id,
            &crate::game_adapter::ArchiveMetadata {
                game_versions: vec!["1.16.5".to_string()],
                ..crate::game_adapter::ArchiveMetadata::default()
            },
        )
        .unwrap();
        std::fs::remove_file(&old).unwrap();
        std::fs::remove_file(&new).unwrap();

        let rows = loadout::entries(&core, lo.id).unwrap();
        assert_eq!(rows[0].game_versions, vec!["1.12.2".to_string()]);
        assert_eq!(
            rows[0].dependencies,
            vec![crate::game_adapter::DeclaredDependency::required("jei")]
        );
        assert_eq!(rows[1].game_versions, vec!["1.16.5".to_string()]);
        assert!(rows[1].dependencies.is_empty());
    }

    #[test]
    fn a_skse_underscore_name_is_not_a_dotted_version() {
        assert_eq!(version_from_file_name("skse64_2_02_06.7z"), None);
        assert_eq!(
            version_from_file_name("Lux-2.2.6.7z").as_deref(),
            Some("2.2.6")
        );
        assert_eq!(version_from_file_name("notes.txt"), None);
        assert_eq!(
            version_from_file_name("SkyUI-12604-6-11-1778020881 (3).7z").as_deref(),
            Some("6.11")
        );
    }

    #[test]
    fn a_raw_nexus_name_is_rewritten_unless_the_human_renamed_it() {
        let core = Core::open_in_memory().unwrap();
        let lo = loadout::create(&core, "skyrimse", "Main").unwrap();
        let file = "SkyUI-12604-6-11-1778020881 (3).7z";
        let started = begin(
            &core,
            "https://example.test/skyui.7z",
            file,
            Path::new("F:/ModLab/downloads/SkyUI-12604-6-11-1778020881 (3).7z"),
            lo.id,
        )
        .unwrap();

        let rows = loadout::entries(&core, lo.id).unwrap();
        assert_eq!(rows[0].name, "SkyUI");
        assert_eq!(rows[0].version.as_deref(), Some("6.11"));
        assert_eq!(rows[0].nexus_mod_id.as_deref(), Some("12604"));
        assert_eq!(
            crate::mod_meta::recorded_layer(&core, started.mod_id, "name")
                .unwrap()
                .map(|layer| layer.as_str()),
            Some("filename")
        );

        // A row saved before provenance existed: the ugly file label, nothing else.
        core.set_mod_name(started.mod_id, "SkyUI-12604-6-11-1778020881 (3)")
            .unwrap();
        core.set_mod_version(started.mod_id, None).unwrap();
        core.set_field_sources(started.mod_id, None).unwrap();
        crate::mod_meta::refresh_stored(
            &core,
            started.mod_id,
            &[file.to_string()],
            Some("nexus"),
            Some("https://www.nexusmods.com/skyrimspecialedition/mods/12604"),
            None,
            None,
            false,
        )
        .unwrap();
        let facts = mod_facts(&core, started.mod_id).unwrap().unwrap();
        assert_eq!(loadout::entries(&core, lo.id).unwrap()[0].name, "SkyUI");
        assert_eq!(facts.version.as_deref(), Some("6.11"));
        assert_eq!(facts.nexus_mod_id.as_deref(), Some("12604"));
        assert_eq!(
            facts.page_url.as_deref(),
            Some("https://www.nexusmods.com/skyrimspecialedition/mods/12604")
        );
        assert_eq!(facts.source_host.as_deref(), Some("nexus"));

        rename(&core, started.mod_id, "Mon SkyUI").unwrap();
        let mut locked = crate::mod_meta::Patch::default();
        locked.version = Some("9.9".to_string());
        crate::mod_meta::apply_layer(
            &core,
            started.mod_id,
            crate::mod_meta::Layer::User,
            &locked,
            &[],
            false,
        )
        .unwrap();
        crate::mod_meta::refresh_stored(
            &core,
            started.mod_id,
            &[file.to_string()],
            None,
            None,
            Some(&crate::game_adapter::ArchiveMetadata {
                name: Some("SkyUI officiel".to_string()),
                version: Some("1.0".to_string()),
                category_raw: Some("User Interface".to_string()),
                category_nexus_id: Some("51".to_string()),
                author: Some("schlangster".to_string()),
                ..crate::game_adapter::ArchiveMetadata::default()
            }),
            None,
            false,
        )
        .unwrap();
        let facts = mod_facts(&core, started.mod_id).unwrap().unwrap();
        assert_eq!(loadout::entries(&core, lo.id).unwrap()[0].name, "Mon SkyUI");
        assert_eq!(facts.version.as_deref(), Some("9.9"));
        assert_eq!(facts.category, None, "the raw label is not the human tag");
        assert_eq!(facts.category_raw.as_deref(), Some("User Interface"));
        assert_eq!(facts.category_nexus_id.as_deref(), Some("51"));
        assert_eq!(facts.author.as_deref(), Some("schlangster"));
    }

    #[test]
    fn the_download_date_is_set_at_the_start_and_survives_the_archive() {
        let core = Core::open_in_memory().unwrap();
        let lo = loadout::create(&core, "skyrimse", "Main").unwrap();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("Lux-2.2.6.7z");
        std::fs::write(&path, b"bytes").unwrap();

        let started = begin_with_origin(
            &core,
            "https://cf-files.nexusmods.com/cdn/lux.7z",
            "Lux-2.2.6.7z",
            &path,
            lo.id,
            &DownloadOrigin {
                source_host: "nexus".to_string(),
                page_url: Some(
                    "https://www.nexusmods.com/skyrimspecialedition/mods/123".to_string(),
                ),
            },
        )
        .unwrap();

        let stamped = loadout::entries(&core, lo.id).unwrap()[0]
            .downloaded_at
            .clone()
            .expect("the row shows a date as soon as the transfer starts");
        let captured = list(&core).unwrap()[0].captured_at.clone();
        assert_eq!(stamped, captured, "the date is the capture, at the start");

        complete(&core, started.download_id, Some(5), None, None).unwrap();
        assert_eq!(
            loadout::entries(&core, lo.id).unwrap()[0]
                .downloaded_at
                .as_deref(),
            Some(stamped.as_str()),
            "finishing the transfer does not move the date"
        );

        mark_installed(&core, started.mod_id, &dir.path().join("installed")).unwrap();
        assert_eq!(
            loadout::entries(&core, lo.id).unwrap()[0]
                .downloaded_at
                .as_deref(),
            Some(stamped.as_str()),
            "installing does not move the date"
        );

        std::fs::remove_file(&path).unwrap();
        core.delete_download(started.download_id).unwrap();
        crate::mod_measure::refresh_one(&core, started.mod_id).unwrap();
        assert_eq!(
            loadout::entries(&core, lo.id).unwrap()[0]
                .downloaded_at
                .as_deref(),
            Some(stamped.as_str()),
            "the date stays after the archive and the ledger row are gone"
        );
    }

    #[test]
    fn attaching_a_capture_uses_the_capture_date_not_the_attach_time() {
        let core = Core::open_in_memory().unwrap();
        let lo = loadout::create(&core, "skyrimse", "Main").unwrap();
        let id = capture(
            &core,
            "https://example.test/lux.7z",
            "lux.7z",
            Path::new("F:/ModLab/downloads/lux.7z"),
            Some(4),
        )
        .unwrap()
        .id();
        core.set_download_captured_at(id, "2019-04-01T00:00:00.000Z")
            .unwrap();

        let mod_id = attach_to_loadout(&core, id, lo.id, None).unwrap();

        assert_eq!(
            loadout::entries(&core, lo.id).unwrap()[0]
                .downloaded_at
                .as_deref(),
            Some("2019-04-01T00:00:00.000Z"),
            "filing the capture later must not replace its date"
        );
        assert_eq!(mod_id, loadout::entries(&core, lo.id).unwrap()[0].mod_id);
    }

    #[test]
    fn a_capture_attached_later_replaces_the_download_date() {
        let core = Core::open_in_memory().unwrap();
        let lo = loadout::create(&core, "skyrimse", "Main").unwrap();
        let dir = tempfile::tempdir().unwrap();
        let existing_path = dir.path().join("lux.7z");
        std::fs::write(&existing_path, b"seven-bytes").unwrap();
        let existing_id = capture(
            &core,
            "https://example.test/x",
            "lux.7z",
            &existing_path,
            Some(11),
        )
        .unwrap()
        .id();
        core.set_download_captured_at(existing_id, "2019-04-01T00:00:00.000Z")
            .unwrap();

        let second = dir.path().join("again").join("lux.7z");
        std::fs::create_dir_all(second.parent().unwrap()).unwrap();
        std::fs::write(&second, b"seven-bytes").unwrap();
        let started = begin(&core, "https://example.test/x", "lux.7z", &second, lo.id).unwrap();
        let started_at = loadout::entries(&core, lo.id).unwrap()[0]
            .downloaded_at
            .clone()
            .unwrap();
        assert_ne!(started_at, "2019-04-01T00:00:00.000Z");

        complete(&core, started.download_id, Some(11), None, None).unwrap();

        assert_eq!(
            loadout::entries(&core, lo.id).unwrap()[0]
                .downloaded_at
                .as_deref(),
            Some("2019-04-01T00:00:00.000Z"),
            "the capture we keep replaces the date stamped at the start"
        );
    }
}
