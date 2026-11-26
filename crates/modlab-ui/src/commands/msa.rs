//! Tauri commands for Minecraft Microsoft account (auth-code + PKCE).
//!
//! Loopback receives the OAuth code, **exchanges tokens before** answering the
//! browser, and only then shows « Connexion réussie ». Showing success at code
//! receipt alone left a window where closing the Compte dialog (or a missed
//! poll) dropped the code with no session on disk.

use crate::msa_store;
use crate::AppState;
use modlab_game_minecraft::msa::{self, ReqwestMsaHttp, StoredMsaSession};
use serde::Serialize;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, State};

/// Event payload when a PKCE login finishes (success or failure).
pub const MSA_LOGIN_FINISHED_EVENT: &str = "msa://login-finished";

#[derive(Debug, Clone)]
pub(crate) enum LoopbackOutcome {
    Waiting,
    Connected { username: String },
    Failed(String),
}

#[derive(Clone)]
pub(crate) struct PendingMsaLogin {
    pub expires_at: Instant,
    pub cancel: Arc<AtomicBool>,
    pub outcome: Arc<Mutex<LoopbackOutcome>>,
    pub redirect_uri: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MsaAccountStatusDto {
    pub connected: bool,
    pub username: Option<String>,
    pub uuid: Option<String>,
    pub client_configured: bool,
    /// Effective MSA application (client) id (public; always set — built-in default).
    pub client_id: Option<String>,
    /// `ui` | `env` | `default` — which layer supplied `client_id`.
    pub client_id_source: Option<String>,
    /// Value saved from Settings (empty → env, then built-in public client).
    pub client_id_override: Option<String>,
    /// `online` | `offline` | `error`
    pub mode: String,
    pub error: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MsaLoginStartDto {
    /// Microsoft authorize URL — open in the system browser.
    pub auth_url: String,
    pub message: String,
    pub expires_in: u64,
    pub interval: u64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MsaPollDto {
    /// `pending` | `slow_down` | `connected` | `declined` | `expired` | `error`
    pub status: String,
    pub username: Option<String>,
    pub error: Option<String>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct MsaLoginFinishedPayload {
    status: String,
    username: Option<String>,
    error: Option<String>,
}

struct ExchangeCtx {
    client_id: String,
    code_verifier: String,
    redirect_uri: String,
    expected_state: String,
    data_root: PathBuf,
}

fn status_from_session(
    data_root: &std::path::Path,
    session: Option<&StoredMsaSession>,
    err: Option<String>,
) -> MsaAccountStatusDto {
    let (id, source) = msa_store::resolve_client_id_with_source(data_root);
    let client_configured = true;
    let client_id = Some(id);
    let client_id_source = Some(
        match source {
            msa_store::ClientIdSource::Ui => "ui",
            msa_store::ClientIdSource::Env => "env",
            msa_store::ClientIdSource::Default => "default",
        }
        .to_string(),
    );
    let client_id_override = msa_store::load_client_id_override(data_root);
    let base = |connected: bool,
                username: Option<String>,
                uuid: Option<String>,
                mode: &str,
                error: Option<String>| MsaAccountStatusDto {
        connected,
        username,
        uuid,
        client_configured,
        client_id: client_id.clone(),
        client_id_source: client_id_source.clone(),
        client_id_override: client_id_override.clone(),
        mode: mode.into(),
        error,
    };
    match (session, err) {
        (_, Some(e)) => base(false, None, None, "error", Some(e)),
        (Some(s), None) => base(
            true,
            Some(s.mc_username.clone()),
            Some(s.mc_uuid.clone()),
            "online",
            None,
        ),
        (None, None) => base(false, None, None, "offline", None),
    }
}

#[tauri::command]
pub(crate) fn msa_account_status(state: State<AppState>) -> Result<MsaAccountStatusDto, String> {
    match msa_store::load_session(&state.data_root) {
        Ok(session) => Ok(status_from_session(
            &state.data_root,
            session.as_ref(),
            None,
        )),
        Err(e) => Ok(status_from_session(&state.data_root, None, Some(e))),
    }
}

#[tauri::command]
pub(crate) fn msa_set_client_id(
    state: State<AppState>,
    client_id: String,
) -> Result<MsaAccountStatusDto, String> {
    msa_store::save_client_id_override(&state.data_root, &client_id)?;
    msa_account_status(state)
}

#[tauri::command]
pub(crate) async fn msa_begin_login(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<MsaLoginStartDto, String> {
    // Cancel any previous attempt so the previous loopback thread exits.
    cancel_pending(&state)?;

    let client_id =
        msa_store::resolve_client_id(&state.data_root).unwrap_or_else(msa::default_client_id);

    let pkce = msa::generate_pkce()?;
    let oauth_state = msa::generate_oauth_state()?;

    let listener = TcpListener::bind(("127.0.0.1", 0))
        .map_err(|e| format!("impossible d’ouvrir le port local pour Microsoft : {e}"))?;
    listener
        .set_nonblocking(true)
        .map_err(|e| format!("socket locale : {e}"))?;
    let port = listener
        .local_addr()
        .map_err(|e| format!("adresse locale : {e}"))?
        .port();
    // Trailing slash kept stable for Azure + token exchange. Use 127.0.0.1
    // (same as the bind) so the browser does not hit ::1 while we listen on v4.
    let redirect_uri = format!("http://127.0.0.1:{port}/");

    let auth_url =
        msa::build_authorize_url(&client_id, &redirect_uri, &oauth_state, &pkce.challenge);

    let cancel = Arc::new(AtomicBool::new(false));
    let outcome = Arc::new(Mutex::new(LoopbackOutcome::Waiting));
    let expires_in: u64 = 600;
    let expires_at = Instant::now() + Duration::from_secs(expires_in);

    let exchange = ExchangeCtx {
        client_id,
        code_verifier: pkce.verifier,
        redirect_uri: redirect_uri.clone(),
        expected_state: oauth_state,
        data_root: state.data_root.clone(),
    };

    {
        let cancel_t = Arc::clone(&cancel);
        let outcome_t = Arc::clone(&outcome);
        thread::spawn(move || {
            run_loopback_login(listener, cancel_t, outcome_t, expires_at, exchange, app);
        });
    }

    let pending = PendingMsaLogin {
        expires_at,
        cancel,
        outcome,
        redirect_uri,
    };
    {
        let mut slot = state.msa_pending.lock().map_err(|e| e.to_string())?;
        *slot = Some(pending);
    }

    Ok(MsaLoginStartDto {
        auth_url,
        message:
            "Une page Microsoft s’ouvre dans le navigateur. Connectez-vous, puis revenez ici — \
                  ne fermez pas cette fenêtre tant que ModLab n’a pas confirmé."
                .into(),
        expires_in,
        interval: 1,
    })
}

fn run_loopback_login(
    listener: TcpListener,
    cancel: Arc<AtomicBool>,
    outcome: Arc<Mutex<LoopbackOutcome>>,
    expires_at: Instant,
    exchange: ExchangeCtx,
    app: AppHandle,
) {
    let finish = |outcome: &Mutex<LoopbackOutcome>, app: &AppHandle, next: LoopbackOutcome| {
        let payload = match &next {
            LoopbackOutcome::Connected { username } => MsaLoginFinishedPayload {
                status: "connected".into(),
                username: Some(username.clone()),
                error: None,
            },
            LoopbackOutcome::Failed(msg) => MsaLoginFinishedPayload {
                status: if msg.contains("refusée") {
                    "declined".into()
                } else if msg.contains("Délai") || msg.contains("expiré") {
                    "expired".into()
                } else {
                    "error".into()
                },
                username: None,
                error: Some(msg.clone()),
            },
            LoopbackOutcome::Waiting => return,
        };
        let _ = set_outcome(outcome, next);
        let _ = app.emit(MSA_LOGIN_FINISHED_EVENT, payload);
    };

    loop {
        if cancel.load(Ordering::SeqCst) {
            finish(
                &outcome,
                &app,
                LoopbackOutcome::Failed("Connexion annulée.".into()),
            );
            return;
        }
        if Instant::now() >= expires_at {
            finish(
                &outcome,
                &app,
                LoopbackOutcome::Failed(
                    "Délai de connexion dépassé — recommencez depuis Compte.".into(),
                ),
            );
            return;
        }
        match listener.accept() {
            Ok((mut stream, _)) => {
                // Cancel wakes accept with an empty connect — exit without
                // treating it as a Microsoft redirect.
                if cancel.load(Ordering::SeqCst) {
                    finish(
                        &outcome,
                        &app,
                        LoopbackOutcome::Failed("Connexion annulée.".into()),
                    );
                    return;
                }

                let req = match read_http_request(&mut stream) {
                    Ok(r) => r,
                    Err(e) => {
                        let _ = write_http_response(&mut stream, &error_html(&e));
                        finish(&outcome, &app, LoopbackOutcome::Failed(e));
                        return;
                    }
                };

                // Ignore empty / non-GET probes; keep listening for the real redirect.
                if req.trim().is_empty() || !req.starts_with("GET ") {
                    continue;
                }

                let parsed = parse_authorize_redirect(&req);
                match parsed {
                    Err(e) => {
                        let _ = write_http_response(&mut stream, &error_html(&e));
                        finish(&outcome, &app, LoopbackOutcome::Failed(e));
                        return;
                    }
                    Ok((code, st)) => {
                        if st != exchange.expected_state {
                            let e = "État OAuth invalide (possible interférence) — réessayez Connecter."
                                .to_string();
                            let _ = write_http_response(&mut stream, &error_html(&e));
                            finish(&outcome, &app, LoopbackOutcome::Failed(e));
                            return;
                        }

                        // Exchange + DPAPI save *before* telling the browser it worked.
                        // Closing ModLab's dialog after this point can no longer drop the code.
                        let result = complete_login_with_code(&exchange, &code);
                        match result {
                            Ok(session) => {
                                let _ = write_http_response(&mut stream, SUCCESS_HTML);
                                finish(
                                    &outcome,
                                    &app,
                                    LoopbackOutcome::Connected {
                                        username: session.mc_username,
                                    },
                                );
                            }
                            Err(e) => {
                                let _ = write_http_response(&mut stream, &error_html(&e));
                                finish(&outcome, &app, LoopbackOutcome::Failed(e));
                            }
                        }
                        return;
                    }
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(50));
            }
            Err(e) => {
                finish(
                    &outcome,
                    &app,
                    LoopbackOutcome::Failed(format!("écoute redirect locale : {e}")),
                );
                return;
            }
        }
    }
}

fn complete_login_with_code(
    exchange: &ExchangeCtx,
    code: &str,
) -> Result<StoredMsaSession, String> {
    let http = ReqwestMsaHttp::default();
    let pair = msa::exchange_auth_code(
        &http,
        &exchange.client_id,
        code,
        &exchange.redirect_uri,
        &exchange.code_verifier,
    )?;
    let session = msa::exchange_for_minecraft(&http, &pair.access_token, &pair.refresh_token)?;
    msa_store::save_session(&exchange.data_root, &session)?;
    Ok(session)
}

fn set_outcome(slot: &Mutex<LoopbackOutcome>, next: LoopbackOutcome) -> Result<(), String> {
    let mut g = slot.lock().map_err(|e| e.to_string())?;
    // Don't overwrite a terminal result.
    if matches!(*g, LoopbackOutcome::Waiting) {
        *g = next;
    }
    Ok(())
}

/// Read one HTTP request (headers) from the accepted stream.
fn read_http_request(stream: &mut TcpStream) -> Result<String, String> {
    let _ = stream.set_nonblocking(false);
    let _ = stream.set_read_timeout(Some(Duration::from_secs(10)));
    let mut buf: Vec<u8> = Vec::with_capacity(4096);
    let mut tmp = [0u8; 2048];
    loop {
        match stream.read(&mut tmp) {
            Ok(0) => break,
            Ok(n) => {
                buf.extend_from_slice(&tmp[..n]);
                if buf.windows(4).any(|w| w == b"\r\n\r\n") || buf.len() > 65_536 {
                    break;
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::TimedOut => break,
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
            Err(e) => return Err(format!("lecture redirect locale : {e}")),
        }
    }
    if buf.is_empty() {
        return Err("requête redirect Microsoft vide".into());
    }
    Ok(String::from_utf8_lossy(&buf).into_owned())
}

/// Parse the first line of an HTTP request for `code` / `state` / `error`.
fn parse_authorize_redirect(req: &str) -> Result<(String, String), String> {
    let first = req.lines().next().unwrap_or("");
    // e.g. GET /?code=...&state=... HTTP/1.1
    let path = first
        .strip_prefix("GET ")
        .and_then(|s| s.split_whitespace().next())
        .ok_or_else(|| "requête redirect Microsoft invalide".to_string())?;
    let query = path.split_once('?').map(|(_, q)| q).unwrap_or("");
    let mut code: Option<String> = None;
    let mut state: Option<String> = None;
    let mut error: Option<String> = None;
    let mut error_desc: Option<String> = None;
    for pair in query.split('&') {
        let mut it = pair.splitn(2, '=');
        let k = it.next().unwrap_or("");
        let v = it.next().unwrap_or("");
        let decoded = url_decode(v);
        match k {
            "code" => code = Some(decoded),
            "state" => state = Some(decoded),
            "error" => error = Some(decoded),
            "error_description" => error_desc = Some(decoded),
            _ => {}
        }
    }
    if let Some(err) = error {
        let desc = error_desc.unwrap_or_default();
        if err == "access_denied" {
            return Err("Connexion refusée sur la page Microsoft.".into());
        }
        if desc.is_empty() {
            return Err(format!("Microsoft a refusé la connexion ({err})."));
        }
        return Err(format!("Microsoft : {desc}"));
    }
    let code = code.filter(|s| !s.is_empty()).ok_or_else(|| {
        format!(
            "Microsoft n’a pas renvoyé de code — vérifiez l’URI {} dans Azure \
             (plateforme Mobile et applications de bureau).",
            msa::AZURE_REDIRECT_URI
        )
    })?;
    let state = state.unwrap_or_default();
    Ok((code, state))
}

fn url_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            b'%' if i + 2 < bytes.len() => {
                let h = u8::from_str_radix(
                    std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or("00"),
                    16,
                )
                .unwrap_or(b'?');
                out.push(h);
                i += 3;
            }
            b => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn write_http_response(stream: &mut impl Write, body: &str) -> Result<(), String> {
    let resp = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    );
    stream
        .write_all(resp.as_bytes())
        .map_err(|e| format!("réponse locale : {e}"))
}

fn error_html(message: &str) -> String {
    let escaped = html_escape(message);
    format!(
        r#"<!doctype html><html lang="fr"><head><meta charset="utf-8"><title>ModLab</title></head><body style="font-family:system-ui;padding:2rem"><h1>Connexion interrompue</h1><p>{escaped}</p><p>Fermez cet onglet et réessayez depuis ModLab → Compte.</p></body></html>"#
    )
}

fn html_escape(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '&' => "&amp;".into(),
            '<' => "&lt;".into(),
            '>' => "&gt;".into(),
            '"' => "&quot;".into(),
            _ => c.to_string(),
        })
        .collect()
}

const SUCCESS_HTML: &str = r#"<!doctype html><html lang="fr"><head><meta charset="utf-8"><title>ModLab</title></head><body style="font-family:system-ui;padding:2rem"><h1>Connexion réussie</h1><p>Vous pouvez fermer cet onglet et revenir à ModLab.</p></body></html>"#;

#[tauri::command]
pub(crate) async fn msa_poll_login(state: State<'_, AppState>) -> Result<MsaPollDto, String> {
    let pending = {
        let slot = state.msa_pending.lock().map_err(|e| e.to_string())?;
        slot.clone()
    };
    let Some(pending) = pending else {
        // Login thread may have finished and cleared itself; check disk.
        if let Ok(Some(session)) = msa_store::load_session(&state.data_root) {
            return Ok(MsaPollDto {
                status: "connected".into(),
                username: Some(session.mc_username),
                error: None,
            });
        }
        return Ok(MsaPollDto {
            status: "error".into(),
            username: None,
            error: Some("Aucune connexion Microsoft en cours — cliquez Connecter.".into()),
        });
    };
    if Instant::now() >= pending.expires_at {
        clear_pending(&state)?;
        return Ok(MsaPollDto {
            status: "expired".into(),
            username: None,
            error: Some("Délai de connexion dépassé — recommencez depuis Compte.".into()),
        });
    }

    let snapshot = {
        let g = pending.outcome.lock().map_err(|e| e.to_string())?;
        g.clone()
    };

    match snapshot {
        LoopbackOutcome::Waiting => Ok(MsaPollDto {
            status: "pending".into(),
            username: None,
            error: None,
        }),
        LoopbackOutcome::Connected { username } => {
            clear_pending(&state)?;
            Ok(MsaPollDto {
                status: "connected".into(),
                username: Some(username),
                error: None,
            })
        }
        LoopbackOutcome::Failed(msg) => {
            clear_pending(&state)?;
            let status = if msg.contains("refusée") {
                "declined"
            } else if msg.contains("Délai") || msg.contains("expiré") {
                "expired"
            } else {
                "error"
            };
            Ok(MsaPollDto {
                status: status.into(),
                username: None,
                error: Some(msg),
            })
        }
    }
}

fn loopback_port(redirect_uri: &str) -> Option<u16> {
    let rest = redirect_uri
        .strip_prefix("http://127.0.0.1:")
        .or_else(|| redirect_uri.strip_prefix("http://localhost:"))?;
    rest.trim_end_matches('/').split('/').next()?.parse().ok()
}

fn cancel_pending(state: &State<'_, AppState>) -> Result<(), String> {
    let mut slot = state.msa_pending.lock().map_err(|e| e.to_string())?;
    if let Some(prev) = slot.take() {
        prev.cancel.store(true, Ordering::SeqCst);
        // Unblock accept by connecting to ourselves if still listening.
        if let Some(port) = loopback_port(&prev.redirect_uri) {
            let _ = std::net::TcpStream::connect_timeout(
                &std::net::SocketAddr::from(([127, 0, 0, 1], port)),
                Duration::from_millis(100),
            );
        }
    }
    Ok(())
}

fn clear_pending(state: &State<'_, AppState>) -> Result<(), String> {
    cancel_pending(state)
}

#[tauri::command]
pub(crate) fn msa_cancel_login(state: State<AppState>) -> Result<(), String> {
    cancel_pending(&state)
}

#[tauri::command]
pub(crate) fn msa_disconnect(state: State<AppState>) -> Result<MsaAccountStatusDto, String> {
    cancel_pending(&state)?;
    msa_store::clear_session(&state.data_root)?;
    msa_account_status(state)
}

/// Load a usable online session for launch, refreshing when needed.
/// Returns `None` on any failure so Jouer can fall back to offline.
pub(crate) fn session_for_launch(data_root: &std::path::Path) -> Option<StoredMsaSession> {
    let client_id = msa_store::resolve_client_id(data_root).unwrap_or_else(msa::default_client_id);
    let session = match msa_store::load_session(data_root) {
        Ok(Some(s)) => s,
        _ => return None,
    };
    if msa::session_is_fresh(&session, 120) {
        return Some(session);
    }
    let http = ReqwestMsaHttp::default();
    match msa::refresh_minecraft_session(&http, &client_id, &session) {
        Ok(next) => {
            let _ = msa_store::save_session(data_root, &next);
            Some(next)
        }
        Err(_) => {
            // Keep stored refresh token; launch falls back to offline.
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_redirect_extracts_code() {
        let req = "GET /?code=abc%2Fde&state=xyz HTTP/1.1\r\nHost: localhost\r\n\r\n";
        let (code, state) = parse_authorize_redirect(req).unwrap();
        assert_eq!(code, "abc/de");
        assert_eq!(state, "xyz");
    }

    #[test]
    fn parse_redirect_access_denied() {
        let req = "GET /?error=access_denied&error_description=nope HTTP/1.1\r\n\r\n";
        let err = parse_authorize_redirect(req).unwrap_err();
        assert!(err.contains("refusée"), "{err}");
    }

    #[test]
    fn error_html_escapes_message() {
        let html = error_html("a <b> & \"c\"");
        assert!(html.contains("a &lt;b&gt; &amp; &quot;c&quot;"), "{html}");
        assert!(html.contains("Connexion interrompue"));
    }

    #[test]
    fn loopback_port_parses_ephemeral() {
        assert_eq!(loopback_port("http://127.0.0.1:19240/"), Some(19240));
        assert_eq!(loopback_port("http://localhost:9"), Some(9));
        assert!(loopback_port("https://example.com").is_none());
    }
}
