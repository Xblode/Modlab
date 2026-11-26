//! Delegate Play to a desktop Minecraft Launcher (CurseForge-bundled or Mojang)
//! via that install's `launcher_profiles.json`.
//!
//! Resolution order for which launcher to open:
//! 1. Human path-slot override `launcher_exe` (Mon jeu)
//! 2. Human path-slot override `root` (Install / `.minecraft` folder)
//! 3. Auto-detect CurseForge `%USERPROFILE%\curseforge\minecraft\Install\minecraft.exe`
//! 4. Auto-detect `MinecraftLauncher.exe` + `%AppData%\.minecraft`
//!
//! Never the Windows Store `minecraft:` URI.
//!
//! Slice: upsert ModLab profile (`gameDir` = instance) → bump `lastUsed` →
//! spawn the resolved exe. No stable API to auto-click Jouer; the human
//! confirms in the launcher UI.

use serde_json::{json, Map, Value};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

/// Where the launcher stores `launcher_profiles.json`, `versions/`, `libraries/`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LauncherKind {
    /// CurseForge-bundled Mojang launcher (`...\curseforge\minecraft\Install`).
    CurseForge,
    /// Classic desktop Minecraft Launcher + `%AppData%\.minecraft`.
    MojangDesktop,
}

#[derive(Debug, Clone)]
pub struct LauncherInstall {
    pub kind: LauncherKind,
    /// `minecraft.exe` or `MinecraftLauncher.exe`.
    pub exe: PathBuf,
    /// Directory containing `launcher_profiles.json` (workDir).
    pub data_dir: PathBuf,
}

/// Inputs to create/update a ModLab profile and open the launcher.
#[derive(Debug, Clone)]
pub struct OfficialLaunchRequest<'a> {
    /// Instance folder (mods, saves, config) — written as profile `gameDir`.
    pub game_dir: &'a Path,
    /// `lastVersionId` (must exist under the launcher install's `versions/`).
    pub version_id: &'a str,
    /// Stable profile key (e.g. `modlab-7`).
    pub profile_id: &'a str,
    /// Display name in the launcher UI.
    pub profile_name: &'a str,
    /// Path-slot override for the launcher `.exe` (Mon jeu). Takes priority.
    pub launcher_exe: Option<&'a Path>,
    /// Path-slot override for the launcher data folder (Install / `.minecraft`).
    pub launcher_data_dir: Option<&'a Path>,
}

#[derive(Debug, Clone)]
pub struct OfficialLaunchResult {
    pub profile_id: String,
    pub profile_name: String,
    pub version_id: String,
    pub game_dir: PathBuf,
    pub launcher_opened: bool,
    pub version_present_in_official: bool,
    pub launcher_exe: Option<PathBuf>,
    pub launcher_data_dir: Option<PathBuf>,
    pub launcher_kind: Option<&'static str>,
    /// French notice for the UI toast / under Jouer.
    pub notice: String,
}

/// `%AppData%\.minecraft` (legacy / Mojang-desktop data root).
pub fn official_minecraft_dir() -> Result<PathBuf, String> {
    let appdata = std::env::var_os("APPDATA").ok_or_else(|| {
        "APPDATA introuvable — impossible de joindre le launcher Minecraft".to_string()
    })?;
    Ok(PathBuf::from(appdata).join(".minecraft"))
}

pub fn launcher_profiles_path() -> Result<PathBuf, String> {
    Ok(preferred_launcher()
        .map(|i| i.data_dir.join("launcher_profiles.json"))
        .unwrap_or(official_minecraft_dir()?.join("launcher_profiles.json")))
}

/// Whether `<data_dir>/versions/<id>/<id>.json` exists.
pub fn version_installed_in(data_dir: &Path, version_id: &str) -> bool {
    data_dir
        .join("versions")
        .join(version_id)
        .join(format!("{version_id}.json"))
        .is_file()
}

/// Whether the preferred launcher install has this version (fallback: AppData).
pub fn official_version_installed(version_id: &str) -> bool {
    if let Some(install) = preferred_launcher() {
        return version_installed_in(&install.data_dir, version_id);
    }
    let Ok(root) = official_minecraft_dir() else {
        return false;
    };
    version_installed_in(&root, version_id)
}

/// Discover desktop launcher installs (CurseForge first, then Mojang).
pub fn discover_launcher_installs() -> Vec<LauncherInstall> {
    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let mut push = |install: LauncherInstall| {
        let key = install.data_dir.to_string_lossy().to_ascii_lowercase();
        if seen.insert(key) && install.exe.is_file() {
            out.push(install);
        }
    };

    // CurseForge-bundled Mojang launcher (user's machine).
    if let Some(home) = std::env::var_os("USERPROFILE") {
        let cf = PathBuf::from(home)
            .join("curseforge")
            .join("minecraft")
            .join("Install");
        let exe = cf.join("minecraft.exe");
        if exe.is_file() {
            push(LauncherInstall {
                kind: LauncherKind::CurseForge,
                exe,
                data_dir: cf,
            });
        }
    }

    // Classic MinecraftLauncher.exe next to / with AppData\.minecraft as data.
    for exe in mojang_desktop_exe_candidates() {
        if let Ok(appdata_mc) = official_minecraft_dir() {
            push(LauncherInstall {
                kind: LauncherKind::MojangDesktop,
                exe,
                data_dir: appdata_mc,
            });
        }
    }

    out
}

fn mojang_desktop_exe_candidates() -> Vec<PathBuf> {
    let mut out = Vec::new();
    let push_if = |out: &mut Vec<PathBuf>, p: PathBuf| {
        if p.is_file() {
            out.push(p);
        }
    };
    if let Some(pf) = std::env::var_os("ProgramFiles(x86)") {
        push_if(
            &mut out,
            PathBuf::from(&pf)
                .join("Minecraft Launcher")
                .join("MinecraftLauncher.exe"),
        );
    }
    if let Some(pf) = std::env::var_os("ProgramFiles") {
        push_if(
            &mut out,
            PathBuf::from(&pf)
                .join("Minecraft Launcher")
                .join("MinecraftLauncher.exe"),
        );
    }
    if let Some(local) = std::env::var_os("LOCALAPPDATA") {
        let local = PathBuf::from(local);
        push_if(
            &mut out,
            local
                .join("Programs")
                .join("Minecraft Launcher")
                .join("MinecraftLauncher.exe"),
        );
        push_if(
            &mut out,
            local
                .join("Programs")
                .join("minecraft-launcher")
                .join("MinecraftLauncher.exe"),
        );
    }
    out
}

/// Prefer CurseForge Install when present; else first Mojang desktop hit.
pub fn preferred_launcher() -> Option<LauncherInstall> {
    let installs = discover_launcher_installs();
    installs
        .iter()
        .find(|i| i.kind == LauncherKind::CurseForge)
        .cloned()
        .or_else(|| installs.into_iter().next())
}

const NO_LAUNCHER_HINT: &str = "Aucun launcher Minecraft desktop trouvé. Dans Mon jeu, choisissez l’exécutable (minecraft.exe sous CurseForge\\minecraft\\Install, ou MinecraftLauncher.exe) ou le dossier Install — pas le Store Windows.";

/// Build a [`LauncherInstall`] from a human-chosen exe and/or data folder
/// (path slots), falling back to auto-discovery. Order: exe override →
/// data-dir override → CurseForge Install → Mojang desktop.
pub fn resolve_play_launcher(
    exe_override: Option<&Path>,
    data_override: Option<&Path>,
) -> Result<LauncherInstall, String> {
    if let Some(exe) = exe_override {
        if let Some(install) = launcher_from_exe(exe) {
            return Ok(install);
        }
        if exe.exists() {
            return Err(format!(
                "Ce fichier n’est pas un launcher Minecraft reconnu : {}. Attendu : minecraft.exe (dossier Install CurseForge) ou MinecraftLauncher.exe.",
                exe.display()
            ));
        }
        return Err(format!(
            "Exécutable du launcher introuvable : {}. Choisissez-en un dans Mon jeu.",
            exe.display()
        ));
    }
    if let Some(data) = data_override {
        if let Some(install) = launcher_from_data_dir(data) {
            return Ok(install);
        }
        if data.is_dir() {
            return Err(format!(
                "Dossier launcher sans exécutable reconnu : {}. Placez minecraft.exe dedans, ou choisissez MinecraftLauncher.exe dans Mon jeu.",
                data.display()
            ));
        }
        return Err(format!(
            "Dossier du launcher introuvable : {}. Choisissez-en un dans Mon jeu.",
            data.display()
        ));
    }
    preferred_launcher().ok_or_else(|| NO_LAUNCHER_HINT.to_string())
}

/// Infer kind + data_dir from a launcher `.exe` the human picked.
pub fn launcher_from_exe(exe: &Path) -> Option<LauncherInstall> {
    if !exe.is_file() {
        return None;
    }
    let name = exe
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let parent = exe.parent()?.to_path_buf();
    if name == "minecraft.exe" {
        // CurseForge (and similar) keep profiles next to the exe.
        return Some(LauncherInstall {
            kind: LauncherKind::CurseForge,
            exe: exe.to_path_buf(),
            data_dir: parent,
        });
    }
    if name == "minecraftlauncher.exe" {
        let data_dir = official_minecraft_dir().unwrap_or(parent);
        return Some(LauncherInstall {
            kind: LauncherKind::MojangDesktop,
            exe: exe.to_path_buf(),
            data_dir,
        });
    }
    None
}

/// Infer exe from an Install / `.minecraft` folder the human picked.
pub fn launcher_from_data_dir(data: &Path) -> Option<LauncherInstall> {
    if !data.is_dir() {
        return None;
    }
    let cf_exe = data.join("minecraft.exe");
    if cf_exe.is_file() {
        return Some(LauncherInstall {
            kind: LauncherKind::CurseForge,
            exe: cf_exe,
            data_dir: data.to_path_buf(),
        });
    }
    // Classic `.minecraft` (profiles here, exe elsewhere).
    if data.join("launcher_profiles.json").is_file() || data.join("versions").is_dir() {
        for exe in mojang_desktop_exe_candidates() {
            return Some(LauncherInstall {
                kind: LauncherKind::MojangDesktop,
                exe,
                data_dir: data.to_path_buf(),
            });
        }
    }
    None
}

/// Candidate install locations for the Java Edition Minecraft Launcher (Windows).
pub fn official_launcher_candidates() -> Vec<PathBuf> {
    discover_launcher_installs()
        .into_iter()
        .map(|i| i.exe)
        .collect()
}

pub fn find_official_launcher() -> Option<PathBuf> {
    preferred_launcher().map(|i| i.exe)
}

/// Create or update a custom profile pointing at ModLab's instance folder.
pub fn upsert_modlab_profile(req: &OfficialLaunchRequest<'_>) -> Result<(), String> {
    let install = preferred_launcher().ok_or_else(|| NO_LAUNCHER_HINT.to_string())?;
    upsert_modlab_profile_at(
        &install.data_dir.join("launcher_profiles.json"),
        req,
        Some(&install.data_dir),
    )
}

/// Same as upsert with an explicit profiles path (tests).
/// When `launcher_data_dir` is set, adds CurseForge-style `libraryDirectory` javaArgs.
pub fn upsert_modlab_profile_at(
    path: &Path,
    req: &OfficialLaunchRequest<'_>,
    launcher_data_dir: Option<&Path>,
) -> Result<(), String> {
    let mut root = if path.is_file() {
        let raw = fs::read_to_string(path).map_err(io_err)?;
        serde_json::from_str::<Value>(&raw).map_err(parse_err)?
    } else {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(io_err)?;
        }
        json!({
            "profiles": {},
            "settings": {},
            "version": 3
        })
    };

    let profiles = root
        .as_object_mut()
        .ok_or_else(|| "launcher_profiles.json invalide".to_string())?
        .entry("profiles")
        .or_insert_with(|| json!({}));
    let profiles_obj = profiles
        .as_object_mut()
        .ok_or_else(|| "profiles invalide dans launcher_profiles.json".to_string())?;

    let now = iso8601_now();
    let game_dir = req
        .game_dir
        .canonicalize()
        .unwrap_or_else(|_| req.game_dir.to_path_buf());
    let game_dir_s = game_dir.to_string_lossy().replace('/', "\\");

    let mut profile = profiles_obj
        .get(req.profile_id)
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_else(|| {
            let mut m = Map::new();
            m.insert("created".into(), json!(now.clone()));
            m.insert("type".into(), json!("custom"));
            m.insert("icon".into(), json!("Grass"));
            m
        });

    profile.insert("name".into(), json!(req.profile_name));
    profile.insert("type".into(), json!("custom"));
    profile.insert("lastVersionId".into(), json!(req.version_id));
    profile.insert("gameDir".into(), json!(game_dir_s.clone()));
    // Make this the most recently used profile so the launcher sorts it first.
    profile.insert("lastUsed".into(), json!(now));
    if !profile.contains_key("created") {
        profile.insert("created".into(), json!(now));
    }
    if !profile.contains_key("icon") {
        profile.insert("icon".into(), json!("Grass"));
    }

    // CurseForge profiles set libraryDirectory to the Install libraries folder
    // so the game resolves jars from the launcher home, not from gameDir.
    if let Some(data) = launcher_data_dir {
        let libs_s = data.join("libraries").to_string_lossy().replace('/', "\\");
        // serde_json escapes backslashes for the file; paths stay Windows-native here.
        let java_args = format!(
            "-Dminecraft.applet.TargetDirectory=\"{game_dir_s}\" -DlibraryDirectory=\"{libs_s}\" -Dfml.ignorePatchDiscrepancies=true -Dfml.ignoreInvalidMinecraftCertificates=true"
        );
        profile.insert("javaArgs".into(), json!(java_args));
    }

    profiles_obj.insert(req.profile_id.to_string(), Value::Object(profile));

    // Older launcher builds read selectedProfile; harmless if ignored.
    if let Some(obj) = root.as_object_mut() {
        obj.insert("selectedProfile".into(), json!(req.profile_id));
        obj.entry("version").or_insert_with(|| json!(3));
    }

    let pretty = serde_json::to_vec_pretty(&root).map_err(parse_err)?;
    fs::write(path, pretty).map_err(io_err)?;
    Ok(())
}

/// Open a discovered desktop launcher. Never uses the Store `minecraft:` URI.
pub fn open_launcher_install(install: &LauncherInstall) -> Result<(), String> {
    Command::new(&install.exe)
        .current_dir(&install.data_dir)
        .spawn()
        .map_err(|e| {
            format!(
                "échec à ouvrir {} ({}) : {e}",
                match install.kind {
                    LauncherKind::CurseForge => "le launcher CurseForge/Minecraft",
                    LauncherKind::MojangDesktop => "le Minecraft Launcher",
                },
                install.exe.display()
            )
        })?;
    Ok(())
}

/// Open the preferred desktop launcher UI.
pub fn open_official_launcher() -> Result<bool, String> {
    let install = preferred_launcher().ok_or_else(|| NO_LAUNCHER_HINT.to_string())?;
    open_launcher_install(&install)?;
    Ok(true)
}

fn kind_label(kind: &LauncherKind) -> &'static str {
    match kind {
        LauncherKind::CurseForge => "CurseForge",
        LauncherKind::MojangDesktop => "Mojang",
    }
}

/// Upsert the ModLab profile into the resolved launcher and open it.
/// Resolution: human path-slot overrides first, then auto-detect.
pub fn play_via_official_launcher(
    req: &OfficialLaunchRequest<'_>,
) -> Result<OfficialLaunchResult, String> {
    if req.version_id.trim().is_empty() {
        return Err("version Minecraft manquante pour le profil launcher".to_string());
    }
    if !req.game_dir.is_dir() {
        return Err(format!(
            "dossier d’instance introuvable : {}",
            req.game_dir.display()
        ));
    }

    let install = resolve_play_launcher(req.launcher_exe, req.launcher_data_dir)?;

    let profiles = install.data_dir.join("launcher_profiles.json");
    upsert_modlab_profile_at(&profiles, req, Some(&install.data_dir))?;
    let version_ok = version_installed_in(&install.data_dir, req.version_id);

    let launcher_opened = match open_launcher_install(&install) {
        Ok(()) => true,
        Err(e) => {
            return Ok(OfficialLaunchResult {
                profile_id: req.profile_id.to_string(),
                profile_name: req.profile_name.to_string(),
                version_id: req.version_id.to_string(),
                game_dir: req.game_dir.to_path_buf(),
                launcher_opened: false,
                version_present_in_official: version_ok,
                launcher_exe: Some(install.exe.clone()),
                launcher_data_dir: Some(install.data_dir.clone()),
                launcher_kind: Some(kind_label(&install.kind)),
                notice: format!(
                    "Profil « {} » enregistré dans {} ({}). {e} Ouvrez ce launcher, sélectionnez le profil ModLab (déjà en tête via lastUsed), puis Jouer.",
                    req.profile_name,
                    install.data_dir.display(),
                    kind_label(&install.kind)
                ),
            });
        }
    };

    let mut notice = format!(
        "Profil « {} » actif dans le launcher {} ({}). Version {}. Sélectionnez ce profil si besoin, puis cliquez Jouer. Compte Microsoft = celui du launcher.",
        req.profile_name,
        kind_label(&install.kind),
        install.exe.display(),
        req.version_id
    );
    if !version_ok {
        notice.push_str(&format!(
            " Attention : « {} » absente de {}\\versions — installez-la une fois dans ce launcher.",
            req.version_id,
            install.data_dir.display()
        ));
    }

    Ok(OfficialLaunchResult {
        profile_id: req.profile_id.to_string(),
        profile_name: req.profile_name.to_string(),
        version_id: req.version_id.to_string(),
        game_dir: req.game_dir.to_path_buf(),
        launcher_opened,
        version_present_in_official: version_ok,
        launcher_exe: Some(install.exe),
        launcher_data_dir: Some(install.data_dir),
        launcher_kind: Some(kind_label(&install.kind)),
        notice,
    })
}

fn iso8601_now() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let days = secs / 86400;
    let rem = secs % 86400;
    let (y, m, d) = civil_from_days(days as i64);
    let hh = rem / 3600;
    let mm = (rem % 3600) / 60;
    let ss = rem % 60;
    format!("{y:04}-{m:02}-{d:02}T{hh:02}:{mm:02}:{ss:02}.000Z")
}

fn civil_from_days(z: i64) -> (i32, u32, u32) {
    let z = z + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = (yoe as i64) + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y as i32, m as u32, d as u32)
}

fn io_err(e: std::io::Error) -> String {
    format!("fichier : {e}")
}

fn parse_err<E: std::fmt::Display>(e: E) -> String {
    format!("données : {e}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn upsert_creates_custom_profile_with_game_dir() {
        let dir = tempdir().unwrap();
        let profiles = dir.path().join("launcher_profiles.json");
        fs::write(&profiles, br#"{"profiles":{},"settings":{},"version":3}"#).unwrap();
        let game = dir.path().join("instance");
        fs::create_dir_all(&game).unwrap();
        let libs_root = dir.path().join("Install");
        fs::create_dir_all(libs_root.join("libraries")).unwrap();

        let req = OfficialLaunchRequest {
            game_dir: &game,
            version_id: "1.20.1",
            profile_id: "modlab-42",
            profile_name: "ModLab — test",
            launcher_exe: None,
            launcher_data_dir: None,
        };
        upsert_modlab_profile_at(&profiles, &req, Some(&libs_root)).unwrap();

        let raw = fs::read_to_string(&profiles).unwrap();
        let v: Value = serde_json::from_str(&raw).unwrap();
        let p = &v["profiles"]["modlab-42"];
        assert_eq!(p["name"], "ModLab — test");
        assert_eq!(p["type"], "custom");
        assert_eq!(p["lastVersionId"], "1.20.1");
        assert_eq!(v["selectedProfile"], "modlab-42");
        let gd = p["gameDir"].as_str().unwrap();
        assert!(
            gd.to_lowercase().contains("instance"),
            "gameDir should point at instance: {gd}"
        );
        let args = p["javaArgs"].as_str().unwrap();
        assert!(args.contains("libraryDirectory"), "{args}");
        assert!(p.get("lastUsed").is_some());
    }

    #[test]
    fn upsert_updates_existing_profile() {
        let dir = tempdir().unwrap();
        let profiles = dir.path().join("launcher_profiles.json");
        fs::write(
            &profiles,
            br#"{
              "profiles": {
                "modlab-1": {
                  "created": "2020-01-01T00:00:00.000Z",
                  "name": "old",
                  "type": "custom",
                  "lastVersionId": "1.19",
                  "gameDir": "C:\\old",
                  "lastUsed": "2020-01-01T00:00:00.000Z",
                  "icon": "Dirt"
                }
              },
              "version": 3
            }"#,
        )
        .unwrap();
        let game = dir.path().join("new-instance");
        fs::create_dir_all(&game).unwrap();
        upsert_modlab_profile_at(
            &profiles,
            &OfficialLaunchRequest {
                game_dir: &game,
                version_id: "26.2",
                profile_id: "modlab-1",
                profile_name: "ModLab — oj",
                launcher_exe: None,
                launcher_data_dir: None,
            },
            None,
        )
        .unwrap();
        let v: Value = serde_json::from_str(&fs::read_to_string(&profiles).unwrap()).unwrap();
        let p = &v["profiles"]["modlab-1"];
        assert_eq!(p["name"], "ModLab — oj");
        assert_eq!(p["lastVersionId"], "26.2");
        assert_eq!(p["created"], "2020-01-01T00:00:00.000Z");
        assert_ne!(p["lastUsed"], "2020-01-01T00:00:00.000Z");
        assert_eq!(v["selectedProfile"], "modlab-1");
    }

    #[test]
    fn discover_prefers_curseforge_layout() {
        let dir = tempdir().unwrap();
        let home = dir.path().join("home");
        let install = home.join("curseforge").join("minecraft").join("Install");
        fs::create_dir_all(&install).unwrap();
        fs::write(install.join("minecraft.exe"), b"fake").unwrap();
        fs::write(
            install.join("launcher_profiles.json"),
            br#"{"profiles":{},"version":3}"#,
        )
        .unwrap();
        std::env::set_var("USERPROFILE", &home);
        let found = discover_launcher_installs();
        std::env::remove_var("USERPROFILE");
        assert!(
            found.iter().any(|i| i.kind == LauncherKind::CurseForge),
            "expected CurseForge install in {found:?}"
        );
        let pref = found
            .iter()
            .find(|i| i.kind == LauncherKind::CurseForge)
            .unwrap();
        assert_eq!(pref.data_dir, install);
        assert!(pref.exe.ends_with("minecraft.exe"));
    }

    #[test]
    fn version_installed_checks_data_dir() {
        let dir = tempdir().unwrap();
        let data = dir.path();
        let vdir = data.join("versions").join("26.2");
        fs::create_dir_all(&vdir).unwrap();
        assert!(!version_installed_in(data, "26.2"));
        fs::write(vdir.join("26.2.json"), b"{}").unwrap();
        assert!(version_installed_in(data, "26.2"));
    }

    #[test]
    fn resolve_prefers_exe_override_over_auto_detect() {
        let dir = tempdir().unwrap();
        let install = dir.path().join("Install");
        fs::create_dir_all(&install).unwrap();
        let exe = install.join("minecraft.exe");
        fs::write(&exe, b"fake").unwrap();
        let got = resolve_play_launcher(Some(&exe), None).unwrap();
        assert_eq!(got.kind, LauncherKind::CurseForge);
        assert_eq!(got.exe, exe);
        assert_eq!(got.data_dir, install);
    }

    #[test]
    fn resolve_accepts_install_folder_override() {
        let dir = tempdir().unwrap();
        let install = dir.path().join("Install");
        fs::create_dir_all(&install).unwrap();
        fs::write(install.join("minecraft.exe"), b"fake").unwrap();
        let got = resolve_play_launcher(None, Some(&install)).unwrap();
        assert_eq!(got.kind, LauncherKind::CurseForge);
        assert_eq!(got.data_dir, install);
        assert!(got.exe.ends_with("minecraft.exe"));
    }

    #[test]
    fn resolve_without_override_errors_clearly_when_nothing_found() {
        let prev_home = std::env::var_os("USERPROFILE");
        let prev_pf = std::env::var_os("ProgramFiles");
        let prev_pf86 = std::env::var_os("ProgramFiles(x86)");
        let prev_local = std::env::var_os("LOCALAPPDATA");
        let empty = tempdir().unwrap();
        std::env::set_var("USERPROFILE", empty.path());
        std::env::set_var("ProgramFiles", empty.path().join("pf"));
        std::env::set_var("ProgramFiles(x86)", empty.path().join("pf86"));
        std::env::set_var("LOCALAPPDATA", empty.path().join("local"));
        let err = resolve_play_launcher(None, None).unwrap_err();
        match prev_home {
            Some(v) => std::env::set_var("USERPROFILE", v),
            None => std::env::remove_var("USERPROFILE"),
        }
        match prev_pf {
            Some(v) => std::env::set_var("ProgramFiles", v),
            None => std::env::remove_var("ProgramFiles"),
        }
        match prev_pf86 {
            Some(v) => std::env::set_var("ProgramFiles(x86)", v),
            None => std::env::remove_var("ProgramFiles(x86)"),
        }
        match prev_local {
            Some(v) => std::env::set_var("LOCALAPPDATA", v),
            None => std::env::remove_var("LOCALAPPDATA"),
        }
        assert!(
            err.contains("Mon jeu"),
            "expected Mon jeu hint in error, got: {err}"
        );
    }
}
