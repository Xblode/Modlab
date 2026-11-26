#[allow(unused_imports)]
use crate::*;
#[allow(unused_imports)]
use modlab_ai::{diagnose, llm_phrase, rules_answer, LoadoutFacts, ModFact};
#[allow(unused_imports)]
use modlab_core::game_adapter::{fuse_path, GameAdapter, PathKind};
#[allow(unused_imports)]
use modlab_core::{conflicts, download, loadout, transaction, Core};
#[allow(unused_imports)]
use modlab_game_minecraft::MinecraftAdapter;
#[allow(unused_imports)]
use modlab_game_skyrim::fomod;
#[allow(unused_imports)]
use modlab_game_skyrim::installer::ArchiveShape;
#[allow(unused_imports)]
use modlab_game_skyrim::plugins;
#[allow(unused_imports)]
use modlab_game_skyrim::vanilla;
#[allow(unused_imports)]
use modlab_game_skyrim::SkyrimAdapter;
#[allow(unused_imports)]
use serde::Serialize;
#[allow(unused_imports)]
use std::path::{Component, Path, PathBuf};
#[allow(unused_imports)]
use std::sync::atomic::{AtomicBool, Ordering};
#[allow(unused_imports)]
use std::sync::Mutex;
#[allow(unused_imports)]
#[cfg(windows)]
use std::time::{Duration, SystemTime};
#[allow(unused_imports)]
use tauri::{Emitter, Manager, State};

/// An http(s) URL the system browser is allowed to open. Rejects every other
/// scheme so this command cannot be turned into a local-file or shell open.
fn external_http_url(raw: &str) -> Result<String, String> {
    let raw = raw.trim();
    if raw.is_empty() || raw.chars().any(|c| c.is_control() || c.is_whitespace()) {
        return Err("adresse invalide".to_string());
    }
    let parsed = url::Url::parse(raw).map_err(|_| "adresse invalide".to_string())?;
    match parsed.scheme() {
        "http" | "https" => {}
        _ => {
            return Err("seules les adresses http et https peuvent être ouvertes".to_string());
        }
    }
    if parsed.host_str().is_none() {
        return Err("adresse invalide".to_string());
    }
    Ok(parsed.to_string())
}

fn open_in_system_browser(url: &str) -> Result<(), String> {
    #[cfg(windows)]
    {
        // One argument, no shell: `start` would interpret `&` and quotes.
        std::process::Command::new("rundll32.exe")
            .args(["url.dll,FileProtocolHandler", url])
            .spawn()
            .map_err(|e| format!("ouverture du navigateur impossible : {e}"))?;
        Ok(())
    }
    #[cfg(not(windows))]
    {
        std::process::Command::new("xdg-open")
            .arg(url)
            .spawn()
            .map_err(|e| format!("ouverture du navigateur impossible : {e}"))?;
        Ok(())
    }
}

/// Open `url` in the system browser. Reusable for any http(s) page; the
/// official SKSE and LOOT pages are `official_tool_pages`.
#[tauri::command]
pub(crate) fn open_external_url(url: String) -> Result<(), String> {
    let url = external_http_url(&url)?;
    open_in_system_browser(&url)
}

/// SKSE and LOOT pages the shell may hand to `open_external_url`. Static:
/// ModLab does not fetch them.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OfficialToolPageDto {
    pub id: String,
    pub name: String,
    pub url: String,
}

#[tauri::command]
pub(crate) fn official_tool_pages() -> Vec<OfficialToolPageDto> {
    modlab_game_skyrim::tool_archive::OFFICIAL_TOOL_PAGES
        .iter()
        .map(|page| OfficialToolPageDto {
            id: page.id.to_string(),
            name: page.name.to_string(),
            url: page.url.to_string(),
        })
        .collect()
}

#[cfg(windows)]
fn normalize_windows_path(path: &Path) -> PathBuf {
    let text = path.display().to_string().replace('/', "\\");
    PathBuf::from(text)
}

#[cfg(not(windows))]
fn normalize_windows_path(path: &Path) -> PathBuf {
    path.to_path_buf()
}

/// Quote for `explorer /select,"…"` when the path contains spaces.
fn explorer_path_token(path: &str) -> String {
    if path.contains(' ') {
        format!("\"{}\"", path.replace('"', "\\\""))
    } else {
        path.to_string()
    }
}

/// Explorer argument for `reveal_path`: existing path only — directories open
/// as-is; files always use `/select,` (without it Explorer often no-ops).
fn windows_explorer_arg(path: &Path, select: bool) -> Result<String, String> {
    let path = normalize_windows_path(path);
    let meta =
        std::fs::metadata(&path).map_err(|_| format!("chemin introuvable : {}", path.display()))?;
    let shown = path.display().to_string();
    if meta.is_dir() {
        return Ok(explorer_path_token(&shown));
    }
    if !meta.is_file() {
        return Err(format!("chemin non pris en charge : {}", path.display()));
    }
    let _ = select;
    Ok(format!("/select,{}", explorer_path_token(&shown)))
}

/// Open Explorer at `path` — either the folder itself, or (`select: true`)
/// the folder containing `path` with that file highlighted. Fire-and-forget
/// by design: `explorer.exe` routinely exits non-zero even on success, so
/// waiting on its status would misreport a perfectly normal open as failed.
fn reveal_path_on_disk(path: &Path, select: bool) -> Result<(), String> {
    #[cfg(windows)]
    {
        let arg = windows_explorer_arg(path, select)?;
        std::process::Command::new("explorer")
            .arg(arg)
            .spawn()
            .map_err(|e| format!("ouverture de l’explorateur impossible : {e}"))?;
        Ok(())
    }
    #[cfg(not(windows))]
    {
        let _ = (path, select);
        Err("ouverture du dossier disponible uniquement sous Windows".to_string())
    }
}

#[tauri::command]
pub(crate) fn reveal_path(path: String, select: bool) -> Result<(), String> {
    let path = path.trim();
    if path.is_empty() {
        return Err("chemin vide".to_string());
    }
    reveal_path_on_disk(Path::new(path), select)
}

/// Open the per-instance launcher folder for a branch (`install_target`).
#[tauri::command]
pub(crate) fn reveal_instance_folder(
    state: State<AppState>,
    loadout_id: i64,
) -> Result<(), String> {
    let core = state.core.lock().map_err(|e| e.to_string())?;
    let target = crate::paths::install_target_for_loadout(&core, &state.data_root, loadout_id)?
        .ok_or_else(|| "Ce jeu n’a pas de dossier d’instance dédié.".to_string())?;
    reveal_path_on_disk(&target.instance_root, false)
}

/// Which instance is on screen. A download that starts in the embedded
/// browser is filed into this loadout immediately. `None` means "no
/// instance" — the capture stays unfiled until something attaches it.
#[tauri::command]
pub(crate) fn note_active_loadout(state: State<AppState>, loadout_id: Option<i64>) {
    if let Ok(mut active) = state.active_loadout.lock() {
        *active = loadout_id;
    }
}

/// Show the embedded browser at `rect` (CSS pixels, measured by the React
/// side), creating it on the first call. `url` is only used for that first
/// call — afterwards the browser keeps whatever page it is on, which is the
/// point of a single persistent tab. `game_id` records which game this open
/// belongs to, so a download captured while this page is up files under
/// that game's own downloads subfolder (`downloads::on_requested`).
#[tauri::command]
pub(crate) fn browser_show(
    state: State<AppState>,
    app: tauri::AppHandle,
    url: String,
    rect: crate::browser::Rect,
    game_id: Option<String>,
) -> Result<(), String> {
    if let Ok(mut active) = state.active_game.lock() {
        *active = game_id;
    }
    crate::browser::show(&app, &url, rect)
}

#[tauri::command]
pub(crate) fn browser_hide(app: tauri::AppHandle) -> Result<(), String> {
    crate::browser::hide(&app)
}

#[tauri::command]
pub(crate) fn browser_navigate(app: tauri::AppHandle, url: String) -> Result<(), String> {
    crate::browser::navigate(&app, &url)
}

/// `-1` back, `1` forward — the address bar's arrows.
#[tauri::command]
pub(crate) fn browser_go(app: tauri::AppHandle, delta: i32) -> Result<(), String> {
    crate::browser::go(&app, delta)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_explorer_arg_resolves_files_and_directories() {
        let dir = std::env::temp_dir().join(format!("modlab_reveal_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let file = dir.join("sample.jar");
        std::fs::write(&file, b"x").expect("fixture file");

        let dir_arg = windows_explorer_arg(&dir, false).expect("dir");
        assert_eq!(dir_arg, dir.display().to_string());

        let select_default = windows_explorer_arg(&file, false).expect("file default select");
        assert_eq!(select_default, format!("/select,{}", file.display()));

        let select_arg = windows_explorer_arg(&file, true).expect("select file");
        assert_eq!(select_arg, format!("/select,{}", file.display()));

        let spaced = dir.join("spaced name.jar");
        std::fs::write(&spaced, b"x").expect("spaced file");
        let spaced_arg = windows_explorer_arg(&spaced, true).expect("spaced select");
        assert_eq!(spaced_arg, format!("/select,\"{}\"", spaced.display()));
        let _ = std::fs::remove_file(&spaced);

        let _ = std::fs::remove_file(&file);
        let _ = std::fs::remove_dir(&dir);
    }

    #[test]
    fn external_http_url_accepts_the_official_tool_pages_only() {
        for page in modlab_game_skyrim::tool_archive::OFFICIAL_TOOL_PAGES {
            let opened = external_http_url(page.url).expect(page.id);
            assert!(opened.starts_with("https://"), "{opened}");
        }
        assert!(external_http_url("file:///C:/Windows/system32").is_err());
        assert!(external_http_url("javascript:alert(1)").is_err());
        assert!(external_http_url("https://skse.silverlock.org/a b").is_err());
        assert!(external_http_url("").is_err());
    }
}
