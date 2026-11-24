//! What SKSE looks like from the outside: is it really in the game folder
//! and does it match the game's version (`detect`), and — after a session —
//! what its own journals say happened (`read_session_report`).
//!
//! Everything here is read-only: nothing in the game folder or in
//! `Documents\My Games\Skyrim Special Edition\SKSE\` is ever written.
//!
//! SKSE is bound to the exact game build: `skse64_loader.exe` reads the
//! version of `SkyrimSE.exe` and loads `skse64_<major>_<minor>_<build>.dll`
//! from its own folder (`skse64_loader.log` shows it: `dll = ...\skse64_1_7_104.dll`).
//! A loader without the matching DLL cannot start SKSE.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

/// A Windows file version (`a.b.c.d`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GameVersion {
    pub major: u16,
    pub minor: u16,
    pub build: u16,
    pub revision: u16,
}

impl GameVersion {
    /// Name of the SKSE runtime DLL this game build needs.
    pub fn skse_dll_name(&self) -> String {
        format!("skse64_{}_{}_{}.dll", self.major, self.minor, self.build)
    }
}

impl std::fmt::Display for GameVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.build)
    }
}

/// The file version of an executable, `None` when it has none (or is not readable).
#[cfg(windows)]
pub fn read_exe_version(exe: &Path) -> Option<GameVersion> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::{
        GetFileVersionInfoSizeW, GetFileVersionInfoW, VerQueryValueW, VS_FIXEDFILEINFO,
    };

    let wide: Vec<u16> = exe
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let root: Vec<u16> = "\\\0".encode_utf16().collect();
    // SAFETY: plain Win32 version-resource calls on buffers owned here; every
    // pointer handed to the API outlives the call, and the fixed-info pointer
    // is only read (unaligned) after its reported length was checked.
    unsafe {
        let mut handle = 0u32;
        let size = GetFileVersionInfoSizeW(wide.as_ptr(), &mut handle);
        if size == 0 {
            return None;
        }
        let mut block = vec![0u8; size as usize];
        if GetFileVersionInfoW(wide.as_ptr(), 0, size, block.as_mut_ptr().cast()) == 0 {
            return None;
        }
        let mut info: *mut std::ffi::c_void = std::ptr::null_mut();
        let mut len = 0u32;
        if VerQueryValueW(block.as_ptr().cast(), root.as_ptr(), &mut info, &mut len) == 0
            || info.is_null()
            || (len as usize) < std::mem::size_of::<VS_FIXEDFILEINFO>()
        {
            return None;
        }
        let fixed = std::ptr::read_unaligned(info as *const VS_FIXEDFILEINFO);
        Some(GameVersion {
            major: (fixed.dwFileVersionMS >> 16) as u16,
            minor: (fixed.dwFileVersionMS & 0xFFFF) as u16,
            build: (fixed.dwFileVersionLS >> 16) as u16,
            revision: (fixed.dwFileVersionLS & 0xFFFF) as u16,
        })
    }
}

#[cfg(not(windows))]
pub fn read_exe_version(_exe: &Path) -> Option<GameVersion> {
    None
}

/// Is SKSE usable with this game?
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkseState {
    /// Loader present and the DLL for the game's version present.
    Ok,
    /// Loader present, but only DLLs for other game versions.
    WrongVersion,
    /// No loader, or no runtime DLL at all.
    Missing,
    /// Everything is there but the game's version could not be read, so the
    /// match cannot be checked.
    Unverifiable,
}

impl SkseState {
    /// Stable code for the UI.
    pub fn code(self) -> &'static str {
        match self {
            SkseState::Ok => "ok",
            SkseState::WrongVersion => "wrong_version",
            SkseState::Missing => "missing",
            SkseState::Unverifiable => "unverifiable",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkseStatus {
    pub state: SkseState,
    /// The game's version, as `1.7.104`.
    pub game_version: Option<String>,
    /// The DLL the loader will look for.
    pub expected_dll: Option<String>,
    pub loader_present: bool,
    /// Every `skse64_X_Y_Z.dll` found in the game folder.
    pub dlls_found: Vec<String>,
    pub install_root: PathBuf,
}

impl SkseStatus {
    /// One sentence for the human, in French.
    pub fn message(&self) -> String {
        let root = self.install_root.display();
        match self.state {
            SkseState::Ok => format!(
                "SKSE est présent et correspond à Skyrim {}.",
                self.game_version.as_deref().unwrap_or("?")
            ),
            SkseState::Unverifiable => {
                "SKSE est présent, mais la version de SkyrimSE.exe n'a pas pu être lue : \
                 impossible de vérifier qu'ils correspondent."
                    .to_string()
            }
            SkseState::WrongVersion => format!(
                "SKSE ne correspond pas à ton Skyrim ({}) : il faut {} et le dossier du jeu \
                 ({root}) ne contient que {}. Installe la version de SKSE qui correspond à ton jeu.",
                self.game_version.as_deref().unwrap_or("?"),
                self.expected_dll.as_deref().unwrap_or("la DLL de SKSE"),
                self.dlls_found.join(", ")
            ),
            SkseState::Missing if !self.loader_present => format!(
                "SKSE est absent : {} est introuvable dans {root}. Active le mod SKSE64 de \
                 ModLab, ou installe SKSE dans le dossier du jeu.",
                crate::engine::LOADER_FILE
            ),
            SkseState::Missing => format!(
                "SKSE est incomplet : {} est là mais aucune DLL skse64_*.dll n'est dans {root}. \
                 Réinstalle SKSE.",
                crate::engine::LOADER_FILE
            ),
        }
    }

    /// `Some(reason)` when a launch must be refused — SKSE absent or built
    /// for another game version. An unverifiable match is not refused: it is
    /// not proven wrong, and the post-session report will tell.
    pub fn launch_refusal(&self) -> Option<String> {
        match self.state {
            SkseState::Missing | SkseState::WrongVersion => {
                Some(format!("Lancement annulé. {}", self.message()))
            }
            SkseState::Ok | SkseState::Unverifiable => None,
        }
    }
}

/// `skse64_1_7_104.dll` → true. `skse64_loader.exe`, `skse64_steam_loader.dll` → false.
fn is_versioned_runtime_dll(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    let Some(middle) = lower
        .strip_prefix("skse64_")
        .and_then(|rest| rest.strip_suffix(".dll"))
    else {
        return false;
    };
    let parts: Vec<&str> = middle.split('_').collect();
    parts.len() == 3
        && parts
            .iter()
            .all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()))
}

/// SKSE's state in `install_root`, for a game whose version is `game_version`.
pub fn detect_with_version(install_root: &Path, game_version: Option<GameVersion>) -> SkseStatus {
    let loader_present = install_root.join(crate::engine::LOADER_FILE).is_file();
    let mut dlls_found: Vec<String> = fs::read_dir(install_root)
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_ok_and(|t| t.is_file()))
        .map(|e| e.file_name().to_string_lossy().to_string())
        .filter(|n| is_versioned_runtime_dll(n))
        .collect();
    dlls_found.sort();

    let expected_dll = game_version.map(|v| v.skse_dll_name());
    let state = if !loader_present || dlls_found.is_empty() {
        SkseState::Missing
    } else {
        match &expected_dll {
            None => SkseState::Unverifiable,
            Some(want) if dlls_found.iter().any(|d| d.eq_ignore_ascii_case(want)) => SkseState::Ok,
            Some(_) => SkseState::WrongVersion,
        }
    };
    SkseStatus {
        state,
        game_version: game_version.map(|v| v.to_string()),
        expected_dll,
        loader_present,
        dlls_found,
        install_root: install_root.to_path_buf(),
    }
}

/// SKSE's state in `install_root`, reading the version off `game_exe`.
pub fn detect(install_root: &Path, game_exe: &Path) -> SkseStatus {
    detect_with_version(install_root, read_exe_version(game_exe))
}

/// `skse64_1_6_1170.dll` → `1.6.1170`. Only for names `is_versioned_runtime_dll` accepts.
fn dll_game_version(name: &str) -> String {
    let lower = name.to_ascii_lowercase();
    lower
        .strip_prefix("skse64_")
        .and_then(|rest| rest.strip_suffix(".dll"))
        .unwrap_or(&lower)
        .replace('_', ".")
}

/// Is the engine mod at `source` (a folder holding `skse64_loader.exe`) built
/// for the game build `game_version`? To be asked *before* any of its files is
/// copied into the game folder: copying a SKSE made for another build over a
/// working one leaves the game without a usable SKSE.
///
/// `Err` is the French reason (the mod's version, the game's version); it has
/// no "Lancement annulé." lead-in, the caller adds what fits its context.
/// `Ok` when the game's version is unreadable (nothing can be compared — the
/// behaviour stays as it was), when the mod ships no versioned runtime DLL (a
/// loader alone has nothing to compare), or when one of its DLLs is the right one.
pub fn check_engine_matches_game(
    mod_name: &str,
    source: &Path,
    game_version: Option<GameVersion>,
) -> Result<(), String> {
    let Some(game) = game_version else {
        return Ok(());
    };
    // An unreadable mod folder is not a version problem: the copy that follows
    // fails on the same read, with its own message and before writing anything.
    let Ok(root_files) = crate::engine::engine_root_files(source) else {
        return Ok(());
    };
    let mut dlls: Vec<String> = root_files
        .into_iter()
        .filter(|n| is_versioned_runtime_dll(n))
        .collect();
    // Folder order is arbitrary: list versions oldest first, numerically
    // (1.9 before 1.10), so the message is the same on every machine.
    dlls.sort_by_key(|d| {
        let version: Vec<u32> = dll_game_version(d)
            .split('.')
            .filter_map(|part| part.parse().ok())
            .collect();
        (version, d.to_ascii_lowercase())
    });
    let wanted = game.skse_dll_name();
    if dlls.is_empty() || dlls.iter().any(|d| d.eq_ignore_ascii_case(&wanted)) {
        return Ok(());
    }
    let mod_versions: Vec<String> = dlls.iter().map(|d| dll_game_version(d)).collect();
    Err(format!(
        "Le mod SKSE « {mod_name} » est fait pour Skyrim {} ({}), mais ton jeu est en version {game} \
         et il faut {wanted}. Rien n'a été copié dans le dossier du jeu : installe la version de \
         SKSE qui correspond à ton jeu.",
        mod_versions.join(", "),
        dlls.join(", "),
    ))
}

/// Everything that must be true before SKSE is launched: check that an
/// enabled engine mod is built for this game version (`check_engine_matches_game`
/// — nothing is copied when it is not), put its root files in place
/// (`engine::ensure_engine_installed` — covers an imported SKSE that was
/// already enabled), then check the game folder really holds a loader and the
/// DLL for `game_version`. `Err` carries the French message to show — never a
/// silent launch of the vanilla exe.
pub fn prepare_launch(
    rows: &[modlab_core::loadout::LoadoutEntryRow],
    install_root: &Path,
    game_version: Option<GameVersion>,
    store: &Path,
) -> Result<(crate::engine::InstallReport, SkseStatus), String> {
    // Every engine mod is checked before the first one is copied, so a
    // refusal never leaves the game folder half-updated.
    for row in crate::engine::enabled_engine_rows(rows) {
        check_engine_matches_game(&row.name, &row.source_path, game_version)
            .map_err(|why| format!("Lancement annulé. {why}"))?;
    }
    let report = crate::engine::ensure_engine_installed(rows, install_root, store)
        .map_err(|e| format!("Impossible de copier SKSE dans le dossier du jeu : {e}"))?;
    let status = detect_with_version(install_root, game_version);
    match status.launch_refusal() {
        Some(reason) => Err(reason),
        None => Ok((report, status)),
    }
}

/// `Documents\My Games\Skyrim Special Edition\SKSE` — where SKSE writes its
/// journals. Same `USERPROFILE`-based lookup as `find_prefs_ini`; the folder
/// is not required to exist (a first launch has not created it yet).
pub fn find_log_dir() -> Option<PathBuf> {
    let profile = std::env::var_os("USERPROFILE")?;
    Some(PathBuf::from(profile).join(r"Documents\My Games\Skyrim Special Edition\SKSE"))
}

/// One SKSE plugin the game refused (or failed) to load, with SKSE's own words.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RejectedPlugin {
    pub name: String,
    pub reason: String,
}

/// What `skse64.log` and `skse64_loader.log` say about one session.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SessionReport {
    /// `skse64.log` exists.
    pub log_found: bool,
    /// ... and was written during this session (not a leftover of an older one).
    pub log_fresh: bool,
    /// SKSE ran its initialisation (`SKSE64 runtime: initialize`).
    pub skse_initialized: bool,
    pub skse_version: Option<String>,
    /// `init complete` was reached.
    pub init_complete: bool,
    /// Plugin DLLs that reported `loaded correctly`.
    pub loaded: Vec<String>,
    pub rejected: Vec<RejectedPlugin>,
    /// Plugins the loadout provides (`Data\SKSE\Plugins\*.dll`) that the log
    /// never mentions at all — SKSE did not even look at them.
    pub not_seen: Vec<String>,
    /// `skse64_loader.log` exists and is from this session.
    pub loader_log_fresh: bool,
    /// The loader got as far as `launching`.
    pub loader_launched: bool,
    /// Lines of the loader journal that look like a problem.
    pub loader_notes: Vec<String>,
}

/// How much older than the session start a journal may be and still count
/// as written by it (clock granularity of the file system).
const FRESHNESS_SLACK: Duration = Duration::from_secs(5);

/// The text of a journal, `None` when missing or older than the session.
/// Journals are ANSI-ish: read as bytes and decoded leniently.
fn read_journal(path: &Path, session_start: SystemTime) -> (bool, Option<String>) {
    let Ok(meta) = fs::metadata(path) else {
        return (false, None);
    };
    let fresh = meta
        .modified()
        .map(|m| m + FRESHNESS_SLACK >= session_start)
        .unwrap_or(false);
    if !fresh {
        return (true, None);
    }
    match fs::read(path) {
        Ok(bytes) => (true, Some(String::from_utf8_lossy(&bytes).to_string())),
        Err(_) => (true, None),
    }
}

/// `C:\...\Plugins\Foo.dll` or `Foo.dll` → `Foo.dll`.
fn base_name(path: &str) -> &str {
    path.rsplit(['\\', '/']).next().unwrap_or(path)
}

/// Parse `plugin <name>.dll (<id> <name> <ver>) <what happened>`.
/// Returns `(file name, what happened)`; `None` for any other line (SKSE
/// also writes `plugin 2 allocated 512 bytes ...`, which has no `.dll`).
fn parse_plugin_line(line: &str) -> Option<(&str, &str)> {
    let rest = line.strip_prefix("plugin ")?;
    let dll_end = rest.to_ascii_lowercase().find(".dll")? + ".dll".len();
    let name = base_name(&rest[..dll_end]);
    let after = rest[dll_end..].trim_start();
    let after = match (after.strip_prefix('('), after.find(')')) {
        (Some(_), Some(close)) => after[close + 1..].trim(),
        _ => after.trim(),
    };
    Some((name, after))
}

/// Parse `couldn't load plugin <path>.dll ...` — SKSE's message when the DLL
/// could not even be loaded. Returns `(file name, what follows the name)`.
fn parse_load_failure(line: &str) -> Option<(&str, &str)> {
    let rest = line.strip_prefix("couldn't load plugin ")?;
    match rest.to_ascii_lowercase().find(".dll") {
        Some(i) => Some((
            base_name(&rest[..i + 4]),
            rest[i + 4..].trim_start_matches([' ', ':', '-']).trim(),
        )),
        None => Some((base_name(rest.trim()), "")),
    }
}

/// Words that make a loader journal line worth showing.
const LOADER_PROBLEM_WORDS: &[&str] = &[
    "fail",
    "error",
    "couldn't",
    "could not",
    "not found",
    "mismatch",
    "unsupported",
    "not supported",
    "unknown version",
];

fn parse_skse_log(text: &str, report: &mut SessionReport) {
    let mut seen: Vec<String> = Vec::new();
    for line in text.lines().map(str::trim) {
        if let Some(init) = line.strip_prefix("SKSE64 runtime: initialize") {
            report.skse_initialized = true;
            report.skse_version = init
                .split("version = ")
                .nth(1)
                .and_then(|v| v.split_whitespace().next())
                .map(str::to_string);
        } else if line == "init complete" {
            report.init_complete = true;
        } else if let Some(name) = line.strip_prefix("checking plugin ") {
            seen.push(base_name(name.trim()).to_ascii_lowercase());
        } else if let Some((name, reason)) = parse_load_failure(line) {
            seen.push(name.to_ascii_lowercase());
            report.rejected.push(RejectedPlugin {
                name: name.to_string(),
                reason: if reason.is_empty() {
                    line.to_string()
                } else {
                    reason.to_string()
                },
            });
        } else if let Some((name, what)) = parse_plugin_line(line) {
            seen.push(name.to_ascii_lowercase());
            if what.starts_with("loaded correctly") {
                report.loaded.push(name.to_string());
            } else {
                report.rejected.push(RejectedPlugin {
                    name: name.to_string(),
                    reason: what.to_string(),
                });
            }
        }
    }
    report.not_seen = std::mem::take(&mut report.not_seen)
        .into_iter()
        .filter(|expected| !seen.contains(&expected.to_ascii_lowercase()))
        .collect();
}

fn parse_loader_log(text: &str, report: &mut SessionReport) {
    for line in text.lines().map(str::trim) {
        if line == "launching" {
            report.loader_launched = true;
        }
        let lower = line.to_ascii_lowercase();
        if LOADER_PROBLEM_WORDS.iter().any(|w| lower.contains(w)) {
            report.loader_notes.push(line.to_string());
        }
    }
}

/// Read what SKSE wrote during the session that started at `session_start`.
/// `expected_plugins` are the plugin DLL names the loadout provides (see
/// `expected_plugins`), used to spot the ones SKSE never looked at. A journal
/// older than the session is not read at all: its content would describe an
/// earlier run.
pub fn read_session_report(
    log_dir: &Path,
    session_start: SystemTime,
    expected_plugins: &[String],
) -> SessionReport {
    let mut report = SessionReport {
        not_seen: expected_plugins.to_vec(),
        ..SessionReport::default()
    };

    let (found, text) = read_journal(&log_dir.join("skse64.log"), session_start);
    report.log_found = found;
    if let Some(text) = text {
        report.log_fresh = true;
        parse_skse_log(&text, &mut report);
    } else {
        // Nothing was read, so nothing can be claimed about the plugins.
        report.not_seen.clear();
    }

    let (_, text) = read_journal(&log_dir.join("skse64_loader.log"), session_start);
    if let Some(text) = text {
        report.loader_log_fresh = true;
        parse_loader_log(&text, &mut report);
    }
    report
}

/// The plugin DLL names (`Data\SKSE\Plugins\*.dll`) the given rows provide —
/// pass the rows USVFS deploys (`engine::virtual_rows`). Enabled mods only;
/// each name once, as written by the first mod that has it.
pub fn expected_plugins(rows: &[modlab_core::loadout::LoadoutEntryRow]) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    for row in rows.iter().filter(|r| r.enabled) {
        for name in plugin_dlls(&row.source_path).unwrap_or_default() {
            if !names.iter().any(|n| n.eq_ignore_ascii_case(&name)) {
                names.push(name);
            }
        }
    }
    names.sort_by_key(|n| n.to_ascii_lowercase());
    names
}

fn child_dir(dir: &Path, name: &str) -> Option<PathBuf> {
    fs::read_dir(dir)
        .ok()?
        .filter_map(|e| e.ok())
        .find(|e| {
            e.file_name().to_string_lossy().eq_ignore_ascii_case(name)
                && e.file_type().is_ok_and(|t| t.is_dir())
        })
        .map(|e| e.path())
}

fn plugin_dlls(mod_dir: &Path) -> io::Result<Vec<String>> {
    let Some(plugins) = child_dir(mod_dir, "SKSE").and_then(|d| child_dir(&d, "Plugins")) else {
        return Ok(Vec::new());
    };
    let mut out = Vec::new();
    for entry in fs::read_dir(plugins)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().to_string();
        if entry.file_type()?.is_file() && name.to_ascii_lowercase().ends_with(".dll") {
            out.push(name);
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use modlab_core::loadout::{LoadoutEntryRow, STATUS_INSTALLED};

    fn v(major: u16, minor: u16, build: u16) -> GameVersion {
        GameVersion {
            major,
            minor,
            build,
            revision: 0,
        }
    }

    fn fake_game(files: &[&str]) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        for f in files {
            fs::write(dir.path().join(f), b"fake").unwrap();
        }
        dir
    }

    #[test]
    fn dll_name_follows_the_game_build() {
        assert_eq!(v(1, 7, 104).skse_dll_name(), "skse64_1_7_104.dll");
        assert_eq!(v(1, 6, 1170).skse_dll_name(), "skse64_1_6_1170.dll");
        assert_eq!(v(1, 7, 104).to_string(), "1.7.104");
    }

    #[test]
    fn present_with_the_matching_dll_is_ok() {
        let game = fake_game(&["SkyrimSE.exe", "skse64_loader.exe", "skse64_1_7_104.dll"]);
        let status = detect_with_version(game.path(), Some(v(1, 7, 104)));
        assert_eq!(status.state, SkseState::Ok);
        assert_eq!(status.launch_refusal(), None);
        assert_eq!(status.game_version.as_deref(), Some("1.7.104"));
    }

    #[test]
    fn present_with_another_versions_dll_is_wrong_version_and_refused() {
        let game = fake_game(&["skse64_loader.exe", "skse64_1_6_1170.dll"]);
        let status = detect_with_version(game.path(), Some(v(1, 7, 104)));
        assert_eq!(status.state, SkseState::WrongVersion);
        let refusal = status.launch_refusal().expect("must be refused");
        assert!(refusal.contains("skse64_1_7_104.dll"), "{refusal}");
        assert!(refusal.contains("skse64_1_6_1170.dll"), "{refusal}");
    }

    #[test]
    fn no_loader_is_missing_and_refused() {
        let game = fake_game(&["SkyrimSE.exe", "skse64_1_7_104.dll"]);
        let status = detect_with_version(game.path(), Some(v(1, 7, 104)));
        assert_eq!(status.state, SkseState::Missing);
        assert!(!status.loader_present);
        assert!(status
            .launch_refusal()
            .unwrap()
            .contains("skse64_loader.exe"));
    }

    #[test]
    fn a_loader_without_any_runtime_dll_is_missing() {
        let game = fake_game(&["skse64_loader.exe", "skse64_steam_loader.dll"]);
        let status = detect_with_version(game.path(), Some(v(1, 7, 104)));
        assert_eq!(status.state, SkseState::Missing);
        assert!(status.loader_present);
        assert!(status.dlls_found.is_empty());
    }

    #[test]
    fn an_unreadable_game_version_is_unverifiable_and_not_refused() {
        let game = fake_game(&["skse64_loader.exe", "skse64_1_7_104.dll"]);
        let status = detect_with_version(game.path(), None);
        assert_eq!(status.state, SkseState::Unverifiable);
        assert_eq!(status.launch_refusal(), None);
    }

    #[test]
    fn an_empty_game_folder_is_missing() {
        let game = fake_game(&[]);
        assert_eq!(
            detect_with_version(game.path(), Some(v(1, 7, 104))).state,
            SkseState::Missing
        );
    }

    #[cfg(windows)]
    #[test]
    fn reads_a_real_file_version_from_a_system_dll() {
        // Read-only look at a Windows system file; the game is never touched.
        let root = std::env::var_os("SystemRoot").expect("SystemRoot");
        let kernel32 = PathBuf::from(root).join(r"System32\kernel32.dll");
        let version = read_exe_version(&kernel32).expect("kernel32 has a version resource");
        assert!(version.major >= 6, "{version:?}");
        assert!(read_exe_version(Path::new(r"C:\no\such\file.exe")).is_none());
    }

    // The two logs below are the real ones of a session on Skyrim 1.7.104
    // with SKSE 2.3.1 and no plugin found, with the user's name and paths
    // anonymised (kept verbatim otherwise).
    const REAL_SKSE_LOG_NO_PLUGINS: &str = "\
SKSE64 runtime: initialize (version = 2.3.1 01070680 01DD4A23C552E1A6, os = 6.2 (9200))
imagebase = 00007FF7CB070000
reloc mgr imagebase = 00007FF7CB070000
plugin directory = X:\\Games\\Skyrim Special Edition\\Data\\SKSE\\Plugins\\
scanning plugin directory X:\\Games\\Skyrim Special Edition\\Data\\SKSE\\Plugins\\
preinit complete
config path = X:\\Games\\Skyrim Special Edition\\Data\\SKSE\\skse.ini
dispatch message (0) to plugin listeners
dispatched message.
dispatch message (1) to plugin listeners
dispatched message.
init complete
hooked dinput
dispatch message (6) to plugin listeners
dispatched message.
dispatch message (8) to plugin listeners
dispatched message.
";

    const REAL_LOADER_LOG: &str = "\
skse64 loader 02030010 01DD4A23C51DD17E 6.2 (9200)
config path = X:\\Games\\Skyrim Special Edition\\Data\\SKSE\\skse.ini
procPath = X:\\Games\\Skyrim Special Edition\\SkyrimSE.exe
launching: SkyrimSE.exe (X:\\Games\\Skyrim Special Edition\\SkyrimSE.exe)
dwFileVersionMS = 00010007
dwFileVersionLS = 00680000
productVersionStr = 1.7.104.0
version = 0001000700680000
product name = TESV: Skyrim
steam exe
dll = X:\\Games\\Skyrim Special Edition\\skse64_1_7_104.dll
WinVerifyTrust: 00000000 00000000 X:\\Games\\Skyrim Special Edition\\skse64_loader.exe
WinVerifyTrust: 00000000 00000000 X:\\Games\\Skyrim Special Edition\\skse64_1_7_104.dll
DLL and EXE signature/serial match
main thread id = 16928
hook thread complete
launching
";

    // Real session of SKSE 2.2.8 with two plugins that loaded fine (the
    // "loaded correctly" and "checking plugin" lines are verbatim).
    // The rejection lines are NOT from a real journal: they follow SKSE's
    // source messages as remembered, and only exercise the parser.
    const SKSE_LOG_WITH_PLUGINS: &str = "\
SKSE64 runtime: initialize (version = 2.2.8 01064920 01DD4343674E73E6, os = 6.2 (9200))
plugin directory = X:\\Games\\Skyrim Special Edition\\Data\\SKSE\\Plugins\\
scanning plugin directory X:\\Games\\Skyrim Special Edition\\Data\\SKSE\\Plugins\\
checking plugin DynamicShaderFrameGen.dll
checking plugin SSEDisplayTweaks.dll
checking plugin Broken.dll
checking plugin Old.dll
preinit complete
loading plugin \"DynamicShaderFrameGen\"
plugin 2 allocated 512 bytes from branch pool
plugin DynamicShaderFrameGen.dll (00000001 DynamicShaderFrameGen 01000000) loaded correctly (handle 1)
plugin SSEDisplayTweaks.dll (00000001 SSEDisplayTweaks 00000519) loaded correctly (handle 2)
plugin Old.dll (00000001 Old 00000001) reported as incompatible during load
couldn't load plugin X:\\Games\\Skyrim Special Edition\\Data\\SKSE\\Plugins\\Broken.dll (Win32 error 126)
init complete
";

    fn session_files(skse_log: &str, loader_log: &str) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("skse64.log"), skse_log).unwrap();
        fs::write(dir.path().join("skse64_loader.log"), loader_log).unwrap();
        dir
    }

    fn just_before_now() -> SystemTime {
        SystemTime::now() - Duration::from_secs(60)
    }

    #[test]
    fn the_real_no_plugin_session_reports_skse_up_but_nothing_loaded() {
        let dir = session_files(REAL_SKSE_LOG_NO_PLUGINS, REAL_LOADER_LOG);

        let report = read_session_report(
            dir.path(),
            just_before_now(),
            &["x.dll".to_string(), "Other.dll".to_string()],
        );

        assert!(report.log_found && report.log_fresh);
        assert!(report.skse_initialized);
        assert_eq!(report.skse_version.as_deref(), Some("2.3.1"));
        assert!(report.init_complete);
        assert!(report.loaded.is_empty());
        assert!(report.rejected.is_empty());
        // The loadout provides two plugins; the journal never mentions them.
        assert_eq!(report.not_seen, ["x.dll", "Other.dll"]);
        assert!(report.loader_log_fresh && report.loader_launched);
        assert!(report.loader_notes.is_empty(), "{:?}", report.loader_notes);
    }

    #[test]
    fn loaded_and_rejected_plugins_are_told_apart() {
        let dir = session_files(SKSE_LOG_WITH_PLUGINS, REAL_LOADER_LOG);

        let report = read_session_report(
            dir.path(),
            just_before_now(),
            &[
                "dynamicshaderframegen.dll".to_string(),
                "SSEDisplayTweaks.dll".to_string(),
                "Broken.dll".to_string(),
                "Old.dll".to_string(),
                "Never.dll".to_string(),
            ],
        );

        assert_eq!(
            report.loaded,
            ["DynamicShaderFrameGen.dll", "SSEDisplayTweaks.dll"]
        );
        assert_eq!(
            report.rejected,
            [
                RejectedPlugin {
                    name: "Old.dll".into(),
                    reason: "reported as incompatible during load".into()
                },
                RejectedPlugin {
                    name: "Broken.dll".into(),
                    reason: "(Win32 error 126)".into()
                },
            ]
        );
        assert_eq!(report.not_seen, ["Never.dll"]);
        assert_eq!(report.skse_version.as_deref(), Some("2.2.8"));
    }

    #[test]
    fn a_journal_older_than_the_session_is_not_read() {
        let dir = session_files(SKSE_LOG_WITH_PLUGINS, REAL_LOADER_LOG);
        // A session that starts an hour from now: both files predate it.
        let later = SystemTime::now() + Duration::from_secs(3600);

        let report = read_session_report(dir.path(), later, &["x.dll".to_string()]);

        assert!(report.log_found);
        assert!(!report.log_fresh);
        assert!(!report.skse_initialized);
        assert!(report.loaded.is_empty());
        assert!(report.not_seen.is_empty());
        assert!(!report.loader_log_fresh);
    }

    #[test]
    fn missing_journals_are_reported_as_not_found() {
        let dir = tempfile::tempdir().unwrap();
        let report = read_session_report(dir.path(), just_before_now(), &[]);
        assert!(!report.log_found);
        assert!(!report.log_fresh);
        assert!(!report.skse_initialized);
    }

    #[test]
    fn loader_problems_are_surfaced() {
        let dir = session_files(
            REAL_SKSE_LOG_NO_PLUGINS,
            "skse64 loader 02030010\ncouldn't find skse64_1_7_104.dll\nDLL and EXE signature/serial match\n",
        );
        let report = read_session_report(dir.path(), just_before_now(), &[]);
        assert!(!report.loader_launched);
        assert_eq!(report.loader_notes, ["couldn't find skse64_1_7_104.dll"]);
    }

    fn fake_skse_mod_for_1_7_104() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("skse64_loader.exe"), b"fake loader").unwrap();
        fs::write(dir.path().join("skse64_1_7_104.dll"), b"fake runtime").unwrap();
        fs::create_dir_all(dir.path().join("Data/Scripts")).unwrap();
        fs::write(dir.path().join("Data/Scripts/Actor.pex"), b"fake pex").unwrap();
        dir
    }

    #[test]
    fn prepare_launch_copies_an_already_enabled_engine_then_accepts() {
        let skse = fake_skse_mod_for_1_7_104();
        let game = fake_game(&["SkyrimSE.exe"]);
        let store = tempfile::tempdir().unwrap();
        let rows = [row("SKSE64", skse.path(), true)];

        let (report, status) =
            prepare_launch(&rows, game.path(), Some(v(1, 7, 104)), store.path()).unwrap();

        assert_eq!(report.copied.len(), 2);
        assert_eq!(status.state, SkseState::Ok);
        assert!(game.path().join("skse64_loader.exe").is_file());
    }

    #[test]
    fn prepare_launch_refuses_without_skse_and_launches_nothing() {
        let game = fake_game(&["SkyrimSE.exe"]);
        let store = tempfile::tempdir().unwrap();

        let err = prepare_launch(&[], game.path(), Some(v(1, 7, 104)), store.path()).unwrap_err();

        assert!(err.starts_with("Lancement annulé."), "{err}");
        assert!(err.contains("SKSE est absent"), "{err}");
    }

    #[test]
    fn prepare_launch_refuses_an_engine_built_for_another_game_version() {
        let skse = fake_skse_mod_for_1_7_104();
        let game = fake_game(&["SkyrimSE.exe"]);
        let store = tempfile::tempdir().unwrap();
        let rows = [row("SKSE64", skse.path(), true)];

        let err =
            prepare_launch(&rows, game.path(), Some(v(1, 6, 1170)), store.path()).unwrap_err();

        assert!(err.contains("skse64_1_6_1170.dll"), "{err}");
        assert!(err.contains("skse64_1_7_104.dll"), "{err}");
    }

    /// A game folder with a SKSE that is right for 1.6.1170, and the store
    /// beside it: the "before" picture a refused launch must leave intact.
    fn game_with_working_skse_1_6_1170() -> (tempfile::TempDir, tempfile::TempDir) {
        let game = fake_game(&["SkyrimSE.exe"]);
        fs::write(game.path().join("skse64_loader.exe"), b"working loader").unwrap();
        fs::write(game.path().join("skse64_1_6_1170.dll"), b"working runtime").unwrap();
        (game, tempfile::tempdir().unwrap())
    }

    fn assert_game_folder_untouched(game: &Path, store: &Path) {
        assert_eq!(
            fs::read(game.join("skse64_loader.exe")).unwrap(),
            b"working loader"
        );
        assert_eq!(
            fs::read(game.join("skse64_1_6_1170.dll")).unwrap(),
            b"working runtime"
        );
        assert!(
            !game.join("skse64_1_7_104.dll").exists(),
            "the mod's DLL must not be copied"
        );
        let mut names: Vec<_> = fs::read_dir(game)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().to_string())
            .collect();
        names.sort();
        assert_eq!(
            names,
            ["SkyrimSE.exe", "skse64_1_6_1170.dll", "skse64_loader.exe"]
        );
        assert_eq!(
            fs::read_dir(store).unwrap().count(),
            0,
            "no backup, manifest or retired folder is created by a refusal"
        );
    }

    #[test]
    fn a_wrong_version_engine_is_refused_before_anything_is_copied() {
        // The mod is built for 1.7.104, the game is 1.6.1170 and already holds
        // a working SKSE: nothing may be copied over it.
        let skse = fake_skse_mod_for_1_7_104();
        let (game, store) = game_with_working_skse_1_6_1170();
        let rows = [row("SKSE64", skse.path(), true)];

        let err =
            prepare_launch(&rows, game.path(), Some(v(1, 6, 1170)), store.path()).unwrap_err();

        assert!(err.starts_with("Lancement annulé."), "{err}");
        assert!(err.contains("SKSE64"), "{err}");
        assert!(err.contains("1.7.104"), "mod's version: {err}");
        assert!(err.contains("1.6.1170"), "game's version: {err}");
        assert!(err.contains("Rien n'a été copié"), "{err}");
        assert_game_folder_untouched(game.path(), store.path());
    }

    #[test]
    fn a_refusal_for_one_engine_mod_copies_none_of_them() {
        // Two engine mods enabled: the first is fine for the game, the second
        // is not. The check runs over all of them before the first copy.
        let right = tempfile::tempdir().unwrap();
        fs::write(right.path().join("skse64_loader.exe"), b"right loader").unwrap();
        fs::write(right.path().join("skse64_1_6_1170.dll"), b"right runtime").unwrap();
        let wrong = fake_skse_mod_for_1_7_104();
        let game = fake_game(&["SkyrimSE.exe"]);
        let store = tempfile::tempdir().unwrap();
        let rows = [
            row("SKSE 1.6", right.path(), true),
            row("SKSE 1.7", wrong.path(), true),
        ];

        let err =
            prepare_launch(&rows, game.path(), Some(v(1, 6, 1170)), store.path()).unwrap_err();

        assert!(err.contains("SKSE 1.7"), "{err}");
        assert!(!game.path().join("skse64_loader.exe").exists());
        assert_eq!(fs::read_dir(store.path()).unwrap().count(), 0);
    }

    #[test]
    fn a_disabled_or_unextracted_wrong_version_engine_does_not_block_a_launch() {
        let wrong = fake_skse_mod_for_1_7_104();
        let (game, store) = game_with_working_skse_1_6_1170();
        let mut archived = row("SKSE archived", wrong.path(), true);
        archived.status = modlab_core::loadout::STATUS_ARCHIVED.to_string();
        let rows = [row("SKSE off", wrong.path(), false), archived];

        let (report, status) =
            prepare_launch(&rows, game.path(), Some(v(1, 6, 1170)), store.path()).unwrap();

        assert_eq!(report, crate::engine::InstallReport::default());
        assert_eq!(status.state, SkseState::Ok);
        assert_game_folder_untouched(game.path(), store.path());
    }

    #[test]
    fn an_unreadable_game_version_keeps_the_previous_behaviour() {
        // Version unknown: nothing to compare, the engine is copied as before
        // and the launch goes ahead (`Unverifiable` is not a refusal).
        let skse = fake_skse_mod_for_1_7_104();
        let game = fake_game(&["SkyrimSE.exe"]);
        let store = tempfile::tempdir().unwrap();
        let rows = [row("SKSE64", skse.path(), true)];

        let (report, status) = prepare_launch(&rows, game.path(), None, store.path()).unwrap();

        assert_eq!(report.copied.len(), 2);
        assert_eq!(status.state, SkseState::Unverifiable);
        assert!(game.path().join("skse64_1_7_104.dll").is_file());
    }

    #[test]
    fn check_accepts_any_matching_dll_whatever_its_case_and_ignores_loader_only_mods() {
        let several = tempfile::tempdir().unwrap();
        fs::write(several.path().join("skse64_loader.exe"), b"l").unwrap();
        fs::write(several.path().join("skse64_1_6_1170.dll"), b"a").unwrap();
        fs::write(several.path().join("SKSE64_1_7_104.DLL"), b"b").unwrap();
        let loader_only = tempfile::tempdir().unwrap();
        fs::write(loader_only.path().join("skse64_loader.exe"), b"l").unwrap();

        assert_eq!(
            check_engine_matches_game("m", several.path(), Some(v(1, 7, 104))),
            Ok(())
        );
        assert_eq!(
            check_engine_matches_game("m", several.path(), Some(v(1, 6, 1170))),
            Ok(())
        );
        assert_eq!(
            check_engine_matches_game("m", loader_only.path(), Some(v(1, 6, 1170))),
            Ok(())
        );
        let err = check_engine_matches_game("m", several.path(), Some(v(1, 5, 97))).unwrap_err();
        assert!(err.contains("1.7.104") && err.contains("1.6.1170"), "{err}");
        assert!(err.contains("skse64_1_5_97.dll"), "{err}");
    }

    fn row(name: &str, source: &Path, enabled: bool) -> LoadoutEntryRow {
        LoadoutEntryRow {
            mod_id: 1,
            name: name.to_string(),
            source_path: source.to_path_buf(),
            enabled,
            position: 0,
            version: None,
            status: STATUS_INSTALLED.to_string(),
            category: None,
            archive_path: None,
        }
    }

    #[test]
    fn expected_plugins_come_from_enabled_mods_skse_plugins_folders() {
        let address_library = tempfile::tempdir().unwrap();
        fs::create_dir_all(address_library.path().join("SKSE/Plugins")).unwrap();
        fs::write(address_library.path().join("SKSE/Plugins/x.dll"), b"d").unwrap();
        fs::write(
            address_library.path().join("SKSE/Plugins/versionlib.bin"),
            b"d",
        )
        .unwrap();
        let disabled = tempfile::tempdir().unwrap();
        fs::create_dir_all(disabled.path().join("skse/plugins")).unwrap();
        fs::write(disabled.path().join("skse/plugins/Off.dll"), b"d").unwrap();
        let plain = tempfile::tempdir().unwrap();
        fs::write(plain.path().join("Mod.esp"), b"d").unwrap();

        let names = expected_plugins(&[
            row(
                "Address Library for SKSE Plugins",
                address_library.path(),
                true,
            ),
            row("Disabled", disabled.path(), false),
            row("Plain", plain.path(), true),
        ]);

        assert_eq!(names, ["x.dll"]);
    }
}
