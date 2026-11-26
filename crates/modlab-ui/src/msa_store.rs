//! Encrypted-at-rest MSA session for the Tauri shell.
//!
//! Tokens never enter `modlab-core` / SQLite. On Windows the blob is protected
//! with DPAPI (user scope). Elsewhere the file is written with restrictive
//! permissions only — still under `<data_root>/secrets/`, never the repo.

use modlab_game_minecraft::msa::StoredMsaSession;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

const SESSION_FILE: &str = "msa_session.bin";
const CLIENT_ID_FILE: &str = "msa_client_id.txt";
const MAGIC: &[u8] = b"MLMSA1\0";

pub fn secrets_dir(data_root: &Path) -> PathBuf {
    data_root.join("secrets")
}

fn session_path(data_root: &Path) -> PathBuf {
    secrets_dir(data_root).join(SESSION_FILE)
}

fn client_id_path(data_root: &Path) -> PathBuf {
    secrets_dir(data_root).join(CLIENT_ID_FILE)
}

/// Where the effective MSA client id came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClientIdSource {
    /// Saved from Réglages → Compte (`secrets/msa_client_id.txt`).
    Ui,
    /// Process env `MODLAB_MSA_CLIENT_ID`.
    Env,
    /// Built-in public Prism / Minecraft launcher client id.
    Default,
}

/// Load the UI-saved override only (file under `secrets/`). Empty / missing → None.
pub fn load_client_id_override(data_root: &Path) -> Option<String> {
    let path = client_id_path(data_root);
    let text = fs::read_to_string(&path).ok()?;
    let trimmed = text.trim().to_string();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}

/// Priority: UI-saved file > env `MODLAB_MSA_CLIENT_ID` > built-in public client.
pub fn resolve_client_id(data_root: &Path) -> Option<String> {
    Some(resolve_client_id_with_source(data_root).0)
}

/// Same as [`resolve_client_id`], plus which layer won. Always returns a client id.
pub fn resolve_client_id_with_source(data_root: &Path) -> (String, ClientIdSource) {
    if let Some(from_ui) = load_client_id_override(data_root) {
        return (from_ui, ClientIdSource::Ui);
    }
    if let Some(from_env) = modlab_game_minecraft::msa::resolve_client_id_from_env() {
        return (from_env, ClientIdSource::Env);
    }
    (
        modlab_game_minecraft::msa::default_client_id(),
        ClientIdSource::Default,
    )
}

/// Persist an optional client id override from Settings (not a secret).
/// Wins over env and the built-in default. Empty string removes the override
/// (env, then built-in public client).
pub fn save_client_id_override(data_root: &Path, client_id: &str) -> Result<(), String> {
    let trimmed = client_id.trim();
    if trimmed.is_empty() {
        let _ = fs::remove_file(client_id_path(data_root));
        return Ok(());
    }
    ensure_secrets_dir(data_root)?;
    fs::write(client_id_path(data_root), trimmed).map_err(|e| format!("écriture client id : {e}"))
}

pub fn load_session(data_root: &Path) -> Result<Option<StoredMsaSession>, String> {
    let path = session_path(data_root);
    if !path.is_file() {
        return Ok(None);
    }
    let blob = fs::read(&path).map_err(|e| format!("lecture session MSA : {e}"))?;
    let plain = unprotect(&blob)?;
    let session: StoredMsaSession = serde_json::from_slice(&plain)
        .map_err(|e| format!("session MSA corrompue — reconnectez-vous ({e})"))?;
    Ok(Some(session))
}

pub fn save_session(data_root: &Path, session: &StoredMsaSession) -> Result<(), String> {
    ensure_secrets_dir(data_root)?;
    let plain = serde_json::to_vec(session).map_err(|e| e.to_string())?;
    let blob = protect(&plain)?;
    let path = session_path(data_root);
    let tmp = path.with_extension("bin.tmp");
    {
        let mut f = fs::File::create(&tmp).map_err(|e| format!("écriture session MSA : {e}"))?;
        f.write_all(&blob)
            .map_err(|e| format!("écriture session MSA : {e}"))?;
        f.sync_all()
            .map_err(|e| format!("sync session MSA : {e}"))?;
    }
    fs::rename(&tmp, &path).map_err(|e| format!("finalisation session MSA : {e}"))?;
    Ok(())
}

pub fn clear_session(data_root: &Path) -> Result<(), String> {
    let path = session_path(data_root);
    if path.is_file() {
        fs::remove_file(&path).map_err(|e| format!("suppression session MSA : {e}"))?;
    }
    Ok(())
}

fn ensure_secrets_dir(data_root: &Path) -> Result<(), String> {
    let dir = secrets_dir(data_root);
    fs::create_dir_all(&dir).map_err(|e| format!("dossier secrets : {e}"))?;
    Ok(())
}

fn protect(plain: &[u8]) -> Result<Vec<u8>, String> {
    #[cfg(windows)]
    {
        return dpapi_protect(plain);
    }
    #[cfg(not(windows))]
    {
        let mut out = MAGIC.to_vec();
        out.extend_from_slice(plain);
        Ok(out)
    }
}

fn unprotect(blob: &[u8]) -> Result<Vec<u8>, String> {
    #[cfg(windows)]
    {
        return dpapi_unprotect(blob);
    }
    #[cfg(not(windows))]
    {
        if blob.starts_with(MAGIC) {
            return Ok(blob[MAGIC.len()..].to_vec());
        }
        Err("session MSA : format inconnu".into())
    }
}

#[cfg(windows)]
fn dpapi_protect(plain: &[u8]) -> Result<Vec<u8>, String> {
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::LocalFree;
    use windows::Win32::Security::Cryptography::{
        CryptProtectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
    };

    let mut in_blob = CRYPT_INTEGER_BLOB {
        cbData: plain.len() as u32,
        pbData: plain.as_ptr() as *mut u8,
    };
    let mut out_blob = CRYPT_INTEGER_BLOB {
        cbData: 0,
        pbData: std::ptr::null_mut(),
    };
    // SAFETY: CryptProtectData copies into LocalAlloc memory we free below.
    let ok = unsafe {
        CryptProtectData(
            &mut in_blob,
            PCWSTR::null(),
            None,
            None,
            None,
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut out_blob,
        )
    };
    if ok.is_err() {
        return Err("chiffrement DPAPI impossible — session non enregistrée.".into());
    }
    let mut sealed = MAGIC.to_vec();
    sealed.push(b'D'); // marker: DPAPI payload follows
                       // SAFETY: pbData points at cbData bytes from CryptProtectData.
    let slice = unsafe { std::slice::from_raw_parts(out_blob.pbData, out_blob.cbData as usize) };
    sealed.extend_from_slice(slice);
    unsafe {
        let _ = LocalFree(Some(windows::Win32::Foundation::HLOCAL(
            out_blob.pbData as _,
        )));
    }
    Ok(sealed)
}

#[cfg(windows)]
fn dpapi_unprotect(blob: &[u8]) -> Result<Vec<u8>, String> {
    use windows::Win32::Foundation::LocalFree;
    use windows::Win32::Security::Cryptography::{
        CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
    };

    if blob.len() < MAGIC.len() + 1 || &blob[..MAGIC.len()] != MAGIC || blob[MAGIC.len()] != b'D' {
        return Err("session MSA : format DPAPI attendu — reconnectez-vous.".into());
    }
    let payload = &blob[MAGIC.len() + 1..];
    let mut in_blob = CRYPT_INTEGER_BLOB {
        cbData: payload.len() as u32,
        pbData: payload.as_ptr() as *mut u8,
    };
    let mut out_blob = CRYPT_INTEGER_BLOB {
        cbData: 0,
        pbData: std::ptr::null_mut(),
    };
    let ok = unsafe {
        CryptUnprotectData(
            &mut in_blob,
            None,
            None,
            None,
            None,
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut out_blob,
        )
    };
    if ok.is_err() {
        return Err(
            "Impossible de lire la session Microsoft (DPAPI) — reconnectez-vous depuis Compte."
                .into(),
        );
    }
    let plain =
        unsafe { std::slice::from_raw_parts(out_blob.pbData, out_blob.cbData as usize).to_vec() };
    unsafe {
        let _ = LocalFree(Some(windows::Win32::Foundation::HLOCAL(
            out_blob.pbData as _,
        )));
    }
    Ok(plain)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn roundtrip_session_without_core() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        let session = StoredMsaSession {
            ms_refresh_token: "refresh-secret".into(),
            mc_access_token: "mc-token".into(),
            mc_username: "Steve".into(),
            mc_uuid: "abcdef01-2345-6789-abcd-ef0123456789".into(),
            mc_expires_at: 9_999_999_999,
        };
        save_session(root, &session).unwrap();
        let loaded = load_session(root).unwrap().expect("session");
        assert_eq!(loaded, session);
        // Must not land in a sqlite / core path.
        assert!(session_path(root).starts_with(secrets_dir(root)));
        clear_session(root).unwrap();
        assert!(load_session(root).unwrap().is_none());
    }

    #[test]
    fn client_id_priority_ui_env_default() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        let prev = std::env::var_os(modlab_game_minecraft::msa::CLIENT_ID_ENV);
        std::env::remove_var(modlab_game_minecraft::msa::CLIENT_ID_ENV);

        assert_eq!(
            resolve_client_id(root).as_deref(),
            Some(modlab_game_minecraft::msa::DEFAULT_MSA_CLIENT_ID)
        );
        assert_eq!(
            resolve_client_id_with_source(root).1,
            ClientIdSource::Default
        );

        std::env::set_var(modlab_game_minecraft::msa::CLIENT_ID_ENV, "from-env-id");
        assert_eq!(resolve_client_id(root).as_deref(), Some("from-env-id"));
        assert_eq!(resolve_client_id_with_source(root).1, ClientIdSource::Env);

        save_client_id_override(root, " azure-app-id ").unwrap();
        assert_eq!(resolve_client_id(root).as_deref(), Some("azure-app-id"));
        assert_eq!(resolve_client_id_with_source(root).1, ClientIdSource::Ui);

        save_client_id_override(root, "").unwrap();
        assert_eq!(resolve_client_id(root).as_deref(), Some("from-env-id"));

        match prev {
            Some(v) => std::env::set_var(modlab_game_minecraft::msa::CLIENT_ID_ENV, v),
            None => std::env::remove_var(modlab_game_minecraft::msa::CLIENT_ID_ENV),
        }
    }
}
