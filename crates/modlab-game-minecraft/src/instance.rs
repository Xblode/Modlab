//! Reading — and writing into — a real modded Minecraft instance's `mods/`
//! folder. Everything here is allowed to know what a `.jar` is and how
//! Fabric/Quilt/Forge declare themselves; none of it belongs in
//! `modlab-core`. See `docs/01-architecture-decidee.md` §3.1.
//!
//! Same read/decide-then-act split as `modlab_game_skyrim::mo2`: `scan_dir`
//! and `read_jar` only parse, `to_import_entries` maps to the Core's generic
//! shape, and only `import_instance` / `install_jar` write anything.
//!
//! **How each loader declares a mod**, and what this does about it:
//! - **Fabric** — `fabric.mod.json` at the jar root (JSON: `id`, `name`,
//!   `version`).
//! - **Quilt** — `quilt.mod.json` at the jar root, everything nested under a
//!   `quilt_loader` object.
//! - **Forge and NeoForge** — both ship `META-INF/mods.toml` with the same
//!   `[[mods]]` table. NeoForge additionally accepts
//!   `META-INF/neoforge.mods.toml`, which is the *only* signal here that
//!   separates the two; a jar using the shared filename is reported as
//!   `Forge` rather than guessing between them.
//! - **Anything else** — a resource-only jar, or a format this doesn't know,
//!   still becomes an entry named from its filename with no version. It is
//!   never silently dropped: a jar sitting in `mods/` is something the human
//!   put there, and a mod manager that hides files is worse than one that
//!   says "I don't know what this is".
//!
//! Deployment note (§3.1, `deployment_label: "dossier mods/"`): unlike
//! Skyrim's USVFS overlay, which writes nothing to real disk (ADR 0001),
//! Minecraft mods are *real files copied into the game's own `mods/`
//! folder*. `install_jar` is therefore a genuine filesystem mutation and
//! cleans up after itself on failure — see its own docs.

use serde::Deserialize;
use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use modlab_core::game_adapter::GameAdapter;
use modlab_core::{loadout, Core};

/// Which loader a jar declares itself for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModLoader {
    Fabric,
    Quilt,
    /// Ships the shared `META-INF/mods.toml` — Forge, or a NeoForge mod
    /// using the compatibility filename. See the module docs.
    Forge,
    NeoForge,
    /// No loader manifest this module recognizes. Not "vanilla" as a claim
    /// about the instance — just "this jar didn't say".
    Unknown,
}

impl ModLoader {
    /// Stable id for clients (the UI badges it, the CLI prints it).
    #[must_use]
    pub fn id(self) -> &'static str {
        match self {
            ModLoader::Fabric => "fabric",
            ModLoader::Quilt => "quilt",
            ModLoader::Forge => "forge",
            ModLoader::NeoForge => "neoforge",
            ModLoader::Unknown => "unknown",
        }
    }
}

/// One `.jar` found in a `mods/` folder, with whatever its own manifest
/// declared.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledMod {
    /// The manifest's display name or id if it had one, else the jar's file
    /// stem — always something readable, never empty.
    pub name: String,
    /// `None` when no manifest declared one — never a guess.
    pub version: Option<String>,
    pub jar_path: PathBuf,
    pub loader: ModLoader,
    /// Minecraft disables a mod by renaming it `*.jar.disabled` — the same
    /// convention every launcher uses, so ModLab reads and writes it rather
    /// than inventing its own.
    pub enabled: bool,
}

/// What one `mods/` folder holds, plus the loader its jars agree on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstanceMods {
    pub loader: ModLoader,
    pub mods: Vec<InstalledMod>,
}

impl InstanceMods {
    /// Map to the Core's generic import shape — the Core never learns what a
    /// jar is, exactly as `mo2::to_import_entries` keeps it from learning
    /// what MO2 is.
    #[must_use]
    pub fn to_import_entries(&self) -> Vec<loadout::ImportEntry> {
        self.mods
            .iter()
            .map(|m| loadout::ImportEntry {
                name: m.name.clone(),
                source_path: m.jar_path.clone(),
                enabled: m.enabled,
                version: m.version.clone(),
            })
            .collect()
    }
}

#[derive(Deserialize)]
struct FabricManifest {
    id: Option<String>,
    name: Option<String>,
    version: Option<String>,
}

#[derive(Deserialize)]
struct QuiltManifest {
    quilt_loader: QuiltLoader,
}

#[derive(Deserialize)]
struct QuiltLoader {
    id: Option<String>,
    version: Option<String>,
    metadata: Option<QuiltMetadata>,
}

#[derive(Deserialize)]
struct QuiltMetadata {
    name: Option<String>,
}

#[derive(Deserialize)]
struct ForgeManifest {
    #[serde(default)]
    mods: Vec<ForgeMod>,
}

#[derive(Deserialize)]
struct ForgeMod {
    #[serde(rename = "modId")]
    mod_id: Option<String>,
    #[serde(rename = "displayName")]
    display_name: Option<String>,
    version: Option<String>,
}

/// Read one jar's loader manifest. `Ok(None)` means "opened fine, no
/// manifest this module knows" — a normal outcome, not a failure. `Err` is
/// reserved for the jar not opening at all.
pub fn read_jar(jar_path: &Path) -> io::Result<Option<(ModLoader, String, Option<String>)>> {
    let file = File::open(jar_path)?;
    let mut archive =
        zip::ZipArchive::new(file).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

    if let Some(m) = entry_as_json::<FabricManifest>(&mut archive, "fabric.mod.json") {
        let name = m.name.or(m.id).unwrap_or_else(|| jar_stem(jar_path));
        return Ok(Some((ModLoader::Fabric, name, m.version)));
    }
    if let Some(m) = entry_as_json::<QuiltManifest>(&mut archive, "quilt.mod.json") {
        let name = m
            .quilt_loader
            .metadata
            .and_then(|meta| meta.name)
            .or(m.quilt_loader.id)
            .unwrap_or_else(|| jar_stem(jar_path));
        return Ok(Some((ModLoader::Quilt, name, m.quilt_loader.version)));
    }
    // NeoForge's own filename first: it is the only thing that distinguishes
    // it from Forge without guessing (see module docs).
    for (entry_name, loader) in [
        ("META-INF/neoforge.mods.toml", ModLoader::NeoForge),
        ("META-INF/mods.toml", ModLoader::Forge),
    ] {
        if let Some(m) = entry_as_toml::<ForgeManifest>(&mut archive, entry_name) {
            if let Some(first) = m.mods.into_iter().next() {
                let name = first
                    .display_name
                    .or(first.mod_id)
                    .unwrap_or_else(|| jar_stem(jar_path));
                return Ok(Some((loader, name, first.version)));
            }
        }
    }
    Ok(None)
}

fn entry_as_json<T: for<'de> Deserialize<'de>>(
    archive: &mut zip::ZipArchive<File>,
    entry_name: &str,
) -> Option<T> {
    serde_json::from_str(&entry_text(archive, entry_name)?).ok()
}

fn entry_as_toml<T: for<'de> Deserialize<'de>>(
    archive: &mut zip::ZipArchive<File>,
    entry_name: &str,
) -> Option<T> {
    toml::from_str(&entry_text(archive, entry_name)?).ok()
}

fn entry_text(archive: &mut zip::ZipArchive<File>, entry_name: &str) -> Option<String> {
    let mut entry = archive.by_name(entry_name).ok()?;
    let mut contents = String::new();
    entry.read_to_string(&mut contents).ok()?;
    Some(contents)
}

fn jar_stem(jar_path: &Path) -> String {
    let name = jar_path.file_name().unwrap_or_default().to_string_lossy();
    // `foo.jar.disabled` → `foo`, `foo.jar` → `foo`.
    name.trim_end_matches(".disabled")
        .trim_end_matches(".jar")
        .to_string()
}

/// Is this a mod jar (enabled), a disabled one, or neither?
fn jar_state(path: &Path) -> Option<bool> {
    let name = path.file_name()?.to_string_lossy().to_ascii_lowercase();
    if name.ends_with(".jar") {
        Some(true)
    } else if name.ends_with(".jar.disabled") {
        Some(false)
    } else {
        None
    }
}

/// Every mod jar directly inside `mods_dir`, sorted by name. A missing or
/// empty folder is not an error — an instance with no mods yet is a normal
/// starting point — it just yields an empty list.
pub fn scan_dir(mods_dir: &Path) -> io::Result<InstanceMods> {
    let mut mods = Vec::new();
    if mods_dir.is_dir() {
        for entry in fs::read_dir(mods_dir)? {
            let path = entry?.path();
            let Some(enabled) = jar_state(&path) else {
                continue;
            };
            // A jar that won't open is still reported, by filename: the
            // human can see ModLab found it and decide, instead of the scan
            // quietly pretending the file isn't there.
            let found = read_jar(&path).ok().flatten();
            let (loader, name, version) =
                found.unwrap_or_else(|| (ModLoader::Unknown, jar_stem(&path), None));
            mods.push(InstalledMod {
                name,
                version,
                jar_path: path,
                loader,
                enabled,
            });
        }
    }
    mods.sort_by_key(|m| m.name.to_lowercase());
    let loader = mods
        .iter()
        .map(|m| m.loader)
        .find(|l| *l != ModLoader::Unknown)
        .unwrap_or(ModLoader::Unknown);
    Ok(InstanceMods { loader, mods })
}

/// `<instance root>/mods` — the one place Minecraft loads mods from, for
/// every loader.
#[must_use]
pub fn mods_dir(instance_root: &Path) -> PathBuf {
    instance_root.join("mods")
}

/// `<instance root>/config` — loader config (Forge, Fabric, NeoForge).
/// Same anchoring as `mods_dir`: derived from the instance root, not a
/// second detection pass.
#[must_use]
pub fn config_dir(instance_root: &Path) -> PathBuf {
    instance_root.join("config")
}

/// Import an existing modded instance into a new ModLab loadout: scan
/// `<instance_root>/mods`, map, create, fill. The counterpart of
/// `mo2::import_into`, and like it, **reads only** — not a single jar is
/// moved, renamed or copied.
pub fn import_instance(
    core: &Core,
    instance_root: &Path,
    loadout_name: &str,
) -> Result<(loadout::Loadout, usize), String> {
    if !instance_root.is_dir() {
        return Err(format!(
            "dossier d'instance introuvable : {}",
            instance_root.display()
        ));
    }
    let scanned = scan_dir(&mods_dir(instance_root)).map_err(|e| e.to_string())?;
    if scanned.mods.is_empty() {
        return Err(format!(
            "aucun mod trouvé dans {}",
            mods_dir(instance_root).display()
        ));
    }
    let entries = scanned.to_import_entries();
    let created = loadout::create(core, crate::MinecraftAdapter.id(), loadout_name)
        .map_err(|e| e.to_string())?;
    let imported = loadout::import_mods(core, created.id, &entries).map_err(|e| e.to_string())?;
    Ok((created, imported))
}

/// Copy one file into `dest_dir` under its own name.
///
/// The copy lands on a temporary name in the same directory and is renamed
/// into place. An existing file of the same name is refused. A failed copy
/// moves the temporary file to the trash instead of deleting it.
pub fn place_file(data_root: &Path, source: &Path, dest_dir: &Path) -> Result<PathBuf, String> {
    if !source.is_file() {
        return Err(format!("fichier introuvable : {}", source.display()));
    }
    fs::create_dir_all(dest_dir).map_err(|e| e.to_string())?;
    let file_name = source
        .file_name()
        .ok_or_else(|| format!("chemin sans nom de fichier : {}", source.display()))?;
    let dest = dest_dir.join(file_name);
    if dest.exists() {
        return Err(format!(
            "{} est déjà installé — désinstalle-le d'abord",
            dest.display()
        ));
    }
    let staging = dest_dir.join(format!(
        ".modlab-installing-{}",
        file_name.to_string_lossy()
    ));
    if staging.exists() {
        let _ = modlab_core::files::move_to_trash(data_root, &staging);
    }
    fs::copy(source, &staging).map_err(|e| e.to_string())?;
    if let Err(err) = fs::rename(&staging, &dest) {
        let _ = modlab_core::files::move_to_trash(data_root, &staging);
        return Err(err.to_string());
    }
    Ok(dest)
}

/// Copy one `.jar` into `mods_dir` and register it in a loadout.
///
/// Same staging rename as [`place_file`]. If the database write fails, the
/// copied jar goes to the trash — it is not deleted. `mods_dir` is the
/// instance's mods folder, not a game install root. The new row is marked
/// `source_host = manual` (disk add, no platform page).
pub fn install_jar(
    core: &Core,
    loadout_id: i64,
    source_jar: &Path,
    mods_dir: &Path,
    data_root: &Path,
) -> Result<InstalledMod, String> {
    if jar_state(source_jar) != Some(true) {
        return Err(format!("ce n'est pas un .jar : {}", source_jar.display()));
    }
    let dest = place_file(data_root, source_jar, mods_dir)?;

    match register_jar_at(core, loadout_id, &dest) {
        Ok(installed) => Ok(installed),
        Err(e) => {
            // The database never learned about this jar, so leaving it in
            // `mods/` would mean Minecraft loads a mod ModLab doesn't know it
            // installed. Undo the one thing that did land.
            let _ = modlab_core::files::move_to_trash(data_root, &dest);
            Err(e)
        }
    }
}

/// Register a `.jar` that already sits in the instance `mods/` folder.
/// No copy. Refuses a path that is not a jar (or `.jar.disabled`), and skips
/// (`Ok(None)`) when this loadout already lists that path. Marked as a
/// manual source.
pub fn register_existing_jar(
    core: &Core,
    loadout_id: i64,
    jar_path: &Path,
) -> Result<Option<InstalledMod>, String> {
    if jar_state(jar_path).is_none() {
        return Err(format!("ce n'est pas un .jar : {}", jar_path.display()));
    }
    if loadout_has_source(core, loadout_id, jar_path)? {
        return Ok(None);
    }
    register_jar_at(core, loadout_id, jar_path).map(Some)
}

/// Scan `mods_dir` and register every jar this loadout does not yet list.
/// Jars stay where they are — same read-only-on-disk idea as
/// [`import_instance`], but for an existing loadout. Returns how many rows
/// were added.
pub fn sync_mods_dir(core: &Core, loadout_id: i64, mods_dir: &Path) -> Result<usize, String> {
    let scanned = scan_dir(mods_dir).map_err(|e| e.to_string())?;
    let mut added = 0;
    for found in scanned.mods {
        if loadout_has_source(core, loadout_id, &found.jar_path)? {
            continue;
        }
        register_jar_at(core, loadout_id, &found.jar_path)?;
        added += 1;
    }
    Ok(added)
}

fn register_jar_at(core: &Core, loadout_id: i64, jar_path: &Path) -> Result<InstalledMod, String> {
    let found = read_jar(jar_path).ok().flatten();
    let (loader, name, version) =
        found.unwrap_or_else(|| (ModLoader::Unknown, jar_stem(jar_path), None));
    let enabled = jar_state(jar_path) == Some(true);
    let installed = InstalledMod {
        name: name.clone(),
        version: version.clone(),
        jar_path: jar_path.to_path_buf(),
        loader,
        enabled,
    };
    let entry = loadout::ImportEntry {
        name,
        source_path: jar_path.to_path_buf(),
        enabled,
        version,
    };
    loadout::import_mods(core, loadout_id, &[entry]).map_err(|e| e.to_string())?;
    let mod_id = entries_mod_id_for(core, loadout_id, jar_path)?;
    loadout::mark_manual_source(core, mod_id).map_err(|e| e.to_string())?;
    Ok(installed)
}

fn loadout_has_source(core: &Core, loadout_id: i64, path: &Path) -> Result<bool, String> {
    let want = path_key(path);
    let rows = loadout::entries(core, loadout_id).map_err(|e| e.to_string())?;
    Ok(rows.iter().any(|row| path_key(&row.source_path) == want))
}

fn entries_mod_id_for(core: &Core, loadout_id: i64, path: &Path) -> Result<i64, String> {
    let want = path_key(path);
    loadout::entries(core, loadout_id)
        .map_err(|e| e.to_string())?
        .into_iter()
        .find(|row| path_key(&row.source_path) == want)
        .map(|row| row.mod_id)
        .ok_or_else(|| format!("mod introuvable après import : {}", path.display()))
}

fn path_key(path: &Path) -> String {
    path.display()
        .to_string()
        .replace('\\', "/")
        .to_ascii_lowercase()
}

/// Suffix launchers use for a disabled mod jar — Prism, CurseForge, MultiMC.
/// Already the convention in this crate's scanner; keep it (not `.jardisabled`).
const JAR_DISABLED_SUFFIX: &str = ".jar.disabled";

/// `foo.jar` → `foo.jar.disabled`. Idempotent when already disabled.
fn disabled_jar_path(path: &Path) -> PathBuf {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    if name.to_ascii_lowercase().ends_with(JAR_DISABLED_SUFFIX) {
        path.to_path_buf()
    } else {
        path.with_file_name(format!("{name}.disabled"))
    }
}

/// `foo.jar.disabled` → `foo.jar`. Idempotent when already enabled.
fn enabled_jar_path(path: &Path) -> PathBuf {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let lower = name.to_ascii_lowercase();
    if lower.ends_with(".disabled") {
        let keep = name.len().saturating_sub(".disabled".len());
        path.with_file_name(&name[..keep])
    } else {
        path.to_path_buf()
    }
}

/// Find the jar on disk for a stored path, tolerating a prior desync
/// (DB says `.jar` while the file is already `.jar.disabled`, or the reverse).
fn resolve_existing_jar(stored: &Path) -> io::Result<PathBuf> {
    if stored.is_file() {
        return Ok(stored.to_path_buf());
    }
    let alt = match jar_state(stored) {
        Some(true) => disabled_jar_path(stored),
        Some(false) => enabled_jar_path(stored),
        None => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("ce n'est pas un .jar : {}", stored.display()),
            ));
        }
    };
    if alt.is_file() {
        Ok(alt)
    } else {
        Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("fichier introuvable : {}", stored.display()),
        ))
    }
}

/// Rename one jar on disk: `foo.jar` ↔ `foo.jar.disabled` to match `enabled`.
/// Returns the path that exists after the call. Idempotent when already right.
pub fn set_jar_enabled(jar_path: &Path, enabled: bool) -> io::Result<PathBuf> {
    let current = resolve_existing_jar(jar_path)?;
    let currently_enabled = jar_state(&current) == Some(true);
    if currently_enabled == enabled {
        return Ok(current);
    }
    let target = if enabled {
        enabled_jar_path(&current)
    } else {
        disabled_jar_path(&current)
    };
    if target.exists() {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!(
                "{} existe déjà — impossible de {}",
                target.display(),
                if enabled {
                    "réactiver le mod"
                } else {
                    "désactiver le mod"
                }
            ),
        ));
    }
    fs::rename(&current, &target)?;
    Ok(target)
}

/// Enable or disable a Minecraft mod: rename the jar on disk, update
/// `source_path`, then flip the loadout flag through the Core cycle.
/// Virtual loader rows (`modlab:branch-mc-loader:…`) only flip the DB flag.
pub fn set_mod_enabled(
    core: &Core,
    loadout_id: i64,
    mod_id: i64,
    enabled: bool,
) -> Result<bool, String> {
    let rows = loadout::entries(core, loadout_id).map_err(|e| e.to_string())?;
    let Some(row) = rows.into_iter().find(|r| r.mod_id == mod_id) else {
        return loadout::set_enabled(core, loadout_id, mod_id, enabled).map_err(|e| e.to_string());
    };

    let path_text = row.source_path.to_string_lossy();
    if path_text.starts_with(crate::branch_loader::SOURCE_PREFIX) {
        return loadout::set_enabled(core, loadout_id, mod_id, enabled).map_err(|e| e.to_string());
    }
    if jar_state(&row.source_path).is_none() {
        return loadout::set_enabled(core, loadout_id, mod_id, enabled).map_err(|e| e.to_string());
    }

    let new_path = set_jar_enabled(&row.source_path, enabled).map_err(|e| e.to_string())?;
    if path_key(&new_path) != path_key(&row.source_path) {
        modlab_core::download::relocate_source(core, mod_id, &new_path)
            .map_err(|e| e.to_string())?;
    }
    loadout::set_enabled(core, loadout_id, mod_id, enabled).map_err(|e| e.to_string())
}

/// Remove a Minecraft mod from the loadout **and** from the instance `mods/`
/// folder. The jar (`.jar` or `.jar.disabled`) and any companion files the
/// adapter recorded are moved to the data-root trash — never deleted outright.
/// If the soft-delete fails, trash guards restore the files. Virtual loader
/// rows only leave the list (no disk path).
pub fn remove_mod(
    core: &Core,
    data_root: &Path,
    loadout_id: i64,
    mod_id: i64,
) -> Result<(), String> {
    let rows = loadout::entries(core, loadout_id).map_err(|e| e.to_string())?;
    let Some(row) = rows.into_iter().find(|r| r.mod_id == mod_id) else {
        return modlab_core::download::remove(core, mod_id).map_err(|e| e.to_string());
    };

    let path_text = row.source_path.to_string_lossy();
    if path_text.starts_with(crate::branch_loader::SOURCE_PREFIX) {
        return modlab_core::download::remove(core, mod_id).map_err(|e| e.to_string());
    }

    let mut to_trash: Vec<PathBuf> = Vec::new();
    if jar_state(&row.source_path).is_some() {
        match resolve_existing_jar(&row.source_path) {
            Ok(path) => to_trash.push(path),
            Err(err) if err.kind() == io::ErrorKind::NotFound => {}
            Err(err) => return Err(err.to_string()),
        }
    }
    if let Some(extra) = crate::install::retract(data_root, mod_id, &row.source_path) {
        for path in extra {
            if !to_trash
                .iter()
                .any(|kept| path_key(kept) == path_key(&path))
            {
                to_trash.push(path);
            }
        }
    }

    let mut guards = Vec::new();
    for path in &to_trash {
        if let Some(guard) = modlab_core::files::trash_for_rollback(data_root, path)
            .map_err(|e| e.to_string())?
        {
            guards.push(guard);
        }
    }

    match modlab_core::download::remove(core, mod_id) {
        Ok(()) => {
            for guard in guards {
                guard.commit();
            }
            Ok(())
        }
        Err(err) => {
            drop(guards);
            Err(err.to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use zip::write::SimpleFileOptions;

    #[test]
    fn mods_and_config_are_children_of_the_instance_root() {
        let root = Path::new(r"F:\instances\fabric");
        assert_eq!(mods_dir(root), root.join("mods"));
        assert_eq!(config_dir(root), root.join("config"));
    }

    /// Real, minimal zip fixtures built at test time — `fixtures/README.md`
    /// explains why these are not checked in as binary files: a format that
    /// actually gets parsed needs a fixture generated against that format.
    fn jar_with(dir: &Path, file_name: &str, entries: &[(&str, &str)]) -> PathBuf {
        let path = dir.join(file_name);
        let file = File::create(&path).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        for (entry_name, contents) in entries {
            zip.start_file(*entry_name, SimpleFileOptions::default())
                .unwrap();
            zip.write_all(contents.as_bytes()).unwrap();
        }
        zip.finish().unwrap();
        path
    }

    fn fabric_jar(dir: &Path, file_name: &str, id: &str, name: &str, version: &str) -> PathBuf {
        jar_with(
            dir,
            file_name,
            &[(
                "fabric.mod.json",
                &format!(
                    r#"{{"schemaVersion":1,"id":"{id}","name":"{name}","version":"{version}"}}"#
                ),
            )],
        )
    }

    #[test]
    fn reads_a_fabric_manifest() {
        let dir = tempfile::tempdir().unwrap();
        let jar = fabric_jar(dir.path(), "sodium.jar", "sodium", "Sodium", "0.5.8");

        let (loader, name, version) = read_jar(&jar).unwrap().expect("manifest found");

        assert_eq!(loader, ModLoader::Fabric);
        assert_eq!(name, "Sodium");
        assert_eq!(version.as_deref(), Some("0.5.8"));
    }

    #[test]
    fn reads_a_quilt_manifest() {
        let dir = tempfile::tempdir().unwrap();
        let jar = jar_with(
            dir.path(),
            "qsl.jar",
            &[(
                "quilt.mod.json",
                r#"{"quilt_loader":{"id":"qsl","version":"7.0.0",
                    "metadata":{"name":"Quilt Standard Libraries"}}}"#,
            )],
        );

        let (loader, name, version) = read_jar(&jar).unwrap().expect("manifest found");

        assert_eq!(loader, ModLoader::Quilt);
        assert_eq!(name, "Quilt Standard Libraries");
        assert_eq!(version.as_deref(), Some("7.0.0"));
    }

    #[test]
    fn reads_a_forge_manifest() {
        let dir = tempfile::tempdir().unwrap();
        let jar = jar_with(
            dir.path(),
            "jei.jar",
            &[(
                "META-INF/mods.toml",
                "modLoader=\"javafml\"\nloaderVersion=\"[47,)\"\n\n\
                 [[mods]]\nmodId=\"jei\"\nversion=\"15.2.0.27\"\ndisplayName=\"Just Enough Items\"\n",
            )],
        );

        let (loader, name, version) = read_jar(&jar).unwrap().expect("manifest found");

        assert_eq!(loader, ModLoader::Forge);
        assert_eq!(name, "Just Enough Items");
        assert_eq!(version.as_deref(), Some("15.2.0.27"));
    }

    /// The only signal that separates NeoForge from Forge without guessing.
    #[test]
    fn a_neoforge_specific_manifest_is_not_reported_as_forge() {
        let dir = tempfile::tempdir().unwrap();
        let jar = jar_with(
            dir.path(),
            "neomod.jar",
            &[(
                "META-INF/neoforge.mods.toml",
                "[[mods]]\nmodId=\"neomod\"\nversion=\"1.0.0\"\ndisplayName=\"Neo Mod\"\n",
            )],
        );

        let (loader, name, _) = read_jar(&jar).unwrap().expect("manifest found");

        assert_eq!(loader, ModLoader::NeoForge);
        assert_eq!(name, "Neo Mod");
    }

    #[test]
    fn a_jar_without_a_known_manifest_is_named_from_its_filename_not_dropped() {
        let dir = tempfile::tempdir().unwrap();
        let mods = dir.path().join("mods");
        fs::create_dir_all(&mods).unwrap();
        jar_with(&mods, "mystery-pack-1.2.jar", &[("assets/thing.png", "x")]);

        let scanned = scan_dir(&mods).unwrap();

        assert_eq!(scanned.mods.len(), 1, "an unknown jar is still listed");
        assert_eq!(scanned.mods[0].name, "mystery-pack-1.2");
        assert_eq!(scanned.mods[0].version, None, "no version is invented");
        assert_eq!(scanned.mods[0].loader, ModLoader::Unknown);
    }

    #[test]
    fn scan_reads_disabled_jars_and_agrees_on_the_loader() {
        let dir = tempfile::tempdir().unwrap();
        let mods = dir.path().join("mods");
        fs::create_dir_all(&mods).unwrap();
        fabric_jar(&mods, "sodium.jar", "sodium", "Sodium", "0.5.8");
        fabric_jar(
            &mods,
            "lithium.jar.disabled",
            "lithium",
            "Lithium",
            "0.11.2",
        );
        fs::write(mods.join("notes.txt"), b"not a mod").unwrap();

        let scanned = scan_dir(&mods).unwrap();

        assert_eq!(scanned.loader, ModLoader::Fabric);
        assert_eq!(scanned.mods.len(), 2, "the .txt is not a mod");
        let lithium = scanned.mods.iter().find(|m| m.name == "Lithium").unwrap();
        let sodium = scanned.mods.iter().find(|m| m.name == "Sodium").unwrap();
        assert!(!lithium.enabled, ".jar.disabled means disabled");
        assert!(sodium.enabled);
    }

    #[test]
    fn missing_mods_folder_is_empty_not_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let scanned = scan_dir(&dir.path().join("mods")).unwrap();
        assert!(scanned.mods.is_empty());
        assert_eq!(scanned.loader, ModLoader::Unknown);
    }

    #[test]
    fn imports_an_instance_into_a_loadout_with_versions() {
        let core = Core::open_in_memory().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let mods = mods_dir(dir.path());
        fs::create_dir_all(&mods).unwrap();
        fabric_jar(&mods, "sodium.jar", "sodium", "Sodium", "0.5.8");
        fabric_jar(
            &mods,
            "lithium.jar.disabled",
            "lithium",
            "Lithium",
            "0.11.2",
        );

        let (created, imported) = import_instance(&core, dir.path(), "Survie Fabric").unwrap();

        assert_eq!(imported, 2);
        assert_eq!(created.name, "Survie Fabric");
        let rows = loadout::entries(&core, created.id).unwrap();
        let sodium = rows.iter().find(|r| r.name == "Sodium").unwrap();
        assert_eq!(
            sodium.version.as_deref(),
            Some("0.5.8"),
            "the jar's own version reaches the Core"
        );
        assert!(!rows.iter().find(|r| r.name == "Lithium").unwrap().enabled);
    }

    #[test]
    fn import_refuses_an_instance_with_no_mods_instead_of_creating_an_empty_loadout() {
        let core = Core::open_in_memory().unwrap();
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(mods_dir(dir.path())).unwrap();

        let err = import_instance(&core, dir.path(), "Vide").unwrap_err();

        assert!(err.contains("aucun mod"), "the refusal says why: {err}");
        assert!(
            loadout::list(&core).unwrap().is_empty(),
            "nothing half-created"
        );
    }

    #[test]
    fn installs_a_jar_into_the_mods_folder_and_registers_it() {
        let core = Core::open_in_memory().unwrap();
        let instance = tempfile::tempdir().unwrap();
        let downloads = tempfile::tempdir().unwrap();
        let source = fabric_jar(downloads.path(), "sodium.jar", "sodium", "Sodium", "0.5.8");
        let lo = loadout::create(&core, "minecraft", "Survie").unwrap();

        let installed = install_jar(
            &core,
            lo.id,
            &source,
            &mods_dir(instance.path()),
            instance.path(),
        )
        .unwrap();

        assert_eq!(installed.name, "Sodium");
        assert_eq!(installed.version.as_deref(), Some("0.5.8"));
        assert!(
            mods_dir(instance.path()).join("sodium.jar").is_file(),
            "the jar really landed in mods/"
        );
        assert!(source.is_file(), "the source download is left alone");
        let rows = loadout::entries(&core, lo.id).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].version.as_deref(), Some("0.5.8"));
        assert_eq!(
            rows[0].source_host.as_deref(),
            Some(loadout::SOURCE_HOST_MANUAL)
        );
        let jar = mods_dir(instance.path()).join("sodium.jar");
        assert_eq!(
            rows[0].size_bytes,
            Some(i64::try_from(fs::metadata(&jar).unwrap().len()).unwrap())
        );
        assert!(rows[0].installed_at.is_some());
        // No staging leftovers.
        let strays: Vec<_> = fs::read_dir(mods_dir(instance.path()))
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().to_string())
            .filter(|n| n.starts_with(".modlab-"))
            .collect();
        assert!(strays.is_empty(), "staging file left behind: {strays:?}");
    }

    #[test]
    fn sync_registers_loose_jars_already_in_mods() {
        let core = Core::open_in_memory().unwrap();
        let instance = tempfile::tempdir().unwrap();
        let mods = mods_dir(instance.path());
        fs::create_dir_all(&mods).unwrap();
        let _ = fabric_jar(&mods, "sodium.jar", "sodium", "Sodium", "0.5.8");
        let lo = loadout::create(&core, "minecraft", "Survie").unwrap();

        let added = sync_mods_dir(&core, lo.id, &mods).unwrap();
        assert_eq!(added, 1);
        assert_eq!(sync_mods_dir(&core, lo.id, &mods).unwrap(), 0, "idempotent");
        let rows = loadout::entries(&core, lo.id).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].name, "Sodium");
        assert_eq!(
            rows[0].source_host.as_deref(),
            Some(loadout::SOURCE_HOST_MANUAL)
        );
    }

    #[test]
    fn installing_over_an_existing_jar_is_refused_not_silently_overwritten() {
        let core = Core::open_in_memory().unwrap();
        let instance = tempfile::tempdir().unwrap();
        let downloads = tempfile::tempdir().unwrap();
        let mods = mods_dir(instance.path());
        fs::create_dir_all(&mods).unwrap();
        fs::write(mods.join("sodium.jar"), b"the human's own copy").unwrap();
        let source = fabric_jar(downloads.path(), "sodium.jar", "sodium", "Sodium", "0.5.8");
        let lo = loadout::create(&core, "minecraft", "Survie").unwrap();

        let err = install_jar(
            &core,
            lo.id,
            &source,
            &mods_dir(instance.path()),
            instance.path(),
        )
        .unwrap_err();

        assert!(err.contains("déjà installé"), "the refusal says why: {err}");
        assert_eq!(
            fs::read(mods.join("sodium.jar")).unwrap(),
            b"the human's own copy",
            "the existing file is untouched"
        );
    }

    #[test]
    fn set_jar_enabled_renames_jar_to_jar_disabled_and_back() {
        let dir = tempfile::tempdir().unwrap();
        let mods = dir.path().join("mods");
        fs::create_dir_all(&mods).unwrap();
        let jar = fabric_jar(&mods, "sodium.jar", "sodium", "Sodium", "0.5.8");

        let disabled = set_jar_enabled(&jar, false).unwrap();
        assert_eq!(disabled, mods.join("sodium.jar.disabled"));
        assert!(disabled.is_file());
        assert!(!jar.is_file());
        assert_eq!(jar_state(&disabled), Some(false));

        let enabled = set_jar_enabled(&disabled, true).unwrap();
        assert_eq!(enabled, mods.join("sodium.jar"));
        assert!(enabled.is_file());
        assert!(!disabled.is_file());
        assert_eq!(jar_state(&enabled), Some(true));

        // Idempotent when already in the requested state.
        assert_eq!(set_jar_enabled(&enabled, true).unwrap(), enabled);
    }

    #[test]
    fn set_mod_enabled_renames_disk_and_updates_source_path() {
        let core = Core::open_in_memory().unwrap();
        let instance = tempfile::tempdir().unwrap();
        let mods = mods_dir(instance.path());
        fs::create_dir_all(&mods).unwrap();
        let _ = fabric_jar(&mods, "sodium.jar", "sodium", "Sodium", "0.5.8");
        let lo = loadout::create(&core, "minecraft", "Survie").unwrap();
        assert_eq!(sync_mods_dir(&core, lo.id, &mods).unwrap(), 1);
        let mod_id = loadout::entries(&core, lo.id).unwrap()[0].mod_id;

        assert!(set_mod_enabled(&core, lo.id, mod_id, false).unwrap());
        let rows = loadout::entries(&core, lo.id).unwrap();
        assert!(!rows[0].enabled);
        assert!(
            rows[0]
                .source_path
                .to_string_lossy()
                .ends_with("sodium.jar.disabled"),
            "source_path must track the rename: {:?}",
            rows[0].source_path
        );
        assert!(mods.join("sodium.jar.disabled").is_file());
        assert!(!mods.join("sodium.jar").exists());

        assert!(set_mod_enabled(&core, lo.id, mod_id, true).unwrap());
        let rows = loadout::entries(&core, lo.id).unwrap();
        assert!(rows[0].enabled);
        assert!(
            rows[0]
                .source_path
                .to_string_lossy()
                .ends_with("sodium.jar"),
            "source_path restored: {:?}",
            rows[0].source_path
        );
        assert!(mods.join("sodium.jar").is_file());

        let scanned = scan_dir(&mods).unwrap();
        assert_eq!(scanned.mods.len(), 1);
        assert!(scanned.mods[0].enabled);
    }

    #[test]
    fn set_mod_enabled_is_a_noop_on_disk_for_virtual_loader_rows() {
        let core = Core::open_in_memory().unwrap();
        let lo = loadout::create(&core, "minecraft", "Survie").unwrap();
        let token = crate::branch_loader::source_token("neoforge", "26.3", Some("1.0"));
        loadout::import_mods(
            &core,
            lo.id,
            &[loadout::ImportEntry {
                name: "NeoForge".into(),
                source_path: PathBuf::from(&token),
                enabled: true,
                version: Some("1.0".into()),
            }],
        )
        .unwrap();
        let mod_id = loadout::entries(&core, lo.id).unwrap()[0].mod_id;

        assert!(set_mod_enabled(&core, lo.id, mod_id, false).unwrap());
        let rows = loadout::entries(&core, lo.id).unwrap();
        assert!(!rows[0].enabled);
        assert_eq!(
            rows[0].source_path.to_string_lossy(),
            token,
            "virtual token must not be rewritten"
        );
    }

    #[test]
    fn remove_mod_trashes_enabled_jar_out_of_mods() {
        let data = tempfile::tempdir().unwrap();
        let core = Core::open_in_memory().unwrap();
        let instance = tempfile::tempdir().unwrap();
        let mods = mods_dir(instance.path());
        fs::create_dir_all(&mods).unwrap();
        let _ = fabric_jar(&mods, "sodium.jar", "sodium", "Sodium", "0.5.8");
        let lo = loadout::create(&core, "minecraft", "Survie").unwrap();
        assert_eq!(sync_mods_dir(&core, lo.id, &mods).unwrap(), 1);
        let mod_id = loadout::entries(&core, lo.id).unwrap()[0].mod_id;

        remove_mod(&core, data.path(), lo.id, mod_id).unwrap();

        assert!(
            !mods.join("sodium.jar").exists(),
            "enabled jar must leave mods/"
        );
        assert!(
            !mods.join("sodium.jar.disabled").exists(),
            "no disabled sibling either"
        );
        assert!(
            loadout::entries(&core, lo.id).unwrap().is_empty(),
            "row soft-deleted from the table"
        );
        let trash = data.path().join("trash");
        assert!(
            trash.is_dir()
                && fs::read_dir(&trash)
                    .unwrap()
                    .any(|e| e.unwrap().file_name().to_string_lossy().contains("sodium")),
            "jar must sit in the data-root trash, not be deleted"
        );
    }

    #[test]
    fn remove_mod_trashes_disabled_jar_out_of_mods() {
        let data = tempfile::tempdir().unwrap();
        let core = Core::open_in_memory().unwrap();
        let instance = tempfile::tempdir().unwrap();
        let mods = mods_dir(instance.path());
        fs::create_dir_all(&mods).unwrap();
        let _ = fabric_jar(&mods, "lithium.jar", "lithium", "Lithium", "0.12");
        let lo = loadout::create(&core, "minecraft", "Survie").unwrap();
        assert_eq!(sync_mods_dir(&core, lo.id, &mods).unwrap(), 1);
        let mod_id = loadout::entries(&core, lo.id).unwrap()[0].mod_id;
        assert!(set_mod_enabled(&core, lo.id, mod_id, false).unwrap());
        assert!(mods.join("lithium.jar.disabled").is_file());

        remove_mod(&core, data.path(), lo.id, mod_id).unwrap();

        assert!(!mods.join("lithium.jar").exists());
        assert!(
            !mods.join("lithium.jar.disabled").exists(),
            "disabled jar must leave mods/"
        );
        assert!(loadout::entries(&core, lo.id).unwrap().is_empty());
    }

    #[test]
    fn remove_mod_is_a_noop_on_disk_for_virtual_loader_rows() {
        let data = tempfile::tempdir().unwrap();
        let core = Core::open_in_memory().unwrap();
        let lo = loadout::create(&core, "minecraft", "Survie").unwrap();
        let token = crate::branch_loader::source_token("neoforge", "26.3", Some("1.0"));
        loadout::import_mods(
            &core,
            lo.id,
            &[loadout::ImportEntry {
                name: "NeoForge".into(),
                source_path: PathBuf::from(&token),
                enabled: true,
                version: Some("1.0".into()),
            }],
        )
        .unwrap();
        let mod_id = loadout::entries(&core, lo.id).unwrap()[0].mod_id;

        remove_mod(&core, data.path(), lo.id, mod_id).unwrap();

        assert!(loadout::entries(&core, lo.id).unwrap().is_empty());
        assert!(
            !data.path().join("trash").exists(),
            "virtual loader has no jar to trash"
        );
    }
}
