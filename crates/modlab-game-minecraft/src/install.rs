//! Install a downloaded Minecraft file into an instance folder.
//!
//! A `.jar` is copied as-is into `mods/`. A `.zip` is only extracted so it
//! can be classified: jars at the root or in `mods/` are copied into `mods/`,
//! a resource pack (`pack.mcmeta`) or a shader pack (`shaders/`) is copied
//! as the original archive. Nothing here looks for a `Data/` folder.

use std::fs;
use std::path::{Path, PathBuf};

use modlab_core::game_adapter::{ArchiveSource, PrepareInstall, PreparedMod, StagedMod};
use modlab_core::install::InstallAdjustments;

use crate::instance::{self, read_jar};

const REFUSAL: &str = "Cette archive ne contient ni mod (.jar à la racine ou dans mods/), \
     ni resource pack (pack.mcmeta), ni shader pack (dossier shaders).";

const BAD_FORMAT: &str = "Minecraft n'installe pas ce format. Attendu : un .jar, ou un .zip \
     qui contient des .jar, un resource pack ou un shader pack.";

enum PackKind {
    Jars(Vec<PathBuf>),
    ResourcePack,
    ShaderPack,
}

pub fn stage(
    data_root: &Path,
    mod_id: i64,
    source: &ArchiveSource<'_>,
) -> Result<StagedMod, String> {
    let archive = source.archive_path;
    if !archive.is_file() {
        return Err(format!("fichier introuvable : {}", archive.display()));
    }
    if is_jar_name(archive) {
        return Ok(staged(
            source.name,
            "jar",
            archive.to_path_buf(),
            archive.to_path_buf(),
            None,
        ));
    }
    let ext = archive
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if ext != "zip" {
        return Ok(staged(
            source.name,
            "unknown",
            archive.to_path_buf(),
            archive.to_path_buf(),
            Some(BAD_FORMAT.to_string()),
        ));
    }

    let staged_dir = data_root.join("staging").join(mod_id.to_string());
    if staged_dir.exists() {
        modlab_core::files::move_to_trash(data_root, &staged_dir).map_err(|e| e.to_string())?;
    }
    fs::create_dir_all(&staged_dir).map_err(|e| e.to_string())?;
    if let Err(err) = modlab_core::archive::extract(archive, &staged_dir) {
        let _ = modlab_core::files::move_to_trash(data_root, &staged_dir);
        return Err(err.to_string());
    }
    match classify_tree(&staged_dir) {
        Ok(PackKind::Jars(_)) => Ok(staged(
            source.name,
            "jars",
            staged_dir.clone(),
            staged_dir,
            None,
        )),
        Ok(PackKind::ResourcePack) => Ok(staged(
            source.name,
            "resource-pack",
            staged_dir.clone(),
            staged_dir,
            None,
        )),
        Ok(PackKind::ShaderPack) => Ok(staged(
            source.name,
            "shader-pack",
            staged_dir.clone(),
            staged_dir,
            None,
        )),
        Err(message) => Ok(staged(
            source.name,
            "unknown",
            staged_dir.clone(),
            staged_dir,
            Some(message),
        )),
    }
}

fn staged(
    name: &str,
    shape: &str,
    staged_path: PathBuf,
    content_root: PathBuf,
    refusal: Option<String>,
) -> StagedMod {
    StagedMod {
        name: name.to_string(),
        shape: shape.to_string(),
        needs_wizard: false,
        staged_path,
        content_root,
        refusal,
    }
}

pub fn prepare(job: &PrepareInstall<'_>) -> Result<PreparedMod, String> {
    if job.instance_root.is_none() {
        return Err("Cette instance Minecraft n'a pas de dossier.".into());
    }
    let mods = job
        .mods_dir
        .ok_or("Dossier mods/ introuvable pour cette instance.")?;
    let packs = job
        .resource_packs
        .ok_or("Dossier resourcepacks/ introuvable pour cette instance.")?;
    let shaders = job
        .shader_packs
        .ok_or("Dossier shaderpacks/ introuvable pour cette instance.")?;

    if job.content_root.is_file() && is_jar_name(job.content_root) {
        return place_many(job, &[job.content_root.to_path_buf()], mods);
    }

    match classify_tree(job.content_root)? {
        PackKind::Jars(jars) => place_many(job, &jars, mods),
        PackKind::ResourcePack => place_many(job, &[job.archive_path.to_path_buf()], packs),
        PackKind::ShaderPack => place_many(job, &[job.archive_path.to_path_buf()], shaders),
    }
}

fn place_many(
    job: &PrepareInstall<'_>,
    sources: &[PathBuf],
    dest_dir: &Path,
) -> Result<PreparedMod, String> {
    if sources.is_empty() {
        return Err(REFUSAL.to_string());
    }
    let mut placed = Vec::new();
    for source in sources {
        match instance::place_file(job.data_root, source, dest_dir) {
            Ok(dest) => placed.push(dest),
            Err(err) => {
                trash_all(job.data_root, &placed);
                return Err(err);
            }
        }
    }
    let primary = placed[0].clone();
    match write_record(job.data_root, job.mod_id, &placed) {
        Ok(record) => placed.push(record),
        Err(err) => {
            trash_all(job.data_root, &placed);
            return Err(err);
        }
    }
    Ok(PreparedMod::Files {
        primary,
        placed,
        adjustments: adjustments_for(&sources[0]),
    })
}

fn adjustments_for(path: &Path) -> InstallAdjustments {
    let version = read_jar(path)
        .ok()
        .flatten()
        .and_then(|(_, _, version)| version);
    InstallAdjustments {
        rename_to: None,
        version_if_empty: version,
    }
}

pub fn retract(data_root: &Path, mod_id: i64, source: &Path) -> Option<Vec<PathBuf>> {
    let mut paths = read_record(data_root, mod_id);
    let record = record_path(data_root, mod_id);
    if record.is_file() && !paths.iter().any(|path| path == &record) {
        paths.push(record);
    }
    let instances = data_root.join("instances");
    if paths.is_empty() && source.is_file() && source.starts_with(&instances) {
        paths.push(source.to_path_buf());
    }
    paths.retain(|path| path.is_file());
    if paths.is_empty() {
        None
    } else {
        Some(paths)
    }
}

fn record_path(data_root: &Path, mod_id: i64) -> PathBuf {
    data_root.join("placed").join(format!("mod-{mod_id}.txt"))
}

fn write_record(data_root: &Path, mod_id: i64, files: &[PathBuf]) -> Result<PathBuf, String> {
    let path = record_path(data_root, mod_id);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let body = files
        .iter()
        .map(|file| file.display().to_string())
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(&path, body).map_err(|e| e.to_string())?;
    Ok(path)
}

fn read_record(data_root: &Path, mod_id: i64) -> Vec<PathBuf> {
    let Ok(text) = fs::read_to_string(record_path(data_root, mod_id)) else {
        return Vec::new();
    };
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(PathBuf::from)
        .collect()
}

fn trash_all(data_root: &Path, paths: &[PathBuf]) {
    for path in paths {
        if path.exists() {
            let _ = modlab_core::files::move_to_trash(data_root, path);
        }
    }
}

fn is_jar_name(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("jar"))
}

fn classify_tree(extracted: &Path) -> Result<PackKind, String> {
    if !extracted.is_dir() {
        return Err("archive extraite introuvable".into());
    }
    let mut current = extracted.to_path_buf();
    loop {
        let jars = jars_here(&current);
        if !jars.is_empty() {
            return Ok(PackKind::Jars(jars));
        }
        if current.join("pack.mcmeta").is_file() {
            return Ok(PackKind::ResourcePack);
        }
        if current.join("shaders").is_dir() {
            return Ok(PackKind::ShaderPack);
        }
        let (dirs, files) = list_entries(&current)?;
        let noise = files.iter().all(|name| is_noise(name));
        if dirs.len() == 1 && noise {
            current = current.join(&dirs[0]);
            continue;
        }
        return Err(REFUSAL.to_string());
    }
}

fn jars_here(dir: &Path) -> Vec<PathBuf> {
    let mut out = jars_in(dir);
    let mods = dir.join("mods");
    if mods.is_dir() {
        out.extend(jars_in(&mods));
    }
    out.sort();
    out
}

fn jars_in(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.is_file() && is_jar_name(path))
        .collect()
}

fn list_entries(dir: &Path) -> Result<(Vec<String>, Vec<String>), String> {
    let mut dirs = Vec::new();
    let mut files = Vec::new();
    for entry in fs::read_dir(dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') {
            continue;
        }
        if entry.file_type().map_err(|e| e.to_string())?.is_dir() {
            dirs.push(name);
        } else {
            files.push(name);
        }
    }
    Ok((dirs, files))
}

fn is_noise(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    if lower == "thumbs.db" || lower == "desktop.ini" {
        return true;
    }
    Path::new(&lower)
        .extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| {
            matches!(
                ext,
                "txt" | "md" | "pdf" | "url" | "html" | "htm" | "rtf" | "jpg" | "jpeg" | "png"
            )
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use modlab_core::download::{attach_to_loadout, capture};
    use modlab_core::game_adapter::GameAdapter;
    use modlab_core::{loadout, Core};
    use std::io::Write;
    use zip::write::SimpleFileOptions;

    fn write_zip(path: &Path, entries: &[(&str, &str)]) {
        let file = fs::File::create(path).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        for (name, body) in entries {
            zip.start_file(*name, SimpleFileOptions::default()).unwrap();
            zip.write_all(body.as_bytes()).unwrap();
        }
        zip.finish().unwrap();
    }

    fn archived(core: &Core, loadout_id: i64, archive: &Path) -> i64 {
        let id = capture(
            core,
            "https://example.test/mod.jar",
            archive.file_name().unwrap().to_str().unwrap(),
            archive,
            None,
        )
        .unwrap()
        .id();
        attach_to_loadout(core, id, loadout_id, None).unwrap()
    }

    #[test]
    fn a_jar_lands_in_the_instance_mods_folder_and_skyrim_paths_stay_untouched() {
        let data = tempfile::tempdir().unwrap();
        let core = Core::open_in_memory().unwrap();
        let created = loadout::create_instance(&core, "minecraft", "Décube").unwrap();
        let root = modlab_core::instance_root::ensure_instance_root(
            &core,
            data.path(),
            created.instance_id,
            crate::MinecraftAdapter.instance_root_policy(),
        )
        .unwrap()
        .unwrap();
        assert_eq!(
            root.file_name().unwrap(),
            ".decube",
            "the folder is .<slug> under instances/minecraft"
        );
        assert!(root.starts_with(data.path().join("instances").join("minecraft")));

        let downloads = data.path().join("downloads");
        fs::create_dir_all(&downloads).unwrap();
        let jar = downloads.join("sodium.jar");
        fs::write(&jar, b"fake jar").unwrap();
        let mod_id = archived(&core, created.id, &jar);

        let adapter = crate::MinecraftAdapter;
        let staged = adapter
            .stage_archive(
                data.path(),
                mod_id,
                &ArchiveSource {
                    name: "Sodium",
                    archive_path: &jar,
                },
            )
            .unwrap();
        assert_eq!(staged.shape, "jar");
        assert!(staged.refusal.is_none());
        assert!(
            !data.path().join("staging").exists(),
            "a jar is not extracted"
        );

        let prepared = adapter
            .prepare_install(&PrepareInstall {
                data_root: data.path(),
                mod_id,
                name: "Sodium",
                archive_path: &jar,
                content_root: &staged.content_root,
                dest: &data.path().join("mods").join(mod_id.to_string()),
                instance_root: Some(&root),
                mods_dir: Some(&root.join("mods")),
                resource_packs: Some(&root.join("resourcepacks")),
                shader_packs: Some(&root.join("shaderpacks")),
                picked: &[],
                game_version: None,
                branch_file: &modlab_core::game_adapter::BranchFileState::missing,
            })
            .unwrap();
        let modlab_core::game_adapter::PreparedMod::Files {
            primary,
            placed,
            adjustments,
        } = prepared
        else {
            panic!("a jar is a file drop-in");
        };
        assert_eq!(primary, root.join("mods").join("sodium.jar"));
        assert!(primary.is_file());
        assert_eq!(fs::read(&primary).unwrap(), b"fake jar");
        assert!(jar.is_file(), "the download is left in place");
        assert!(adjustments.version_if_empty.is_none());
        assert!(!data.path().join("mods").exists(), "no Skyrim mod folder");
        assert!(!root.join("Data").exists());

        modlab_core::install::commit_placed_path(&core, created.id, mod_id, &primary, adjustments)
            .unwrap();
        let row = loadout::entries(&core, created.id).unwrap();
        assert_eq!(row[0].status, loadout::STATUS_INSTALLED);
        assert_eq!(row[0].source_path, primary);

        let retracted = adapter
            .retract_files(data.path(), mod_id, &row[0].source_path)
            .expect("the placed jar is listed");
        assert!(retracted.iter().any(|path| path == &primary));
        assert!(!placed.is_empty());
    }

    #[test]
    fn a_zip_of_jars_is_copied_into_mods_and_a_data_folder_is_refused() {
        let data = tempfile::tempdir().unwrap();
        let core = Core::open_in_memory().unwrap();
        let created = loadout::create(&core, "minecraft", "decube").unwrap();
        let root = modlab_core::instance_root::ensure_instance_root(
            &core,
            data.path(),
            created.instance_id,
            crate::MinecraftAdapter.instance_root_policy(),
        )
        .unwrap()
        .unwrap();

        let downloads = data.path().join("downloads");
        fs::create_dir_all(&downloads).unwrap();
        let zip_path = downloads.join("pack.zip");
        write_zip(
            &zip_path,
            &[
                ("mods/sodium.jar", "jar-a"),
                ("lithium.jar", "jar-b"),
                ("readme.txt", "note"),
            ],
        );
        let mod_id = archived(&core, created.id, &zip_path);
        let staged = crate::MinecraftAdapter
            .stage_archive(
                data.path(),
                mod_id,
                &ArchiveSource {
                    name: "Pack",
                    archive_path: &zip_path,
                },
            )
            .unwrap();
        assert_eq!(staged.shape, "jars");
        let prepared = crate::MinecraftAdapter
            .prepare_install(&PrepareInstall {
                data_root: data.path(),
                mod_id,
                name: "Pack",
                archive_path: &zip_path,
                content_root: &staged.content_root,
                dest: &data.path().join("mods").join("9"),
                instance_root: Some(&root),
                mods_dir: Some(&root.join("mods")),
                resource_packs: Some(&root.join("resourcepacks")),
                shader_packs: Some(&root.join("shaderpacks")),
                picked: &[],
                game_version: None,
                branch_file: &modlab_core::game_adapter::BranchFileState::missing,
            })
            .unwrap();
        let modlab_core::game_adapter::PreparedMod::Files { placed, .. } = prepared else {
            panic!("jars are files");
        };
        let names: Vec<_> = placed
            .iter()
            .filter_map(|path| path.file_name().map(|n| n.to_string_lossy().into_owned()))
            .collect();
        assert!(names.iter().any(|name| name == "sodium.jar"));
        assert!(names.iter().any(|name| name == "lithium.jar"));
        assert!(root.join("mods").join("sodium.jar").is_file());
        assert!(root.join("mods").join("lithium.jar").is_file());

        let bad = downloads.join("skyrim.zip");
        write_zip(
            &bad,
            &[("Data/meshes/x.nif", "mesh"), ("plugin.esp", "esp")],
        );
        let bad_id = archived(&core, created.id, &bad);
        let refused = crate::MinecraftAdapter
            .stage_archive(
                data.path(),
                bad_id,
                &ArchiveSource {
                    name: "Skyrim mod",
                    archive_path: &bad,
                },
            )
            .unwrap();
        assert_eq!(refused.shape, "unknown");
        let message = refused.refusal.expect("a French refusal");
        assert!(message.contains("pack.mcmeta"), "{message}");
        assert!(!message.contains("Data"), "{message}");
    }

    #[test]
    fn a_resource_pack_and_a_shader_pack_keep_their_zip() {
        let data = tempfile::tempdir().unwrap();
        let core = Core::open_in_memory().unwrap();
        let created = loadout::create(&core, "minecraft", "decube").unwrap();
        let root = modlab_core::instance_root::ensure_instance_root(
            &core,
            data.path(),
            created.instance_id,
            crate::MinecraftAdapter.instance_root_policy(),
        )
        .unwrap()
        .unwrap();
        let downloads = data.path().join("downloads");
        fs::create_dir_all(&downloads).unwrap();

        let pack = downloads.join("faithful.zip");
        write_zip(&pack, &[("pack.mcmeta", "{}"), ("assets/x.png", "png")]);
        let pack_id = archived(&core, created.id, &pack);
        let staged = crate::MinecraftAdapter
            .stage_archive(
                data.path(),
                pack_id,
                &ArchiveSource {
                    name: "Faithful",
                    archive_path: &pack,
                },
            )
            .unwrap();
        assert_eq!(staged.shape, "resource-pack");
        let prepared = crate::MinecraftAdapter
            .prepare_install(&PrepareInstall {
                data_root: data.path(),
                mod_id: pack_id,
                name: "Faithful",
                archive_path: &pack,
                content_root: &staged.content_root,
                dest: &data.path().join("mods").join("1"),
                instance_root: Some(&root),
                mods_dir: Some(&root.join("mods")),
                resource_packs: Some(&root.join("resourcepacks")),
                shader_packs: Some(&root.join("shaderpacks")),
                picked: &[],
                game_version: None,
                branch_file: &modlab_core::game_adapter::BranchFileState::missing,
            })
            .unwrap();
        let modlab_core::game_adapter::PreparedMod::Files { primary, .. } = prepared else {
            panic!("the zip is copied whole");
        };
        assert_eq!(primary, root.join("resourcepacks").join("faithful.zip"));
        assert!(primary.is_file());

        let shader = downloads.join("complementary.zip");
        write_zip(&shader, &[("shaders/final.fsh", "void main(){}")]);
        let shader_id = archived(&core, created.id, &shader);
        let staged = crate::MinecraftAdapter
            .stage_archive(
                data.path(),
                shader_id,
                &ArchiveSource {
                    name: "Complementary",
                    archive_path: &shader,
                },
            )
            .unwrap();
        assert_eq!(staged.shape, "shader-pack");
        let prepared = crate::MinecraftAdapter
            .prepare_install(&PrepareInstall {
                data_root: data.path(),
                mod_id: shader_id,
                name: "Complementary",
                archive_path: &shader,
                content_root: &staged.content_root,
                dest: &data.path().join("mods").join("2"),
                instance_root: Some(&root),
                mods_dir: Some(&root.join("mods")),
                resource_packs: Some(&root.join("resourcepacks")),
                shader_packs: Some(&root.join("shaderpacks")),
                picked: &[],
                game_version: None,
                branch_file: &modlab_core::game_adapter::BranchFileState::missing,
            })
            .unwrap();
        let modlab_core::game_adapter::PreparedMod::Files { primary, .. } = prepared else {
            panic!("shader zip");
        };
        assert_eq!(primary, root.join("shaderpacks").join("complementary.zip"));
    }
}
