//! Byte progress for a download the embedded browser is already capturing.
//!
//! Tauri's `on_download` hook has no progress event. WebView2 does: a second
//! `DownloadStarting` handler (Tauri's stays registered) listens to
//! `BytesReceivedChanged` and emits `download://progress`. This handler never
//! sets `Handled`, `ResultFilePath` or `Cancel` — the capture path owns those.

use crate::downloads::{InFlight, PendingOrigin};
use crate::AppState;
use serde::Serialize;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager, Webview};

/// At most four progress events per second, unless the integer percent moved.
const MIN_GAP: Duration = Duration::from_millis(250);

/// Emitted while bytes are arriving. Nothing is written to the database:
/// the shell drops this when the transfer ends.
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct DownloadProgress {
    download_id: Option<i64>,
    url: String,
    received: u64,
    total: Option<u64>,
}

/// What the file URL corresponds to in the in-memory transfer lists.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ProgressTarget {
    /// `begin` filed a row. The id is the ledger id, which is also the mod's download.
    Row(i64),
    /// The URL was snapshotted at start, but no loadout was on screen, so there is no id.
    Unfiled,
    /// Not a transfer this shell recorded.
    Unknown,
}

impl ProgressTarget {
    fn download_id(self) -> Option<i64> {
        match self {
            Self::Row(id) => Some(id),
            Self::Unfiled | Self::Unknown => None,
        }
    }
}

/// Per-transfer throttle. Owned by the bytes callback, which is the only caller.
#[derive(Default)]
struct Gate {
    last_at: Option<Instant>,
    last_percent: Option<u8>,
}

impl Gate {
    /// `true` when this sample should be emitted.
    fn allow(&mut self, received: u64, total: Option<u64>, now: Instant, force: bool) -> bool {
        let percent = percent_points(received, total);
        let stepped = match (percent, self.last_percent) {
            (Some(next), Some(prev)) => next > prev,
            (Some(_), None) => true,
            (None, _) => false,
        };
        let due = match self.last_at {
            None => true,
            Some(prev) => now.saturating_duration_since(prev) >= MIN_GAP,
        };
        if !(force || stepped || due) {
            return false;
        }
        self.last_at = Some(now);
        if let Some(percent) = percent {
            self.last_percent = Some(percent);
        }
        true
    }
}

fn percent_points(received: u64, total: Option<u64>) -> Option<u8> {
    let total = total.filter(|n| *n > 0)?;
    Some((received.saturating_mul(100) / total).min(100) as u8)
}

fn urls_match(stored: &str, reported: &str) -> bool {
    if stored == reported {
        return true;
    }
    match (url::Url::parse(stored), url::Url::parse(reported)) {
        (Ok(left), Ok(right)) => {
            left.scheme() == right.scheme()
                && left.host_str() == right.host_str()
                && left.port_or_known_default() == right.port_or_known_default()
                && left.path() == right.path()
        }
        _ => false,
    }
}

/// Newest in-flight row wins when the same URL was started twice.
fn matching_download_id(in_flight: &[InFlight], url: &str) -> Option<i64> {
    in_flight
        .iter()
        .rev()
        .find(|item| urls_match(&item.url, url))
        .map(|item| item.download_id)
}

fn classify(in_flight: &[InFlight], pending: &[PendingOrigin], url: &str) -> ProgressTarget {
    if let Some(id) = matching_download_id(in_flight, url) {
        return ProgressTarget::Row(id);
    }
    if pending.iter().any(|item| urls_match(&item.file_url, url)) {
        return ProgressTarget::Unfiled;
    }
    ProgressTarget::Unknown
}

fn clone_locked<T: Clone>(mutex: &Mutex<Vec<T>>) -> Vec<T> {
    match mutex.lock() {
        Ok(guard) => guard.clone(),
        Err(_) => Vec::new(),
    }
}

/// Id for `url`, if `begin` filed one. A pending origin (no instance on screen)
/// is a known transfer with no id. Locks are not held past this call.
fn download_id_for(state: &AppState, url: &str) -> Option<i64> {
    let in_flight = clone_locked(&state.in_flight);
    let pending = clone_locked(&state.pending_origins);
    classify(&in_flight, &pending, url).download_id()
}

/// Install the WebView2 progress listener on the embedded browser.
///
/// Safe to call once, when that webview has just been created. On the main
/// thread Tauri runs the closure immediately, so the handler is in place
/// before the window starts accepting clicks.
pub fn attach(webview: &Webview) {
    #[cfg(windows)]
    {
        let app = webview.app_handle().clone();
        if let Err(err) = webview.with_webview(move |pw| platform::install(app, pw)) {
            eprintln!("[dl] progression non branchée : {err}");
        }
    }
    #[cfg(not(windows))]
    {
        let _ = webview;
    }
}

fn emit_progress(app: &AppHandle, url: &str, received: u64, total: Option<u64>) {
    let download_id = app.try_state::<AppState>().and_then(|state| {
        let id = download_id_for(&state, url);
        // Core state machine: queued → progress (idempotent).
        if let Some(download_id) = id {
            if let Ok(core) = state.core.lock() {
                let _ = modlab_core::download::note_progress(&core, download_id);
            }
        }
        id
    });
    let _ = app.emit(
        "download://progress",
        DownloadProgress {
            download_id,
            url: url.to_string(),
            received,
            total,
        },
    );
}

#[cfg(windows)]
mod platform {
    use super::{emit_progress, Gate};
    use std::cell::Cell;
    use std::rc::Rc;
    use std::time::Instant;
    use tauri::webview::PlatformWebview;
    use tauri::AppHandle;
    use std::path::PathBuf;
    use webview2_com::Microsoft::Web::WebView2::Win32::{
        ICoreWebView2DownloadOperation, ICoreWebView2DownloadStartingEventArgs, ICoreWebView2_4,
        COREWEBVIEW2_DOWNLOAD_STATE, COREWEBVIEW2_DOWNLOAD_STATE_COMPLETED,
        COREWEBVIEW2_DOWNLOAD_STATE_IN_PROGRESS,
    };
    use webview2_com::{
        take_pwstr, BytesReceivedChangedEventHandler, DownloadStartingEventHandler,
        StateChangedEventHandler,
    };
    use windows::core::{Interface, PWSTR};

    pub fn install(app: AppHandle, pw: PlatformWebview) {
        let core = match unsafe { pw.controller().CoreWebView2() } {
            Ok(core) => core,
            Err(err) => {
                eprintln!("[dl] progression : CoreWebView2 indisponible ({err})");
                return;
            }
        };
        let webview4 = match core.cast::<ICoreWebView2_4>() {
            Ok(webview4) => webview4,
            Err(err) => {
                eprintln!("[dl] progression : WebView2 trop ancien ({err})");
                return;
            }
        };
        let mut token = 0_i64;
        let handler = DownloadStartingEventHandler::create(Box::new(move |_sender, args| {
            if let Some(args) = args {
                if let Err(err) = watch(&app, &args) {
                    eprintln!("[dl] progression non suivie : {err}");
                }
            }
            // Do not touch Handled, ResultFilePath or Cancel. Tauri's own
            // DownloadStarting handler is the one that redirects the file.
            Ok(())
        }));
        if let Err(err) = unsafe { webview4.add_DownloadStarting(&handler, &mut token) } {
            eprintln!("[dl] abonnement DownloadStarting : {err}");
        }
    }

    fn watch(
        app: &AppHandle,
        args: &ICoreWebView2DownloadStartingEventArgs,
    ) -> windows::core::Result<()> {
        let operation = unsafe { args.DownloadOperation()? };
        let url = read_uri(&operation)?;
        if url.is_empty() {
            return Ok(());
        }
        subscribe(app, &operation, url)
    }

    fn subscribe(
        app: &AppHandle,
        operation: &ICoreWebView2DownloadOperation,
        url: String,
    ) -> windows::core::Result<()> {
        let tokens = Rc::new(Cell::new([0_i64, 0_i64]));
        let mut gate = Gate::default();

        let app_bytes = app.clone();
        let url_bytes = url.clone();
        let bytes_handler = BytesReceivedChangedEventHandler::create(Box::new(move |op, _| {
            let Some(op) = op else {
                return Ok(());
            };
            publish(&app_bytes, &url_bytes, &op, &mut gate, false)
        }));
        let mut bytes_token = 0_i64;
        unsafe { operation.add_BytesReceivedChanged(&bytes_handler, &mut bytes_token)? };

        let app_state = app.clone();
        let url_state = url;
        let tokens_state = Rc::clone(&tokens);
        let state_handler = StateChangedEventHandler::create(Box::new(move |op, _| {
            let Some(op) = op else {
                return Ok(());
            };
            on_state(&app_state, &url_state, &op, &tokens_state)
        }));
        let mut state_token = 0_i64;
        unsafe { operation.add_StateChanged(&state_handler, &mut state_token)? };
        tokens.set([bytes_token, state_token]);
        Ok(())
    }

    fn on_state(
        app: &AppHandle,
        url: &str,
        operation: &ICoreWebView2DownloadOperation,
        tokens: &Cell<[i64; 2]>,
    ) -> windows::core::Result<()> {
        let mut state = COREWEBVIEW2_DOWNLOAD_STATE::default();
        unsafe { operation.State(&mut state)? };
        if state == COREWEBVIEW2_DOWNLOAD_STATE_IN_PROGRESS {
            return Ok(());
        }
        // One last sample (often 100 %), then drop both subscriptions.
        // WebView2 holds its own ref for the duration of this callback, so
        // removing the state handler here does not free it until we return.
        let published = publish(app, url, operation, &mut Gate::default(), true);
        // Tauri/wry also listens to StateChanged. When that Finished event is
        // lost (some CDNs), progress sits at 100 % forever — promote here as
        // a TrackedOnly backup that is a no-op if wry already finished.
        let success = state == COREWEBVIEW2_DOWNLOAD_STATE_COMPLETED;
        let path = if success {
            read_result_path(operation).ok()
        } else {
            None
        };
        crate::downloads::on_operation_terminal(app, url, path, success);
        release(operation, tokens);
        published
    }

    fn read_result_path(
        operation: &ICoreWebView2DownloadOperation,
    ) -> windows::core::Result<PathBuf> {
        let mut raw = PWSTR::null();
        if let Err(err) = unsafe { operation.ResultFilePath(&mut raw) } {
            if !raw.is_null() {
                let _ = take_pwstr(raw);
            }
            return Err(err);
        }
        Ok(PathBuf::from(take_pwstr(raw)))
    }

    fn publish(
        app: &AppHandle,
        url: &str,
        operation: &ICoreWebView2DownloadOperation,
        gate: &mut Gate,
        force: bool,
    ) -> windows::core::Result<()> {
        let (received, total) = read_counts(operation)?;
        if !gate.allow(received, total, Instant::now(), force) {
            return Ok(());
        }
        emit_progress(app, url, received, total);
        Ok(())
    }

    fn release(operation: &ICoreWebView2DownloadOperation, tokens: &Cell<[i64; 2]>) {
        let [bytes_token, state_token] = tokens.get();
        tokens.set([0, 0]);
        if bytes_token != 0 {
            let _ = unsafe { operation.remove_BytesReceivedChanged(bytes_token) };
        }
        if state_token != 0 {
            let _ = unsafe { operation.remove_StateChanged(state_token) };
        }
    }

    fn read_uri(operation: &ICoreWebView2DownloadOperation) -> windows::core::Result<String> {
        let mut raw = PWSTR::null();
        if let Err(err) = unsafe { operation.Uri(&mut raw) } {
            if !raw.is_null() {
                let _ = take_pwstr(raw);
            }
            return Err(err);
        }
        Ok(take_pwstr(raw))
    }

    fn read_counts(
        operation: &ICoreWebView2DownloadOperation,
    ) -> windows::core::Result<(u64, Option<u64>)> {
        let mut received = 0_i64;
        let mut total = 0_i64;
        unsafe {
            operation.BytesReceived(&mut received)?;
            operation.TotalBytesToReceive(&mut total)?;
        }
        Ok((received_bytes(received), known_total(total)))
    }

    fn received_bytes(raw: i64) -> u64 {
        u64::try_from(raw).unwrap_or(0)
    }

    /// `0` and `-1` both mean the server sent no Content-Length.
    fn known_total(raw: i64) -> Option<u64> {
        u64::try_from(raw).ok().filter(|n| *n > 0)
    }
}

#[cfg(test)]
mod tests {
    use super::{classify, percent_points, urls_match, Gate, ProgressTarget, MIN_GAP};
    use crate::downloads::{InFlight, PendingOrigin};
    use std::path::PathBuf;
    use std::time::{Duration, Instant};

    fn flight(id: i64, url: &str) -> InFlight {
        InFlight {
            download_id: id,
            loadout_id: 1,
            file_name: "mod.7z".into(),
            url: url.into(),
            path: PathBuf::from("mod.7z"),
        }
    }

    fn pending(url: &str) -> PendingOrigin {
        PendingOrigin {
            file_url: url.into(),
            source_host: "nexus".into(),
            page_url: None,
        }
    }

    #[test]
    fn percent_is_an_integer_clamped_at_100() {
        assert_eq!(percent_points(0, Some(100)), Some(0));
        assert_eq!(percent_points(42, Some(100)), Some(42));
        assert_eq!(percent_points(150, Some(100)), Some(100));
        assert_eq!(percent_points(10, None), None);
        assert_eq!(percent_points(10, Some(0)), None);
    }

    #[test]
    fn gate_emits_on_a_new_percent_or_four_times_a_second() {
        let mut gate = Gate::default();
        let t0 = Instant::now();
        assert!(gate.allow(0, Some(100), t0, false));
        assert!(!gate.allow(0, Some(100), t0 + Duration::from_millis(10), false));
        assert!(gate.allow(1, Some(100), t0 + Duration::from_millis(10), false));
        assert!(gate.allow(
            1,
            Some(100),
            t0 + MIN_GAP + Duration::from_millis(20),
            false
        ));

        let mut unknown = Gate::default();
        assert!(unknown.allow(100, None, t0, false));
        assert!(!unknown.allow(200, None, t0 + Duration::from_millis(249), false));
        assert!(unknown.allow(300, None, t0 + MIN_GAP, false));
        assert!(unknown.allow(300, None, t0 + Duration::from_millis(10), true));
    }

    #[test]
    fn classify_uses_the_newest_row_or_a_pending_origin_without_an_id() {
        let url = "https://Example.com/file.7z";
        let rows = vec![flight(1, url), flight(2, "https://example.com/file.7z")];
        assert_eq!(classify(&rows, &[], url), ProgressTarget::Row(2));
        assert_eq!(classify(&[], &[pending(url)], url), ProgressTarget::Unfiled);
        assert_eq!(classify(&[], &[], url), ProgressTarget::Unknown);
        assert_eq!(ProgressTarget::Row(2).download_id(), Some(2));
        assert_eq!(ProgressTarget::Unfiled.download_id(), None);
        assert!(urls_match(url, "https://example.com/file.7z"));
        assert!(urls_match(
            url,
            "https://example.com/file.7z?token=rotated"
        ));
        assert!(!urls_match(url, "https://example.com/other.7z"));
    }
}
