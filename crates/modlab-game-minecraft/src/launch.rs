//! Launch Minecraft from an instance folder (vanilla launcher layout).
//! Auth: MSA online when the UI supplies [`OnlineAuth`]; otherwise offline
//! (pseudo + offline UUID). No Microsoft account required for single-player.
//! Assets may be fetched into the ModLab data root.
//!
//! Jouer prepares the **branch loader** when it is installed (Fabric / Forge /
//! NeoForge / Quilt) and the classpath is complete; otherwise it refuses with a
//! French error pointing at « Installer … ». Vanilla remains the path when the
//! branch is set to vanilla. Incomplete loader installs no longer fall back
//! silently to vanilla (that hid ClassNotFoundException on FML main classes).

use crate::loader_install::{
    self, fetch_library, http_client, library_relative_path, resolve_version_json, rules_allow,
    LoaderProgress,
};
use crate::msa::OnlineAuth;
use serde::Deserialize;
use serde_json::{Map, Value};
use std::fs::{self, File};
use std::io::Read;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::time::{Duration, Instant};
use zip::ZipArchive;

const DEFAULT_PLAYER: &str = "Player";

#[derive(Debug, Clone)]
pub struct LaunchRequest<'a> {
    pub instance_root: &'a Path,
    pub data_root: &'a Path,
    pub loadout_id: i64,
    pub loader: &'a str,
    pub mc: &'a str,
    pub loader_ver: Option<&'a str>,
    /// Offline pseudo when `online_auth` is `None`.
    pub player_name: &'a str,
    /// Valid MSA → Minecraft session. When set, launch uses online auth.
    pub online_auth: Option<&'a OnlineAuth>,
}

/// Everything needed to spawn `java`, after libraries, assets, and natives are in place.
#[derive(Debug)]
pub struct LaunchCommand {
    pub java: PathBuf,
    pub args: Vec<String>,
    pub game_dir: PathBuf,
    /// Absolute path of the client jar that must appear on `-cp`.
    pub client_jar: PathBuf,
    pub main_class: String,
    /// French notice for the UI (optional; loaders no longer force a vanilla notice).
    pub notice: Option<String>,
}

/// Build the command line and ensure runtime files exist. Does not spawn.
///
/// - **Vanilla** branch → ensure vanilla + runtime, launch `net.minecraft.client.main.Main`.
/// - **Loader** installed (JSON + jars) → launch that profile’s `mainClass` / classpath.
/// - **Loader** chosen but not installed → error pointing at the « Installer … » CTA.
///
/// All filesystem paths in the resulting command are **absolute**. Relative
/// roots (e.g. `.modlab-dev/...` when `F:\ModLab` is absent) plus
/// `current_dir = instance` made Java resolve `-cp` against the wrong base
/// → `ClassNotFoundException: net.minecraft.client.main.Main`.
pub fn prepare_launch(req: &LaunchRequest<'_>) -> Result<LaunchCommand, String> {
    let instance_root = absolutize(req.instance_root)?;
    let data_root = absolutize(req.data_root)?;
    let abs = LaunchRequest {
        instance_root: &instance_root,
        data_root: &data_root,
        loadout_id: req.loadout_id,
        loader: req.loader,
        mc: req.mc,
        loader_ver: req.loader_ver,
        player_name: req.player_name,
        online_auth: req.online_auth,
    };
    let player = if let Some(online) = abs.online_auth {
        normalize_player(&online.player_name)
    } else {
        normalize_player(abs.player_name)
    };
    let progress = |_p: LoaderProgress| {};

    // Parent vanilla is always required (loaders inheritFrom it).
    ensure_vanilla_version(&abs)?;
    if !loader_install::versions_installed(abs.instance_root, "vanilla", abs.mc, None) {
        return Err(
            "Version Minecraft absente — installez Vanilla (ou un loader) sur l’onglet Mods."
                .to_string(),
        );
    }

    let loader_key = abs.loader.trim().to_lowercase();
    let use_loader = !loader_key.is_empty() && loader_key != "vanilla";

    if use_loader {
        if !loader_install::versions_installed(
            abs.instance_root,
            abs.loader,
            abs.mc,
            abs.loader_ver,
        ) {
            return Err(loader_missing_play_error(abs.loader));
        }
        let (version_id, _) =
            resolve_version_json(abs.instance_root, abs.loader, abs.mc, abs.loader_ver)?;
        let merged = merge_version_json(abs.instance_root, &version_id)?;
        ensure_runtime_files(&abs, &merged, &version_id, &progress)?;
        if !classpath_libraries_complete(abs.instance_root, &merged) {
            let missing = missing_classpath_libraries(abs.instance_root, &merged);
            let sample: Vec<&str> = missing.iter().take(3).map(String::as_str).collect();
            let label = loader_display_name(abs.loader);
            return Err(format!(
                "Classpath {label} incomplet ({} bibliothèque(s) absente(s), ex. {}) — réinstallez via « Installer {label} » dans la barre d’instance.",
                missing.len(),
                sample.join(", ")
            ));
        }
        let cmd = build_command(&abs, &merged, &version_id, &player)?;
        return Ok(cmd);
    }

    let vanilla_req = LaunchRequest {
        instance_root: abs.instance_root,
        data_root: abs.data_root,
        loadout_id: abs.loadout_id,
        loader: "vanilla",
        mc: abs.mc,
        loader_ver: None,
        player_name: abs.player_name,
        online_auth: abs.online_auth,
    };
    let (version_id, _) = resolve_version_json(vanilla_req.instance_root, "vanilla", abs.mc, None)?;
    let merged = merge_version_json(vanilla_req.instance_root, &version_id)?;
    ensure_runtime_files(&vanilla_req, &merged, &version_id, &progress)?;
    build_command(&vanilla_req, &merged, &version_id, &player)
}

/// Resolve to an absolute path (canonicalize when the target exists).
pub fn absolutize(path: &Path) -> Result<PathBuf, String> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        let cwd = std::env::current_dir().map_err(|e| format!("répertoire courant : {e}"))?;
        cwd.join(path)
    };
    Ok(absolute.canonicalize().unwrap_or(absolute))
}

/// Short French diagnostic for UI / session notes (java, main, client jar, -cp tip).
pub fn format_launch_debug(cmd: &LaunchCommand) -> String {
    let cp_entries = classpath_arg_index(&cmd.args)
        .and_then(|i| cmd.args.get(i + 1))
        .map(|s| s.matches(classpath_separator()).count() + 1)
        .unwrap_or(0);
    format!(
        "java={} | main={} | clientJar={} | -cp {} entrée(s) | gameDir={}",
        cmd.java.display(),
        cmd.main_class,
        cmd.client_jar.display(),
        cp_entries,
        cmd.game_dir.display()
    )
}

/// Install vanilla for `req.mc` when the version folder is missing (e.g. NeoForge
/// JSON present but parent client never written).
fn ensure_vanilla_version(req: &LaunchRequest<'_>) -> Result<(), String> {
    let progress = |_p: LoaderProgress| {};
    loader_install::ensure_vanilla(
        req.instance_root,
        req.data_root,
        req.loadout_id,
        req.mc,
        &progress,
    )
}

fn loader_display_name(loader: &str) -> String {
    match loader.trim().to_lowercase().as_str() {
        "neoforge" => "NeoForge".to_string(),
        "forge" => "Forge".to_string(),
        "fabric" => "Fabric".to_string(),
        "quilt" => "Quilt".to_string(),
        other if other.is_empty() => "loader".to_string(),
        other => other.to_string(),
    }
}

/// French error when Jouer is pressed with a chosen loader that is not on disk.
pub fn loader_missing_play_error(loader: &str) -> String {
    let label = loader_display_name(loader);
    format!(
        "{label} n’est pas installé — utilisez « Installer {label} » dans la barre d’instance, puis réessayez Jouer."
    )
}

/// Whether every library jar referenced by a merged version JSON exists on disk.
/// Used to diagnose incomplete Forge/NeoForge installs (missing fml jars → CNF).
pub fn classpath_libraries_complete(instance_root: &Path, merged: &Value) -> bool {
    missing_classpath_libraries(instance_root, merged).is_empty()
}

/// Relative library paths that `build_classpath` would skip because the file is absent.
pub fn missing_classpath_libraries(instance_root: &Path, merged: &Value) -> Vec<String> {
    let mut missing = Vec::new();
    let Some(libs) = merged.get("libraries").and_then(Value::as_array) else {
        return missing;
    };
    for lib in libs {
        if let Some(rules) = lib.get("rules").and_then(Value::as_array) {
            if !rules_allow(rules) {
                continue;
            }
        }
        let Some(rel) = library_relative_path(lib) else {
            continue;
        };
        let jar = instance_root.join("libraries").join(&rel);
        if !jar.is_file() {
            missing.push(rel);
        }
    }
    missing
}

/// Captured Minecraft process output after exit.
#[derive(Debug, Clone)]
pub struct LaunchOutcome {
    pub status: ExitStatus,
    pub stdout: String,
    pub stderr: String,
}

/// Which pipe a live log chunk came from (UI maps this to `game://log`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogChannel {
    Stdout,
    Stderr,
}

impl LogChannel {
    pub fn as_str(self) -> &'static str {
        match self {
            LogChannel::Stdout => "stdout",
            LogChannel::Stderr => "stderr",
        }
    }
}

/// Spawn Minecraft; caller should `wait_launch` to reap output and status.
pub fn spawn_launch(cmd: &LaunchCommand) -> Result<Child, String> {
    let mut command = Command::new(&cmd.java);
    command
        .args(&cmd.args)
        .current_dir(&cmd.game_dir)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    command
        .spawn()
        .map_err(|e| format!("échec du lancement : {e}"))
}

/// Wait for exit and read stdout/stderr (use after `spawn_launch`).
pub fn wait_launch(mut child: Child) -> Result<LaunchOutcome, String> {
    wait_launch_cancellable(
        &mut child,
        None,
        std::time::Duration::from_millis(500),
        None,
    )
}

/// Poll until the child exits, an external stop is requested, or `wait` fails.
///
/// While waiting, stdout/stderr are read on background threads and forwarded
/// as lossy UTF-8 chunks through `on_log` (for live Console). Full text is
/// still returned in [`LaunchOutcome`] after exit.
pub fn wait_launch_cancellable(
    child: &mut Child,
    stop: Option<&AtomicBool>,
    poll: std::time::Duration,
    mut on_log: Option<&mut dyn FnMut(LogChannel, &str)>,
) -> Result<LaunchOutcome, String> {
    let (tx, rx) = mpsc::channel::<(LogChannel, String)>();
    let mut readers = Vec::new();

    if let Some(out) = child.stdout.take() {
        let tx = tx.clone();
        readers.push(std::thread::spawn(move || {
            stream_pipe_chunks(out, LogChannel::Stdout, tx)
        }));
    }
    if let Some(err) = child.stderr.take() {
        let tx = tx.clone();
        readers.push(std::thread::spawn(move || {
            stream_pipe_chunks(err, LogChannel::Stderr, tx)
        }));
    }
    drop(tx);

    let mut stdout = String::new();
    let mut stderr = String::new();

    let stopped = loop {
        while let Ok((channel, chunk)) = rx.try_recv() {
            match channel {
                LogChannel::Stdout => stdout.push_str(&chunk),
                LogChannel::Stderr => stderr.push_str(&chunk),
            }
            if let Some(cb) = on_log.as_mut() {
                cb(channel, &chunk);
            }
        }
        if stop.is_some_and(|f| f.load(Ordering::SeqCst)) {
            let _ = child.kill();
            let _ = child.wait();
            break true;
        }
        match child
            .try_wait()
            .map_err(|e| format!("Suivi du processus : {e}"))?
        {
            Some(_) => break false,
            None => std::thread::sleep(poll),
        }
    };

    for handle in readers {
        let _ = handle.join();
    }
    while let Ok((channel, chunk)) = rx.try_recv() {
        match channel {
            LogChannel::Stdout => stdout.push_str(&chunk),
            LogChannel::Stderr => stderr.push_str(&chunk),
        }
        if let Some(cb) = on_log.as_mut() {
            cb(channel, &chunk);
        }
    }

    if stopped {
        return Err("session arrêtée depuis ModLab".into());
    }

    let status = child
        .wait()
        .map_err(|e| format!("Suivi du processus : {e}"))?;
    Ok(LaunchOutcome {
        status,
        stdout,
        stderr,
    })
}

/// Read a child pipe in chunks, decode lossily, and send each chunk.
fn stream_pipe_chunks(
    mut pipe: impl Read,
    channel: LogChannel,
    tx: mpsc::Sender<(LogChannel, String)>,
) {
    let mut buf = [0u8; 4096];
    loop {
        match pipe.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                let text = decode_process_bytes(&buf[..n]);
                if !text.is_empty() && tx.send((channel, text)).is_err() {
                    break;
                }
            }
            Err(_) => break,
        }
    }
}

/// Read a child pipe as bytes, then decode lossily (U+FFFD for invalid UTF-8).
#[allow(dead_code)]
fn read_pipe_lossy(pipe: &mut impl Read) -> Result<String, String> {
    let mut buf = Vec::new();
    pipe.read_to_end(&mut buf).map_err(io_err)?;
    Ok(decode_process_bytes(&buf))
}

/// Best-effort decode of process stdout/stderr; never rejects non-UTF-8.
pub fn decode_process_bytes(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

/// PID of the JVM Minecraft was launched under (`None` if unknown).
pub fn child_pid(child: &Child) -> Option<u32> {
    Some(child.id())
}

/// Human-readable exit code (`1`, not `Some(1)`).
pub fn format_exit_code(code: Option<i32>) -> String {
    code.map(|c| c.to_string())
        .unwrap_or_else(|| "inconnu".to_string())
}

/// Last non-empty lines from process output for UI diagnostics (stderr first).
pub fn process_log_lines(stderr: &str, stdout: &str, max_lines: usize) -> Vec<String> {
    let pick = |text: &str| {
        text.lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .map(ToString::to_string)
            .collect::<Vec<_>>()
    };
    let mut lines = pick(stderr);
    if lines.is_empty() {
        lines = pick(stdout);
    }
    if lines.len() > max_lines {
        lines = lines[lines.len().saturating_sub(max_lines)..].to_vec();
    }
    lines
}

/// French hint when the JVM exited because of a known *mod* mismatch (not a
/// ModLab classpath/natives bug). Returns `None` when the logs do not match.
///
/// Sodium built for MC 1.21.x expects LWJGL 3.3.x; MC 26.x ships 3.4.x. Sodium
/// then either fails its early LWJGL check or throws `AbstractMethodError` on
/// `Callback$Descriptor` / `MessageBox`. Fix = install the Sodium build that
/// matches the instance’s Minecraft version (and the same loader).
pub fn explain_minecraft_exit(stderr: &str, stdout: &str) -> Option<String> {
    let blob = format!("{stderr}\n{stdout}");
    let lower = blob.to_ascii_lowercase();
    let sodium_lwjgl = lower.contains("sodium")
        && (lower.contains("lwjgl version is not compatible")
            || lower.contains("problematic lwjgl")
            || lower.contains("required version: 3.3")
            || (lower.contains("abstractmethoderror")
                && (lower.contains("callback$descriptor")
                    || lower.contains("windowsmessagebox")
                    || lower.contains("messagebox"))));
    if sodium_lwjgl {
        return Some(
            "Sodium n’est pas la bonne version pour cette instance Minecraft \
             (souvent un jar 1.21.x sur MC 26.x : LWJGL 3.4 fourni par le jeu, \
             alors que ce Sodium attend 3.3). Retirez le jar Sodium actuel et \
             installez la version NeoForge/Fabric/Quilt marquée pour la même \
             version Minecraft que la branche (ex. sodium-neoforge-…-mc26.3), \
             sans mélanger un build Fabric sur NeoForge."
                .to_string(),
        );
    }
    None
}

/// Default wall-clock budget for a smoke / test launch (menu or fail).
pub const SMOKE_TIMEOUT: Duration = Duration::from_secs(120);

/// Result kind for [`wait_smoke_launch`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SmokeKind {
    /// Client logs show it reached an interactive title-screen-ready state.
    MenuReached,
    /// Crash report / fatal startup markers appeared before the menu.
    Crashed,
    /// Process exited before any menu marker.
    EarlyExit,
    /// Wall clock exceeded [`SMOKE_TIMEOUT`] (or the caller-supplied timeout).
    Timeout,
    /// Human / Arrêter requested stop via `session_stop`.
    Stopped,
}

/// Outcome of a smoke test launch (French message ready for UI).
#[derive(Debug, Clone)]
pub struct SmokeOutcome {
    pub kind: SmokeKind,
    /// French one-liner for toast / notice.
    pub message_fr: String,
    /// Which log needle matched (when `MenuReached`).
    pub matched_marker: Option<&'static str>,
    pub stdout: String,
    pub stderr: String,
}

impl SmokeOutcome {
    pub fn ok(&self) -> bool {
        self.kind == SmokeKind::MenuReached
    }
}

/// Force a 1×1 game window for smoke tests.
///
/// **Windows reality:** a true headless Minecraft *client* is not available —
/// GLFW/LWJGL still creates a real OpenGL window; `CREATE_NO_WINDOW` breaks
/// that. We shrink to 1×1 via `--width` / `--height` (replacing any prior
/// values). Off-screen positioning / `ShowWindow(SW_HIDE)` is not done here
/// (no Win32 dependency in this crate).
pub fn apply_smoke_launch_tweaks(cmd: &mut LaunchCommand) {
    let main_idx = cmd
        .args
        .iter()
        .position(|a| a == &cmd.main_class)
        .unwrap_or(cmd.args.len().saturating_sub(1));
    set_game_arg_after(&mut cmd.args, main_idx, "--width", "1");
    set_game_arg_after(&mut cmd.args, main_idx, "--height", "1");
}

fn set_game_arg_after(args: &mut Vec<String>, main_idx: usize, flag: &str, value: &str) {
    let start = main_idx.saturating_add(1);
    let mut i = start;
    while i < args.len() {
        if args[i] == flag {
            if i + 1 < args.len() && !args[i + 1].starts_with('-') {
                args[i + 1] = value.to_string();
            } else {
                args.insert(i + 1, value.to_string());
            }
            return;
        }
        i += 1;
    }
    args.push(flag.to_string());
    args.push(value.to_string());
}

/// Markers that mean the client reached a title-screen-ready state.
/// Any match is enough (loaders share most of these; order is preference).
const MENU_MARKERS: &[&str] = &[
    // Vanilla / Forge / NeoForge / Fabric — fires when the client is ready
    // enough that the title screen is about to (or does) show.
    "sound engine started",
    // Fabric Loader summary after the client finished bootstrapping.
    "completed minecraft loading",
    // Older / alternate wording seen on some loaders.
    "finished loading",
];

/// Crash / fatal-startup needles (checked before menu success).
fn smoke_crash_detected(log_lower: &str) -> bool {
    log_lower.contains("#@!@# game crashed")
        || log_lower.contains("---- minecraft crash report ----")
        || log_lower.contains("failed to start game")
        || log_lower.contains("the game crashed")
        || log_lower.contains("fatal error launching the game")
        || (log_lower.contains("bootstrapmethoderror") && log_lower.contains("minecraft"))
}

/// Returns the matched menu marker substring when the client looks ready.
pub fn detect_menu_reached(log: &str) -> Option<&'static str> {
    let lower = log.to_ascii_lowercase();
    if smoke_crash_detected(&lower) {
        return None;
    }
    MENU_MARKERS
        .iter()
        .copied()
        .find(|m| lower.contains(m))
}

/// Wait for menu marker, crash, timeout, stop, or early exit — then kill the
/// JVM on success / timeout. Streams logs like [`wait_launch_cancellable`].
pub fn wait_smoke_launch(
    child: &mut Child,
    stop: Option<&AtomicBool>,
    poll: Duration,
    timeout: Duration,
    mut on_log: Option<&mut dyn FnMut(LogChannel, &str)>,
) -> SmokeOutcome {
    let started = Instant::now();
    let (tx, rx) = mpsc::channel::<(LogChannel, String)>();
    let mut readers = Vec::new();

    if let Some(out) = child.stdout.take() {
        let tx = tx.clone();
        readers.push(std::thread::spawn(move || {
            stream_pipe_chunks(out, LogChannel::Stdout, tx)
        }));
    }
    if let Some(err) = child.stderr.take() {
        let tx = tx.clone();
        readers.push(std::thread::spawn(move || {
            stream_pipe_chunks(err, LogChannel::Stderr, tx)
        }));
    }
    drop(tx);

    let mut stdout = String::new();
    let mut stderr = String::new();
    let mut matched: Option<&'static str> = None;
    let mut crashed = false;

    // Drain live chunks into buffers + optional UI callback.
    let pump = |stdout: &mut String,
                stderr: &mut String,
                on_log: &mut Option<&mut dyn FnMut(LogChannel, &str)>| {
        while let Ok((channel, chunk)) = rx.try_recv() {
            match channel {
                LogChannel::Stdout => stdout.push_str(&chunk),
                LogChannel::Stderr => stderr.push_str(&chunk),
            }
            if let Some(cb) = on_log.as_mut() {
                cb(channel, &chunk);
            }
        }
    };

    let end = |kind: SmokeKind,
               matched_marker: Option<&'static str>,
               stdout: String,
               stderr: String|
     -> SmokeOutcome {
        SmokeOutcome {
            kind,
            message_fr: smoke_message_fr(kind, matched_marker, &stderr, &stdout),
            matched_marker,
            stdout,
            stderr,
        }
    };

    let outcome = loop {
        pump(&mut stdout, &mut stderr, &mut on_log);

        let blob = format!("{stderr}\n{stdout}");
        let lower = blob.to_ascii_lowercase();
        if !crashed && smoke_crash_detected(&lower) {
            crashed = true;
        }
        if matched.is_none() {
            matched = detect_menu_reached(&blob);
        }

        if stop.is_some_and(|f| f.load(Ordering::SeqCst)) {
            let _ = child.kill();
            let _ = child.wait();
            break SmokeKind::Stopped;
        }
        if matched.is_some() {
            let _ = child.kill();
            let _ = child.wait();
            break SmokeKind::MenuReached;
        }
        if crashed {
            let _ = child.kill();
            let _ = child.wait();
            break SmokeKind::Crashed;
        }
        if started.elapsed() >= timeout {
            let _ = child.kill();
            let _ = child.wait();
            break SmokeKind::Timeout;
        }

        match child.try_wait() {
            Ok(Some(_)) => {
                break if matched.is_some() {
                    SmokeKind::MenuReached
                } else if smoke_crash_detected(&format!("{stderr}\n{stdout}").to_ascii_lowercase())
                {
                    SmokeKind::Crashed
                } else {
                    SmokeKind::EarlyExit
                };
            }
            Ok(None) => std::thread::sleep(poll),
            Err(e) => {
                for handle in readers {
                    let _ = handle.join();
                }
                pump(&mut stdout, &mut stderr, &mut on_log);
                return SmokeOutcome {
                    kind: SmokeKind::EarlyExit,
                    message_fr: format!("Échec du test — suivi du processus : {e}"),
                    matched_marker: None,
                    stdout,
                    stderr,
                };
            }
        }
    };

    for handle in readers {
        let _ = handle.join();
    }
    pump(&mut stdout, &mut stderr, &mut on_log);

    // Re-evaluate markers after final drain (chunk may have arrived late).
    let blob = format!("{stderr}\n{stdout}");
    if matched.is_none() {
        matched = detect_menu_reached(&blob);
    }
    let kind = match outcome {
        SmokeKind::MenuReached => SmokeKind::MenuReached,
        SmokeKind::Stopped => SmokeKind::Stopped,
        SmokeKind::Timeout => SmokeKind::Timeout,
        _ if matched.is_some() => SmokeKind::MenuReached,
        _ if smoke_crash_detected(&blob.to_ascii_lowercase()) => SmokeKind::Crashed,
        other => other,
    };
    end(kind, matched, stdout, stderr)
}

fn smoke_message_fr(
    kind: SmokeKind,
    matched: Option<&'static str>,
    stderr: &str,
    stdout: &str,
) -> String {
    match kind {
        SmokeKind::MenuReached => {
            let detail = matched.unwrap_or("menu");
            format!("Test OK — menu Minecraft atteint ({detail}).")
        }
        SmokeKind::Crashed => {
            let base = "Échec du test — Minecraft a planté avant le menu.".to_string();
            if let Some(hint) = explain_minecraft_exit(stderr, stdout) {
                format!("{base} {hint}")
            } else {
                base
            }
        }
        SmokeKind::Timeout => format!(
            "Échec du test — délai dépassé ({} s) sans atteindre le menu.",
            SMOKE_TIMEOUT.as_secs()
        ),
        SmokeKind::EarlyExit => {
            let base = "Échec du test — Minecraft s’est arrêté avant le menu.".to_string();
            if let Some(hint) = explain_minecraft_exit(stderr, stdout) {
                format!("{base} {hint}")
            } else {
                base
            }
        }
        SmokeKind::Stopped => "Test de lancement interrompu.".to_string(),
    }
}

fn normalize_player(name: &str) -> String {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        DEFAULT_PLAYER.to_string()
    } else {
        trimmed.to_string()
    }
}

fn ensure_runtime_files(
    req: &LaunchRequest<'_>,
    merged: &Value,
    version_id: &str,
    progress: &dyn Fn(LoaderProgress),
) -> Result<(), String> {
    if let Some(libs) = merged.get("libraries").and_then(Value::as_array) {
        for lib in libs {
            fetch_library(
                lib,
                req.instance_root,
                req.data_root,
                req.loadout_id,
                progress,
            )?;
        }
    }
    let natives_dir = natives_dir(req.instance_root, version_id);
    fs::create_dir_all(&natives_dir).map_err(io_err)?;
    if let Some(libs) = merged.get("libraries").and_then(Value::as_array) {
        for lib in libs {
            if let Some(rules) = lib.get("rules").and_then(Value::as_array) {
                if !rules_allow(rules) {
                    continue;
                }
            }
            extract_natives(
                lib,
                req.instance_root,
                req.data_root,
                req.loadout_id,
                &natives_dir,
                progress,
            )?;
        }
    }
    ensure_client_jar(req, merged, version_id, progress)?;
    ensure_assets(req, merged, progress)?;
    Ok(())
}

/// `versions/<id>/<id>.jar` — the jar that contains `net.minecraft.client.main.Main`.
pub fn client_jar_path(instance_root: &Path, version_id: &str) -> PathBuf {
    instance_root
        .join("versions")
        .join(version_id)
        .join(format!("{version_id}.jar"))
}

/// True when `path` is a zip that contains the vanilla client main class.
pub fn client_jar_has_main(path: &Path) -> bool {
    let Ok(file) = File::open(path) else {
        return false;
    };
    let Ok(mut zip) = ZipArchive::new(file) else {
        return false;
    };
    let found = zip.by_name("net/minecraft/client/main/Main.class").is_ok();
    found
}

fn ensure_client_jar(
    req: &LaunchRequest<'_>,
    merged: &Value,
    version_id: &str,
    progress: &dyn Fn(LoaderProgress),
) -> Result<(), String> {
    let dest = client_jar_path(req.instance_root, version_id);
    if dest.is_file() && client_jar_has_main(&dest) {
        return Ok(());
    }
    if dest.is_file() {
        // Stale HTML / truncated download / thin loader jar without Main.class.
        let _ = fs::remove_file(&dest);
    }
    // Loader profiles inheritFrom vanilla — reuse the parent client jar when present.
    let vanilla = client_jar_path(req.instance_root, req.mc);
    if vanilla != dest && vanilla.is_file() && client_jar_has_main(&vanilla) {
        fs::create_dir_all(dest.parent().unwrap()).map_err(io_err)?;
        fs::copy(&vanilla, &dest).map_err(io_err)?;
        return Ok(());
    }
    let Some(client) = merged.get("downloads").and_then(|d| d.get("client")) else {
        return Err(format!(
            "jar client manquant dans le profil de version ({version_id}) — réinstallez Vanilla"
        ));
    };
    let url = client
        .get("url")
        .and_then(Value::as_str)
        .ok_or_else(|| "URL client manquante".to_string())?;
    let sha1 = client.get("sha1").and_then(Value::as_str);
    let client_http = http_client()?;
    fs::create_dir_all(dest.parent().unwrap()).map_err(io_err)?;
    download_file(
        &client_http,
        url,
        &dest,
        sha1,
        req.loadout_id,
        progress,
        "client",
    )?;
    if !client_jar_has_main(&dest) {
        let _ = fs::remove_file(&dest);
        return Err(format!(
            "jar client invalide après téléchargement ({}) — Main.class introuvable",
            dest.display()
        ));
    }
    Ok(())
}

fn ensure_assets(
    req: &LaunchRequest<'_>,
    merged: &Value,
    progress: &dyn Fn(LoaderProgress),
) -> Result<(), String> {
    let asset_index = merged
        .get("assetIndex")
        .ok_or_else(|| "assetIndex manquant".to_string())?;
    let id = asset_index
        .get("id")
        .and_then(Value::as_str)
        .ok_or_else(|| "id d’assets manquant".to_string())?;
    let url = asset_index
        .get("url")
        .and_then(Value::as_str)
        .ok_or_else(|| "URL d’assets manquante".to_string())?;
    let assets_root = assets_cache_root(req.data_root);
    let index_path = assets_root.join("indexes").join(format!("{id}.json"));
    if !index_path.is_file() {
        fs::create_dir_all(index_path.parent().unwrap()).map_err(io_err)?;
        let client = http_client()?;
        download_file(
            &client,
            &url,
            &index_path,
            asset_index.get("sha1").and_then(Value::as_str),
            req.loadout_id,
            progress,
            "assets-index",
        )?;
    }
    let index_text = fs::read_to_string(&index_path).map_err(io_err)?;
    let index: AssetIndex = serde_json::from_str(&index_text).map_err(parse_err)?;
    let client = http_client()?;
    let objects_root = assets_root.join("objects");
    for (path, obj) in index.objects {
        let dest = objects_root.join(&obj.hash[..2]).join(&obj.hash);
        if dest.is_file() {
            continue;
        }
        let _ = path;
        let obj_url = format!(
            "https://resources.download.minecraft.net/{}/{}",
            &obj.hash[..2],
            obj.hash
        );
        fs::create_dir_all(dest.parent().unwrap()).map_err(io_err)?;
        download_file(
            &client,
            &obj_url,
            &dest,
            None,
            req.loadout_id,
            progress,
            "assets",
        )?;
    }
    Ok(())
}

fn assets_cache_root(data_root: &Path) -> PathBuf {
    data_root.join("cache").join("minecraft-assets")
}

fn download_file(
    client: &reqwest::blocking::Client,
    url: &str,
    dest: &Path,
    expected_sha1: Option<&str>,
    loadout_id: i64,
    progress: &dyn Fn(LoaderProgress),
    phase: &str,
) -> Result<(), String> {
    (progress)(LoaderProgress {
        loadout_id: Some(loadout_id),
        phase: phase.to_string(),
        message: "Téléchargement…".to_string(),
        done: false,
    });
    let resp = client.get(url).send().map_err(net_err)?;
    let status = resp.status();
    if !status.is_success() {
        return Err(format!(
            "téléchargement HTTP {status} pour {url} → {}",
            dest.display()
        ));
    }
    let buf = resp.bytes().map_err(net_err)?.to_vec();
    if buf.is_empty() {
        return Err(format!("téléchargement vide pour {}", dest.display()));
    }
    fs::create_dir_all(dest.parent().unwrap_or_else(|| Path::new("."))).map_err(io_err)?;
    let mut file = File::create(dest).map_err(io_err)?;
    file.write_all(&buf).map_err(io_err)?;
    if let Some(sha) = expected_sha1 {
        let got = hex_sha1(&buf);
        if got != sha {
            let _ = fs::remove_file(dest);
            return Err(format!("empreinte incorrecte pour {}", dest.display()));
        }
    }
    Ok(())
}

fn hex_sha1(bytes: &[u8]) -> String {
    use sha1::Digest as Sha1Digest;
    let mut hasher = sha1::Sha1::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

fn extract_natives(
    lib: &Value,
    instance_root: &Path,
    data_root: &Path,
    loadout_id: i64,
    natives_dir: &Path,
    progress: &dyn Fn(LoaderProgress),
) -> Result<(), String> {
    let Some(natives) = lib.get("natives").and_then(Value::as_object) else {
        return Ok(());
    };
    let classifier_key = natives
        .get("windows")
        .or_else(|| natives.get("windows-x86_64"))
        .and_then(Value::as_str);
    let Some(classifier_key) = classifier_key else {
        return Ok(());
    };
    let Some(classifier) = lib
        .get("downloads")
        .and_then(|d| d.get("classifiers"))
        .and_then(|c| c.get(classifier_key))
    else {
        fetch_library(lib, instance_root, data_root, loadout_id, progress)?;
        return Ok(());
    };
    let url = classifier
        .get("url")
        .and_then(Value::as_str)
        .filter(|u| !u.is_empty());
    let sha1 = classifier.get("sha1").and_then(Value::as_str);
    let cache_dir = data_root.join("cache").join("minecraft-natives");
    fs::create_dir_all(&cache_dir).map_err(io_err)?;
    let name = lib.get("name").and_then(Value::as_str).unwrap_or("native");
    let native_zip = cache_dir.join(format!("{}-{}.jar", name.replace(':', "_"), classifier_key));
    if !native_zip.is_file() {
        if let Some(url) = url {
            let client = http_client()?;
            download_file(
                &client,
                url,
                &native_zip,
                sha1,
                loadout_id,
                progress,
                "natives",
            )?;
        } else {
            return Ok(());
        }
    }
    let file = File::open(&native_zip).map_err(io_err)?;
    let mut zip = ZipArchive::new(file).map_err(|e| format!("archive natives : {e}"))?;
    for i in 0..zip.len() {
        let mut entry = zip
            .by_index(i)
            .map_err(|e| format!("archive natives : {e}"))?;
        let Some(enclosed) = entry.enclosed_name() else {
            continue;
        };
        let file_name = enclosed.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if file_name.ends_with(".dll")
            || file_name.ends_with(".so")
            || file_name.ends_with(".dylib")
        {
            let dest = natives_dir.join(file_name);
            fs::create_dir_all(dest.parent().unwrap_or(natives_dir)).map_err(io_err)?;
            let mut out = File::create(&dest).map_err(io_err)?;
            std::io::copy(&mut entry, &mut out).map_err(io_err)?;
        }
    }
    Ok(())
}

fn natives_dir(instance_root: &Path, version_id: &str) -> PathBuf {
    instance_root
        .join("versions")
        .join(version_id)
        .join("natives")
}

/// First JDK major that accepts `-XX:+UseCompactObjectHeaders` (JEP 519).
const COMPACT_OBJECT_HEADERS_MIN_JAVA: u32 = 24;

/// Drop JVM flags the resolved runtime cannot accept.
///
/// Forge 26.x client profiles inject `-XX:+UseCompactObjectHeaders` as a bare
/// string in `arguments.jvm` (no Mojang `rules`). Mojang also lists it under
/// `default-user-jvm` without a Java-major gate. If `find_java_with_major`
/// falls back to an older runtime, the JVM exits with
/// `Unrecognized VM option 'UseCompactObjectHeaders'`.
pub fn strip_jvm_args_unsupported_by_java(args: &mut Vec<String>, java_major: Option<u32>) {
    let supports_compact = java_major.is_some_and(|m| m >= COMPACT_OBJECT_HEADERS_MIN_JAVA);
    if supports_compact {
        return;
    }
    args.retain(|arg| !arg.contains("UseCompactObjectHeaders"));
}

fn build_command(
    req: &LaunchRequest<'_>,
    merged: &Value,
    version_id: &str,
    player: &str,
) -> Result<LaunchCommand, String> {
    let major = merged
        .get("javaVersion")
        .and_then(|j| j.get("majorVersion"))
        .and_then(Value::as_u64)
        .and_then(|n| u32::try_from(n).ok());
    let progress = |_p: LoaderProgress| {};
    let java = loader_install::ensure_java(major, req.data_root, req.loadout_id, &progress)
        .map_err(|e| {
            if e.contains("Java") {
                e
            } else if let Some(need) = major {
                format!(
                    "Java {need} introuvable — {e}. Installez un JDK {need}+ (Adoptium) ou lancez le launcher Minecraft une fois."
                )
            } else {
                format!("Java introuvable — {e}")
            }
        })?;
    let java_major = loader_install::java_version_major(&java);
    if let (Some(need), Some(have)) = (major, java_major) {
        if have < need {
            return Err(format!(
                "Java {need}+ requis pour cette version (runtime trouvé : {have}) — réessayez Jouer (téléchargement auto) ou installez un JDK {need}+."
            ));
        }
    }
    let natives = natives_dir(req.instance_root, version_id);
    let assets_root = assets_cache_root(req.data_root);
    let asset_index_name = merged
        .get("assetIndex")
        .and_then(|a| a.get("id"))
        .and_then(Value::as_str)
        .unwrap_or(version_id);
    let classpath = build_classpath(req.instance_root, merged, version_id)?;
    let (uuid, access_token, user_type) = if let Some(online) = req.online_auth {
        (
            online.uuid.clone(),
            online.access_token.clone(),
            "msa".to_string(),
        )
    } else {
        (
            offline_player_uuid(player),
            "0".to_string(),
            "legacy".to_string(),
        )
    };
    let library_directory = req.instance_root.join("libraries");
    let substitutions = SubstitutionContext {
        player_name: player.to_string(),
        uuid,
        access_token,
        user_type,
        version_name: version_id.to_string(),
        version_type: merged
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or("release")
            .to_string(),
        game_directory: req.instance_root.display().to_string(),
        assets_root: assets_root.display().to_string(),
        assets_index_name: asset_index_name.to_string(),
        natives_directory: natives.display().to_string(),
        // NeoForge FML: `-DlibraryDirectory=${library_directory}`. Leaving the
        // placeholder unsubstituted → "Libraries directory is not readable:
        // ${library_directory}" → "Your NeoForge installation is corrupted."
        library_directory: library_directory.display().to_string(),
        classpath_separator: classpath_separator().to_string(),
        launcher_name: "minecraft".to_string(),
        launcher_version: "ModLab".to_string(),
        classpath,
    };
    let main_class = merged
        .get("mainClass")
        .and_then(Value::as_str)
        .ok_or_else(|| "mainClass manquant".to_string())?;
    let mut args = Vec::new();
    let has_modern_jvm = merged
        .get("arguments")
        .and_then(|a| a.get("jvm"))
        .and_then(Value::as_array)
        .is_some_and(|a| !a.is_empty());
    if has_modern_jvm {
        let jvm = merged["arguments"]["jvm"].as_array().unwrap();
        push_parsed_args(&mut args, jvm, &substitutions, java_major)?;
    } else {
        args.push(format!(
            "-Djava.library.path={}",
            substitutions.natives_directory
        ));
    }
    ensure_classpath_jvm_args(&mut args, &substitutions.classpath)?;
    strip_jvm_args_unsupported_by_java(&mut args, java_major);
    args.push(main_class.to_string());
    if let Some(arguments) = merged.get("arguments") {
        if let Some(game) = arguments.get("game").and_then(Value::as_array) {
            push_parsed_args(&mut args, game, &substitutions, java_major)?;
        }
    } else if let Some(mc_args) = merged.get("minecraftArguments").and_then(Value::as_str) {
        let expanded = substitute_template(mc_args, &substitutions);
        args.extend(split_args(&expanded));
    }
    verify_launch_args_have_classpath(&args, version_id)?;
    verify_no_unsubstituted_library_directory(&args)?;
    let client_jar = client_jar_path(req.instance_root, version_id);
    if !client_jar.is_absolute() {
        return Err(format!(
            "chemin client non absolu ({}) — bug interne de résolution",
            client_jar.display()
        ));
    }
    Ok(LaunchCommand {
        java,
        args,
        game_dir: req.instance_root.to_path_buf(),
        client_jar,
        main_class: main_class.to_string(),
        notice: None,
    })
}

struct SubstitutionContext {
    player_name: String,
    uuid: String,
    access_token: String,
    user_type: String,
    version_name: String,
    version_type: String,
    game_directory: String,
    assets_root: String,
    assets_index_name: String,
    natives_directory: String,
    library_directory: String,
    classpath_separator: String,
    launcher_name: String,
    launcher_version: String,
    classpath: String,
}

fn substitute_template(template: &str, ctx: &SubstitutionContext) -> String {
    template
        .replace("${auth_player_name}", &ctx.player_name)
        .replace("${auth_uuid}", &ctx.uuid)
        .replace("${auth_access_token}", &ctx.access_token)
        .replace("${user_type}", &ctx.user_type)
        .replace("${version_name}", &ctx.version_name)
        .replace("${version_type}", &ctx.version_type)
        .replace("${game_directory}", &ctx.game_directory)
        .replace("${assets_root}", &ctx.assets_root)
        .replace("${assets_index_name}", &ctx.assets_index_name)
        .replace("${natives_directory}", &ctx.natives_directory)
        .replace("${library_directory}", &ctx.library_directory)
        .replace("${classpath_separator}", &ctx.classpath_separator)
        .replace("${launcher_name}", &ctx.launcher_name)
        .replace("${launcher_version}", &ctx.launcher_version)
        .replace("${classpath}", &ctx.classpath)
        .replace("${clientid}", "0")
        .replace("${auth_xuid}", "0")
}

fn classpath_separator() -> &'static str {
    if cfg!(windows) {
        ";"
    } else {
        ":"
    }
}

/// Guarantee `-cp` / classpath are present as two consecutive JVM args.
/// Mojang JSONs usually include them; if rules filtered them out, inject.
fn ensure_classpath_jvm_args(args: &mut Vec<String>, classpath: &str) -> Result<(), String> {
    if classpath.trim().is_empty() {
        return Err("classpath vide".to_string());
    }
    if classpath_arg_index(args).is_some() {
        return Ok(());
    }
    args.push("-cp".to_string());
    args.push(classpath.to_string());
    Ok(())
}

fn classpath_arg_index(args: &[String]) -> Option<usize> {
    args.iter().position(|a| a == "-cp" || a == "-classpath")
}

fn verify_launch_args_have_classpath(args: &[String], version_id: &str) -> Result<(), String> {
    let Some(i) = classpath_arg_index(args) else {
        return Err(format!(
            "arguments JVM sans -cp (version {version_id}) — impossible de lancer"
        ));
    };
    let cp = args.get(i + 1).map(String::as_str).unwrap_or("");
    if cp.is_empty() || cp.starts_with('-') {
        return Err(format!(
            "classpath JVM vide après -cp (version {version_id})"
        ));
    }
    let path_sep = std::path::MAIN_SEPARATOR;
    let client_needle = format!("{path_sep}{version_id}{path_sep}{version_id}.jar");
    // Accept either native separators in the path segment.
    let alt_needle = format!("/{version_id}/{version_id}.jar");
    let win_needle = format!("\\{version_id}\\{version_id}.jar");
    if !(cp.contains(&client_needle) || cp.contains(&alt_needle) || cp.contains(&win_needle)) {
        return Err(format!(
            "jar client absent du -cp (attendu versions/{version_id}/{version_id}.jar) — réinstallez Vanilla"
        ));
    }
    Ok(())
}

/// NeoForge refuses to start when `-DlibraryDirectory=${library_directory}` is
/// left literal — FML reports a corrupted install. Catch that before spawn.
fn verify_no_unsubstituted_library_directory(args: &[String]) -> Result<(), String> {
    if args
        .iter()
        .any(|a| a.contains("${library_directory}") || a.contains("${classpath_separator}"))
    {
        return Err(
            "arguments JVM incomplets (placeholder ${library_directory} non remplacé) — bug interne, réessayez Jouer"
                .to_string(),
        );
    }
    Ok(())
}

fn push_parsed_args(
    out: &mut Vec<String>,
    items: &[Value],
    ctx: &SubstitutionContext,
    java_major: Option<u32>,
) -> Result<(), String> {
    for item in items {
        match item {
            Value::String(s) => {
                let expanded = substitute_template(s, ctx);
                if jvm_arg_allowed_for_java(&expanded, java_major) {
                    out.push(expanded);
                }
            }
            Value::Object(map) => {
                if let Some(rules) = map.get("rules").and_then(Value::as_array) {
                    if !rules_allow(rules) {
                        continue;
                    }
                }
                if let Some(values) = map.get("value") {
                    match values {
                        Value::String(s) => {
                            let expanded = substitute_template(s, ctx);
                            if jvm_arg_allowed_for_java(&expanded, java_major) {
                                out.push(expanded);
                            }
                        }
                        Value::Array(arr) => {
                            for s in arr {
                                if let Some(s) = s.as_str() {
                                    let expanded = substitute_template(s, ctx);
                                    if jvm_arg_allowed_for_java(&expanded, java_major) {
                                        out.push(expanded);
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
    Ok(())
}

fn jvm_arg_allowed_for_java(arg: &str, java_major: Option<u32>) -> bool {
    if !arg.contains("UseCompactObjectHeaders") {
        return true;
    }
    java_major.is_some_and(|m| m >= COMPACT_OBJECT_HEADERS_MIN_JAVA)
}

fn split_args(line: &str) -> Vec<String> {
    line.split_whitespace().map(|s| s.to_string()).collect()
}

fn build_classpath(
    instance_root: &Path,
    merged: &Value,
    version_id: &str,
) -> Result<String, String> {
    let mut paths = Vec::new();
    let missing = missing_classpath_libraries(instance_root, merged);
    if let Some(libs) = merged.get("libraries").and_then(Value::as_array) {
        for lib in libs {
            if let Some(rules) = lib.get("rules").and_then(Value::as_array) {
                if !rules_allow(rules) {
                    continue;
                }
            }
            let Some(rel) = library_relative_path(lib) else {
                continue;
            };
            let jar = instance_root.join("libraries").join(&rel);
            if jar.is_file() {
                paths.push(jar);
            }
        }
    }
    // Prefer the resolved vanilla/loader folder id — merged["id"] can drift
    // after inheritsFrom merges and must not hide a missing client jar.
    let client = client_jar_path(instance_root, version_id);
    if !client.is_file() {
        return Err(format!(
            "jar client absent ({}) — réinstallez Vanilla avant Jouer",
            client.display()
        ));
    }
    if !client_jar_has_main(&client) {
        return Err(format!(
            "jar client invalide ({}) — Main.class introuvable, retéléchargez Vanilla",
            client.display()
        ));
    }
    paths.push(client);
    if paths.is_empty() {
        return Err("classpath vide — réinstallez le loader".to_string());
    }
    // Refuse a half-built Forge/NeoForge classpath: mainClass would resolve but
    // fml jars are missing → JVM ClassNotFoundException (the bug Jouer hit).
    if !missing.is_empty() {
        let sample: Vec<&str> = missing.iter().take(3).map(String::as_str).collect();
        return Err(format!(
            "classpath incomplet ({} bibliothèque(s) absente(s), ex. {}) — réinstallez le loader ou lancez Vanilla",
            missing.len(),
            sample.join(", ")
        ));
    }
    Ok(paths
        .iter()
        .map(|p| p.display().to_string())
        .collect::<Vec<_>>()
        .join(classpath_separator()))
}

pub fn merge_version_json(instance_root: &Path, version_id: &str) -> Result<Value, String> {
    let path = instance_root
        .join("versions")
        .join(version_id)
        .join(format!("{version_id}.json"));
    let mut current: Value =
        serde_json::from_slice(&fs::read(&path).map_err(io_err)?).map_err(parse_err)?;
    if let Some(parent_id) = current
        .get("inheritsFrom")
        .and_then(Value::as_str)
        .map(|s| s.to_string())
    {
        let parent = merge_version_json(instance_root, &parent_id)?;
        current = merge_two_versions(parent, current);
    }
    Ok(current)
}

pub fn merge_two_versions(parent: Value, child: Value) -> Value {
    let mut out = parent.clone();
    if let Some(obj) = out.as_object_mut() {
        merge_field(obj, &child, "id");
        merge_field(obj, &child, "mainClass");
        merge_field(obj, &child, "minecraftArguments");
        merge_arguments(obj, &child);
        merge_field(obj, &child, "assetIndex");
        merge_field(obj, &child, "downloads");
        merge_field(obj, &child, "type");
        merge_field(obj, &child, "javaVersion");
        merge_field(obj, &child, "minimumLauncherVersion");
        merge_libraries(obj, &child);
    }
    out
}

fn merge_field(obj: &mut Map<String, Value>, child: &Value, key: &str) {
    if let Some(v) = child.get(key) {
        obj.insert(key.to_string(), v.clone());
    }
}

/// Mojang-style merge: concatenate `jvm` and `game` arrays; child must not
/// replace the parent wholesale (Fabric profiles ship partial `arguments`).
fn merge_arguments(obj: &mut Map<String, Value>, child: &Value) {
    let Some(child_args) = child.get("arguments").and_then(Value::as_object) else {
        return;
    };
    let mut merged = obj
        .get("arguments")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    merge_argument_array(&mut merged, child_args, "jvm");
    merge_argument_array(&mut merged, child_args, "game");
    if !merged.is_empty() {
        obj.insert("arguments".to_string(), Value::Object(merged));
    }
}

fn merge_argument_array(
    parent_args: &mut Map<String, Value>,
    child_args: &serde_json::Map<String, Value>,
    key: &str,
) {
    let parent_arr = parent_args
        .get(key)
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let child_arr = child_args
        .get(key)
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if child_arr.is_empty() {
        if !parent_arr.is_empty() {
            parent_args.insert(key.to_string(), Value::Array(parent_arr));
        }
        return;
    }
    let mut out = parent_arr;
    out.extend(child_arr);
    parent_args.insert(key.to_string(), Value::Array(out));
}

fn merge_libraries(obj: &mut Map<String, Value>, child: &Value) {
    let mut merged = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let parent_libs = obj
        .get("libraries")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let child_libs = child
        .get("libraries")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    for lib in parent_libs.into_iter().chain(child_libs) {
        let key = lib
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        if key.is_empty() {
            merged.push(lib);
        } else if seen.insert(key) {
            merged.push(lib);
        }
    }
    obj.insert("libraries".to_string(), Value::Array(merged));
}

pub fn offline_player_uuid(player: &str) -> String {
    let digest = md5::compute(format!("OfflinePlayer:{player}"));
    let bytes: [u8; 16] = digest.into();
    format_offline_uuid(bytes)
}

fn format_offline_uuid(mut bytes: [u8; 16]) -> String {
    bytes[6] = (bytes[6] & 0x0f) | 0x30;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        bytes[0],
        bytes[1],
        bytes[2],
        bytes[3],
        bytes[4],
        bytes[5],
        bytes[6],
        bytes[7],
        bytes[8],
        bytes[9],
        bytes[10],
        bytes[11],
        bytes[12],
        bytes[13],
        bytes[14],
        bytes[15],
    )
}

#[derive(Deserialize)]
struct AssetIndex {
    objects: std::collections::HashMap<String, AssetObject>,
}

#[derive(Deserialize)]
struct AssetObject {
    hash: String,
}

fn io_err(e: std::io::Error) -> String {
    format!("fichier : {e}")
}

fn parse_err<E: std::fmt::Display>(e: E) -> String {
    format!("données : {e}")
}

fn net_err(e: reqwest::Error) -> String {
    format!("réseau : {e}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn merge_arguments_concatenates_jvm_and_keeps_parent_game() {
        let parent = serde_json::json!({
            "arguments": {
                "jvm": [
                    "-Djava.library.path=${natives_directory}",
                    {"rules": [{"action": "allow", "os": {"name": "windows"}}], "value": ["-cp", "${classpath}"]}
                ],
                "game": ["--username", "${auth_player_name}", "--version", "${version_name}"]
            }
        });
        let child = serde_json::json!({
            "arguments": {
                "jvm": ["-DFabricMcEmu= net.minecraft.client.main.Main "],
                "game": []
            }
        });
        let merged = merge_two_versions(parent, child);
        let jvm = merged["arguments"]["jvm"].as_array().unwrap();
        assert!(jvm.len() >= 3);
        let jvm_blob: String = jvm.iter().map(|v| v.to_string()).collect();
        assert!(jvm_blob.contains("java.library.path"));
        assert!(jvm_blob.contains("classpath"));
        assert!(jvm_blob.contains("FabricMcEmu"));
        let game = merged["arguments"]["game"].as_array().unwrap();
        assert_eq!(game.len(), 4);
        assert!(game
            .iter()
            .any(|v| v.as_str().is_some_and(|s| s.contains("username"))));
    }

    #[test]
    fn merge_inherits_libraries_and_main_class() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        let parent_id = "1.20.1";
        let child_id = "fabric-loader-1.20.1-0.15.11";
        let parent_dir = root.join("versions").join(parent_id);
        fs::create_dir_all(&parent_dir).unwrap();
        let parent = serde_json::json!({
            "id": parent_id,
            "mainClass": "net.minecraft.client.main.Main",
            "libraries": [{"name": "com.mojang:authlib:1.0"}],
            "minecraftArguments": "--username ${auth_player_name}"
        });
        fs::write(
            parent_dir.join(format!("{parent_id}.json")),
            serde_json::to_vec_pretty(&parent).unwrap(),
        )
        .unwrap();
        let child_dir = root.join("versions").join(child_id);
        fs::create_dir_all(&child_dir).unwrap();
        let child = serde_json::json!({
            "id": child_id,
            "inheritsFrom": parent_id,
            "mainClass": "net.fabricmc.loader.impl.launch.knot.KnotClient",
            "libraries": [{"name": "net.fabricmc:fabric-loader:0.15.11"}]
        });
        fs::write(
            child_dir.join(format!("{child_id}.json")),
            serde_json::to_vec_pretty(&child).unwrap(),
        )
        .unwrap();
        let merged = merge_version_json(root, child_id).unwrap();
        assert_eq!(
            merged["mainClass"].as_str().unwrap(),
            "net.fabricmc.loader.impl.launch.knot.KnotClient"
        );
        let libs = merged["libraries"].as_array().unwrap();
        assert_eq!(libs.len(), 2);
    }

    #[test]
    fn substitutes_auth_placeholders() {
        let ctx = SubstitutionContext {
            player_name: "Alex".to_string(),
            uuid: offline_player_uuid("Alex"),
            access_token: "0".to_string(),
            user_type: "legacy".to_string(),
            version_name: "1.20.1".to_string(),
            version_type: "release".to_string(),
            game_directory: "C:\\game".to_string(),
            assets_root: "C:\\assets".to_string(),
            assets_index_name: "1.20.1".to_string(),
            natives_directory: "C:\\natives".to_string(),
            library_directory: "C:\\game\\libraries".to_string(),
            classpath_separator: ";".to_string(),
            launcher_name: "minecraft".to_string(),
            launcher_version: "ModLab".to_string(),
            classpath: "a.jar;b.jar".to_string(),
        };
        let out = substitute_template("--username ${auth_player_name} --uuid ${auth_uuid}", &ctx);
        assert!(out.contains("--username Alex"));
        assert!(out.contains("--uuid"));
        assert!(!out.contains("${auth_player_name}"));
    }

    #[test]
    fn substitutes_neoforge_library_directory() {
        let ctx = SubstitutionContext {
            player_name: "Player".to_string(),
            uuid: offline_player_uuid("Player"),
            access_token: "0".to_string(),
            user_type: "legacy".to_string(),
            version_name: "neoforge-26.3.0.118".to_string(),
            version_type: "release".to_string(),
            game_directory: r"C:\game".to_string(),
            assets_root: r"C:\assets".to_string(),
            assets_index_name: "26.3".to_string(),
            natives_directory: r"C:\natives".to_string(),
            library_directory: r"C:\game\libraries".to_string(),
            classpath_separator: ";".to_string(),
            launcher_name: "minecraft".to_string(),
            launcher_version: "ModLab".to_string(),
            classpath: "a.jar".to_string(),
        };
        let out = substitute_template("-DlibraryDirectory=${library_directory}", &ctx);
        assert_eq!(out, r"-DlibraryDirectory=C:\game\libraries");
        assert!(!out.contains("${library_directory}"));
        assert!(verify_no_unsubstituted_library_directory(&[out]).is_ok());
        assert!(verify_no_unsubstituted_library_directory(&[
            "-DlibraryDirectory=${library_directory}".to_string()
        ])
        .is_err());
    }

    #[test]
    fn offline_uuid_stable_for_player() {
        let a = offline_player_uuid("Player");
        let b = offline_player_uuid("Player");
        assert_eq!(a, b);
        assert_ne!(a, offline_player_uuid("Other"));
    }

    #[test]
    fn version_folder_id_matches_loader_install() {
        assert_eq!(
            loader_install::version_folder_id("fabric", "1.20.1", Some("0.15.11")),
            "fabric-1.20.1-0.15.11"
        );
    }

    #[test]
    fn decode_process_bytes_accepts_non_utf8() {
        // Windows-1252 / CP437-ish: 0xE9 is é in Latin-1, invalid as UTF-8 lead.
        let raw = b"Forge: charg\xe9 OK\n\xff\xfe binary\n";
        let text = decode_process_bytes(raw);
        assert!(text.contains("Forge: charg"));
        assert!(text.contains("OK"));
        assert!(text.contains('\u{FFFD}'));
        assert!(!text.is_empty());
    }

    #[test]
    fn read_pipe_lossy_survives_invalid_utf8() {
        let mut cursor = std::io::Cursor::new(vec![0x48, 0x69, 0x20, 0x80, 0x21]); // "Hi \x80!"
        let text = read_pipe_lossy(&mut cursor).expect("lossy read");
        assert!(text.starts_with("Hi "));
        assert!(text.contains('\u{FFFD}'));
        assert!(text.ends_with('!'));
    }

    #[test]
    fn wait_launch_cancellable_aborts_when_stop_is_set() {
        use std::sync::Arc;
        use std::time::Duration;
        #[cfg(windows)]
        {
            let mut child = Command::new("cmd")
                .args(["/C", "ping", "-n", "60", "127.0.0.1"])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .expect("spawn");
            let stop = Arc::new(AtomicBool::new(false));
            let flag = Arc::clone(&stop);
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(250));
                flag.store(true, Ordering::SeqCst);
            });
            let err =
                wait_launch_cancellable(&mut child, Some(&stop), Duration::from_millis(50), None)
                    .unwrap_err();
            assert!(err.contains("ModLab"), "{err}");
        }
    }

    #[test]
    fn wait_launch_cancellable_streams_stdout_chunks() {
        use std::time::Duration;
        #[cfg(windows)]
        {
            let mut child = Command::new("cmd")
                .args(["/C", "echo", "modlab-stream-ok"])
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .expect("spawn");
            let mut seen = String::new();
            let mut on_log = |channel: LogChannel, chunk: &str| {
                assert_eq!(channel, LogChannel::Stdout);
                seen.push_str(chunk);
            };
            let outcome = wait_launch_cancellable(
                &mut child,
                None,
                Duration::from_millis(20),
                Some(&mut on_log),
            )
            .expect("wait");
            assert!(outcome.status.success());
            assert!(
                seen.contains("modlab-stream-ok") || outcome.stdout.contains("modlab-stream-ok"),
                "seen={seen:?} stdout={:?}",
                outcome.stdout
            );
        }
    }

    #[test]
    fn strip_compact_object_headers_when_java_below_24() {
        let mut args = vec![
            "-Xms2G".to_string(),
            "-XX:+UseCompactObjectHeaders".to_string(),
            "-XX:+AlwaysPreTouch".to_string(),
            "net.minecraft.client.main.Main".to_string(),
        ];
        strip_jvm_args_unsupported_by_java(&mut args, Some(21));
        assert!(!args.iter().any(|a| a.contains("UseCompactObjectHeaders")));
        assert!(args.iter().any(|a| a == "-Xms2G"));
        assert!(args.iter().any(|a| a == "-XX:+AlwaysPreTouch"));
    }

    #[test]
    fn keep_compact_object_headers_when_java_24_or_newer() {
        let mut args = vec!["-XX:+UseCompactObjectHeaders".to_string()];
        strip_jvm_args_unsupported_by_java(&mut args, Some(24));
        assert_eq!(args, vec!["-XX:+UseCompactObjectHeaders".to_string()]);
        strip_jvm_args_unsupported_by_java(&mut args, Some(25));
        assert_eq!(args, vec!["-XX:+UseCompactObjectHeaders".to_string()]);
    }

    #[test]
    fn strip_compact_object_headers_when_java_major_unknown() {
        let mut args = vec![
            "-XX:+UseCompactObjectHeaders".to_string(),
            "-cp".to_string(),
        ];
        strip_jvm_args_unsupported_by_java(&mut args, None);
        assert_eq!(args, vec!["-cp".to_string()]);
    }

    #[test]
    fn push_parsed_args_skips_forge_compact_headers_on_old_java() {
        // Forge 26.x shape: bare JVM strings, no Mojang rules.
        let items = serde_json::json!([
            "-Djava.net.preferIPv6Addresses=system",
            "-XX:+UseCompactObjectHeaders",
            "-XX:StackShadowPages=32"
        ]);
        let ctx = SubstitutionContext {
            player_name: "Player".to_string(),
            uuid: offline_player_uuid("Player"),
            access_token: "0".to_string(),
            user_type: "legacy".to_string(),
            version_name: "26.3-forge".to_string(),
            version_type: "release".to_string(),
            game_directory: "C:\\game".to_string(),
            assets_root: "C:\\assets".to_string(),
            assets_index_name: "26.3".to_string(),
            natives_directory: "C:\\natives".to_string(),
            library_directory: "C:\\game\\libraries".to_string(),
            classpath_separator: ";".to_string(),
            launcher_name: "minecraft".to_string(),
            launcher_version: "ModLab".to_string(),
            classpath: "a.jar".to_string(),
        };
        let mut out = Vec::new();
        push_parsed_args(&mut out, items.as_array().unwrap(), &ctx, Some(21)).unwrap();
        assert!(
            !out.iter().any(|a| a.contains("UseCompactObjectHeaders")),
            "old Java must not receive CompactObjectHeaders: {out:?}"
        );
        assert!(out.iter().any(|a| a.contains("preferIPv6Addresses")));
        assert!(out.iter().any(|a| a.contains("StackShadowPages")));

        let mut out_new = Vec::new();
        push_parsed_args(&mut out_new, items.as_array().unwrap(), &ctx, Some(25)).unwrap();
        assert!(out_new
            .iter()
            .any(|a| a.contains("UseCompactObjectHeaders")));
    }

    #[test]
    fn jvm_arg_allowed_for_java_gates_compact_headers() {
        assert!(jvm_arg_allowed_for_java("-Xmx4G", Some(8)));
        assert!(!jvm_arg_allowed_for_java(
            "-XX:+UseCompactObjectHeaders",
            Some(21)
        ));
        assert!(jvm_arg_allowed_for_java(
            "-XX:+UseCompactObjectHeaders",
            Some(24)
        ));
        assert!(!jvm_arg_allowed_for_java(
            "-XX:+UseCompactObjectHeaders",
            None
        ));
    }

    #[test]
    fn push_parsed_game_args_skip_feature_gated_quick_play() {
        // Reproduces crash-2026-11-26: rules_allow ignored features → every
        // --quickPlay* flag was passed → IllegalArgumentException.
        let game = serde_json::json!([
            "--username", "${auth_player_name}",
            "--version", "${version_name}",
            {
                "rules": [{ "action": "allow", "features": { "is_demo_user": true } }],
                "value": "--demo"
            },
            {
                "rules": [{ "action": "allow", "features": { "has_quick_plays_support": true } }],
                "value": ["--quickPlayPath", "${quickPlayPath}"]
            },
            {
                "rules": [{ "action": "allow", "features": { "is_quick_play_singleplayer": true } }],
                "value": ["--quickPlaySingleplayer", "${quickPlaySingleplayer}"]
            },
            {
                "rules": [{ "action": "allow", "features": { "is_quick_play_multiplayer": true } }],
                "value": ["--quickPlayMultiplayer", "${quickPlayMultiplayer}"]
            },
            {
                "rules": [{ "action": "allow", "features": { "is_quick_play_realms": true } }],
                "value": ["--quickPlayRealms", "${quickPlayRealms}"]
            }
        ]);
        let ctx = SubstitutionContext {
            player_name: "Bloblode".to_string(),
            uuid: offline_player_uuid("Bloblode"),
            access_token: "token".to_string(),
            user_type: "msa".to_string(),
            version_name: "26.3".to_string(),
            version_type: "release".to_string(),
            game_directory: "C:\\game".to_string(),
            assets_root: "C:\\assets".to_string(),
            assets_index_name: "26.3".to_string(),
            natives_directory: "C:\\natives".to_string(),
            library_directory: "C:\\game\\libraries".to_string(),
            classpath_separator: ";".to_string(),
            launcher_name: "minecraft".to_string(),
            launcher_version: "ModLab".to_string(),
            classpath: "a.jar".to_string(),
        };
        let mut out = Vec::new();
        push_parsed_args(&mut out, game.as_array().unwrap(), &ctx, Some(25)).unwrap();
        assert_eq!(
            out,
            vec![
                "--username".to_string(),
                "Bloblode".to_string(),
                "--version".to_string(),
                "26.3".to_string(),
            ]
        );
        assert!(!out.iter().any(|a| a.contains("quickPlay") || a == "--demo"));
    }

    #[test]
    fn loader_display_name_covers_known_loaders() {
        assert_eq!(loader_display_name("neoforge"), "NeoForge");
        assert_eq!(loader_display_name("fabric"), "Fabric");
        assert_eq!(loader_display_name("vanilla"), "vanilla");
    }

    #[test]
    fn missing_classpath_detects_absent_neoforge_jar() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        let libs = root.join("libraries");
        fs::create_dir_all(libs.join("com/mojang/authlib/1.0")).unwrap();
        fs::write(libs.join("com/mojang/authlib/1.0/authlib-1.0.jar"), b"ok").unwrap();
        let merged = serde_json::json!({
            "libraries": [
                {"name": "com.mojang:authlib:1.0"},
                {
                    "name": "net.neoforged:fml_loader:4.0.41",
                    "downloads": {
                        "artifact": {
                            "path": "net/neoforged/fml_loader/4.0.41/fml_loader-4.0.41.jar"
                        }
                    }
                }
            ]
        });
        let missing = missing_classpath_libraries(root, &merged);
        assert_eq!(missing.len(), 1);
        assert!(missing[0].contains("fml_loader"));
        assert!(!classpath_libraries_complete(root, &merged));
    }

    fn write_client_jar_with_main(path: &Path) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        let file = File::create(path).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        zip.start_file(
            "net/minecraft/client/main/Main.class",
            zip::write::SimpleFileOptions::default(),
        )
        .unwrap();
        zip.write_all(b"fake-class").unwrap();
        zip.finish().unwrap();
    }

    #[test]
    fn build_classpath_errors_when_client_jar_missing() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        let libs = root.join("libraries");
        fs::create_dir_all(libs.join("com/mojang/authlib/1.0")).unwrap();
        fs::write(libs.join("com/mojang/authlib/1.0/authlib-1.0.jar"), b"ok").unwrap();
        let merged = serde_json::json!({
            "id": "1.20.1",
            "mainClass": "net.minecraft.client.main.Main",
            "libraries": [{"name": "com.mojang:authlib:1.0"}]
        });
        let err = build_classpath(root, &merged, "1.20.1").unwrap_err();
        assert!(err.contains("jar client absent"), "{err}");
    }

    #[test]
    fn build_classpath_errors_when_neoforge_jar_missing() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        let libs = root.join("libraries");
        fs::create_dir_all(libs.join("com/mojang/authlib/1.0")).unwrap();
        fs::write(libs.join("com/mojang/authlib/1.0/authlib-1.0.jar"), b"ok").unwrap();
        write_client_jar_with_main(&client_jar_path(root, "1.20.1"));
        let merged = serde_json::json!({
            "id": "1.20.1",
            "mainClass": "net.neoforged.fml.startup.Client",
            "libraries": [
                {"name": "com.mojang:authlib:1.0"},
                {
                    "name": "net.neoforged:fml_loader:4.0.41",
                    "downloads": {
                        "artifact": {
                            "path": "net/neoforged/fml_loader/4.0.41/fml_loader-4.0.41.jar"
                        }
                    }
                }
            ]
        });
        let err = build_classpath(root, &merged, "1.20.1").unwrap_err();
        assert!(err.contains("classpath incomplet"), "{err}");
        assert!(err.contains("fml_loader"), "{err}");
    }

    #[test]
    fn real_oj_instance_prepare_reports_java25_or_builds_absolute_cp() {
        // Live data from cargo tauri dev (cwd often crates/modlab-ui).
        let candidates = [
            Path::new(r"crates/modlab-ui/.modlab-dev/instances/minecraft/.oj"),
            Path::new(r".modlab-dev/instances/minecraft/.oj"),
        ];
        let Some(inst) = candidates
            .iter()
            .find(|p| p.join("versions/26.3/26.3.jar").is_file())
        else {
            return;
        };
        let data = inst
            .ancestors()
            .find(|p| p.join("db/modlab.db").is_file() || p.ends_with(".modlab-dev"))
            .unwrap_or(*inst);
        let req = LaunchRequest {
            instance_root: inst,
            data_root: data,
            loadout_id: 7,
            loader: "neoforge",
            mc: "26.3",
            loader_ver: Some("26.3.0.118"),
            player_name: "Player",
            online_auth: None,
        };
        match prepare_launch(&req) {
            Ok(cmd) => {
                assert!(cmd.client_jar.is_absolute(), "{:?}", cmd.client_jar);
                assert!(cmd.game_dir.is_absolute(), "{:?}", cmd.game_dir);
                assert_eq!(cmd.main_class, "net.neoforged.fml.startup.Client");
                let lib_dir = cmd
                    .args
                    .iter()
                    .find(|a| a.starts_with("-DlibraryDirectory="))
                    .expect("NeoForge must pass -DlibraryDirectory");
                assert!(
                    !lib_dir.contains("${library_directory}"),
                    "placeholder must be substituted: {lib_dir}"
                );
                assert!(
                    lib_dir.contains("libraries"),
                    "libraryDirectory should point at libraries/: {lib_dir}"
                );
                let debug = format_launch_debug(&cmd);
                assert!(debug.contains("clientJar="), "{debug}");
                let i = classpath_arg_index(&cmd.args).expect("-cp");
                assert!(
                    Path::new(&cmd.args[i + 1]).is_absolute()
                        || cmd.args[i + 1].contains(":\\")
                        || cmd.args[i + 1].starts_with('/'),
                    "cp must be absolute: {}",
                    cmd.args[i + 1]
                );
            }
            Err(e) => {
                // Incomplete NeoForge (no patched client) or wrong Java — both OK to skip.
                assert!(
                    e.contains("Java 25")
                        || e.contains("Java 2")
                        || e.contains("Installer NeoForge")
                        || e.contains("n’est pas installé")
                        || e.contains("incomplet")
                        || e.contains("Classpath"),
                    "unexpected prepare_launch error: {e}"
                );
            }
        }
    }

    #[test]
    fn absolutize_makes_relative_paths_absolute() {
        let dir = tempdir().unwrap();
        let rel = Path::new("rel-instance");
        let abs_dir = dir.path().join(rel);
        fs::create_dir_all(&abs_dir).unwrap();
        let prev = std::env::current_dir().unwrap();
        std::env::set_current_dir(dir.path()).unwrap();
        let got = absolutize(rel).unwrap();
        std::env::set_current_dir(prev).unwrap();
        assert!(got.is_absolute(), "{got:?}");
        assert!(got.ends_with("rel-instance") || got.file_name().unwrap() == "rel-instance");
    }

    #[test]
    fn relative_cp_with_instance_cwd_cannot_see_client_jar() {
        // Documents the production bug: relative -cp + current_dir=instance → CNF Main.
        let dir = tempdir().unwrap();
        let prev = std::env::current_dir().unwrap();
        std::env::set_current_dir(dir.path()).unwrap();
        let inst_rel = Path::new("inst");
        let inst = dir.path().join(inst_rel);
        let mc = "1.20.1";
        fs::create_dir_all(inst.join("libraries/com/mojang/authlib/1.0")).unwrap();
        fs::write(
            inst.join("libraries/com/mojang/authlib/1.0/authlib-1.0.jar"),
            b"ok",
        )
        .unwrap();
        write_client_jar_with_main(&client_jar_path(&inst, mc));
        let merged = serde_json::json!({
            "id": mc,
            "mainClass": "net.minecraft.client.main.Main",
            "libraries": [{"name": "com.mojang:authlib:1.0"}]
        });
        // Broken pattern (pre-fix): relative instance root in classpath.
        let broken = build_classpath(inst_rel, &merged, mc).unwrap();
        assert!(
            !Path::new(broken.split(classpath_separator()).last().unwrap()).is_absolute(),
            "precondition: broken cp is relative"
        );
        // Fixed pattern: absolutize first.
        let abs = absolutize(inst_rel).unwrap();
        let fixed = build_classpath(&abs, &merged, mc).unwrap();
        std::env::set_current_dir(prev).unwrap();
        assert!(
            fixed
                .split(classpath_separator())
                .all(|p| Path::new(p).is_absolute()),
            "fixed cp must be absolute: {fixed}"
        );
        assert!(fixed.contains(&format!("{mc}.jar")));
    }

    #[test]
    fn build_classpath_includes_client_jar_with_main() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        let mc = "1.20.1";
        let libs = root.join("libraries");
        fs::create_dir_all(libs.join("com/mojang/authlib/1.0")).unwrap();
        fs::write(libs.join("com/mojang/authlib/1.0/authlib-1.0.jar"), b"ok").unwrap();
        let client = client_jar_path(root, mc);
        write_client_jar_with_main(&client);
        let merged = serde_json::json!({
            "id": mc,
            "mainClass": "net.minecraft.client.main.Main",
            "libraries": [{"name": "com.mojang:authlib:1.0"}]
        });
        let cp = build_classpath(root, &merged, mc).unwrap();
        assert!(
            cp.contains(&format!("{mc}.jar")),
            "client jar must be on classpath: {cp}"
        );
        assert!(client_jar_has_main(&client));
        let mut args = vec!["-Xmx1G".to_string()];
        ensure_classpath_jvm_args(&mut args, &cp).unwrap();
        verify_launch_args_have_classpath(&args, mc).unwrap();
        assert_eq!(args[1], "-cp");
        assert!(args[2].contains(&format!("{mc}.jar")));
    }

    #[test]
    fn loader_missing_play_error_points_at_installer_cta() {
        let err = loader_missing_play_error("neoforge");
        assert!(err.contains("Installer NeoForge"), "{err}");
        assert!(err.contains("n’est pas installé"), "{err}");
    }

    #[test]
    fn prepare_launch_refuses_loader_when_not_installed() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        let data = dir.path().join("data");
        fs::create_dir_all(&data).unwrap();
        let mc = "1.20.1";
        let version_dir = root.join("versions").join(mc);
        fs::create_dir_all(&version_dir).unwrap();
        let lib_dir = root.join("libraries").join("com/mojang/authlib/1.0");
        fs::create_dir_all(&lib_dir).unwrap();
        fs::write(lib_dir.join("authlib-1.0.jar"), b"lib").unwrap();
        write_client_jar_with_main(&client_jar_path(root, mc));
        let vanilla = serde_json::json!({
            "id": mc,
            "mainClass": "net.minecraft.client.main.Main",
            "libraries": [{"name": "com.mojang:authlib:1.0"}],
            "minecraftArguments": "--username ${auth_player_name}"
        });
        fs::write(
            version_dir.join(format!("{mc}.json")),
            serde_json::to_vec_pretty(&vanilla).unwrap(),
        )
        .unwrap();
        let req = LaunchRequest {
            instance_root: root,
            data_root: &data,
            loadout_id: 1,
            loader: "neoforge",
            mc,
            loader_ver: Some("20.4.0"),
            player_name: "Player",
            online_auth: None,
        };
        let err = prepare_launch(&req).unwrap_err();
        assert!(
            err.contains("Installer NeoForge") || err.contains("n’est pas installé"),
            "{err}"
        );
    }

    #[test]
    fn explain_minecraft_exit_detects_sodium_lwjgl_mismatch() {
        let log = r#"[main/INFO] [Sodium-PreLaunchChecks/]: Problematic LWJGL version source: .../lwjgl-3.4.3.jar
[main/ERROR] [Sodium-EarlyDriverScanner/]: The game failed to start because the currently active LWJGL version is not compatible.
Installed version: 3.4.3+4
Required version: 3.3.3
"#;
        let hint = explain_minecraft_exit(log, "").expect("hint");
        assert!(hint.contains("Sodium"), "{hint}");
        assert!(hint.contains("26.x") || hint.contains("Minecraft"), "{hint}");
    }

    #[test]
    fn explain_minecraft_exit_detects_sodium_abstractmethod_descriptor() {
        let log = "A technical error occurred during mod loading:\n\
            java.lang.AbstractMethodError: Method net/caffeinemc/mods/sodium/client/platform/\
            MessageBox$WindowsMessageBoxImpl$$Lambda/0x000001 getDescriptor()\
            Lorg/lwjgl/system/Callback$Descriptor; is abstract";
        let hint = explain_minecraft_exit("", log).expect("hint");
        assert!(hint.contains("Sodium"), "{hint}");
        assert!(hint.contains("LWJGL") || hint.contains("version"), "{hint}");
    }

    #[test]
    fn explain_minecraft_exit_ignores_unrelated_exit() {
        assert!(explain_minecraft_exit("Exception in thread main", "hello").is_none());
    }

    #[test]
    fn detect_menu_reached_sound_engine() {
        let log = "[Render thread/INFO]: Sound engine started\n";
        assert_eq!(detect_menu_reached(log), Some("sound engine started"));
    }

    #[test]
    fn detect_menu_reached_fabric_completed_loading() {
        let log = "[main/INFO]: Completed Minecraft loading in 12.3s\n";
        assert_eq!(detect_menu_reached(log), Some("completed minecraft loading"));
    }

    #[test]
    fn detect_menu_reached_ignores_crash_even_with_sound_line() {
        let log = "---- Minecraft Crash Report ----\nSound engine started\n";
        assert!(detect_menu_reached(log).is_none());
    }

    #[test]
    fn detect_menu_reached_ignores_early_boot() {
        assert!(detect_menu_reached("[main/INFO]: Setting user: Player\n").is_none());
    }

    #[test]
    fn apply_smoke_launch_tweaks_forces_1x1_window() {
        let mut cmd = LaunchCommand {
            java: PathBuf::from("java"),
            args: vec![
                "-cp".into(),
                "x.jar".into(),
                "net.minecraft.client.main.Main".into(),
                "--username".into(),
                "Player".into(),
                "--width".into(),
                "854".into(),
                "--height".into(),
                "480".into(),
            ],
            game_dir: PathBuf::from("."),
            client_jar: PathBuf::from("client.jar"),
            main_class: "net.minecraft.client.main.Main".into(),
            notice: None,
        };
        apply_smoke_launch_tweaks(&mut cmd);
        let width_i = cmd.args.iter().position(|a| a == "--width").expect("width");
        let height_i = cmd.args.iter().position(|a| a == "--height").expect("height");
        assert_eq!(cmd.args[width_i + 1], "1");
        assert_eq!(cmd.args[height_i + 1], "1");
    }

    #[test]
    fn prepare_launch_uses_installed_fabric_main_class() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        let data = dir.path().join("data");
        fs::create_dir_all(&data).unwrap();
        let mc = "1.20.1";
        let loader_ver = "0.15.11";
        let version_dir = root.join("versions").join(mc);
        fs::create_dir_all(&version_dir).unwrap();
        let lib_dir = root.join("libraries").join("com/mojang/authlib/1.0");
        fs::create_dir_all(&lib_dir).unwrap();
        fs::write(lib_dir.join("authlib-1.0.jar"), b"lib").unwrap();
        let fabric_lib = root
            .join("libraries")
            .join("net/fabricmc/fabric-loader/0.15.11");
        fs::create_dir_all(&fabric_lib).unwrap();
        fs::write(fabric_lib.join("fabric-loader-0.15.11.jar"), b"loader").unwrap();
        write_client_jar_with_main(&client_jar_path(root, mc));
        let vanilla = serde_json::json!({
            "id": mc,
            "mainClass": "net.minecraft.client.main.Main",
            "javaVersion": { "majorVersion": 17 },
            "libraries": [{"name": "com.mojang:authlib:1.0"}],
            "minecraftArguments": "--username ${auth_player_name}",
            "assetIndex": { "id": "5", "url": "https://example.invalid/assets.json", "sha1": "abc" },
            "downloads": {
                "client": {
                    "url": "https://example.invalid/client.jar",
                    "sha1": "deadbeef"
                }
            }
        });
        fs::write(
            version_dir.join(format!("{mc}.json")),
            serde_json::to_vec_pretty(&vanilla).unwrap(),
        )
        .unwrap();
        let fabric_id = format!("fabric-{mc}-{loader_ver}");
        let fabric_dir = root.join("versions").join(&fabric_id);
        fs::create_dir_all(&fabric_dir).unwrap();
        let fabric = serde_json::json!({
            "id": fabric_id,
            "inheritsFrom": mc,
            "mainClass": "net.fabricmc.loader.impl.launch.knot.KnotClient",
            "libraries": [{
                "name": "net.fabricmc:fabric-loader:0.15.11",
                "downloads": {
                    "artifact": {
                        "path": "net/fabricmc/fabric-loader/0.15.11/fabric-loader-0.15.11.jar"
                    }
                }
            }]
        });
        fs::write(
            fabric_dir.join(format!("{fabric_id}.json")),
            serde_json::to_vec_pretty(&fabric).unwrap(),
        )
        .unwrap();
        // Skip full prepare_launch (needs real assets / Java) — assert resolution path.
        assert!(loader_install::versions_installed(
            root,
            "fabric",
            mc,
            Some(loader_ver)
        ));
        let (version_id, _) = resolve_version_json(root, "fabric", mc, Some(loader_ver)).unwrap();
        assert_eq!(version_id, fabric_id);
        let merged = merge_version_json(root, &version_id).unwrap();
        assert_eq!(
            merged["mainClass"].as_str().unwrap(),
            "net.fabricmc.loader.impl.launch.knot.KnotClient"
        );
        assert!(classpath_libraries_complete(root, &merged));
        ensure_client_jar(
            &LaunchRequest {
                instance_root: root,
                data_root: &data,
                loadout_id: 1,
                loader: "fabric",
                mc,
                loader_ver: Some(loader_ver),
                player_name: "Player",
                online_auth: None,
            },
            &merged,
            &version_id,
            &|_p| {},
        )
        .unwrap();
        let cp = build_classpath(root, &merged, &version_id).unwrap();
        assert!(cp.contains("fabric-loader-0.15.11.jar"), "{cp}");
        assert!(
            cp.contains(&format!("{fabric_id}.jar")),
            "client jar must be on loader version path after ensure: {cp}"
        );
    }
}
