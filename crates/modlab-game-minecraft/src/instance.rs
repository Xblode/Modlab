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
            let (loader, name, version) = found
                .unwrap_or_else(|| (ModLoader::Unknown, jar_stem(&path), None));
            mods.push(InstalledMod {
                name,
                version,
                jar_path: path,
                loader,
                enabled,
            });
        }
    }
    mods.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
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

/// Copy one `.jar` into an instance's `mods/` folder and register it in a
/// loadout.
///
/// This is a **real write into the game's own folder** — the one thing
/// Skyrim's USVFS backend never does (ADR 0001) — so it is deliberately
/// careful:
/// - the copy lands on a temp name in the *same* directory and is renamed
///   into place only once complete, so a crash mid-copy can never leave a
///   half-written jar that Minecraft would try to load (same
///   write-then-rename rule as `modlab_core::store`);
/// - it refuses rather than overwriting an existing jar of the same name —
///   silently replacing a mod the human installed is exactly the kind of
///   loss the transaction cycle exists to prevent (§5);
/// - if registering the mod in the database fails *after* the copy landed,
///   the copied file is removed again, so a failed install leaves nothing
///   behind. `modlab_core::transaction`'s rollback restores database state
///   only (ADR 0003), so this file-level undo has to be explicit here.
pub fn install_jar(
    core: &Core,
    loadout_id: i64,
    source_jar: &Path,
    instance_root: &Path,
) -> Result<InstalledMod, String> {
    if jar_state(source_jar) != Some(true) {
        return Err(format!(
            "ce n'est pas un .jar : {}",
            source_jar.display()
        ));
    }
    if !source_jar.is_file() {
        return Err(format!("fichier introuvable : {}", source_jar.display()));
    }
    let dest_dir = mods_dir(instance_root);
    fs::create_dir_all(&dest_dir).map_err(|e| e.to_string())?;

    let file_name = source_jar
        .file_name()
        .ok_or_else(|| format!("chemin sans nom de fichier : {}", source_jar.display()))?;
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
    let _ = fs::remove_file(&staging);
    fs::copy(source_jar, &staging).map_err(|e| e.to_string())?;
    if let Err(e) = fs::rename(&staging, &dest) {
        let _ = fs::remove_file(&staging);
        return Err(e.to_string());
    }

    let found = read_jar(&dest).ok().flatten();
    let (loader, name, version) =
        found.unwrap_or_else(|| (ModLoader::Unknown, jar_stem(&dest), None));
    let installed = InstalledMod {
        name: name.clone(),
        version: version.clone(),
        jar_path: dest.clone(),
        loader,
        enabled: true,
    };

    let entry = loadout::ImportEntry {
        name,
        source_path: dest.clone(),
        enabled: true,
        version,
    };
    if let Err(e) = loadout::import_mods(core, loadout_id, &[entry]) {
        // The database never learned about this jar, so leaving it in
        // `mods/` would mean Minecraft loads a mod ModLab doesn't know it
        // installed. Undo the one thing that did land.
        let _ = fs::remove_file(&dest);
        return Err(e.to_string());
    }
    Ok(installed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use zip::write::SimpleFileOptions;

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
        fabric_jar(&mods, "lithium.jar.disabled", "lithium", "Lithium", "0.11.2");
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
        fabric_jar(&mods, "lithium.jar.disabled", "lithium", "Lithium", "0.11.2");

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

        let installed = install_jar(&core, lo.id, &source, instance.path()).unwrap();

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
        // No staging leftovers.
        let strays: Vec<_> = fs::read_dir(mods_dir(instance.path()))
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().to_string())
            .filter(|n| n.starts_with(".modlab-"))
            .collect();
        assert!(strays.is_empty(), "staging file left behind: {strays:?}");
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

        let err = install_jar(&core, lo.id, &source, instance.path()).unwrap_err();

        assert!(err.contains("déjà installé"), "the refusal says why: {err}");
        assert_eq!(
            fs::read(mods.join("sodium.jar")).unwrap(),
            b"the human's own copy",
            "the existing file is untouched"
        );
    }
}
