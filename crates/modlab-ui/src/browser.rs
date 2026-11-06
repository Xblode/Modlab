//! The embedded browser tab: a real second WebView2 control, layered over the
//! shell's own webview and positioned to cover the content area.
//!
//! Why a native child webview and not an `<iframe>`: Nexus (and most sites
//! worth browsing here) send `X-Frame-Options`/frame-ancestors headers that
//! forbid embedding. A child webview is a separate browser control with its
//! own cookie jar and its own download pipeline — which is exactly what makes
//! download capture possible at all.
//!
//! The React side owns *when* this is visible (it's a tab like any other) and
//! *where* it goes (it reports the content area's rectangle); this module only
//! creates, moves, hides and navigates it.

use std::sync::atomic::{AtomicBool, Ordering};
use tauri::webview::{DownloadEvent, WebviewBuilder};
use tauri::{AppHandle, LogicalPosition, LogicalSize, Manager, WebviewUrl};

/// The one embedded browser. A single persistent tab, per the design
/// decision — so its label is a constant, not a generated id.
pub const BROWSER_LABEL: &str = "modlab-browser";

/// Where the browser sits inside the shell window, in CSS pixels as the React
/// side measures them. Tauri's `Logical*` types use the same unit, so the
/// scale factor never has to be reasoned about on either side.
#[derive(serde::Deserialize, Clone, Copy, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// Hidden state is "moved off-screen and shrunk", not `close()`: closing would
/// throw away the page, the history and the Nexus session every time the user
/// looks at another tab.
const HIDDEN: Rect = Rect {
    x: -20000.0,
    y: -20000.0,
    width: 1.0,
    height: 1.0,
};

/// Every webview manipulation goes through the main thread.
///
/// This is not defensive style, it is the difference between working and
/// hanging: creating a WebView2 blocks the calling thread until the main
/// event loop has processed it, so calling it *from* a command handler
/// deadlocks — the call never returns, and the tab stays empty forever with
/// no error. Posting the work to the main loop instead makes every one of
/// these fire-and-forget, which is why `show` cannot report creation
/// failures by return value and emits `browser://error` instead.
fn on_main<F>(app: &AppHandle, work: F) -> Result<(), String>
where
    F: FnOnce(&AppHandle) + Send + 'static,
{
    let handle = app.clone();
    app.run_on_main_thread(move || work(&handle))
        .map_err(|e| e.to_string())
}

fn apply(app: &AppHandle, rect: Rect) -> Result<(), String> {
    on_main(app, move |app| {
        let Some(webview) = app.get_webview(BROWSER_LABEL) else {
            return;
        };
        let _ = webview.set_position(LogicalPosition::new(rect.x, rect.y));
        let _ = webview.set_size(LogicalSize::new(rect.width, rect.height));
    })
}

/// Build the browser once, parked off-screen, during `setup` — *before* the
/// event loop starts.
///
/// Creating it lazily on first use does not work: `Window::add_child`
/// blocks forever once the loop is running, from a command thread and from
/// the main thread alike (it waits on a loop that is either not pumping or
/// is itself inside the call). Creating it up front sidesteps that entirely
/// and leaves `show` with nothing to do but move an existing control, which
/// is a plain setter.
///
/// The cost is one idle WebView2 from startup, so it is parked on
/// `about:blank`: no page is fetched until the human opens the tab.
pub fn create_parked(app: &AppHandle) -> Result<(), String> {
    let window = app
        .get_window("main")
        .ok_or_else(|| "fenêtre principale introuvable".to_string())?;

    let blank: tauri::Url = "about:blank".parse().expect("about:blank parses");
    let builder = WebviewBuilder::new(BROWSER_LABEL, WebviewUrl::External(blank)).on_download(
        move |webview, event| {
            // Capture is the whole point of this webview, but it stays a
            // *capture* of what the human clicked: nothing here starts a
            // download, it only observes the ones the page starts.
            match event {
                DownloadEvent::Requested { url, destination } => {
                    crate::downloads::on_requested(&webview, url.as_str(), destination);
                }
                DownloadEvent::Finished { url, path, success } => {
                    crate::downloads::on_finished(&webview, url.as_str(), path, success);
                }
                _ => {}
            }
            // `true` = let it proceed. Refusing would just break the page's
            // own button for no benefit.
            true
        },
    );

    window
        .add_child(
            builder,
            LogicalPosition::new(HIDDEN.x, HIDDEN.y),
            LogicalSize::new(HIDDEN.width, HIDDEN.height),
        )
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Move the (already built) browser to `rect`, navigating it on the very
/// first open — it was parked on `about:blank`.
pub fn show(app: &AppHandle, url: &str, rect: Rect) -> Result<(), String> {
    let first_open = !OPENED.swap(true, Ordering::SeqCst);
    if first_open {
        navigate(app, url)?;
    }
    apply(app, rect)
}

/// Whether the browser has ever been brought on screen — decides if `show`
/// needs to leave `about:blank`.
static OPENED: AtomicBool = AtomicBool::new(false);

/// Park the browser off-screen — the user switched to another tab. Nothing
/// to hide is a success: the shell hides on every tab change, including ones
/// where the browser was never opened.
pub fn hide(app: &AppHandle) -> Result<(), String> {
    apply(app, HIDDEN)
}

/// Point the browser at another URL — the address bar, and the "go to this
/// game's Nexus page" default.
pub fn navigate(app: &AppHandle, url: &str) -> Result<(), String> {
    let parsed: tauri::Url = url.parse().map_err(|_| format!("URL invalide : {url}"))?;
    on_main(app, move |app| {
        if let Some(webview) = app.get_webview(BROWSER_LABEL) {
            let _ = webview.navigate(parsed);
        }
    })
}

/// Back/forward, driven through the page's own history rather than a runtime
/// API: `history.go` is what the buttons in any browser do, and it behaves
/// the same way on every page.
pub fn go(app: &AppHandle, delta: i32) -> Result<(), String> {
    on_main(app, move |app| {
        if let Some(webview) = app.get_webview(BROWSER_LABEL) {
            let _ = webview.eval(&format!("history.go({delta})"));
        }
    })
}
