//! Microsoft account (MSA) → Xbox Live → Minecraft Services authentication.
//!
//! Authorization-code + PKCE for a public desktop client (loopback
//! `http://127.0.0.1:<port>/`). Device-code is intentionally unused: personal
//! Microsoft accounts often reject it with AADSTS70002 unless the app is marked
//! mobile. Tokens themselves are **not** persisted here — the UI shell owns
//! encrypted storage. This module only speaks the HTTP protocol and returns
//! session material for launch.
//!
//! Client id: default is the **public** MSA client id published by Prism
//! Launcher (allowlisted for Minecraft Java game services — see
//! https://aka.ms/AppRegInfo). A personal Azure App Registration with only
//! Graph `User.Read` can complete Microsoft login but fails Xbox / Minecraft
//! with HTTP 403 « Invalid app registration ». Override via Réglages → Compte
//! or env `MODLAB_MSA_CLIENT_ID` only if you have a Partner Center / Mojang-
//! allowlisted app. Scopes match Prism: `XboxLive.SignIn XboxLive.offline_access`.

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const AUTHORIZE_URL: &str = "https://login.microsoftonline.com/consumers/oauth2/v2.0/authorize";
const TOKEN_URL: &str = "https://login.microsoftonline.com/consumers/oauth2/v2.0/token";
const XBL_AUTH_URL: &str = "https://user.auth.xboxlive.com/user/authenticate";
const XSTS_URL: &str = "https://xsts.auth.xboxlive.com/xsts/authorize";
const MC_LOGIN_URL: &str = "https://api.minecraftservices.com/authentication/login_with_xbox";
const MC_PROFILE_URL: &str = "https://api.minecraftservices.com/minecraft/profile";
/// Same scopes as Prism Launcher (`MSAStep.cpp`).
const MSA_SCOPE: &str = "XboxLive.SignIn XboxLive.offline_access";

/// Env var holding an optional Azure AD public client id override (not a secret).
pub const CLIENT_ID_ENV: &str = "MODLAB_MSA_CLIENT_ID";

/// Public MSA client id used by Prism Launcher (CMakeLists `Launcher_MSA_CLIENT_ID`).
/// Not a secret — published in FOSS launcher sources. Existing launchers stay on
/// Mojang’s Minecraft Java game-service allow list (aka.ms/AppRegInfo).
pub const DEFAULT_MSA_CLIENT_ID: &str = "c36a9fb6-4f2a-41ff-90bd-ae7cc92031eb";

/// Redirect URI family for public desktop clients (loopback).
/// Runtime uses `http://127.0.0.1:<ephemeral-port>/` — any port matches for
/// public clients that allow the `http://127.0.0.1` family. Prefer `127.0.0.1`
/// over `localhost` so the browser hits the IPv4 loopback we bind (avoids `::1`).
pub const AZURE_REDIRECT_URI: &str = "http://127.0.0.1";

/// Credentials passed into `prepare_launch` when an MSA session is valid.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OnlineAuth {
    pub player_name: String,
    pub uuid: String,
    pub access_token: String,
}

/// Full session material the UI may encrypt and reload.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct StoredMsaSession {
    pub ms_refresh_token: String,
    pub mc_access_token: String,
    pub mc_username: String,
    pub mc_uuid: String,
    /// Unix seconds when `mc_access_token` should be treated as expired.
    pub mc_expires_at: u64,
}

/// PKCE verifier + S256 challenge for the authorize request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PkcePair {
    pub verifier: String,
    pub challenge: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MsTokenPair {
    pub access_token: String,
    pub refresh_token: String,
}

/// Minimal HTTP surface so unit tests can mock Microsoft / Xbox / Mojang.
pub trait MsaHttp {
    fn post_form(&self, url: &str, form: &[(&str, &str)]) -> Result<String, String>;
    /// Like `post_form`, but HTTP 400 with a JSON body is returned as `Ok`.
    fn post_form_soft(&self, url: &str, form: &[(&str, &str)]) -> Result<String, String> {
        self.post_form(url, form)
    }
    fn post_json(&self, url: &str, body: &Value) -> Result<String, String>;
    fn get_bearer(&self, url: &str, token: &str) -> Result<String, String>;
}

/// Blocking reqwest client used in production.
pub struct ReqwestMsaHttp {
    client: reqwest::blocking::Client,
}

impl Default for ReqwestMsaHttp {
    fn default() -> Self {
        Self {
            client: reqwest::blocking::Client::builder()
                .timeout(Duration::from_secs(30))
                .user_agent("ModLab/0.0.1 (Minecraft MSA)")
                .build()
                .expect("reqwest client"),
        }
    }
}

impl ReqwestMsaHttp {
    fn send_form(&self, url: &str, form: &[(&str, &str)]) -> Result<(u16, String), String> {
        let res = self
            .client
            .post(url)
            .header("Content-Type", "application/x-www-form-urlencoded")
            .form(form)
            .send()
            .map_err(|e| format!("réseau Microsoft : {e}"))?;
        let status = res.status().as_u16();
        let text = res
            .text()
            .map_err(|e| format!("réponse Microsoft illisible : {e}"))?;
        Ok((status, text))
    }
}

impl MsaHttp for ReqwestMsaHttp {
    fn post_form(&self, url: &str, form: &[(&str, &str)]) -> Result<String, String> {
        let (status, text) = self.send_form(url, form)?;
        if !(200..300).contains(&status) {
            return Err(friendly_oauth_error(&text, status));
        }
        Ok(text)
    }

    fn post_form_soft(&self, url: &str, form: &[(&str, &str)]) -> Result<String, String> {
        let (status, text) = self.send_form(url, form)?;
        if (200..300).contains(&status) || status == 400 {
            return Ok(text);
        }
        Err(friendly_oauth_error(&text, status))
    }

    fn post_json(&self, url: &str, body: &Value) -> Result<String, String> {
        let res = self
            .client
            .post(url)
            .header("Content-Type", "application/json")
            .header("Accept", "application/json")
            .json(body)
            .send()
            .map_err(|e| format!("réseau Xbox / Minecraft : {e}"))?;
        let status = res.status();
        let text = res
            .text()
            .map_err(|e| format!("réponse Xbox / Minecraft illisible : {e}"))?;
        if !status.is_success() {
            return Err(friendly_service_error(url, &text, status.as_u16()));
        }
        Ok(text)
    }

    fn get_bearer(&self, url: &str, token: &str) -> Result<String, String> {
        let res = self
            .client
            .get(url)
            .bearer_auth(token)
            .send()
            .map_err(|e| format!("réseau profil Minecraft : {e}"))?;
        let status = res.status();
        let text = res
            .text()
            .map_err(|e| format!("profil Minecraft illisible : {e}"))?;
        if !status.is_success() {
            return Err(friendly_service_error(url, &text, status.as_u16()));
        }
        Ok(text)
    }
}

/// Resolve an optional Azure public client id from env (trimmed). Empty → None.
pub fn resolve_client_id_from_env() -> Option<String> {
    std::env::var(CLIENT_ID_ENV)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Built-in public client id (Prism / FOSS launcher practice). Never empty.
pub fn default_client_id() -> String {
    DEFAULT_MSA_CLIENT_ID.to_string()
}

/// French hint when a caller passed an empty client id (should not happen with default).
pub fn missing_client_id_message() -> String {
    format!(
        "Client Microsoft manquant — ModLab utilise normalement le client public Minecraft \
         ({DEFAULT_MSA_CLIENT_ID}). Effacez un éventuel ID Azure perso dans Réglages → Compte, \
         ou définissez {CLIENT_ID_ENV} seulement si vous avez une appli allowlistée Mojang \
         (https://aka.ms/AppRegInfo)."
    )
}

/// Random PKCE verifier (43 chars URL-safe) + S256 challenge.
pub fn generate_pkce() -> Result<PkcePair, String> {
    let mut bytes = [0u8; 32];
    getrandom::getrandom(&mut bytes).map_err(|e| format!("entropie PKCE : {e}"))?;
    let verifier = URL_SAFE_NO_PAD.encode(bytes);
    let hash = Sha256::digest(verifier.as_bytes());
    let challenge = URL_SAFE_NO_PAD.encode(hash);
    Ok(PkcePair {
        verifier,
        challenge,
    })
}

/// Opaque CSRF `state` for the authorize → loopback round-trip.
pub fn generate_oauth_state() -> Result<String, String> {
    let mut bytes = [0u8; 16];
    getrandom::getrandom(&mut bytes).map_err(|e| format!("entropie state : {e}"))?;
    Ok(URL_SAFE_NO_PAD.encode(bytes))
}

/// Build the Microsoft authorize URL (open in the system browser).
pub fn build_authorize_url(
    client_id: &str,
    redirect_uri: &str,
    state: &str,
    code_challenge: &str,
) -> String {
    format!(
        "{AUTHORIZE_URL}?client_id={}&response_type=code&redirect_uri={}&response_mode=query&scope={}&state={}&code_challenge={}&code_challenge_method=S256",
        url_encode(client_id.trim()),
        url_encode(redirect_uri),
        url_encode(MSA_SCOPE),
        url_encode(state),
        url_encode(code_challenge),
    )
}

/// Exchange an authorization code (+ PKCE verifier) for MS tokens.
pub fn exchange_auth_code<H: MsaHttp>(
    http: &H,
    client_id: &str,
    code: &str,
    redirect_uri: &str,
    code_verifier: &str,
) -> Result<MsTokenPair, String> {
    let client_id = client_id.trim();
    if client_id.is_empty() {
        return Err(missing_client_id_message());
    }
    let body = http.post_form(
        TOKEN_URL,
        &[
            ("grant_type", "authorization_code"),
            ("client_id", client_id),
            ("code", code),
            ("redirect_uri", redirect_uri),
            ("code_verifier", code_verifier),
        ],
    )?;
    parse_token_pair(&body)
}

pub fn refresh_ms_tokens<H: MsaHttp>(
    http: &H,
    client_id: &str,
    refresh_token: &str,
) -> Result<MsTokenPair, String> {
    let body = http.post_form(
        TOKEN_URL,
        &[
            ("grant_type", "refresh_token"),
            ("client_id", client_id.trim()),
            ("refresh_token", refresh_token),
        ],
    )?;
    let v: Value =
        serde_json::from_str(&body).map_err(|e| format!("rafraîchissement invalide : {e}"))?;
    let access_token = str_field(&v, "access_token")?;
    // Microsoft may omit a new refresh_token; keep the old one.
    let refresh_token = v
        .get("refresh_token")
        .and_then(Value::as_str)
        .unwrap_or(refresh_token)
        .to_string();
    Ok(MsTokenPair {
        access_token,
        refresh_token,
    })
}

/// MS access token → Minecraft access token + profile.
pub fn exchange_for_minecraft<H: MsaHttp>(
    http: &H,
    ms_access_token: &str,
    ms_refresh_token: &str,
) -> Result<StoredMsaSession, String> {
    let xbl = xbox_authenticate(http, ms_access_token)?;
    let (xsts, uhs) = xsts_authorize(http, &xbl)?;
    let mc_token = minecraft_login(http, &uhs, &xsts)?;
    let profile = minecraft_profile(http, &mc_token.access_token)?;
    let expires_at = now_unix().saturating_add(mc_token.expires_in.saturating_sub(60));
    Ok(StoredMsaSession {
        ms_refresh_token: ms_refresh_token.to_string(),
        mc_access_token: mc_token.access_token,
        mc_username: profile.name,
        mc_uuid: normalize_uuid(&profile.id),
        mc_expires_at: expires_at,
    })
}

/// Refresh MSA + Minecraft tokens when the MC token is near expiry.
pub fn refresh_minecraft_session<H: MsaHttp>(
    http: &H,
    client_id: &str,
    session: &StoredMsaSession,
) -> Result<StoredMsaSession, String> {
    let pair = refresh_ms_tokens(http, client_id, &session.ms_refresh_token)?;
    exchange_for_minecraft(http, &pair.access_token, &pair.refresh_token)
}

pub fn session_is_fresh(session: &StoredMsaSession, skew_secs: u64) -> bool {
    let now = now_unix();
    session.mc_expires_at > now.saturating_add(skew_secs)
        && !session.mc_access_token.is_empty()
        && !session.mc_uuid.is_empty()
        && !session.mc_username.is_empty()
}

pub fn online_auth_from_session(session: &StoredMsaSession) -> OnlineAuth {
    OnlineAuth {
        player_name: session.mc_username.clone(),
        uuid: session.mc_uuid.clone(),
        access_token: session.mc_access_token.clone(),
    }
}

fn parse_token_pair(body: &str) -> Result<MsTokenPair, String> {
    let v: Value =
        serde_json::from_str(body).map_err(|e| format!("réponse jeton invalide : {e}"))?;
    if let Some(err) = v.get("error").and_then(Value::as_str) {
        let desc = v
            .get("error_description")
            .and_then(Value::as_str)
            .unwrap_or(err);
        return Err(friendly_oauth_error(
            &json!({ "error": err, "error_description": desc }).to_string(),
            400,
        ));
    }
    Ok(MsTokenPair {
        access_token: str_field(&v, "access_token")?,
        refresh_token: str_field(&v, "refresh_token")?,
    })
}

fn xbox_authenticate<H: MsaHttp>(http: &H, ms_access_token: &str) -> Result<String, String> {
    let body = json!({
        "Properties": {
            "AuthMethod": "RPS",
            "SiteName": "user.auth.xboxlive.com",
            "RpsTicket": format!("d={ms_access_token}")
        },
        "RelyingParty": "http://auth.xboxlive.com",
        "TokenType": "JWT"
    });
    let text = http.post_json(XBL_AUTH_URL, &body)?;
    let v: Value =
        serde_json::from_str(&text).map_err(|e| format!("réponse Xbox Live invalide : {e}"))?;
    str_field(&v, "Token")
}

fn xsts_authorize<H: MsaHttp>(http: &H, xbl_token: &str) -> Result<(String, String), String> {
    let body = json!({
        "Properties": {
            "SandboxId": "RETAIL",
            "UserTokens": [xbl_token]
        },
        "RelyingParty": "rp://api.minecraftservices.com/",
        "TokenType": "JWT"
    });
    let text = match http.post_json(XSTS_URL, &body) {
        Ok(t) => t,
        Err(e) => {
            if e.contains("2148916233") {
                return Err(
                    "Ce compte Microsoft n’a pas de profil Xbox — créez-en un, puis réessayez."
                        .into(),
                );
            }
            if e.contains("2148916238") {
                return Err(
                    "Ce compte est un compte enfant ou nécessite une validation familiale.".into(),
                );
            }
            return Err(e);
        }
    };
    let v: Value =
        serde_json::from_str(&text).map_err(|e| format!("réponse XSTS invalide : {e}"))?;
    let token = str_field(&v, "Token")?;
    let uhs = v
        .pointer("/DisplayClaims/xui/0/uhs")
        .and_then(Value::as_str)
        .ok_or_else(|| "uhs Xbox manquant".to_string())?
        .to_string();
    Ok((token, uhs))
}

struct McLoginToken {
    access_token: String,
    expires_in: u64,
}

fn minecraft_login<H: MsaHttp>(http: &H, uhs: &str, xsts: &str) -> Result<McLoginToken, String> {
    let body = json!({
        "identityToken": format!("XBL3.0 x={uhs};{xsts}")
    });
    let text = http.post_json(MC_LOGIN_URL, &body)?;
    let v: Value =
        serde_json::from_str(&text).map_err(|e| format!("login Minecraft invalide : {e}"))?;
    let access_token = str_field(&v, "access_token")?;
    let expires_in = v
        .get("expires_in")
        .and_then(Value::as_u64)
        .unwrap_or(86_400);
    Ok(McLoginToken {
        access_token,
        expires_in,
    })
}

#[derive(Debug)]
struct McProfile {
    id: String,
    name: String,
}

fn minecraft_profile<H: MsaHttp>(http: &H, mc_token: &str) -> Result<McProfile, String> {
    let text = http.get_bearer(MC_PROFILE_URL, mc_token)?;
    let v: Value =
        serde_json::from_str(&text).map_err(|e| format!("profil Minecraft invalide : {e}"))?;
    if v.get("error").is_some() || v.get("errorMessage").is_some() {
        return Err(
            "Ce compte Microsoft n’a pas Minecraft Java (achat / Game Pass requis).".into(),
        );
    }
    Ok(McProfile {
        id: str_field(&v, "id")?,
        name: str_field(&v, "name")?,
    })
}

fn str_field(v: &Value, key: &str) -> Result<String, String> {
    v.get(key)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .ok_or_else(|| format!("champ « {key} » manquant"))
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Mojang profile ids are undashed; launch args expect dashed UUID.
pub fn normalize_uuid(id: &str) -> String {
    let hex: String = id.chars().filter(|c| *c != '-').collect();
    if hex.len() != 32 {
        return id.to_string();
    }
    format!(
        "{}-{}-{}-{}-{}",
        &hex[0..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..32]
    )
}

fn url_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

fn friendly_oauth_error(body: &str, status: u16) -> String {
    if let Ok(v) = serde_json::from_str::<Value>(body) {
        let desc = v
            .get("error_description")
            .and_then(Value::as_str)
            .unwrap_or("");
        let err = v.get("error").and_then(Value::as_str).unwrap_or("");
        if desc.contains("AADSTS70002")
            || desc.contains("must be marked as 'mobile'")
            || err == "unauthorized_client"
        {
            return format!(
                "Microsoft ({status}) : cette appli Azure n’accepte pas ce mode de connexion. \
                 Dans le portail Azure → Authentification → ajoutez la plateforme \
                 « Mobile et applications de bureau » avec l’URI de redirection \
                 « {AZURE_REDIRECT_URI} » (client public), puis réessayez Connecter."
            );
        }
        if !desc.is_empty() {
            return format!("Microsoft ({status}) : {desc}");
        }
        if !err.is_empty() {
            return format!("Microsoft ({status}) : {err}");
        }
    }
    format!("Microsoft a refusé la demande (HTTP {status}).")
}

fn friendly_service_error(url: &str, body: &str, status: u16) -> String {
    if body.contains("2148916233") {
        return "Ce compte Microsoft n’a pas de profil Xbox — créez-en un, puis réessayez.".into();
    }
    if body.contains("2148916238") {
        return "Ce compte est un compte enfant ou nécessite une validation familiale.".into();
    }
    // aka.ms/AppRegInfo — personal Azure apps are not on Mojang’s game-service allow list.
    if status == 403
        && (body.contains("AppRegInfo")
            || body.contains("Invalid app registration")
            || body.contains("aka.ms/AppRegInfo"))
    {
        return format!(
            "Cette appli Azure n’est pas autorisée pour Xbox / Minecraft (HTTP {status}). \
             Une App Registration perso (portail Azure, Graph User.Read) ne suffit pas — \
             Mojang n’autorise que les clients allowlistés (https://aka.ms/AppRegInfo). \
             Dans Réglages → Compte, effacez l’ID Azure perso pour utiliser le client public \
             Minecraft par défaut ({DEFAULT_MSA_CLIENT_ID}), puis réessayez Connecter."
        );
    }
    if url.contains("minecraftservices") && status == 404 {
        return "Ce compte Microsoft n’a pas Minecraft Java (achat / Game Pass requis).".into();
    }
    if let Ok(v) = serde_json::from_str::<Value>(body) {
        if let Some(msg) = v
            .get("errorMessage")
            .or_else(|| v.get("message"))
            .and_then(Value::as_str)
        {
            if msg.contains("AppRegInfo") || msg.contains("Invalid app registration") {
                return friendly_service_error(url, msg, status);
            }
            return format!("Service Microsoft ({status}) : {msg}");
        }
    }
    format!("Service Microsoft a échoué (HTTP {status}).")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::collections::VecDeque;

    struct MockHttp {
        form: RefCell<VecDeque<Result<String, String>>>,
        json: RefCell<VecDeque<Result<String, String>>>,
        get: RefCell<VecDeque<Result<String, String>>>,
    }

    impl MockHttp {
        fn new(
            form: Vec<Result<String, String>>,
            json: Vec<Result<String, String>>,
            get: Vec<Result<String, String>>,
        ) -> Self {
            Self {
                form: RefCell::new(form.into()),
                json: RefCell::new(json.into()),
                get: RefCell::new(get.into()),
            }
        }
    }

    impl MsaHttp for MockHttp {
        fn post_form(&self, _url: &str, _form: &[(&str, &str)]) -> Result<String, String> {
            self.form
                .borrow_mut()
                .pop_front()
                .unwrap_or_else(|| Err("form queue empty".into()))
        }
        fn post_json(&self, _url: &str, _body: &Value) -> Result<String, String> {
            self.json
                .borrow_mut()
                .pop_front()
                .unwrap_or_else(|| Err("json queue empty".into()))
        }
        fn get_bearer(&self, _url: &str, _token: &str) -> Result<String, String> {
            self.get
                .borrow_mut()
                .pop_front()
                .unwrap_or_else(|| Err("get queue empty".into()))
        }
    }

    #[test]
    fn normalize_uuid_inserts_dashes() {
        assert_eq!(
            normalize_uuid("abcdef0123456789abcdef0123456789"),
            "abcdef01-2345-6789-abcd-ef0123456789"
        );
        assert_eq!(
            normalize_uuid("abcdef01-2345-6789-abcd-ef0123456789"),
            "abcdef01-2345-6789-abcd-ef0123456789"
        );
    }

    #[test]
    fn missing_client_id_is_french() {
        let err = exchange_auth_code(
            &MockHttp::new(vec![], vec![], vec![]),
            "",
            "code",
            "http://localhost:1/",
            "verifier",
        )
        .unwrap_err();
        assert!(err.contains(CLIENT_ID_ENV), "{err}");
        assert!(err.contains(DEFAULT_MSA_CLIENT_ID), "{err}");
        assert!(
            err.contains("AppRegInfo") || err.contains("allowlist"),
            "{err}"
        );
    }

    #[test]
    fn appreginfo_403_is_actionable_french() {
        let msg = friendly_service_error(
            "https://user.auth.xboxlive.com/user/authenticate",
            r#"{"Message":"Invalid app registration, see https://aka.ms/AppRegInfo"}"#,
            403,
        );
        assert!(msg.contains("pas autorisée"), "{msg}");
        assert!(msg.contains("effacez"), "{msg}");
        assert!(msg.contains(DEFAULT_MSA_CLIENT_ID), "{msg}");
        assert!(msg.contains("AppRegInfo"), "{msg}");
    }

    #[test]
    fn default_client_id_is_prism_public() {
        assert_eq!(default_client_id(), DEFAULT_MSA_CLIENT_ID);
        assert!(!DEFAULT_MSA_CLIENT_ID.is_empty());
    }

    #[test]
    fn pkce_challenge_is_s256() {
        let pair = generate_pkce().unwrap();
        assert!(pair.verifier.len() >= 43);
        let expected = URL_SAFE_NO_PAD.encode(Sha256::digest(pair.verifier.as_bytes()));
        assert_eq!(pair.challenge, expected);
    }

    #[test]
    fn authorize_url_includes_pkce() {
        let url = build_authorize_url("client-id", "http://localhost:1234/", "st", "challenge");
        assert!(url.contains("response_type=code"), "{url}");
        assert!(url.contains("code_challenge=challenge"), "{url}");
        assert!(url.contains("code_challenge_method=S256"), "{url}");
        assert!(
            url.contains("redirect_uri=http%3A%2F%2Flocalhost%3A1234%2F"),
            "{url}"
        );
    }

    #[test]
    fn exchange_auth_code_parses() {
        let http = MockHttp::new(
            vec![Ok(
                r#"{"access_token":"a","refresh_token":"r","token_type":"Bearer"}"#.into(),
            )],
            vec![],
            vec![],
        );
        let pair = exchange_auth_code(&http, "c", "code", "http://localhost:1/", "ver").unwrap();
        assert_eq!(pair.access_token, "a");
        assert_eq!(pair.refresh_token, "r");
    }

    #[test]
    fn exchange_builds_session() {
        let http = MockHttp::new(
            vec![],
            vec![
                Ok(r#"{"Token":"xbl"}"#.into()),
                Ok(r#"{"Token":"xsts","DisplayClaims":{"xui":[{"uhs":"uhs1"}]}}"#.into()),
                Ok(r#"{"access_token":"mc","expires_in":3600}"#.into()),
            ],
            vec![Ok(
                r#"{"id":"abcdef0123456789abcdef0123456789","name":"Steve"}"#.into(),
            )],
        );
        let session = exchange_for_minecraft(&http, "ms", "refresh").unwrap();
        assert_eq!(session.mc_username, "Steve");
        assert_eq!(session.mc_uuid, "abcdef01-2345-6789-abcd-ef0123456789");
        assert_eq!(session.mc_access_token, "mc");
        assert_eq!(session.ms_refresh_token, "refresh");
        assert!(session_is_fresh(&session, 0));
        let auth = online_auth_from_session(&session);
        assert_eq!(auth.player_name, "Steve");
    }

    #[test]
    fn session_freshness_respects_expiry() {
        let mut s = StoredMsaSession {
            ms_refresh_token: "r".into(),
            mc_access_token: "t".into(),
            mc_username: "A".into(),
            mc_uuid: "u".into(),
            mc_expires_at: now_unix().saturating_sub(1),
        };
        assert!(!session_is_fresh(&s, 0));
        s.mc_expires_at = now_unix() + 600;
        assert!(session_is_fresh(&s, 60));
    }

    #[test]
    fn aadsts70002_is_actionable_french() {
        let msg = friendly_oauth_error(
            r#"{"error":"unauthorized_client","error_description":"AADSTS70002: The provided client is not supported for this feature. The client application must be marked as 'mobile.'"}"#,
            401,
        );
        assert!(msg.contains("Mobile et applications de bureau"), "{msg}");
        assert!(msg.contains(AZURE_REDIRECT_URI), "{msg}");
    }
}
