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

/// `hydrate_metadata`: when `false`, entries come straight from the Core
/// without re-walking archives or replaying metadata layers — fast enough for
/// an instance switch. Defaults to `true` (first paint, post-mutation refresh).
#[tauri::command]
pub(crate) fn get_shell_snapshot(
    state: State<AppState>,
    loadout_id: Option<i64>,
    hydrate_metadata: Option<bool>,
) -> Result<ShellSnapshot, String> {
    let core = state.core.lock().map_err(|e| e.to_string())?;
    let loadouts: Vec<LoadoutDto> = loadout::list(&core)
        .map_err(|e| e.to_string())?
        .into_iter()
        .map(|l| crate::loadout_dto(&core, &l))
        .collect::<Result<Vec<_>, _>>()?;
    let selected = loadout_id.or_else(|| loadouts.first().map(|l| l.id));
    // Opening a branch fills missing lengths, then replays the file-name
    // and archive layers on every row that is already on disk, then fills
    // any category that is still empty. A choice the human made stays.
    // Instance switches pass `hydrate_metadata: false` so the UI paints SQLite
    // state immediately without a disk walk. Full hydration runs on first load
    // and after mutations via the default `true`.
    if let Some(id) = selected {
        let _ = crate::sync_mc_branch_loader(&core, &state.data_root, id);
    }
    if hydrate_metadata.unwrap_or(true) {
        if let Some(id) = selected {
            modlab_core::mod_measure::fill_branch(&core, id).map_err(|e| e.to_string())?;
            crate::refresh_branch_metadata(&core, id)?;
            crate::fill_branch_categories(&core, id)?;
        }
    }
    let entries = match selected {
        Some(id) => entries_dto(&core, id)?,
        None => Vec::new(),
    };
    Ok(ShellSnapshot {
        data_root: state.data_root.display().to_string(),
        games: detect_games(),
        loadouts,
        entries,
        selected_loadout_id: selected,
        usvfs_dll: resolve_slot(&core, &SkyrimAdapter, "usvfs_dll")
            .map(|p| p.display().to_string()),
    })
}

/// One-shot PowerShell run for the Console → PowerShell REPL (UI only).
/// Not a PTY: each call starts a fresh `powershell -NoProfile -Command`.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PowerShellResult {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
    pub cwd: String,
}

const PS_CWD_MARK: &str = "__MODLAB_PS_CWD__";

/// Run `command` in PowerShell from `cwd` (or the process default). Returns
/// stdout/stderr, exit code, and the working directory after the command.
#[tauri::command]
pub(crate) fn run_powershell(
    command: String,
    cwd: Option<String>,
) -> Result<PowerShellResult, String> {
    #[cfg(not(windows))]
    {
        let _ = (command, cwd);
        return Err("PowerShell n’est disponible que sous Windows.".into());
    }
    #[cfg(windows)]
    {
        run_powershell_windows(command, cwd)
    }
}

#[cfg(windows)]
fn run_powershell_windows(
    command: String,
    cwd: Option<String>,
) -> Result<PowerShellResult, String> {
    use std::os::windows::process::CommandExt;
    use std::process::Command;

    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    let trimmed = command.trim();
    if trimmed.is_empty() {
        return Err("Commande vide.".into());
    }

    let start_cwd = cwd
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
        .filter(|p| p.is_dir())
        .unwrap_or_else(|| {
            std::env::var_os("USERPROFILE")
                .map(PathBuf::from)
                .filter(|p| p.is_dir())
                .unwrap_or_else(|| PathBuf::from("."))
        });

    // Quote for PowerShell single-quoted string (escape ' as '').
    let cwd_lit = start_cwd.display().to_string().replace('\'', "''");
    // Append cwd marker after the user command; keep it simple (no PTY).
    let script = format!(
        "Set-Location -LiteralPath '{cwd_lit}'; {trimmed}; Write-Output ('{PS_CWD_MARK}' + (Get-Location).Path)"
    );

    let output = Command::new("powershell.exe")
        .args([
            "-NoLogo",
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            &script,
        ])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| format!("Impossible de lancer PowerShell : {e}"))?;

    let raw_out = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    let (stdout, end_cwd) = split_ps_cwd(&raw_out, &start_cwd);

    Ok(PowerShellResult {
        stdout,
        stderr,
        exit_code: output.status.code().unwrap_or(-1),
        cwd: end_cwd,
    })
}

#[cfg(windows)]
fn split_ps_cwd(raw: &str, fallback: &Path) -> (String, String) {
    if let Some(idx) = raw.rfind(PS_CWD_MARK) {
        let stdout = raw[..idx].trim_end_matches(['\r', '\n']).to_string();
        let cwd = raw[idx + PS_CWD_MARK.len()..]
            .trim()
            .lines()
            .next()
            .unwrap_or("")
            .trim()
            .to_string();
        if !cwd.is_empty() {
            return (stdout, cwd);
        }
        return (stdout, fallback.display().to_string());
    }
    (
        raw.trim_end_matches(['\r', '\n']).to_string(),
        fallback.display().to_string(),
    )
}
