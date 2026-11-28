//! One-click Minecraft loader installs into an instance folder (vanilla layout).

use serde::Deserialize;
use serde_json::Value;
use sha1::{Digest, Sha1};
use std::cmp::Ordering;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use zip::ZipArchive;

const PISTON_MANIFEST: &str = "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LoaderProgress {
    pub loadout_id: Option<i64>,
    pub phase: String,
    pub message: String,
    pub done: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LoaderStatus {
    pub state: &'static str,
    pub message: String,
}

pub type ProgressCb<'a> = &'a dyn Fn(LoaderProgress);

pub fn list_game_versions() -> Result<Vec<String>, String> {
    let client = http_client()?;
    let manifest: VersionManifest = client
        .get(PISTON_MANIFEST)
        .send()
        .map_err(net_err)?
        .json()
        .map_err(parse_err)?;
    Ok(manifest
        .versions
        .into_iter()
        .filter(|v| v.r#type == "release")
        .map(|v| v.id)
        .collect())
}

pub fn list_loader_versions(loader: &str, mc: &str) -> Result<Vec<String>, String> {
    let loader = loader.trim().to_lowercase();
    match loader.as_str() {
        "fabric" => list_fabric_loaders(mc),
        "quilt" => list_quilt_loaders(mc),
        "forge" => list_forge_versions(mc),
        "neoforge" => list_neoforge_versions(mc),
        "vanilla" => Ok(Vec::new()),
        other => Err(format!("loader inconnu : {other}")),
    }
}

pub fn status(
    instance_root: &Path,
    loader: Option<&str>,
    mc: Option<&str>,
    loader_ver: Option<&str>,
) -> LoaderStatus {
    let Some(loader) = loader.map(str::trim).filter(|s| !s.is_empty()) else {
        return LoaderStatus {
            state: "unset",
            message: "Choisissez un loader.".to_string(),
        };
    };
    let Some(mc) = mc.map(str::trim).filter(|s| !s.is_empty()) else {
        return LoaderStatus {
            state: "unset",
            message: "Choisissez une version Minecraft.".to_string(),
        };
    };
    if loader != "vanilla"
        && loader_ver
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .is_none()
    {
        return LoaderStatus {
            state: "unset",
            message: "Choisissez une version du loader.".to_string(),
        };
    }
    if versions_installed(instance_root, loader, mc, loader_ver) {
        LoaderStatus {
            state: "installed",
            message: "Installé.".to_string(),
        }
    } else {
        LoaderStatus {
            state: "missing",
            message: "Prêt à installer.".to_string(),
        }
    }
}

pub fn versions_installed(
    instance_root: &Path,
    loader: &str,
    mc: &str,
    loader_ver: Option<&str>,
) -> bool {
    let Some(json_path) = find_version_json_path(instance_root, loader, mc, loader_ver) else {
        return false;
    };
    // Forge/NeoForge write the version JSON early; the patched client jar
    // only appears after processors finish. Treat a JSON-only tree as missing
    // so the CTA « Installer … » stays available after an interrupted install.
    let loader = loader.trim().to_lowercase();
    if loader == "forge" || loader == "neoforge" {
        let Ok(raw) = fs::read_to_string(&json_path) else {
            return false;
        };
        let Ok(detail) = serde_json::from_str::<Value>(&raw) else {
            return false;
        };
        if !missing_installer_libraries(instance_root, &detail).is_empty() {
            return false;
        }
        // NeoForge 26.x lists downloadable FML jars in the version JSON but the
        // patched Minecraft client is only produced by PROCESS_MINECRAFT_JAR and
        // is discovered via `-DlibraryDirectory` — it is NOT an empty-URL library
        // entry. Require that artefact (and the universal jar) explicitly.
        // Legacy 1.20.1 NeoForge still uses the Forge-style empty-URL client jar.
        if loader == "neoforge" && !neoforge_uses_legacy_forge_artifact(mc) {
            let Some(ver) = loader_ver.map(str::trim).filter(|s| !s.is_empty()) else {
                return false;
            };
            return missing_neoforge_processor_artifacts(instance_root, ver).is_empty();
        }
        return true;
    }
    true
}

/// Relative paths of NeoForge jars produced by installer processors (not on CDN
/// as empty-URL version-JSON libs in modern NeoForge).
pub fn missing_neoforge_processor_artifacts(instance_root: &Path, loader_ver: &str) -> Vec<String> {
    let ver = loader_ver.trim();
    if ver.is_empty() {
        return vec!["(version NeoForge vide)".to_string()];
    }
    let mut missing = Vec::new();
    for rel in [
        neoforge_patched_client_rel(ver),
        neoforge_universal_rel(ver),
    ] {
        let jar = instance_root.join("libraries").join(&rel);
        if !jar.is_file() || jar.metadata().map(|m| m.len()).unwrap_or(0) == 0 {
            missing.push(rel);
        }
    }
    missing
}

fn neoforge_patched_client_rel(loader_ver: &str) -> String {
    format!(
        "net/neoforged/minecraft-client-patched/{loader_ver}/minecraft-client-patched-{loader_ver}.jar"
    )
}

fn neoforge_universal_rel(loader_ver: &str) -> String {
    format!("net/neoforged/neoforge/{loader_ver}/neoforge-{loader_ver}-universal.jar")
}

/// Vanilla game version ids that pass [`versions_installed`] under the instance.
pub fn list_installed_game_versions(instance_root: &Path) -> Vec<String> {
    let versions = instance_root.join("versions");
    let Ok(read) = fs::read_dir(&versions) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for entry in read.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if name.is_empty() || name.contains(' ') {
            continue;
        }
        // Vanilla folders are named exactly like the game version.
        if !entry.path().join(format!("{name}.json")).is_file() {
            continue;
        }
        // Skip loader trees (fabric-*, quilt-*, 1.20.1-47.x forge builds, …).
        if ["fabric", "quilt", "forge", "neoforge"]
            .iter()
            .any(|l| folder_belongs_to_loader(instance_root, &name, l))
        {
            continue;
        }
        if versions_installed(instance_root, "vanilla", &name, None) {
            out.push(name);
        }
    }
    out.sort_by(|a, b| compare_versions_desc(a, b));
    out
}

/// Loader builds already on disk for `loader` + `mc` (source of truth:
/// [`versions_installed`]). Empty for vanilla — use [`list_installed_game_versions`].
pub fn list_installed_loader_versions(instance_root: &Path, loader: &str, mc: &str) -> Vec<String> {
    let loader = loader.trim().to_lowercase();
    let mc = mc.trim();
    if loader.is_empty() || mc.is_empty() || loader == "vanilla" {
        return Vec::new();
    }
    let versions = instance_root.join("versions");
    let Ok(read) = fs::read_dir(&versions) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for entry in read.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if !name.contains(mc) {
            continue;
        }
        if !folder_belongs_to_loader(instance_root, &name, &loader) {
            continue;
        }
        let Some(lv) = extract_loader_version_from_folder(&loader, mc, &name) else {
            continue;
        };
        if versions_installed(instance_root, &loader, mc, Some(&lv)) && !out.contains(&lv) {
            out.push(lv);
        }
    }
    sort_loader_versions(&mut out);
    out
}

/// Whether any complete install of `loader` exists for this Minecraft version.
pub fn loader_installed_for_mc(instance_root: &Path, loader: &str, mc: &str) -> bool {
    let loader = loader.trim().to_lowercase();
    let mc = mc.trim();
    if loader.is_empty() || mc.is_empty() {
        return false;
    }
    if loader == "vanilla" {
        return versions_installed(instance_root, "vanilla", mc, None);
    }
    !list_installed_loader_versions(instance_root, &loader, mc).is_empty()
}

fn folder_belongs_to_loader(instance_root: &Path, folder_name: &str, loader: &str) -> bool {
    let json = instance_root
        .join("versions")
        .join(folder_name)
        .join(format!("{folder_name}.json"));
    let Ok(raw) = fs::read_to_string(json) else {
        return false;
    };
    let lower = raw.to_lowercase();
    match loader {
        "neoforge" => lower.contains("neoforge") || lower.contains("neoforged"),
        "forge" => {
            (lower.contains("minecraftforge") || lower.contains("net.minecraftforge"))
                && !lower.contains("neoforge")
                && !lower.contains("neoforged")
        }
        "fabric" => {
            folder_name.starts_with("fabric-")
                || (lower.contains("fabricmc") || lower.contains("net.fabricmc"))
                    && !lower.contains("quilt")
        }
        "quilt" => {
            folder_name.starts_with("quilt-")
                || lower.contains("quiltmc")
                || lower.contains("org.quiltmc")
        }
        _ => false,
    }
}

fn extract_loader_version_from_folder(loader: &str, mc: &str, folder_name: &str) -> Option<String> {
    match loader {
        "fabric" | "quilt" => {
            let prefix = format!("{loader}-{mc}-");
            folder_name.strip_prefix(&prefix).map(str::to_string)
        }
        "forge" => {
            let prefix = format!("{mc}-");
            let rest = folder_name.strip_prefix(&prefix)?;
            // Installer may produce `{mc}-{ver}` or `{mc}-forge-{ver}`.
            Some(rest.strip_prefix("forge-").unwrap_or(rest).to_string())
        }
        "neoforge" => {
            if let Some(rest) = folder_name.strip_prefix("neoforge-") {
                return Some(rest.to_string());
            }
            let prefix = format!("{mc}-");
            let rest = folder_name.strip_prefix(&prefix)?;
            Some(rest.strip_prefix("neoforge-").unwrap_or(rest).to_string())
        }
        _ => None,
    }
}

/// `versions/<id>/<id>.json` when present (preferred id, then scan).
fn find_version_json_path(
    instance_root: &Path,
    loader: &str,
    mc: &str,
    loader_ver: Option<&str>,
) -> Option<PathBuf> {
    let versions = instance_root.join("versions");
    if !versions.is_dir() {
        return None;
    }
    let version_id = version_folder_id(loader, mc, loader_ver);
    let preferred = versions
        .join(&version_id)
        .join(format!("{version_id}.json"));
    if preferred.is_file() {
        return Some(preferred);
    }
    let Ok(read) = fs::read_dir(&versions) else {
        return None;
    };
    for entry in read.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if !name.contains(mc) {
            continue;
        }
        let json = entry.path().join(format!("{name}.json"));
        if !json.is_file() {
            continue;
        }
        if loader == "vanilla" && name == mc {
            return Some(json);
        }
        if let Some(lv) = loader_ver {
            if name.contains(lv) {
                return Some(json);
            }
        }
    }
    None
}

/// Relative paths of classpath jars that only the Forge/NeoForge installer
/// produces (`downloads.artifact` with a path but no download URL).
pub fn missing_installer_libraries(instance_root: &Path, version_json: &Value) -> Vec<String> {
    let mut missing = Vec::new();
    let Some(libs) = version_json.get("libraries").and_then(Value::as_array) else {
        return missing;
    };
    for lib in libs {
        if let Some(rules) = lib.get("rules").and_then(Value::as_array) {
            if !rules_allow(rules) {
                continue;
            }
        }
        if !is_installer_only_library(lib) {
            continue;
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

fn is_installer_only_library(lib: &Value) -> bool {
    let Some(artifact) = lib.get("downloads").and_then(|d| d.get("artifact")) else {
        return false;
    };
    let has_path = artifact
        .get("path")
        .and_then(Value::as_str)
        .map(str::trim)
        .is_some_and(|p| !p.is_empty());
    if !has_path {
        return false;
    }
    let url = artifact
        .get("url")
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or("");
    url.is_empty()
}

/// Download vanilla client/json/libs for `mc` when the version folder is missing.
/// Reused by Jouer (`prepare_launch`) and by every non-vanilla loader install.
pub fn ensure_vanilla(
    instance_root: &Path,
    data_root: &Path,
    loadout_id: i64,
    mc: &str,
    progress: ProgressCb<'_>,
) -> Result<(), String> {
    if versions_installed(instance_root, "vanilla", mc, None) {
        return Ok(());
    }
    install_vanilla(instance_root, data_root, loadout_id, mc, progress)
}

/// Refuse a loader version that is not published for this Minecraft version.
pub fn assert_loader_version_for_mc(
    loader: &str,
    mc: &str,
    loader_ver: &str,
) -> Result<(), String> {
    let loader = loader.trim().to_lowercase();
    if loader == "vanilla" {
        return Ok(());
    }
    let versions = list_loader_versions(&loader, mc)?;
    if versions.iter().any(|v| v == loader_ver) {
        Ok(())
    } else {
        Err(format!(
            "La version {loader_ver} de {loader} ne correspond pas à Minecraft {mc}."
        ))
    }
}

pub fn install(
    instance_root: &Path,
    data_root: &Path,
    loadout_id: i64,
    loader: &str,
    mc: &str,
    loader_ver: Option<&str>,
    force: bool,
    progress: ProgressCb<'_>,
) -> Result<(), String> {
    refuse_mdk(loader_ver)?;
    ensure_launcher_profiles(instance_root)?;
    let loader = loader.trim().to_lowercase();
    emit(progress, loadout_id, "start", "Démarrage…", false);
    if !force && versions_installed(instance_root, &loader, mc, loader_ver) {
        emit(progress, loadout_id, "done", "Déjà installé.", true);
        return Ok(());
    }
    match loader.as_str() {
        "vanilla" => install_vanilla(instance_root, data_root, loadout_id, mc, progress)?,
        "fabric" | "quilt" => {
            let ver = loader_ver.ok_or_else(|| "version du loader requise".to_string())?;
            assert_loader_version_for_mc(&loader, mc, ver)?;
            ensure_vanilla(instance_root, data_root, loadout_id, mc, progress)?;
            install_profile_loader(
                instance_root,
                data_root,
                loadout_id,
                &loader,
                mc,
                ver,
                progress,
            )?;
        }
        "forge" | "neoforge" => {
            let ver = loader_ver.ok_or_else(|| "version du loader requise".to_string())?;
            assert_loader_version_for_mc(&loader, mc, ver)?;
            ensure_vanilla(instance_root, data_root, loadout_id, mc, progress)?;
            if force {
                wipe_forge_like_install(instance_root, &loader, mc, ver)?;
            }
            install_forge_like(
                instance_root,
                data_root,
                loadout_id,
                &loader,
                mc,
                ver,
                progress,
            )?;
        }
        other => return Err(format!("loader inconnu : {other}")),
    }
    emit(progress, loadout_id, "done", "Terminé.", true);
    Ok(())
}

pub fn forge_installer_url(mc: &str, ver: &str) -> Result<String, String> {
    let coord = forge_coordinate(mc, ver)?;
    Ok(format!(
        "https://maven.minecraftforge.net/net/minecraftforge/forge/{coord}/forge-{coord}-installer.jar"
    ))
}

pub fn neoforge_installer_url(mc: &str, ver: &str) -> Result<String, String> {
    let mc = mc.trim();
    if neoforge_uses_legacy_forge_artifact(mc) {
        let coord = forge_coordinate(mc, ver)?;
        return Ok(format!(
            "https://maven.neoforged.net/releases/net/neoforged/forge/{coord}/forge-{coord}-installer.jar"
        ));
    }
    let full = neoforge_modern_version_id(ver)?;
    Ok(format!(
        "https://maven.neoforged.net/releases/net/neoforged/neoforge/{full}/neoforge-{full}-installer.jar"
    ))
}

/// Minecraft game version → NeoForge maven version prefix (modern artefact only).
pub fn neoforge_version_prefix_for_mc(mc: &str) -> Result<String, String> {
    let mc = mc.trim();
    if mc.is_empty() {
        return Err("version Minecraft vide".to_string());
    }
    if neoforge_uses_legacy_forge_artifact(mc) {
        return Err("1.20.1 utilise l'artefact forge hérité".to_string());
    }
    Ok(neoforge_modern_prefix(mc))
}

fn neoforge_uses_legacy_forge_artifact(mc: &str) -> bool {
    mc == "1.20.1"
}

fn neoforge_modern_version_id(ver: &str) -> Result<String, String> {
    let ver = ver.trim();
    if ver.is_empty() {
        return Err("version NeoForge vide".to_string());
    }
    Ok(ver.to_string())
}

fn neoforge_modern_prefix(mc: &str) -> String {
    let parts: Vec<&str> = mc.split('.').collect();
    if parts.first() == Some(&"1") && parts.len() >= 2 {
        let minor = parts[1];
        let patch = parts.get(2).copied().unwrap_or("0");
        format!("{minor}.{patch}.")
    } else if parts.len() >= 3 {
        format!("{}.{}.{}.", parts[0], parts[1], parts[2])
    } else if parts.len() == 2 {
        format!("{}.{}.0.", parts[0], parts[1])
    } else {
        format!("{}.", parts[0])
    }
}

pub fn refuse_mdk(loader_ver: Option<&str>) -> Result<(), String> {
    let Some(raw) = loader_ver else {
        return Ok(());
    };
    let low = raw.to_ascii_lowercase();
    if low.contains("mdk") {
        return Err(
            "Le MDK Forge n'est pas pris en charge — utilisez l'installateur client.".to_string(),
        );
    }
    Ok(())
}

pub fn ensure_launcher_profiles(instance_root: &Path) -> Result<(), String> {
    let path = instance_root.join("launcher_profiles.json");
    if path.is_file() {
        return Ok(());
    }
    fs::create_dir_all(instance_root).map_err(io_err)?;
    fs::write(&path, r#"{"profiles":{}}"#).map_err(io_err)?;
    Ok(())
}

pub fn find_java() -> Option<PathBuf> {
    find_java_with_major(None)
}

/// Prefer a launcher / JDK runtime whose major satisfies the game JSON
/// (`javaVersion.majorVersion`). When a major is required, never return an
/// older runtime (UnsupportedClassVersionError). Search order: Mojang
/// `.minecraft/runtime`, CurseForge `Install/runtime` + `Install/java`,
/// common Adoptium/Oracle/Microsoft installs, `JAVA_HOME`, then PATH.
pub fn find_java_with_major(major: Option<u32>) -> Option<PathBuf> {
    find_java_with_major_in(major, &java_runtime_search_roots())
}

/// Like [`find_java_with_major`], also scanning `extra_roots` first (e.g. ModLab
/// cache after an Adoptium ensure).
pub fn find_java_with_major_in(major: Option<u32>, extra_roots: &[PathBuf]) -> Option<PathBuf> {
    let mut roots: Vec<PathBuf> = extra_roots.to_vec();
    for root in java_runtime_search_roots() {
        if !roots.iter().any(|r| r == &root) {
            roots.push(root);
        }
    }

    if let Some(need) = major {
        // Exact major first (folder name hint, then `java -version`).
        for root in &roots {
            if let Some(java) = find_java_exe_matching_major(root, need) {
                return Some(java);
            }
        }
        if let Some(java) = java_home_exe() {
            if java_version_major(&java) == Some(need) {
                return Some(java);
            }
        }
        if let Some(java) = which_java("java") {
            if java_version_major(&java) == Some(need) {
                return Some(java);
            }
        }
        // Accept a newer runtime (class-file forward compatible). Prefer the
        // smallest major that still satisfies `need`.
        let mut best: Option<(u32, PathBuf)> = None;
        for root in &roots {
            for java in find_all_java_exe_under(root) {
                if let Some(have) = java_version_major(&java) {
                    if have >= need {
                        match &best {
                            Some((b, _)) if have >= *b => {}
                            _ => best = Some((have, java)),
                        }
                    }
                }
            }
        }
        if let Some((_, java)) = best {
            return Some(java);
        }
        if let Some(java) = java_home_exe() {
            if java_version_major(&java).is_some_and(|h| h >= need) {
                return Some(java);
            }
        }
        if let Some(java) = which_java("java") {
            if java_version_major(&java).is_some_and(|h| h >= need) {
                return Some(java);
            }
        }
        return None;
    }

    for root in &roots {
        if let Some(java) = find_java_exe_under(root) {
            return Some(java);
        }
    }
    java_home_exe().or_else(|| which_java("java"))
}

/// Locate or download a JRE/JDK that satisfies `major` (game `javaVersion`).
/// Cache lives under `<data_root>/cache/java-runtimes/major-<N>/` (Adoptium).
pub fn ensure_java(
    major: Option<u32>,
    data_root: &Path,
    loadout_id: i64,
    progress: ProgressCb<'_>,
) -> Result<PathBuf, String> {
    let cache_roots = major
        .map(|m| vec![java_cache_dir(data_root, m)])
        .unwrap_or_default();
    if let Some(java) = find_java_with_major_in(major, &cache_roots) {
        if let Some(need) = major {
            if java_version_major(&java).is_some_and(|h| h >= need) {
                return Ok(java);
            }
        } else {
            return Ok(java);
        }
    }

    let Some(need) = major else {
        return Err(
            "Java introuvable — installez Minecraft (CurseForge / launcher) ou un JDK, puis réessayez."
                .to_string(),
        );
    };

    let dest = java_cache_dir(data_root, need);
    if let Some(java) = find_java_exe_under(&dest) {
        if java_version_major(&java).is_some_and(|h| h >= need) {
            return Ok(java);
        }
    }

    emit(
        progress,
        loadout_id,
        "java",
        &format!("Téléchargement de Java {need} (Adoptium)…"),
        false,
    );
    download_adoptium_jre(need, &dest, loadout_id, progress)?;
    find_java_exe_under(&dest)
        .filter(|java| java_version_major(java).is_some_and(|h| h >= need))
        .ok_or_else(|| {
            format!(
                "Java {need} introuvable après téléchargement — vérifiez le réseau, ou installez un JDK {need}+ (Adoptium / launcher Minecraft), puis réessayez."
            )
        })
}

fn java_cache_dir(data_root: &Path, major: u32) -> PathBuf {
    data_root
        .join("cache")
        .join("java-runtimes")
        .join(format!("major-{major}"))
}

/// Roots where Mojang / CurseForge / Adoptium drop `java.exe` (Windows-first).
pub fn java_runtime_search_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Some(appdata) = std::env::var_os("APPDATA") {
        roots.push(PathBuf::from(&appdata).join(".minecraft").join("runtime"));
    }
    if let Some(home) = std::env::var_os("USERPROFILE") {
        let cf = PathBuf::from(home)
            .join("curseforge")
            .join("minecraft")
            .join("Install");
        roots.push(cf.join("runtime"));
        // CF also keeps runtimes flat under Install/java (e.g. java-runtime-epsilon = 25).
        roots.push(cf.join("java"));
    }
    if let Some(local) = std::env::var_os("LOCALAPPDATA") {
        let packages = PathBuf::from(local).join("Packages");
        if packages.is_dir() {
            if let Ok(read) = fs::read_dir(&packages) {
                for entry in read.flatten() {
                    let name = entry.file_name();
                    let name = name.to_string_lossy();
                    if name.contains("Minecraft") || name.contains("minecraft") {
                        roots.push(
                            entry
                                .path()
                                .join("LocalCache")
                                .join("Local")
                                .join("runtime"),
                        );
                    }
                }
            }
        }
    }
    for base in [
        PathBuf::from(r"C:\Program Files\Eclipse Adoptium"),
        PathBuf::from(r"C:\Program Files\Java"),
        PathBuf::from(r"C:\Program Files\Microsoft"),
        PathBuf::from(r"C:\Program Files\Temurin"),
    ] {
        if base.is_dir() {
            roots.push(base);
        }
    }
    roots
}

fn java_home_exe() -> Option<PathBuf> {
    let home = PathBuf::from(std::env::var_os("JAVA_HOME")?);
    let java = if cfg!(windows) {
        home.join("bin").join("java.exe")
    } else {
        home.join("bin").join("java")
    };
    java.is_file().then_some(java)
}

fn download_adoptium_jre(
    major: u32,
    dest_root: &Path,
    loadout_id: i64,
    progress: ProgressCb<'_>,
) -> Result<(), String> {
    let (os, arch) = adoptium_os_arch()?;
    // Prefer JRE; fall back to JDK if that image is missing for this major.
    let urls = [
        format!(
            "https://api.adoptium.net/v3/binary/latest/{major}/ga/{os}/{arch}/jre/hotspot/normal/eclipse?project=jdk"
        ),
        format!(
            "https://api.adoptium.net/v3/binary/latest/{major}/ga/{os}/{arch}/jdk/hotspot/normal/eclipse?project=jdk"
        ),
    ];
    let client = http_client()?;
    let zip_path = dest_root
        .parent()
        .unwrap_or(dest_root)
        .join(format!("adoptium-{major}-{os}-{arch}.zip"));
    let mut last_err = String::new();
    for url in &urls {
        emit(
            progress,
            loadout_id,
            "java",
            &format!("Téléchargement de Java {major}…"),
            false,
        );
        match download_to(
            &client, url, &zip_path, None, dest_root, loadout_id, progress,
        ) {
            Ok(()) => {
                last_err.clear();
                break;
            }
            Err(e) => {
                last_err = e;
            }
        }
    }
    if !last_err.is_empty() && !zip_path.is_file() {
        return Err(format!(
            "Impossible de télécharger Java {major} (Adoptium) — {last_err}. Installez un JDK {major}+ ou lancez le launcher Minecraft une fois."
        ));
    }
    if dest_root.is_dir() {
        let _ = fs::remove_dir_all(dest_root);
    }
    fs::create_dir_all(dest_root).map_err(io_err)?;
    emit(
        progress,
        loadout_id,
        "java",
        &format!("Extraction de Java {major}…"),
        false,
    );
    extract_zip_to(&zip_path, dest_root)?;
    let _ = fs::remove_file(&zip_path);
    Ok(())
}

fn adoptium_os_arch() -> Result<(&'static str, &'static str), String> {
    let os = if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "macos") {
        "mac"
    } else if cfg!(target_os = "linux") {
        "linux"
    } else {
        return Err("système non pris en charge pour le téléchargement Java.".to_string());
    };
    let arch = if cfg!(target_arch = "x86_64") {
        "x64"
    } else if cfg!(target_arch = "aarch64") {
        "aarch64"
    } else if cfg!(target_arch = "x86") {
        "x86"
    } else {
        return Err("architecture non prise en charge pour le téléchargement Java.".to_string());
    };
    Ok((os, arch))
}

fn extract_zip_to(zip_path: &Path, dest_root: &Path) -> Result<(), String> {
    let file = File::open(zip_path).map_err(io_err)?;
    let mut archive = ZipArchive::new(file).map_err(|e| format!("archive Java : {e}"))?;
    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .map_err(|e| format!("archive Java : {e}"))?;
        let Some(enclosed) = entry.enclosed_name() else {
            continue;
        };
        // Strip the single top-level folder Adoptium puts in the zip.
        let rel = {
            let mut comps = enclosed.components();
            let _ = comps.next();
            comps.as_path().to_path_buf()
        };
        if rel.as_os_str().is_empty() {
            continue;
        }
        let out = dest_root.join(&rel);
        if entry.is_dir() {
            fs::create_dir_all(&out).map_err(io_err)?;
        } else {
            if let Some(parent) = out.parent() {
                fs::create_dir_all(parent).map_err(io_err)?;
            }
            let mut dest = File::create(&out).map_err(io_err)?;
            std::io::copy(&mut entry, &mut dest).map_err(io_err)?;
        }
    }
    Ok(())
}

/// Locates `versions/<id>/<id>.json` for an installed loader (heuristic id,
/// then scan like `versions_installed`).
pub fn resolve_version_json(
    instance_root: &Path,
    loader: &str,
    mc: &str,
    loader_ver: Option<&str>,
) -> Result<(String, PathBuf), String> {
    let Some(json_path) = find_version_json_path(instance_root, loader, mc, loader_ver) else {
        if !instance_root.join("versions").is_dir() {
            return Err("Aucune version installée — installez le loader d’abord.".to_string());
        }
        return Err("Installez le loader d’abord.".to_string());
    };
    let version_id = json_path
        .parent()
        .and_then(|p| p.file_name())
        .map(|n| n.to_string_lossy().into_owned())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| version_folder_id(loader, mc, loader_ver));
    Ok((version_id, json_path))
}

fn install_vanilla(
    instance_root: &Path,
    data_root: &Path,
    loadout_id: i64,
    mc: &str,
    progress: ProgressCb<'_>,
) -> Result<(), String> {
    let client = http_client()?;
    emit(
        progress,
        loadout_id,
        "meta",
        "Lecture des métadonnées…",
        false,
    );
    let manifest: VersionManifest = client
        .get(PISTON_MANIFEST)
        .send()
        .map_err(net_err)?
        .json()
        .map_err(parse_err)?;
    let entry = manifest
        .versions
        .into_iter()
        .find(|v| v.id == mc)
        .ok_or_else(|| format!("version Minecraft inconnue : {mc}"))?;
    let detail: Value = client
        .get(&entry.url)
        .send()
        .map_err(net_err)?
        .json()
        .map_err(parse_err)?;
    let version_dir = instance_root.join("versions").join(mc);
    fs::create_dir_all(&version_dir).map_err(io_err)?;
    let json_path = version_dir.join(format!("{mc}.json"));
    fs::write(
        &json_path,
        serde_json::to_vec_pretty(&detail).map_err(parse_err)?,
    )
    .map_err(io_err)?;
    if let Some(client_dl) = detail.get("downloads").and_then(|d| d.get("client")) {
        let url = client_dl
            .get("url")
            .and_then(Value::as_str)
            .ok_or_else(|| "URL client manquante".to_string())?;
        let sha1 = client_dl.get("sha1").and_then(Value::as_str);
        let dest = version_dir.join(format!("{mc}.jar"));
        download_to(&client, url, &dest, sha1, data_root, loadout_id, progress)?;
    }
    if let Some(libs) = detail.get("libraries").and_then(Value::as_array) {
        for lib in libs {
            fetch_library(lib, instance_root, data_root, loadout_id, progress)?;
        }
    }
    Ok(())
}

fn install_profile_loader(
    instance_root: &Path,
    data_root: &Path,
    loadout_id: i64,
    loader: &str,
    mc: &str,
    loader_ver: &str,
    progress: ProgressCb<'_>,
) -> Result<(), String> {
    let url = match loader {
        "fabric" => {
            format!("https://meta.fabricmc.net/v2/versions/loader/{mc}/{loader_ver}/profile/json")
        }
        "quilt" => {
            format!("https://meta.quiltmc.org/v3/versions/loader/{mc}/{loader_ver}/profile/json")
        }
        other => return Err(format!("loader profil inconnu : {other}")),
    };
    emit(
        progress,
        loadout_id,
        "meta",
        "Téléchargement du profil…",
        false,
    );
    let client = http_client()?;
    let detail: Value = client
        .get(&url)
        .send()
        .map_err(net_err)?
        .json()
        .map_err(parse_err)?;
    let version_id = detail
        .get("id")
        .and_then(Value::as_str)
        .ok_or_else(|| "id de version manquant dans le profil".to_string())?;
    let version_dir = instance_root.join("versions").join(version_id);
    fs::create_dir_all(&version_dir).map_err(io_err)?;
    fs::write(
        version_dir.join(format!("{version_id}.json")),
        serde_json::to_vec_pretty(&detail).map_err(parse_err)?,
    )
    .map_err(io_err)?;
    if let Some(libs) = detail.get("libraries").and_then(Value::as_array) {
        for lib in libs {
            fetch_library(lib, instance_root, data_root, loadout_id, progress)?;
        }
    }
    if let Some(downloads) = detail.get("downloads").and_then(|d| d.get("client")) {
        let url = downloads
            .get("url")
            .and_then(Value::as_str)
            .ok_or_else(|| "URL client manquante".to_string())?;
        let sha1 = downloads.get("sha1").and_then(Value::as_str);
        let dest = version_dir.join(format!("{version_id}.jar"));
        download_to(&client, url, &dest, sha1, data_root, loadout_id, progress)?;
    }
    Ok(())
}

fn install_forge_like(
    instance_root: &Path,
    data_root: &Path,
    loadout_id: i64,
    loader: &str,
    mc: &str,
    loader_ver: &str,
    progress: ProgressCb<'_>,
) -> Result<(), String> {
    // Vanilla client for `mc` is ensured by `install()` before this runs.
    let url = match loader {
        "forge" => forge_installer_url(mc, loader_ver)?,
        "neoforge" => neoforge_installer_url(mc, loader_ver)?,
        other => return Err(format!("loader forge inconnu : {other}")),
    };
    let cache = absolutize_path(&data_root.join("cache").join("minecraft-installers"))?;
    fs::create_dir_all(&cache).map_err(io_err)?;
    let file_name = url.rsplit('/').next().unwrap_or("installer.jar");
    if file_name.to_ascii_lowercase().contains("mdk") {
        return Err(
            "Le MDK Forge n'est pas pris en charge — utilisez l'installateur client.".to_string(),
        );
    }
    let jar = cache.join(file_name);
    if !jar.is_file() {
        emit(
            progress,
            loadout_id,
            "download",
            "Téléchargement de l'installateur…",
            false,
        );
        let client = http_client()?;
        download_to(&client, &url, &jar, None, data_root, loadout_id, progress)?;
    }
    let java = find_java().ok_or_else(|| {
        "Java introuvable — installez le launcher Minecraft / CurseForge ou un JDK, puis réessayez."
            .to_string()
    })?;
    // Absolute game dir + cwd = installer cache. The Forge/NeoForge installer
    // writes `*-installer.jar.log` into cwd; if that is the Tauri crate root
    // (`crates/modlab-ui` under `tauri:dev`), the file watcher rebuilds and
    // kills the app mid-patch — leaving `*-client.jar` missing.
    //
    // Pass the jar by *relative* name under that cwd. Windows `canonicalize`
    // yields `\\?\…` paths; `java -jar` cannot load those → ClassNotFoundException
    // for `net.minecraftforge.installer.SimpleInstaller` (Forge and NeoForge).
    let instance_abs = absolutize_path(instance_root)?;
    emit(
        progress,
        loadout_id,
        "install",
        "Exécution de l'installateur…",
        false,
    );
    let mut cmd = Command::new(&java);
    cmd.current_dir(&cache);
    cmd.arg("-jar").arg(file_name);
    if loader == "neoforge" {
        cmd.arg("--install-client").arg(&instance_abs);
    } else {
        cmd.arg("--installClient").arg(&instance_abs);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let output = cmd.output().map_err(|e| format!("échec java : {e}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let detail = if !stderr.trim().is_empty() {
            stderr
        } else {
            stdout
        };
        return Err(format!("installateur en échec : {detail}"));
    }
    verify_forge_like_artifacts(&instance_abs, loader, mc, loader_ver)?;
    Ok(())
}

/// Refuse a half-finished Forge/NeoForge tree (JSON present, patched jars absent).
fn verify_forge_like_artifacts(
    instance_root: &Path,
    loader: &str,
    mc: &str,
    loader_ver: &str,
) -> Result<(), String> {
    let (version_id, json_path) =
        resolve_version_json(instance_root, loader, mc, Some(loader_ver))?;
    let raw = fs::read_to_string(&json_path).map_err(io_err)?;
    let detail: Value = serde_json::from_str(&raw).map_err(parse_err)?;
    let mut missing = missing_installer_libraries(instance_root, &detail);
    if loader == "neoforge" && !neoforge_uses_legacy_forge_artifact(mc) {
        missing.extend(missing_neoforge_processor_artifacts(
            instance_root,
            loader_ver,
        ));
    }
    if missing.is_empty() {
        return Ok(());
    }
    let sample: Vec<&str> = missing.iter().take(3).map(String::as_str).collect();
    Err(format!(
        "Installation {version_id} incomplète ({} artefact(s) manquant(s), ex. {}). Relancez « Installer … » et laissez l’opération se terminer — ne fermez pas ModLab pendant le patch.",
        missing.len(),
        sample.join(", ")
    ))
}

/// Drop a prior Forge/NeoForge version folder + processor outputs so a forced
/// reinstall re-runs PROCESS_MINECRAFT_JAR instead of trusting stale files.
fn wipe_forge_like_install(
    instance_root: &Path,
    loader: &str,
    mc: &str,
    loader_ver: &str,
) -> Result<(), String> {
    if let Some(json_path) = find_version_json_path(instance_root, loader, mc, Some(loader_ver)) {
        if let Some(dir) = json_path.parent() {
            if dir.is_dir() {
                fs::remove_dir_all(dir).map_err(io_err)?;
            }
        }
    }
    if loader == "neoforge" && !neoforge_uses_legacy_forge_artifact(mc) {
        for rel in [
            neoforge_patched_client_rel(loader_ver),
            neoforge_universal_rel(loader_ver),
        ] {
            let jar = instance_root.join("libraries").join(&rel);
            if jar.is_file() {
                let _ = fs::remove_file(&jar);
            }
            if let Some(parent) = jar.parent() {
                let _ = fs::remove_dir(parent);
            }
        }
    }
    Ok(())
}

fn absolutize_path(path: &Path) -> Result<PathBuf, String> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        let cwd = std::env::current_dir().map_err(|e| format!("répertoire courant : {e}"))?;
        cwd.join(path)
    };
    let canonical = absolute.canonicalize().unwrap_or(absolute);
    // Strip Windows verbatim prefix before handing paths to the JVM.
    Ok(strip_windows_verbatim(&canonical))
}

/// Windows `std::fs::canonicalize` returns `\\?\C:\…` (and `\\?\UNC\…`).
/// The HotSpot `-jar` / classpath loader fails on those paths with
/// `ClassNotFoundException` for the jar Main-Class.
fn strip_windows_verbatim(path: &Path) -> PathBuf {
    let s = path.to_string_lossy();
    if let Some(rest) = s.strip_prefix(r"\\?\UNC\") {
        PathBuf::from(format!(r"\\{rest}"))
    } else if let Some(rest) = s.strip_prefix(r"\\?\") {
        PathBuf::from(rest)
    } else {
        path.to_path_buf()
    }
}

fn list_fabric_loaders(mc: &str) -> Result<Vec<String>, String> {
    let client = http_client()?;
    let url = format!("https://meta.fabricmc.net/v2/versions/loader/{mc}");
    let rows: Vec<FabricLoaderRow> = client
        .get(&url)
        .send()
        .map_err(net_err)?
        .json()
        .map_err(parse_err)?;
    Ok(rows.into_iter().map(|r| r.loader.version).collect())
}

fn list_quilt_loaders(mc: &str) -> Result<Vec<String>, String> {
    let client = http_client()?;
    let url = format!("https://meta.quiltmc.org/v3/versions/loader/{mc}");
    let rows: Vec<QuiltLoaderRow> = client
        .get(&url)
        .send()
        .map_err(net_err)?
        .json()
        .map_err(parse_err)?;
    Ok(rows.into_iter().map(|r| r.loader.version).collect())
}

fn list_forge_versions(mc: &str) -> Result<Vec<String>, String> {
    let client = http_client()?;
    let url =
        "https://maven.minecraftforge.net/net/minecraftforge/forge/maven-metadata.xml".to_string();
    let body = client
        .get(url)
        .send()
        .map_err(net_err)?
        .text()
        .map_err(net_err)?;
    parse_maven_versions_for_mc(&body, mc)
}

fn list_neoforge_versions(mc: &str) -> Result<Vec<String>, String> {
    let client = http_client()?;
    let mc = mc.trim();
    let body = if neoforge_uses_legacy_forge_artifact(mc) {
        let url = "https://maven.neoforged.net/releases/net/neoforged/forge/maven-metadata.xml";
        client
            .get(url)
            .send()
            .map_err(net_err)?
            .text()
            .map_err(net_err)?
    } else {
        let url = "https://maven.neoforged.net/releases/net/neoforged/neoforge/maven-metadata.xml";
        client
            .get(url)
            .send()
            .map_err(net_err)?
            .text()
            .map_err(net_err)?
    };
    let mut out = if neoforge_uses_legacy_forge_artifact(mc) {
        parse_maven_versions_for_mc(&body, mc)?
    } else {
        let prefix = neoforge_modern_prefix(mc);
        parse_maven_versions_with_prefix(&body, &prefix)?
    };
    sort_loader_versions(&mut out);
    Ok(out)
}

fn parse_maven_versions_for_mc(xml: &str, mc: &str) -> Result<Vec<String>, String> {
    let prefix = format!("{mc}-");
    let mut out = Vec::new();
    for line in xml.lines() {
        let line = line.trim();
        if !line.starts_with("<version>") {
            continue;
        }
        let ver = line
            .trim_start_matches("<version>")
            .trim_end_matches("</version>");
        if ver.starts_with(&prefix) {
            out.push(ver.strip_prefix(&prefix).unwrap_or(ver).to_string());
        }
    }
    out.reverse();
    Ok(out)
}

fn parse_maven_versions_with_prefix(xml: &str, prefix: &str) -> Result<Vec<String>, String> {
    let mut out = Vec::new();
    for line in xml.lines() {
        let line = line.trim();
        if !line.starts_with("<version>") {
            continue;
        }
        let ver = line
            .trim_start_matches("<version>")
            .trim_end_matches("</version>");
        if ver.starts_with(prefix) {
            out.push(ver.to_string());
        }
    }
    Ok(out)
}

fn sort_loader_versions(versions: &mut [String]) {
    versions.sort_by(|a, b| {
        let a_pre = loader_version_is_prerelease(a);
        let b_pre = loader_version_is_prerelease(b);
        match a_pre.cmp(&b_pre) {
            Ordering::Equal => compare_versions_desc(a, b),
            ord => ord,
        }
    });
}

fn loader_version_is_prerelease(ver: &str) -> bool {
    let low = ver.to_ascii_lowercase();
    low.contains("-beta") || low.contains("-alpha")
}

fn compare_versions_desc(a: &str, b: &str) -> Ordering {
    let a_parts = version_sort_key(a);
    let b_parts = version_sort_key(b);
    for (ap, bp) in a_parts.iter().zip(b_parts.iter()) {
        match ap.cmp(bp) {
            Ordering::Equal => continue,
            ord => return ord.reverse(),
        }
    }
    a_parts.len().cmp(&b_parts.len()).reverse()
}

fn version_sort_key(ver: &str) -> Vec<u64> {
    let stem = ver.split('-').next().unwrap_or(ver);
    stem.split('.')
        .filter_map(|p| p.parse::<u64>().ok())
        .collect()
}

pub fn fetch_library(
    lib: &Value,
    instance_root: &Path,
    data_root: &Path,
    loadout_id: i64,
    progress: ProgressCb<'_>,
) -> Result<(), String> {
    if let Some(rules) = lib.get("rules").and_then(Value::as_array) {
        if !rules_allow(rules) {
            return Ok(());
        }
    }
    // Natives-only entries are handled by extract_natives, not the classpath.
    let Some(path) = library_relative_path(lib) else {
        return Ok(());
    };
    let client = http_client()?;
    let dest = instance_root.join("libraries").join(&path);
    if dest.is_file() {
        return Ok(());
    }
    if let Some(downloads) = lib.get("downloads").and_then(|d| d.get("artifact")) {
        let url = downloads
            .get("url")
            .and_then(Value::as_str)
            .filter(|u| !u.is_empty());
        let sha1 = downloads.get("sha1").and_then(Value::as_str);
        if let Some(url) = url {
            fs::create_dir_all(dest.parent().unwrap()).map_err(io_err)?;
            download_to(&client, url, &dest, sha1, data_root, loadout_id, progress)?;
            return Ok(());
        }
        // Forge/NeoForge installer-produced jars often have a path but no URL.
        // Do not invent a Mojang CDN download — that yields HTML-as-jar noise.
        // Typical after an interrupted install (patched `*-client.jar` missing).
        return Err(format!(
            "bibliothèque locale absente (installeur) : {path} — réinstallez le loader via « Installer … » et laissez l’opération se terminer."
        ));
    }
    let url = format!("https://libraries.minecraft.net/{path}");
    fs::create_dir_all(dest.parent().unwrap()).map_err(io_err)?;
    download_to(&client, &url, &dest, None, data_root, loadout_id, progress)?;
    Ok(())
}

fn download_to(
    client: &reqwest::blocking::Client,
    url: &str,
    dest: &Path,
    expected_sha1: Option<&str>,
    _data_root: &Path,
    loadout_id: i64,
    progress: ProgressCb<'_>,
) -> Result<(), String> {
    if dest.is_file() {
        if let Some(sha) = expected_sha1 {
            if file_sha1(dest)? == sha {
                return Ok(());
            }
        } else {
            return Ok(());
        }
    }
    emit(progress, loadout_id, "download", "Téléchargement…", false);
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

/// Mojang version-JSON OS names: `windows` / `osx` / `linux`.
fn current_os_name() -> &'static str {
    if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "macos") {
        "osx"
    } else {
        "linux"
    }
}

/// Mojang `os.arch` values commonly used in version JSON (`x86` / `x86_64`).
fn current_os_arch() -> &'static str {
    if cfg!(target_arch = "x86_64") {
        "x86_64"
    } else if cfg!(target_arch = "x86") {
        "x86"
    } else if cfg!(target_arch = "aarch64") {
        "arm64"
    } else {
        "unknown"
    }
}

/// Evaluate Mojang-style `rules` for the current host OS / arch / features.
/// Empty rules → allowed. Default is deny until an applicable `allow` wins.
///
/// Feature gates (`is_demo_user`, `has_custom_resolution`, Quick Play, …) are
/// all **off** for a normal ModLab « Jouer » session. A rule that requires any
/// feature to be `true` therefore does not apply — otherwise every
/// `--quickPlay*` flag is passed at once and the client crashes with
/// `Only one quick play option can be specified`.
pub fn rules_allow(rules: &[Value]) -> bool {
    if rules.is_empty() {
        return true;
    }
    let mut allow = false;
    for rule in rules {
        let action = rule
            .get("action")
            .and_then(Value::as_str)
            .unwrap_or("allow");
        let os = rule.get("os");
        let name_match = os
            .and_then(|os| os.get("name"))
            .and_then(Value::as_str)
            .map(|name| name == current_os_name())
            .unwrap_or(true);
        let arch_match = os
            .and_then(|os| os.get("arch"))
            .and_then(Value::as_str)
            .map(|arch| arch == current_os_arch())
            .unwrap_or(true);
        let features_match = rule_features_match(rule);
        if name_match && arch_match && features_match {
            allow = action == "allow";
        }
    }
    allow
}

/// Whether this rule's `features` block matches a normal (non-demo, no Quick
/// Play, no custom resolution) launch. Missing `features` → match.
fn rule_features_match(rule: &Value) -> bool {
    let Some(wanted) = rule.get("features").and_then(Value::as_object) else {
        return true;
    };
    // Enabled launcher features for Jouer: all false until ModLab grows
    // demo / resolution / Quick Play UI toggles.
    wanted.iter().all(|(_key, val)| {
        let required = val.as_bool().unwrap_or(false);
        let enabled = false;
        enabled == required
    })
}

pub fn maven_path(name: &str) -> String {
    let parts: Vec<&str> = name.split(':').collect();
    if parts.len() < 3 {
        return name.replace('.', "/");
    }
    let group = parts[0].replace('.', "/");
    let artifact = parts[1];
    let version = parts[2];
    // Mojang / Forge coords: `group:artifact:version` or
    // `group:artifact:version:classifier` → `artifact-version-classifier.jar`.
    let file = if parts.len() > 3 {
        format!("{}-{}-{}.jar", artifact, version, parts[3])
    } else {
        format!("{artifact}-{version}.jar")
    };
    format!("{group}/{artifact}/{version}/{file}")
}

/// Relative path of a **classpath** jar (not natives-only classifiers).
/// Prefers `downloads.artifact.path` (Forge/NeoForge patched names), else
/// derives from `name`. Returns `None` when the entry is natives-only.
pub fn library_relative_path(lib: &Value) -> Option<String> {
    if let Some(artifact) = lib.get("downloads").and_then(|d| d.get("artifact")) {
        if let Some(path) = artifact
            .get("path")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|p| !p.is_empty())
        {
            return Some(path.replace('\\', "/"));
        }
        return lib.get("name").and_then(Value::as_str).map(maven_path);
    }
    // `downloads` present but no `artifact` → classifiers/natives only.
    if lib.get("downloads").is_some() {
        return None;
    }
    // Legacy version JSON without a downloads block.
    lib.get("name").and_then(Value::as_str).map(maven_path)
}

fn forge_coordinate(mc: &str, ver: &str) -> Result<String, String> {
    if ver.contains('-') {
        Ok(ver.to_string())
    } else {
        Ok(format!("{mc}-{ver}"))
    }
}

pub fn version_folder_id(loader: &str, mc: &str, loader_ver: Option<&str>) -> String {
    match loader {
        "vanilla" => mc.to_string(),
        "fabric" | "quilt" => {
            if let Some(lv) = loader_ver {
                format!("{loader}-{mc}-{lv}")
            } else {
                format!("{loader}-{mc}")
            }
        }
        "forge" | "neoforge" => {
            if let Some(lv) = loader_ver {
                format!("{mc}-{lv}")
            } else {
                mc.to_string()
            }
        }
        _ => mc.to_string(),
    }
}

fn file_sha1(path: &Path) -> Result<String, String> {
    let mut file = File::open(path).map_err(io_err)?;
    let mut hasher = Sha1::new();
    let mut buf = [0u8; 8192];
    loop {
        let n = file.read(&mut buf).map_err(io_err)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn hex_sha1(bytes: &[u8]) -> String {
    let mut hasher = Sha1::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

fn find_java_exe_matching_major(root: &Path, major: u32) -> Option<PathBuf> {
    let major_s = major.to_string();
    let java_name = if cfg!(windows) { "java.exe" } else { "java" };
    let mut stack = vec![root.to_path_buf()];
    let mut candidates = Vec::new();
    while let Some(dir) = stack.pop() {
        if let Ok(read) = fs::read_dir(&dir) {
            for entry in read.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                    if name.contains(&major_s) {
                        if let Some(java) = find_java_exe_under(&path) {
                            if java_version_major(&java).is_some_and(|h| h >= major) {
                                return Some(java);
                            }
                        }
                    }
                    stack.push(path);
                } else if path.file_name().and_then(|n| n.to_str()) == Some(java_name) {
                    let parent = path.to_string_lossy();
                    if parent.contains(&major_s) {
                        if java_version_major(&path).is_some_and(|h| h >= major) {
                            return Some(path);
                        }
                    }
                    candidates.push(path);
                }
            }
        }
    }
    for java in candidates {
        if java_version_major(&java) == Some(major) {
            return Some(java);
        }
    }
    None
}

fn find_all_java_exe_under(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let java_name = if cfg!(windows) { "java.exe" } else { "java" };
    if !root.is_dir() {
        return out;
    }
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        if let Ok(read) = fs::read_dir(&dir) {
            for entry in read.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    stack.push(path);
                } else if path.file_name().and_then(|n| n.to_str()) == Some(java_name) {
                    out.push(path);
                }
            }
        }
    }
    out
}

/// Parse the major version from `java -version` (e.g. `21` from `21.0.2`, `8` from `1.8.0`).
pub fn java_version_major(java: &Path) -> Option<u32> {
    let output = Command::new(java).arg("-version").output().ok()?;
    let stderr = String::from_utf8_lossy(&output.stderr);
    let first = stderr.lines().next()?;
    let quote = first.split('"').nth(1)?;
    let major = quote.split('.').next()?;
    if major == "1" {
        quote.split('.').nth(1)?.parse().ok()
    } else {
        major.parse().ok()
    }
}

fn find_java_exe_under(root: &Path) -> Option<PathBuf> {
    find_all_java_exe_under(root).into_iter().next()
}

fn which_java(cmd: &str) -> Option<PathBuf> {
    let output = Command::new("where").arg(cmd).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let line = stdout.lines().next()?.trim();
    if line.is_empty() {
        None
    } else {
        Some(PathBuf::from(line))
    }
}

pub fn http_client() -> Result<reqwest::blocking::Client, String> {
    reqwest::blocking::Client::builder()
        .user_agent("ModLab/0.0.1")
        .build()
        .map_err(|e| e.to_string())
}

fn emit(progress: ProgressCb<'_>, loadout_id: i64, phase: &str, message: &str, done: bool) {
    (progress)(LoaderProgress {
        loadout_id: Some(loadout_id),
        phase: phase.to_string(),
        message: message.to_string(),
        done,
    });
}

fn net_err(e: reqwest::Error) -> String {
    format!("réseau : {e}")
}

fn parse_err<E: std::fmt::Display>(e: E) -> String {
    format!("données : {e}")
}

fn io_err(e: std::io::Error) -> String {
    format!("fichier : {e}")
}

#[derive(Deserialize)]
struct VersionManifest {
    versions: Vec<ManifestEntry>,
}

#[derive(Deserialize)]
struct ManifestEntry {
    id: String,
    url: String,
    #[serde(rename = "type")]
    r#type: String,
}

#[derive(Deserialize)]
struct FabricLoaderRow {
    loader: FabricLoaderVersion,
}

#[derive(Deserialize)]
struct FabricLoaderVersion {
    version: String,
}

#[derive(Deserialize)]
struct QuiltLoaderRow {
    loader: QuiltLoaderVersion,
}

#[derive(Deserialize)]
struct QuiltLoaderVersion {
    version: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::tempdir;

    #[test]
    fn strip_windows_verbatim_drive_and_unc() {
        assert_eq!(
            strip_windows_verbatim(Path::new(r"\\?\F:\ModLab\cache\minecraft-installers")),
            PathBuf::from(r"F:\ModLab\cache\minecraft-installers")
        );
        assert_eq!(
            strip_windows_verbatim(Path::new(r"\\?\UNC\server\share\mods")),
            PathBuf::from(r"\\server\share\mods")
        );
        assert_eq!(
            strip_windows_verbatim(Path::new(r"F:\ModLab\cache")),
            PathBuf::from(r"F:\ModLab\cache")
        );
    }

    #[test]
    fn forge_installer_url_for_1_20_1() {
        let url = forge_installer_url("1.20.1", "47.2.0").unwrap();
        assert!(url.contains("minecraftforge.net"));
        assert!(url.contains("1.20.1-47.2.0-installer.jar"));
    }

    #[test]
    fn neoforge_installer_url_for_1_20_1() {
        let url = neoforge_installer_url("1.20.1", "47.1.79").unwrap();
        assert!(url.contains("neoforged.net"));
        assert!(url.contains("/net/neoforged/forge/"));
        assert!(url.contains("forge-1.20.1-47.1.79-installer.jar"));
    }

    #[test]
    fn neoforge_installer_url_for_1_21_1() {
        let url = neoforge_installer_url("1.21.1", "21.1.128").unwrap();
        assert!(url.contains("/net/neoforged/neoforge/21.1.128/"));
        assert!(url.ends_with("neoforge-21.1.128-installer.jar"));
    }

    #[test]
    fn neoforge_version_prefix_mapping() {
        assert_eq!(neoforge_version_prefix_for_mc("1.20.2").unwrap(), "20.2.");
        assert_eq!(neoforge_version_prefix_for_mc("1.21.1").unwrap(), "21.1.");
        assert_eq!(neoforge_version_prefix_for_mc("1.21").unwrap(), "21.0.");
        assert_eq!(neoforge_version_prefix_for_mc("26.1").unwrap(), "26.1.0.");
        assert_eq!(neoforge_version_prefix_for_mc("26.1.0").unwrap(), "26.1.0.");
        assert_eq!(neoforge_version_prefix_for_mc("26.1.2").unwrap(), "26.1.2.");
        assert!(neoforge_version_prefix_for_mc("1.20.1").is_err());
    }

    #[test]
    fn neoforge_modern_prefix_filters_xml() {
        let xml = r#"
        <versions>
          <version>21.1.100</version>
          <version>21.1.5-beta</version>
          <version>21.0.3</version>
          <version>20.2.1</version>
        </versions>"#;
        let rows = parse_maven_versions_with_prefix(xml, "21.1.").unwrap();
        assert_eq!(rows, vec!["21.1.100", "21.1.5-beta"]);
    }

    #[test]
    fn sort_loader_versions_stable_before_prerelease() {
        let mut rows = vec![
            "21.1.5-beta".to_string(),
            "21.1.100".to_string(),
            "21.1.99-alpha".to_string(),
            "21.1.101".to_string(),
        ];
        sort_loader_versions(&mut rows);
        assert_eq!(
            rows,
            vec!["21.1.101", "21.1.100", "21.1.99-alpha", "21.1.5-beta",]
        );
    }

    #[test]
    fn refuses_mdk_version_string() {
        assert!(refuse_mdk(Some("1.20.1-mdk")).is_err());
    }

    #[test]
    fn ensure_vanilla_skips_when_version_folder_exists() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        let data = tempdir().unwrap();
        let ver = "1.20.1";
        let folder = root.join("versions").join(ver);
        fs::create_dir_all(&folder).unwrap();
        fs::write(folder.join(format!("{ver}.json")), b"{}").unwrap();
        let progress = |_p: LoaderProgress| {};
        ensure_vanilla(root, data.path(), 1, ver, &progress).unwrap();
    }

    #[test]
    fn assert_loader_accepts_vanilla_without_lookup() {
        assert!(assert_loader_version_for_mc("vanilla", "1.20.1", "").is_ok());
    }

    #[test]
    fn creates_launcher_profiles_stub() {
        let dir = tempdir().unwrap();
        ensure_launcher_profiles(dir.path()).unwrap();
        let text = fs::read_to_string(dir.path().join("launcher_profiles.json")).unwrap();
        assert_eq!(text, r#"{"profiles":{}}"#);
    }

    #[test]
    fn java_search_roots_include_minecraft_and_curseforge() {
        let roots = java_runtime_search_roots();
        let joined: Vec<String> = roots
            .iter()
            .map(|p| p.to_string_lossy().to_ascii_lowercase())
            .collect();
        assert!(
            joined
                .iter()
                .any(|p| p.contains(".minecraft") && p.ends_with("runtime")),
            "expected .minecraft/runtime in {joined:?}"
        );
        assert!(
            joined
                .iter()
                .any(|p| p.contains("curseforge") && p.ends_with("java")),
            "expected curseforge/.../Install/java in {joined:?}"
        );
        assert!(
            joined
                .iter()
                .any(|p| p.contains("curseforge") && p.ends_with("runtime")),
            "expected curseforge/.../Install/runtime in {joined:?}"
        );
    }

    #[test]
    fn find_java_25_from_curseforge_install_java_when_present() {
        let cf_java = std::env::var_os("USERPROFILE").map(|h| {
            PathBuf::from(h)
                .join("curseforge")
                .join("minecraft")
                .join("Install")
                .join("java")
        });
        let Some(cf_java) = cf_java.filter(|p| p.is_dir()) else {
            return;
        };
        let java = find_java_with_major_in(Some(25), &[cf_java]);
        let Some(java) = java else {
            // Machine may lack epsilon — skip rather than fail CI.
            return;
        };
        assert_eq!(
            java_version_major(&java),
            Some(25),
            "expected Java 25 at {}",
            java.display()
        );
    }

    #[test]
    fn java_cache_dir_is_under_data_root() {
        let dir = tempdir().unwrap();
        let cache = java_cache_dir(dir.path(), 25);
        assert!(
            cache.ends_with(Path::new("cache/java-runtimes/major-25"))
                || cache.ends_with(Path::new(r"cache\java-runtimes\major-25"))
        );
    }

    #[test]
    fn extract_zip_strips_top_level_folder() {
        let dir = tempdir().unwrap();
        let zip_path = dir.path().join("jre.zip");
        {
            let file = File::create(&zip_path).unwrap();
            let mut zip = zip::ZipWriter::new(file);
            let opts = zip::write::SimpleFileOptions::default();
            zip.add_directory("jdk-25.0.0-jre/", opts).unwrap();
            zip.start_file("jdk-25.0.0-jre/bin/java.exe", opts).unwrap();
            zip.write_all(b"fake-java").unwrap();
            zip.finish().unwrap();
        }
        let dest = dir.path().join("out");
        fs::create_dir_all(&dest).unwrap();
        extract_zip_to(&zip_path, &dest).unwrap();
        let java = dest.join("bin").join("java.exe");
        assert!(java.is_file(), "expected {}", java.display());
        assert_eq!(fs::read(&java).unwrap(), b"fake-java");
    }

    #[test]
    fn rules_allow_osx_only_matches_host() {
        let rules = serde_json::json!([{ "action": "allow", "os": { "name": "osx" } }]);
        let rules = rules.as_array().unwrap();
        #[cfg(target_os = "macos")]
        assert!(rules_allow(rules));
        #[cfg(not(target_os = "macos"))]
        assert!(!rules_allow(rules));
    }

    #[test]
    fn rules_allow_without_os_is_true() {
        let rules = serde_json::json!([{ "action": "allow" }]);
        assert!(rules_allow(rules.as_array().unwrap()));
    }

    #[test]
    fn rules_deny_feature_gated_demo_and_quick_play() {
        // Mojang 26.3 game-arg shape: allow only when the launcher enables
        // the named feature. ModLab Jouer keeps all features off.
        let demo = serde_json::json!([{
            "action": "allow",
            "features": { "is_demo_user": true }
        }]);
        assert!(!rules_allow(demo.as_array().unwrap()));

        let quick_sp = serde_json::json!([{
            "action": "allow",
            "features": { "is_quick_play_singleplayer": true }
        }]);
        assert!(!rules_allow(quick_sp.as_array().unwrap()));

        let quick_mp = serde_json::json!([{
            "action": "allow",
            "features": { "is_quick_play_multiplayer": true }
        }]);
        assert!(!rules_allow(quick_mp.as_array().unwrap()));

        let quick_path = serde_json::json!([{
            "action": "allow",
            "features": { "has_quick_plays_support": true }
        }]);
        assert!(!rules_allow(quick_path.as_array().unwrap()));

        let custom_res = serde_json::json!([{
            "action": "allow",
            "features": { "has_custom_resolution": true }
        }]);
        assert!(!rules_allow(custom_res.as_array().unwrap()));
    }

    #[test]
    fn rules_allow_when_feature_required_false() {
        // Unusual but valid: rule applies when the feature is off (our default).
        let rules = serde_json::json!([{
            "action": "allow",
            "features": { "is_demo_user": false }
        }]);
        assert!(rules_allow(rules.as_array().unwrap()));
    }

    #[test]
    fn rules_allow_then_disallow_osx_on_windows() {
        let rules = serde_json::json!([
            { "action": "allow" },
            { "action": "disallow", "os": { "name": "osx" } }
        ]);
        let rules = rules.as_array().unwrap();
        #[cfg(target_os = "windows")]
        assert!(rules_allow(rules));
        #[cfg(target_os = "macos")]
        assert!(!rules_allow(rules));
        #[cfg(target_os = "linux")]
        assert!(rules_allow(rules));
    }

    #[test]
    fn rules_reject_xstart_on_first_thread_on_windows() {
        // Mojang JVM arg shape for -XstartOnFirstThread (macOS-only).
        let item = serde_json::json!({
            "rules": [{ "action": "allow", "os": { "name": "osx" } }],
            "value": "-XstartOnFirstThread"
        });
        let rules = item["rules"].as_array().unwrap();
        #[cfg(target_os = "windows")]
        assert!(
            !rules_allow(rules),
            "-XstartOnFirstThread must not apply on Windows"
        );
        #[cfg(target_os = "macos")]
        assert!(rules_allow(rules));
    }

    #[test]
    fn rules_empty_means_allow() {
        assert!(rules_allow(&[]));
    }

    #[test]
    fn maven_path_classifier_uses_dash_and_jar() {
        assert_eq!(
            maven_path("org.lwjgl:lwjgl:3.3.1:natives-windows"),
            "org/lwjgl/lwjgl/3.3.1/lwjgl-3.3.1-natives-windows.jar"
        );
        assert_eq!(
            maven_path("com.mojang:authlib:1.0"),
            "com/mojang/authlib/1.0/authlib-1.0.jar"
        );
    }

    #[test]
    fn library_relative_path_prefers_artifact_path() {
        let lib = serde_json::json!({
            "name": "net.neoforged:neoforge:21.1.128",
            "downloads": {
                "artifact": {
                    "path": "net/neoforged/neoforge/21.1.128/neoforge-21.1.128-universal.jar"
                }
            }
        });
        assert_eq!(
            library_relative_path(&lib).as_deref(),
            Some("net/neoforged/neoforge/21.1.128/neoforge-21.1.128-universal.jar")
        );
    }

    #[test]
    fn missing_installer_libraries_detects_empty_url_client_jar() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        let rel = "net/minecraftforge/forge/26.3-66.1.9/forge-26.3-66.1.9-client.jar";
        let detail = serde_json::json!({
            "libraries": [
                {
                    "name": "net.minecraftforge:forge:26.3-66.1.9:client",
                    "downloads": {
                        "artifact": {
                            "path": rel,
                            "url": ""
                        }
                    }
                },
                {
                    "name": "net.minecraftforge:forge:26.3-66.1.9:universal",
                    "downloads": {
                        "artifact": {
                            "path": "net/minecraftforge/forge/26.3-66.1.9/forge-26.3-66.1.9-universal.jar",
                            "url": "https://maven.minecraftforge.net/…/universal.jar"
                        }
                    }
                }
            ]
        });
        let missing = missing_installer_libraries(root, &detail);
        assert_eq!(missing, vec![rel.to_string()]);
        let dest = root.join("libraries").join(rel);
        fs::create_dir_all(dest.parent().unwrap()).unwrap();
        fs::write(&dest, b"patched").unwrap();
        assert!(missing_installer_libraries(root, &detail).is_empty());
    }

    #[test]
    fn versions_installed_forge_requires_client_jar() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        let id = "26.3-forge-66.1.9";
        let version_dir = root.join("versions").join(id);
        fs::create_dir_all(&version_dir).unwrap();
        let rel = "net/minecraftforge/forge/26.3-66.1.9/forge-26.3-66.1.9-client.jar";
        let detail = serde_json::json!({
            "id": id,
            "libraries": [{
                "name": "net.minecraftforge:forge:26.3-66.1.9:client",
                "downloads": { "artifact": { "path": rel, "url": "" } }
            }]
        });
        fs::write(
            version_dir.join(format!("{id}.json")),
            serde_json::to_vec_pretty(&detail).unwrap(),
        )
        .unwrap();
        assert!(
            !versions_installed(root, "forge", "26.3", Some("66.1.9")),
            "JSON alone must not count as installed"
        );
        let jar = root.join("libraries").join(rel);
        fs::create_dir_all(jar.parent().unwrap()).unwrap();
        fs::write(&jar, b"patched").unwrap();
        assert!(versions_installed(root, "forge", "26.3", Some("66.1.9")));
    }

    #[test]
    fn versions_installed_neoforge_requires_processor_jars() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        let id = "neoforge-26.3.0.118";
        let ver = "26.3.0.118";
        let version_dir = root.join("versions").join(id);
        fs::create_dir_all(&version_dir).unwrap();
        // Modern NeoForge: version JSON libraries are downloadable (have URLs).
        // Completeness hinges on processor outputs, not empty-URL entries.
        let detail = serde_json::json!({
            "id": id,
            "libraries": [{
                "name": "net.neoforged.fancymodloader:loader:12.0.17",
                "downloads": {
                    "artifact": {
                        "path": "net/neoforged/fancymodloader/loader/12.0.17/loader-12.0.17.jar",
                        "url": "https://maven.neoforged.net/releases/net/neoforged/fancymodloader/loader/12.0.17/loader-12.0.17.jar"
                    }
                }
            }]
        });
        fs::write(
            version_dir.join(format!("{id}.json")),
            serde_json::to_vec_pretty(&detail).unwrap(),
        )
        .unwrap();
        let loader_jar =
            root.join("libraries/net/neoforged/fancymodloader/loader/12.0.17/loader-12.0.17.jar");
        fs::create_dir_all(loader_jar.parent().unwrap()).unwrap();
        fs::write(&loader_jar, b"fml").unwrap();
        assert!(
            !versions_installed(root, "neoforge", "26.3", Some(ver)),
            "JSON + CDN libs without patched client must not count as installed"
        );
        assert_eq!(
            missing_neoforge_processor_artifacts(root, ver).len(),
            2,
            "patched + universal"
        );
        for rel in [
            neoforge_patched_client_rel(ver),
            neoforge_universal_rel(ver),
        ] {
            let jar = root.join("libraries").join(&rel);
            fs::create_dir_all(jar.parent().unwrap()).unwrap();
            fs::write(&jar, b"patched").unwrap();
        }
        assert!(versions_installed(root, "neoforge", "26.3", Some(ver)));
        assert!(missing_neoforge_processor_artifacts(root, ver).is_empty());
    }

    #[test]
    fn wipe_forge_like_removes_neoforge_version_and_patched() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        let ver = "26.3.0.118";
        let id = "neoforge-26.3.0.118";
        let version_dir = root.join("versions").join(id);
        fs::create_dir_all(&version_dir).unwrap();
        fs::write(
            version_dir.join(format!("{id}.json")),
            b"{\"id\":\"neoforge\"}",
        )
        .unwrap();
        let patched = root
            .join("libraries")
            .join(neoforge_patched_client_rel(ver));
        fs::create_dir_all(patched.parent().unwrap()).unwrap();
        fs::write(&patched, b"old").unwrap();
        wipe_forge_like_install(root, "neoforge", "26.3", ver).unwrap();
        assert!(!version_dir.is_dir());
        assert!(!patched.is_file());
    }

    #[test]
    fn list_installed_game_versions_skips_loader_trees() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        let vanilla = root.join("versions").join("1.20.1");
        fs::create_dir_all(&vanilla).unwrap();
        fs::write(vanilla.join("1.20.1.json"), b"{}").unwrap();
        let forge_id = "1.20.1-47.2.0";
        let forge_dir = root.join("versions").join(forge_id);
        fs::create_dir_all(&forge_dir).unwrap();
        fs::write(
            forge_dir.join(format!("{forge_id}.json")),
            br#"{"libraries":[{"name":"net.minecraftforge:forge:1.20.1-47.2.0"}]}"#,
        )
        .unwrap();
        assert_eq!(
            list_installed_game_versions(root),
            vec!["1.20.1".to_string()]
        );
    }

    #[test]
    fn list_installed_loader_versions_fabric() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        let id = "fabric-1.20.1-0.16.0";
        let version_dir = root.join("versions").join(id);
        fs::create_dir_all(&version_dir).unwrap();
        fs::write(
            version_dir.join(format!("{id}.json")),
            br#"{"id":"fabric-1.20.1-0.16.0","libraries":[{"name":"net.fabricmc:fabric-loader:0.16.0"}]}"#,
        )
        .unwrap();
        assert_eq!(
            list_installed_loader_versions(root, "fabric", "1.20.1"),
            vec!["0.16.0".to_string()]
        );
        assert!(loader_installed_for_mc(root, "fabric", "1.20.1"));
        assert!(!loader_installed_for_mc(root, "forge", "1.20.1"));
    }

    #[test]
    fn library_relative_path_skips_natives_only() {
        let lib = serde_json::json!({
            "name": "org.lwjgl:lwjgl:3.3.1",
            "downloads": {
                "classifiers": {
                    "natives-windows": { "path": "org/lwjgl/lwjgl/3.3.1/lwjgl-3.3.1-natives-windows.jar" }
                }
            }
        });
        assert!(library_relative_path(&lib).is_none());
    }

    #[test]
    fn fabric_profile_paths_from_fixture() {
        let dir = tempdir().unwrap();
        let instance = dir.path();
        let fixture = include_str!("../tests/fixtures/fabric-profile.json");
        let detail: Value = serde_json::from_str(fixture).unwrap();
        let version_id = detail["id"].as_str().unwrap();
        let version_dir = instance.join("versions").join(version_id);
        fs::create_dir_all(&version_dir).unwrap();
        fs::write(version_dir.join(format!("{version_id}.json")), fixture).unwrap();
        let lib = &detail["libraries"][0];
        let name = lib["name"].as_str().unwrap();
        let rel = maven_path(name);
        assert!(rel.contains("net/fabricmc/fabric-loader/"));
        let progress = |_p: LoaderProgress| {};
        let client = http_client().unwrap();
        let dest = instance.join("libraries").join(&rel);
        // Dry-run path only — do not hit the network in CI: just ensure parent logic.
        fs::create_dir_all(dest.parent().unwrap()).unwrap();
        let mut f = File::create(&dest).unwrap();
        f.write_all(b"stub").unwrap();
        assert!(dest.is_file());
        let _ = client;
        let _ = progress;
    }
}
