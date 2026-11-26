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
use std::time::Duration;
#[allow(unused_imports)]
#[cfg(windows)]
use std::time::SystemTime;
#[allow(unused_imports)]
use tauri::{Emitter, Manager, State};

/// Is SKSE really in the game folder, and does it match the game's version?
/// (`modlab_game_skyrim::skse::detect`.) This is what the sidebar's SKSE
/// indicator shows — read from the files, not guessed from a mod's name.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkseStatusDto {
    /// `"ok"`, `"wrong_version"`, `"missing"` or `"unverifiable"`.
    pub state: &'static str,
    pub game_version: Option<String>,
    pub expected_dll: Option<String>,
    pub loader_present: bool,
    pub dlls_found: Vec<String>,
    /// One French sentence saying what is wrong, or that all is well.
    pub message: String,
}

impl From<modlab_game_skyrim::skse::SkseStatus> for SkseStatusDto {
    fn from(s: modlab_game_skyrim::skse::SkseStatus) -> Self {
        Self {
            state: s.state.code(),
            message: s.message(),
            game_version: s.game_version,
            expected_dll: s.expected_dll,
            loader_present: s.loader_present,
            dlls_found: s.dlls_found,
        }
    }
}

/// Where the game's install root and executable are, honouring the
/// Réglages → Jeux overrides.
pub(crate) fn skyrim_install_paths(core: &Core) -> Option<(PathBuf, PathBuf)> {
    let root = resolve_slot(core, &SkyrimAdapter, "root")?;
    let exe =
        resolve_slot(core, &SkyrimAdapter, "exe").unwrap_or_else(|| root.join("SkyrimSE.exe"));
    Some((root, exe))
}

/// Read-only (Lecture tier, §7.2). `None` when Skyrim is not installed.
#[tauri::command]
pub(crate) fn skse_status(state: State<AppState>) -> Result<Option<SkseStatusDto>, String> {
    let core = state.core.lock().map_err(|e| e.to_string())?;
    Ok(skyrim_install_paths(&core)
        .map(|(root, exe)| modlab_game_skyrim::skse::detect(&root, &exe).into()))
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RejectedPluginDto {
    pub name: String,
    pub reason: String,
}

/// The short balance sheet of one finished game session, read back from
/// SKSE's own journals (`skse64.log`, `skse64_loader.log`) — the proof of
/// whether SKSE and its plugins really loaded.
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SessionReportDto {
    /// `"skyrim"` (default) or `"minecraft"`.
    #[serde(default = "default_session_kind")]
    pub session_kind: String,
    pub loadout_id: i64,
    /// Unix time in milliseconds.
    pub ended_at: u64,
    /// A problem while following the session (the game never registered with
    /// the virtual filesystem, teardown not confirmed...), if any.
    pub error: Option<String>,
    pub log_found: bool,
    /// `skse64.log` was written during this session.
    pub log_fresh: bool,
    pub skse_initialized: bool,
    pub skse_version: Option<String>,
    pub init_complete: bool,
    pub loaded: Vec<String>,
    pub rejected: Vec<RejectedPluginDto>,
    /// Plugins the loadout provides that SKSE never even looked at.
    pub not_seen: Vec<String>,
    pub loader_launched: bool,
    pub loader_notes: Vec<String>,
}

fn default_session_kind() -> String {
    "skyrim".to_string()
}

impl SessionReportDto {
    fn new(
        loadout_id: i64,
        report: modlab_game_skyrim::skse::SessionReport,
        error: Option<String>,
    ) -> Self {
        let ended_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX));
        Self {
            session_kind: default_session_kind(),
            loadout_id,
            ended_at,
            error,
            log_found: report.log_found,
            log_fresh: report.log_fresh,
            skse_initialized: report.skse_initialized,
            skse_version: report.skse_version,
            init_complete: report.init_complete,
            loaded: report.loaded,
            rejected: report
                .rejected
                .into_iter()
                .map(|r| RejectedPluginDto {
                    name: r.name,
                    reason: r.reason,
                })
                .collect(),
            not_seen: report.not_seen,
            loader_launched: report.loader_launched,
            loader_notes: report.loader_notes,
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameSessionDto {
    /// A game session is running (launch in progress included).
    pub running: bool,
    pub last_report: Option<SessionReportDto>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StopGameSessionDto {
    /// The stop request was accepted (process kill attempted).
    pub accepted: bool,
    /// No session was marked running in the Core.
    pub already_stopped: bool,
    /// French hint for the toast, if any.
    pub message: Option<String>,
}

#[cfg(windows)]
fn kill_pid_tree(pid: u32) -> Result<(), String> {
    use std::process::Command;
    let status = Command::new("taskkill")
        .args(["/PID", &pid.to_string(), "/T", "/F"])
        .status()
        .map_err(|e| format!("taskkill : {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "Impossible de fermer le processus {pid} (déjà arrêté ou accès refusé)."
        ))
    }
}

#[cfg(windows)]
fn kill_skyrim_related_processes() -> Vec<String> {
    use std::process::Command;
    let mut problems = Vec::new();
    for image in ["SkyrimSE.exe", "skse64_loader.exe"] {
        let status = Command::new("taskkill").args(["/IM", image, "/F"]).status();
        match status {
            Ok(s) if s.success() => {}
            Ok(_) => problems.push(format!("{image} : fermeture refusée ou déjà arrêté.")),
            Err(e) => problems.push(format!("{image} : {e}")),
        }
    }
    problems
}

fn begin_session_tracking(state: &AppState, loadout_id: i64) {
    state.session_stop.store(false, Ordering::SeqCst);
    if let Ok(mut slot) = state.active_session_loadout.lock() {
        *slot = Some(loadout_id);
    }
    if let Ok(mut pid) = state.minecraft_pid.lock() {
        *pid = None;
    }
}

fn end_session_tracking(state: &AppState) {
    if let Ok(mut slot) = state.active_session_loadout.lock() {
        *slot = None;
    }
    if let Ok(mut pid) = state.minecraft_pid.lock() {
        *pid = None;
    }
    state.session_stop.store(false, Ordering::SeqCst);
}

/// If the JVM is gone but the flag stayed up, nudge the session thread.
fn reconcile_stale_session(state: &AppState) {
    if !state.game_running.load(Ordering::SeqCst) {
        return;
    }
    let stale_mc = state
        .minecraft_pid
        .lock()
        .ok()
        .and_then(|g| *g)
        .is_some_and(|pid| !modlab_game_skyrim::usvfs::process_pid_alive(pid));
    if stale_mc {
        state.session_stop.store(true, Ordering::SeqCst);
    }
}

/// Ask the running game to exit (kill process tree). The session thread still
/// emits `game://ended` once teardown finishes.
#[tauri::command]
pub(crate) fn stop_game_session(state: State<AppState>) -> Result<StopGameSessionDto, String> {
    if !state.game_running.load(Ordering::SeqCst) {
        return Ok(StopGameSessionDto {
            accepted: false,
            already_stopped: true,
            message: None,
        });
    }
    state.session_stop.store(true, Ordering::SeqCst);
    let mut problems = Vec::new();
    if let Ok(guard) = state.minecraft_pid.lock() {
        if let Some(pid) = *guard {
            if modlab_game_skyrim::usvfs::process_pid_alive(pid) {
                if let Err(e) = kill_pid_tree(pid) {
                    problems.push(e);
                }
            }
        }
    }
    #[cfg(windows)]
    problems.extend(kill_skyrim_related_processes());
    #[cfg(not(windows))]
    problems.push("Arrêt forcé non pris en charge hors Windows.".into());

    let message = if problems.is_empty() {
        Some("Fermeture du jeu demandée…".into())
    } else {
        Some(format!("Fermeture demandée, mais : {}", problems.join(" ")))
    };
    Ok(StopGameSessionDto {
        accepted: true,
        already_stopped: false,
        message,
    })
}

/// What a freshly (re)loaded window needs to know: is a game running, and
/// what did the last session's journals say. Read-only.
#[tauri::command]
pub(crate) fn game_session_state(state: State<AppState>) -> Result<GameSessionDto, String> {
    reconcile_stale_session(&state);
    Ok(GameSessionDto {
        running: state.game_running.load(Ordering::SeqCst),
        last_report: state
            .last_session
            .lock()
            .map_err(|e| e.to_string())?
            .clone(),
    })
}

/// Name of the event the window listens to for the end of a session.
const GAME_ENDED_EVENT: &str = "game://ended";
/// Live stdout/stderr chunks while a Minecraft JVM session is running.
const GAME_LOG_EVENT: &str = "game://log";
/// French hint while the JVM is still up (e.g. Sodium Fatal Startup MessageBox).
const GAME_LAUNCH_HINT_EVENT: &str = "game://launch-hint";

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct GameLogChunkDto {
    /// `"stdout"` or `"stderr"`.
    stream: &'static str,
    text: String,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct GameLaunchHintDto {
    /// French sentence for toast / notice under Jouer.
    message: String,
}

/// Balance sheet for a finished Minecraft JVM session (no SKSE journals).
fn minecraft_session_report(
    loadout_id: i64,
    error: Option<String>,
    loader_notes: Vec<String>,
) -> SessionReportDto {
    let ended_at = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX));
    SessionReportDto {
        session_kind: "minecraft".to_string(),
        loadout_id,
        ended_at,
        error,
        log_found: false,
        log_fresh: false,
        skse_initialized: false,
        skse_version: None,
        init_complete: false,
        loaded: Vec::new(),
        rejected: Vec::new(),
        not_seen: Vec::new(),
        loader_launched: true,
        loader_notes,
    }
}

/// Jouer Minecraft : ensure version/loader + spawn JVM in the instance folder,
/// then follow the process like Skyrim (`game_running` / Arrêter / `game://ended`).
/// Streams stdout/stderr as `game://log`. Auth = MSA online when a valid
/// session exists in the encrypted store; otherwise offline pseudo (`player_name`).
/// Returns an optional French notice once the process has started.
#[tauri::command]
pub(crate) async fn launch_minecraft(
    app: tauri::AppHandle,
    loadout_id: i64,
    player_name: Option<String>,
) -> Result<Option<String>, String> {
    let player = player_name
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "Player".to_string());
    let (started_tx, started_rx) = std::sync::mpsc::channel::<Result<Option<String>, String>>();
    std::thread::Builder::new()
        .name("modlab-mc-session".into())
        .spawn(move || run_minecraft_session(&app, loadout_id, player, &started_tx))
        .map_err(|e| e.to_string())?;
    tauri::async_runtime::spawn_blocking(move || started_rx.recv())
        .await
        .map_err(|e| e.to_string())?
        .map_err(|_| "La session Minecraft s'est arrêtée avant d'avoir démarré.".to_string())?
}

/// Smoke / « Tester le lancement »: same prepare/spawn/log stream as Jouer,
/// tiny 1×1 window, wait until menu marker / crash / timeout, then kill JVM.
/// Resolves when the test finishes (not when the process merely starts).
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct MinecraftSmokeResultDto {
    /// `ok` | `fail` | `timeout` | `stopped`
    pub status: String,
    /// French one-liner for toast / notice.
    pub message: String,
    /// Log needle that matched, when status is `ok`.
    pub matched_marker: Option<String>,
}

#[tauri::command]
pub(crate) async fn test_launch_minecraft(
    app: tauri::AppHandle,
    loadout_id: i64,
    player_name: Option<String>,
) -> Result<MinecraftSmokeResultDto, String> {
    let player = player_name
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "Player".to_string());
    let (done_tx, done_rx) =
        std::sync::mpsc::channel::<Result<MinecraftSmokeResultDto, String>>();
    std::thread::Builder::new()
        .name("modlab-mc-smoke".into())
        .spawn(move || run_minecraft_smoke_session(&app, loadout_id, player, &done_tx))
        .map_err(|e| e.to_string())?;
    tauri::async_runtime::spawn_blocking(move || done_rx.recv())
        .await
        .map_err(|e| e.to_string())?
        .map_err(|_| "Le test de lancement s'est arrêté avant d'avoir démarré.".to_string())?
}

struct PreparedMinecraft {
    cmd: modlab_game_minecraft::launch::LaunchCommand,
}

fn prepare_minecraft_launch(
    state: &AppState,
    loadout_id: i64,
    player_name: &str,
) -> Result<PreparedMinecraft, String> {
    let (root, mc, loader, loader_ver) = {
        let core = state.core.lock().map_err(|e| e.to_string())?;
        let branch = loadout::list(&core)
            .map_err(|e| e.to_string())?
            .into_iter()
            .find(|b| b.id == loadout_id)
            .ok_or_else(|| format!("branche {loadout_id} introuvable"))?;
        if branch.game_id != "minecraft" {
            return Err("Cette branche n’est pas une instance Minecraft.".to_string());
        }
        let loader = branch
            .mc_loader
            .clone()
            .unwrap_or_else(|| "vanilla".to_string());
        let mc = branch
            .mc_game_version
            .clone()
            .ok_or_else(|| "Choisissez une version Minecraft.".to_string())?;
        let loader_ver = branch.mc_loader_version.clone();
        let root = crate::paths::install_target_for_loadout(&core, &state.data_root, loadout_id)?
            .ok_or_else(|| "Cette instance n'a pas de dossier dédié.".to_string())?
            .instance_root;
        (root, mc, loader, loader_ver)
    };

    // MSA online when a fresh (or refreshed) session exists; otherwise offline.
    let online_session = crate::commands::msa::session_for_launch(&state.data_root);
    let online_auth = online_session
        .as_ref()
        .map(modlab_game_minecraft::msa::online_auth_from_session);

    let req = modlab_game_minecraft::launch::LaunchRequest {
        instance_root: &root,
        data_root: &state.data_root,
        loadout_id,
        loader: &loader,
        mc: &mc,
        loader_ver: loader_ver.as_deref(),
        player_name,
        online_auth: online_auth.as_ref(),
    };
    let mut cmd = modlab_game_minecraft::launch::prepare_launch(&req)?;
    if online_auth.is_some() && cmd.notice.is_none() {
        if let Some(name) = online_auth.as_ref().map(|a| a.player_name.as_str()) {
            cmd.notice = Some(format!("Session Microsoft : {name} (en ligne)."));
        }
    }
    Ok(PreparedMinecraft { cmd })
}

/// Smoke test session: prepare like Jouer, tweak window, wait for menu, kill.
fn run_minecraft_smoke_session(
    app: &tauri::AppHandle,
    loadout_id: i64,
    player_name: String,
    done: &std::sync::mpsc::Sender<Result<MinecraftSmokeResultDto, String>>,
) {
    use modlab_game_minecraft::launch::{SmokeKind, SMOKE_TIMEOUT};

    let state = app.state::<AppState>();
    if state
        .game_running
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        let _ = done.send(Err("Une partie est déjà en cours.".to_string()));
        return;
    }
    let session = GameSessionGuard(&state.game_running);
    begin_session_tracking(&state, loadout_id);

    let prepared = match prepare_minecraft_launch(&state, loadout_id, &player_name) {
        Ok(p) => p,
        Err(e) => {
            let _ = done.send(Err(e));
            end_session_tracking(&state);
            return;
        }
    };

    let mut cmd = prepared.cmd;
    modlab_game_minecraft::launch::apply_smoke_launch_tweaks(&mut cmd);
    let mut notes = Vec::new();
    if let Some(n) = &cmd.notice {
        notes.push(n.clone());
    }
    notes.push(
        "Test de lancement : fenêtre 1×1 (pas de headless GLFW sous Windows).".to_string(),
    );

    let mut child = match modlab_game_minecraft::launch::spawn_launch(&cmd) {
        Ok(c) => c,
        Err(e) => {
            let _ = done.send(Err(e));
            end_session_tracking(&state);
            return;
        }
    };

    if let Some(pid) = modlab_game_minecraft::launch::child_pid(&child) {
        if let Ok(mut slot) = state.minecraft_pid.lock() {
            *slot = Some(pid);
        }
    }

    let app_for_log = app.clone();
    let mut sodium_hint_sent = false;
    let mut on_log = move |channel: modlab_game_minecraft::launch::LogChannel, text: &str| {
        let _ = app_for_log.emit(
            GAME_LOG_EVENT,
            GameLogChunkDto {
                stream: channel.as_str(),
                text: text.to_string(),
            },
        );
        if !sodium_hint_sent {
            if let Some(hint) =
                modlab_game_minecraft::launch::explain_minecraft_exit(text, "")
            {
                sodium_hint_sent = true;
                let _ = app_for_log.emit(
                    GAME_LAUNCH_HINT_EVENT,
                    GameLaunchHintDto { message: hint },
                );
            }
        }
    };

    let smoke = modlab_game_minecraft::launch::wait_smoke_launch(
        &mut child,
        Some(&state.session_stop),
        Duration::from_millis(500),
        SMOKE_TIMEOUT,
        Some(&mut on_log),
    );

    let status = match smoke.kind {
        SmokeKind::MenuReached => "ok",
        SmokeKind::Timeout => "timeout",
        SmokeKind::Stopped => "stopped",
        SmokeKind::Crashed | SmokeKind::EarlyExit => "fail",
    };
    let dto_result = MinecraftSmokeResultDto {
        status: status.to_string(),
        message: smoke.message_fr.clone(),
        matched_marker: smoke.matched_marker.map(str::to_string),
    };

    notes.push(smoke.message_fr.clone());
    if let Some(m) = smoke.matched_marker {
        notes.push(format!("Marqueur : {m}"));
    }
    if !smoke.ok() {
        notes.push(modlab_game_minecraft::launch::format_launch_debug(&cmd));
        for line in
            modlab_game_minecraft::launch::process_log_lines(&smoke.stderr, &smoke.stdout, 8)
        {
            notes.push(line);
        }
    }

    let error = if smoke.ok() {
        None
    } else if smoke.kind == SmokeKind::Stopped {
        None
    } else {
        Some(smoke.message_fr.clone())
    };

    let mut report = minecraft_session_report(loadout_id, error, notes);
    report.session_kind = "minecraft-smoke".to_string();
    if let Ok(mut last) = state.last_session.lock() {
        *last = Some(report.clone());
    }
    end_session_tracking(&state);
    drop(session);
    let _ = app.emit(GAME_ENDED_EVENT, &report);
    let _ = done.send(Ok(dto_result));
}

/// Whole life of one Minecraft JVM session on its own thread.
fn run_minecraft_session(
    app: &tauri::AppHandle,
    loadout_id: i64,
    player_name: String,
    started: &std::sync::mpsc::Sender<Result<Option<String>, String>>,
) {
    let state = app.state::<AppState>();
    if state
        .game_running
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        let _ = started.send(Err("Une partie est déjà en cours.".to_string()));
        return;
    }
    let session = GameSessionGuard(&state.game_running);
    begin_session_tracking(&state, loadout_id);

    let prepared = match prepare_minecraft_launch(&state, loadout_id, &player_name) {
        Ok(p) => p,
        Err(e) => {
            let _ = started.send(Err(e));
            end_session_tracking(&state);
            return;
        }
    };

    let notice = prepared.cmd.notice.clone();
    let mut notes = Vec::new();
    if let Some(n) = &notice {
        notes.push(n.clone());
    }

    let mut child = match modlab_game_minecraft::launch::spawn_launch(&prepared.cmd) {
        Ok(c) => c,
        Err(e) => {
            let _ = started.send(Err(e));
            end_session_tracking(&state);
            return;
        }
    };

    if let Some(pid) = modlab_game_minecraft::launch::child_pid(&child) {
        if let Ok(mut slot) = state.minecraft_pid.lock() {
            *slot = Some(pid);
        }
    }

    let _ = started.send(Ok(notice));

    let app_for_log = app.clone();
    let mut sodium_hint_sent = false;
    let mut on_log = move |channel: modlab_game_minecraft::launch::LogChannel, text: &str| {
        let _ = app_for_log.emit(
            GAME_LOG_EVENT,
            GameLogChunkDto {
                stream: channel.as_str(),
                text: text.to_string(),
            },
        );
        // Fatal Startup (MessageBox) can leave the JVM alive until the human
        // closes it — surface the known Sodium/LWJGL hint as soon as the
        // stacktrace hits the log, not only on process exit.
        if !sodium_hint_sent {
            if let Some(hint) =
                modlab_game_minecraft::launch::explain_minecraft_exit(text, "")
            {
                sodium_hint_sent = true;
                let _ = app_for_log.emit(
                    GAME_LAUNCH_HINT_EVENT,
                    GameLaunchHintDto { message: hint },
                );
            }
        }
    };
    let outcome = modlab_game_minecraft::launch::wait_launch_cancellable(
        &mut child,
        Some(&state.session_stop),
        Duration::from_millis(500),
        Some(&mut on_log),
    );

    let error = match outcome {
        Ok(out) => {
            if out.status.success() {
                None
            } else {
                notes.push(modlab_game_minecraft::launch::format_launch_debug(
                    &prepared.cmd,
                ));
                for line in
                    modlab_game_minecraft::launch::process_log_lines(&out.stderr, &out.stdout, 8)
                {
                    notes.push(line);
                }
                let code = modlab_game_minecraft::launch::format_exit_code(out.status.code());
                if let Some(hint) =
                    modlab_game_minecraft::launch::explain_minecraft_exit(&out.stderr, &out.stdout)
                {
                    notes.insert(0, hint.clone());
                    Some(format!("Minecraft s’est arrêté (code {code}). {hint}"))
                } else {
                    Some(format!("Minecraft s’est arrêté (code {code})."))
                }
            }
        }
        Err(e) => Some(e),
    };

    let dto = minecraft_session_report(loadout_id, error, notes);
    if let Ok(mut last) = state.last_session.lock() {
        *last = Some(dto.clone());
    }
    end_session_tracking(&state);
    drop(session);
    let _ = app.emit(GAME_ENDED_EVENT, &dto);
}

/// Secondary path: upsert a ModLab profile in the official / CurseForge
/// launcher and open it. Kept for « Ouvrir le launcher » — not used by Jouer.
#[allow(dead_code)]
fn launch_minecraft_via_official_launcher(
    state: &AppState,
    loadout_id: i64,
) -> Result<modlab_game_minecraft::official_launcher::OfficialLaunchResult, String> {
    let (root, mc, branch_name, loader, launcher_exe, launcher_data) = {
        let core = state.core.lock().map_err(|e| e.to_string())?;
        let branch = loadout::list(&core)
            .map_err(|e| e.to_string())?
            .into_iter()
            .find(|b| b.id == loadout_id)
            .ok_or_else(|| format!("branche {loadout_id} introuvable"))?;
        if branch.game_id != "minecraft" {
            return Err("Cette branche n’est pas une instance Minecraft.".to_string());
        }
        let loader = branch
            .mc_loader
            .clone()
            .unwrap_or_else(|| "vanilla".to_string());
        let mc = branch
            .mc_game_version
            .clone()
            .ok_or_else(|| "Choisissez une version Minecraft.".to_string())?;
        let root = crate::paths::install_target_for_loadout(&core, &state.data_root, loadout_id)?
            .ok_or_else(|| "Cette instance n'a pas de dossier dédié.".to_string())?
            .instance_root;
        let launcher_exe = core
            .game_path_override("minecraft", "launcher_exe")
            .map_err(|e| e.to_string())?
            .filter(|p| !p.trim().is_empty())
            .map(std::path::PathBuf::from);
        let launcher_data = core
            .game_path_override("minecraft", "root")
            .map_err(|e| e.to_string())?
            .filter(|p| !p.trim().is_empty())
            .map(std::path::PathBuf::from);
        (
            root,
            mc,
            branch.name.clone(),
            loader,
            launcher_exe,
            launcher_data,
        )
    };

    let version_id = mc.clone();
    let profile_id = format!("modlab-{loadout_id}");
    let profile_name = format!("ModLab — {branch_name}");
    let req = modlab_game_minecraft::official_launcher::OfficialLaunchRequest {
        game_dir: &root,
        version_id: &version_id,
        profile_id: &profile_id,
        profile_name: &profile_name,
        launcher_exe: launcher_exe.as_deref(),
        launcher_data_dir: launcher_data.as_deref(),
    };
    let mut result = modlab_game_minecraft::official_launcher::play_via_official_launcher(&req)?;
    if loader != "vanilla" {
        result.notice.push_str(&format!(
            " Loader « {loader} » : pour les mods, installez aussi ce loader dans le launcher officiel (versions globales), ou jouez vanilla sur ce profil."
        ));
    }
    Ok(result)
}

#[tauri::command]
pub(crate) async fn launch_skse(app: tauri::AppHandle, loadout_id: i64) -> Result<(), String> {
    #[cfg(not(windows))]
    {
        let _ = (app, loadout_id);
        return Err("le lancement SKSE n'est disponible que sous Windows".into());
    }
    #[cfg(windows)]
    {
        let (started_tx, started_rx) = std::sync::mpsc::channel::<Result<(), String>>();
        std::thread::Builder::new()
            .name("modlab-game-session".into())
            .spawn(move || run_game_session(&app, loadout_id, &started_tx))
            .map_err(|e| e.to_string())?;
        tauri::async_runtime::spawn_blocking(move || started_rx.recv())
            .await
            .map_err(|e| e.to_string())?
            .map_err(|_| "La session de jeu s'est arrêtée avant d'avoir démarré.".to_string())?
    }
}

/// A game process started under a virtual filesystem, not yet finished.
#[cfg(windows)]
struct LaunchedSession {
    vfs: modlab_game_skyrim::usvfs::Vfs,
    instance_name: String,
    started_at: SystemTime,
    /// Where SKSE writes its journals; `None` when it cannot be located.
    log_dir: Option<PathBuf>,
    /// The plugin DLLs the loadout provides, to spot the ones never looked at.
    expected_plugins: Vec<String>,
}

/// Everything from "the human clicked Jouer" up to "the game process exists".
/// Any `Err` is a French message that is shown as is, and nothing was
/// launched. Order matters: SKSE is put in place and verified *before* a
/// virtual filesystem is even created, so a refusal costs nothing.
#[cfg(windows)]
fn start_game_session(state: &AppState, loadout_id: i64) -> Result<LaunchedSession, String> {
    // A human override from Réglages → Jeux must win here — this is the
    // one place that actually launches the game, so it's the one place
    // an ignored override would be most visible (and most confusing).
    let (install_root, game_exe, usvfs_dll, data_dir, rows, log_dir) = {
        let core = state.core.lock().map_err(|e| e.to_string())?;
        let (install_root, game_exe) = skyrim_install_paths(&core)
            .ok_or_else(|| "Skyrim SE introuvable sur cette machine".to_string())?;
        let usvfs_dll = resolve_slot(&core, &SkyrimAdapter, "usvfs_dll").ok_or_else(|| {
            "usvfs_x64.dll introuvable — installe MO2, ou renseigne son chemin dans \
             Réglages → Jeux"
                .to_string()
        })?;
        let data_dir = resolve_slot(&core, &SkyrimAdapter, "data_dir")
            .unwrap_or_else(|| install_root.join("Data"));
        let rows = loadout::entries(&core, loadout_id).map_err(|e| e.to_string())?;
        // SKSE writes its journals next to SkyrimPrefs.ini (Documents).
        let log_dir = resolve_slot(&core, &SkyrimAdapter, "prefs_ini")
            .and_then(|p| p.parent().map(|d| d.join("SKSE")))
            .or_else(modlab_game_skyrim::skse::find_log_dir);
        (install_root, game_exe, usvfs_dll, data_dir, rows, log_dir)
    };

    // SKSE's root files go for real into the game folder (safely: see
    // `engine`), then the folder must really hold a loader and the DLL of
    // this game's version. No silent fallback to the vanilla exe: that would
    // look like a modded launch and load nothing.
    let (engine_report, _status) = modlab_game_skyrim::skse::prepare_launch(
        &rows,
        &install_root,
        modlab_game_skyrim::skse::read_exe_version(&game_exe),
        &engine_store_dir(&state.data_root),
    )?;
    if !engine_report.copied.is_empty() || !engine_report.backed_up.is_empty() {
        let core = state.core.lock().map_err(|e| e.to_string())?;
        let mut tx = transaction::Transaction::new(&core, "skyrim.engine");
        tx.step(format!(
            "avant lancement : {} fichier(s) SKSE copié(s), {} mis de côté",
            engine_report.copied.len(),
            engine_report.backed_up.len()
        ));
        tx.commit().map_err(|e| e.to_string())?;
    }
    let loader = install_root.join(modlab_game_skyrim::engine::LOADER_FILE);

    // The engine mod contributes only its `Data\` part to the virtual layer.
    let vfs_rows = virtually_deployed_rows(&rows);
    let expected_plugins = modlab_game_skyrim::skse::expected_plugins(&vfs_rows);

    // Deterministic per-loadout name, not a shared literal: two loadouts
    // launching independently must never reconnect to the same VFS session
    // (see modlab_game_skyrim::usvfs::instance_name's doc).
    let instance_name = modlab_game_skyrim::usvfs::instance_name("skyrim", loadout_id);
    let vfs = modlab_game_skyrim::usvfs::Vfs::create(&usvfs_dll, &instance_name)
        .map_err(|e| e.to_string())?;
    {
        let core = state.core.lock().map_err(|e| e.to_string())?;
        let mut tx = transaction::Transaction::new(&core, "skyrim.usvfs");
        tx.step(format!("usvfs instance '{instance_name}' connected"));
        tx.commit().map_err(|e| e.to_string())?;
    }
    modlab_game_skyrim::usvfs::deploy_enabled_mods(&vfs, &vfs_rows, &data_dir)
        .map_err(|e| e.to_string())?;
    // Skyrim loads only the plugins its `Plugins.txt` lists. Write the
    // instance's own (in the data root) and link it onto the game's path
    // for the hooked process only — the real AppData file is never touched.
    let game_plugins_txt = plugins::find_game_plugins_txt()
        .ok_or_else(|| "%LOCALAPPDATA% introuvable — impossible de lier plugins.txt".to_string())?;
    let game_ctx = plugins::GameContext {
        install_root: Some(install_root.clone()),
        data_dir: Some(data_dir.clone()),
    };
    let order = modlab_game_skyrim::usvfs::deploy_plugins_txt(
        &vfs,
        &vfs_rows,
        &game_ctx,
        &instance_plugins_txt(&state.data_root, loadout_id),
        &game_plugins_txt,
    )
    .map_err(|e| e.to_string())?;
    {
        let core = state.core.lock().map_err(|e| e.to_string())?;
        let (full, light) = order.active_counts();
        let mut tx = transaction::Transaction::new(&core, "skyrim.plugins_txt");
        tx.step(format!(
            "plugins.txt linked: {full} full + {light} light active, {} issue(s)",
            order.issues.len()
        ));
        tx.commit().map_err(|e| e.to_string())?;
    }

    let started_at = SystemTime::now();
    let _loader_pid = vfs
        .launch_hooked_visible(&format!("\"{}\"", loader.display()), &install_root)
        .map_err(|e| e.to_string())?;
    Ok(LaunchedSession {
        vfs,
        instance_name,
        started_at,
        log_dir,
        expected_plugins,
    })
}

/// The whole life of one session, on its own thread: start it (reporting the
/// outcome of the start on `started`), follow it until the game has fully
/// exited, tear the virtual filesystem down, read SKSE's journals and tell
/// the window.
#[cfg(windows)]
fn run_game_session(
    app: &tauri::AppHandle,
    loadout_id: i64,
    started: &std::sync::mpsc::Sender<Result<(), String>>,
) {
    let state = app.state::<AppState>();
    // One session at a time; claimed here, released when this function ends
    // — on every path, the guard sees to it. While it is set `ai_ask` refuses
    // to run the local LLM (§6) and the engine mod's files are not touched.
    if state
        .game_running
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        let _ = started.send(Err("Une partie est déjà en cours.".to_string()));
        return;
    }
    let session = GameSessionGuard(&state.game_running);
    begin_session_tracking(&state, loadout_id);

    let mut launched = match start_game_session(&state, loadout_id) {
        Ok(l) => l,
        Err(e) => {
            let _ = started.send(Err(e));
            end_session_tracking(&state);
            return;
        }
    };
    let _ = started.send(Ok(()));

    let mut problem = launched
        .vfs
        .wait_for_session_end_cancellable(
            Duration::from_secs(2),
            Duration::from_secs(60),
            Some(&state.session_stop),
        )
        .err()
        .map(|e| e.to_string());

    // Explicit, verified disconnect rather than leaving it to `Drop`: `Drop`
    // can't propagate an error, so a teardown that silently fails there
    // could leave a stale session for the next launch to collide with.
    let disconnect_result = launched.vfs.disconnect();
    if let Ok(core) = state.core.lock() {
        let step = match &disconnect_result {
            Ok(()) => format!(
                "usvfs instance '{}' disconnected (confirmed)",
                launched.instance_name
            ),
            Err(e) => format!(
                "usvfs instance '{}' disconnect FAILED: {e}",
                launched.instance_name
            ),
        };
        let mut tx = transaction::Transaction::new(&core, "skyrim.usvfs");
        tx.step(step);
        let _ = tx.commit();
    }
    if let Err(e) = disconnect_result {
        problem.get_or_insert_with(|| e.to_string());
    }

    let report = match &launched.log_dir {
        Some(dir) => modlab_game_skyrim::skse::read_session_report(
            dir,
            launched.started_at,
            &launched.expected_plugins,
        ),
        None => modlab_game_skyrim::skse::SessionReport::default(),
    };
    let dto = SessionReportDto::new(loadout_id, report, problem);
    if let Ok(mut last) = state.last_session.lock() {
        *last = Some(dto.clone());
    }
    end_session_tracking(&state);
    // The flag drops with the guard; the event comes after it so that a
    // window reacting to it already sees "not running".
    drop(session);
    let _ = app.emit(GAME_ENDED_EVENT, &dto);
}

#[derive(Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphicsSettingsDto {
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub fullscreen: Option<bool>,
    pub vsync: Option<bool>,
    pub antialiasing: Option<u32>,
    pub fxaa: Option<bool>,
}

impl From<modlab_game_skyrim::graphics::GraphicsSettings> for GraphicsSettingsDto {
    fn from(s: modlab_game_skyrim::graphics::GraphicsSettings) -> Self {
        Self {
            width: s.width,
            height: s.height,
            fullscreen: s.fullscreen,
            vsync: s.vsync,
            antialiasing: s.antialiasing,
            fxaa: s.fxaa,
        }
    }
}

impl From<GraphicsSettingsDto> for modlab_game_skyrim::graphics::GraphicsSettings {
    fn from(s: GraphicsSettingsDto) -> Self {
        Self {
            width: s.width,
            height: s.height,
            fullscreen: s.fullscreen,
            vsync: s.vsync,
            antialiasing: s.antialiasing,
            fxaa: s.fxaa,
        }
    }
}

fn prefs_ini_path(core: &Core) -> Result<PathBuf, String> {
    resolve_slot(core, &SkyrimAdapter, "prefs_ini").ok_or_else(|| {
        "SkyrimPrefs.ini introuvable — lance le jeu au moins une fois pour qu'il le crée, \
         ou renseigne son chemin dans Réglages → Jeux"
            .to_string()
    })
}

/// The dashboard's "Graphismes" quick action — read-only (Lecture tier,
/// §7.2). An empty settings object (every field `None`) means the file
/// exists but doesn't set that key yet — the dialog shows the game's own
/// hardcoded default in that case, not a blank field.
#[tauri::command]
pub(crate) fn skyrim_graphics_settings(
    state: State<AppState>,
) -> Result<GraphicsSettingsDto, String> {
    let core = state.core.lock().map_err(|e| e.to_string())?;
    let path = prefs_ini_path(&core)?;
    modlab_game_skyrim::graphics::read(&path)
        .map(GraphicsSettingsDto::from)
        .map_err(|e| e.to_string())
}

/// Mutation réversible tier (§7.2): writes straight to `SkyrimPrefs.ini`,
/// but only the handful of keys this dialog exposes, and every other line
/// in the file is preserved untouched (see `modlab_game_skyrim::graphics`'s
/// doc) — nothing here can corrupt an install, and the previous values are
/// always still readable/restorable by hand if a human wants to undo it.
#[tauri::command]
pub(crate) fn set_skyrim_graphics_settings(
    state: State<AppState>,
    settings: GraphicsSettingsDto,
) -> Result<GraphicsSettingsDto, String> {
    let core = state.core.lock().map_err(|e| e.to_string())?;
    let path = prefs_ini_path(&core)?;
    modlab_game_skyrim::graphics::write(&path, &settings.into()).map_err(|e| e.to_string())?;
    modlab_game_skyrim::graphics::read(&path)
        .map(GraphicsSettingsDto::from)
        .map_err(|e| e.to_string())
}
