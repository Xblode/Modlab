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

/// File conflicts for a loadout — the same scan `modlab loadout-conflicts`
/// prints (`modlab_core::conflicts::report_for_entries`), so what the UI
/// badges and what the CLI reports can never drift apart.
///
/// Read-only, so it sits in the "Lecture" permission tier (§7.2): nothing is
/// written, no confirmation is involved. Deliberately recomputed on every
/// call instead of cached in `AppState`: a cache would have to be
/// invalidated on every enable/disable, and a stale conflict badge is a wrong
/// statement about the user's install. The UI debounces its calls instead.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Mo2ImportResultDto {
    pub loadout: LoadoutDto,
    /// How many mods actually landed in the loadout — the dialog's success
    /// message. Informational profile entries (DLC, Creation Club, unmanaged)
    /// are deliberately not part of it.
    pub imported: usize,
}

/// Create an independent instance (initial branch `principale`). Not a
/// branch of whatever instance is on screen.
#[tauri::command]
pub(crate) fn create_loadout(
    state: State<AppState>,
    game_id: String,
    name: String,
) -> Result<LoadoutDto, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("Donne un nom à l’instance.".into());
    }
    if name.chars().count() > 100 {
        return Err("Le nom de l’instance ne peut pas dépasser 100 caractères.".into());
    }
    let core = state.core.lock().map_err(|e| e.to_string())?;
    let loadout = loadout::create_instance(&core, &game_id, name).map_err(|e| {
        let msg = e.to_string();
        if msg.contains("déjà ce nom") {
            "Une instance porte déjà ce nom pour ce jeu.".to_string()
        } else {
            msg
        }
    })?;
    if let Some(adapter) = crate::adapters()
        .into_iter()
        .find(|adapter| adapter.id() == game_id)
    {
        modlab_core::instance_root::ensure_instance_root(
            &core,
            &state.data_root,
            loadout.instance_id,
            adapter.instance_root_policy(),
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(LoadoutDto {
        id: loadout.id,
        game_id,
        name: loadout.name,
        instance_id: loadout.instance_id,
        instance_name: loadout.instance_name,
        mod_count: 0,
        channel: "stable".to_string(),
        revision: 0,
        mc_loader: None,
        mc_game_version: None,
        mc_loader_version: None,
    })
}

/// Create a branch of the instance that owns `loadout_id`. Copies that
/// branch's mod list. Does not create an instance and does not copy
/// snapshots, so Enregistrer / Annuler stay on the branch you were on.
#[tauri::command]
pub(crate) fn create_branch(
    state: State<AppState>,
    loadout_id: i64,
    name: String,
) -> Result<LoadoutDto, String> {
    let core = state.core.lock().map_err(|e| e.to_string())?;
    let branch = loadout::create_branch(&core, loadout_id, &name).map_err(|e| e.to_string())?;
    let summary = loadout::list(&core)
        .map_err(|e| e.to_string())?
        .into_iter()
        .find(|l| l.id == branch.id)
        .ok_or_else(|| "branche créée introuvable".to_string())?;
    crate::loadout_dto(&core, &summary)
}

/// Rename the instance that owns this branch. The branch name stays.
/// Mutation réversible tier (§7.2) — a metadata edit, not a configuration
/// change, so it doesn't go through the snapshot cycle.
#[tauri::command]
pub(crate) fn rename_loadout(
    state: State<AppState>,
    loadout_id: i64,
    name: String,
) -> Result<(), String> {
    let core = state.core.lock().map_err(|e| e.to_string())?;
    loadout::rename_instance_of_loadout(&core, loadout_id, &name).map_err(|e| e.to_string())
}

/// Move a loadout to a different channel ("stable" / "beta" / "alpha").
#[tauri::command]
pub(crate) fn set_loadout_channel(
    state: State<AppState>,
    loadout_id: i64,
    channel: String,
) -> Result<(), String> {
    let core = state.core.lock().map_err(|e| e.to_string())?;
    loadout::set_channel(&core, loadout_id, &channel).map_err(|e| e.to_string())
}

/// Soft-delete a loadout — see `loadout::delete`. Reversible in the Core
/// (the row and its entries survive) — `list_deleted_loadouts`/
/// `restore_loadout` are the trash screen that undoes it.
#[tauri::command]
pub(crate) fn delete_loadout(state: State<AppState>, loadout_id: i64) -> Result<(), String> {
    let core = state.core.lock().map_err(|e| e.to_string())?;
    loadout::delete(&core, loadout_id).map_err(|e| e.to_string())
}

/// One soft-deleted loadout, as the trash screen lists it — deliberately
/// thinner than `LoadoutDto`: mod count and revision don't change while a
/// loadout sits in the trash, so there is nothing to compute for them.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeletedLoadoutDto {
    pub id: i64,
    pub game_id: String,
    pub name: String,
}

#[tauri::command]
pub(crate) fn list_deleted_loadouts(
    state: State<AppState>,
) -> Result<Vec<DeletedLoadoutDto>, String> {
    let core = state.core.lock().map_err(|e| e.to_string())?;
    Ok(loadout::list_trash(&core)
        .map_err(|e| e.to_string())?
        .into_iter()
        .map(|l| DeletedLoadoutDto {
            id: l.id,
            game_id: l.game_id,
            name: l.name,
        })
        .collect())
}

#[tauri::command]
pub(crate) fn restore_loadout(state: State<AppState>, loadout_id: i64) -> Result<(), String> {
    let core = state.core.lock().map_err(|e| e.to_string())?;
    loadout::restore(&core, loadout_id).map_err(|e| e.to_string())
}

/// What `ActiveInstanceCard`'s version badge shows: how many revisions this
/// loadout has (`r.{revision}`, `b.{revision}`… — the channel-letter prefix
/// is the UI's own presentation, this DTO just carries the number) and
/// whether the live state has drifted from the latest one. Read-only
/// (Lecture tier, §7.2) and recomputed per call, same rule as
/// `get_loadout_conflicts`: a cached dirty count would go stale the moment
/// a mod is toggled elsewhere.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LoadoutVersionDto {
    pub revision: i64,
    pub dirty_plus: i64,
    pub dirty_minus: i64,
}

#[tauri::command]
pub(crate) fn get_loadout_version(
    state: State<AppState>,
    loadout_id: i64,
) -> Result<LoadoutVersionDto, String> {
    let core = state.core.lock().map_err(|e| e.to_string())?;
    let revision = transaction::revision_count(&core, loadout_id).map_err(|e| e.to_string())?;
    let dirty = transaction::dirty_summary(&core, loadout_id).map_err(|e| e.to_string())?;
    Ok(LoadoutVersionDto {
        revision,
        dirty_plus: dirty.plus,
        dirty_minus: dirty.minus,
    })
}

/// Import an MO2 profile into a new Skyrim loadout. A mutation like any
/// other: journaled by the Core, and the UI stays a client of the engine.
/// The heavy lifting (profile parsing, folder verification, import order)
/// lives in `modlab_game_skyrim::mo2::import_into`, the same code the CLI
/// runs — this host only validates what the user typed and wraps the answer.
/// The whole import keeps the database lock, on a background thread: the
/// folder check and the insert are one mutation.
#[tauri::command]
pub(crate) async fn mo2_import(
    app: tauri::AppHandle,
    profile: String,
    mods_dir: String,
    name: String,
) -> Result<Mo2ImportResultDto, String> {
    let name = name.trim().to_string();
    if name.is_empty() {
        return Err("Donne un nom à l'instance.".into());
    }
    if name.chars().count() > 100 {
        return Err("Le nom de l'instance ne peut pas dépasser 100 caractères.".into());
    }
    off_ui(move || {
        let state = app.state::<AppState>();
        let core = state.core.lock().map_err(|e| e.to_string())?;
        let (created, imported) = modlab_game_skyrim::mo2::import_into(
            &core,
            Path::new(&profile),
            Path::new(&mods_dir),
            &name,
        )?;
        Ok(Mo2ImportResultDto {
            loadout: LoadoutDto {
                id: created.id,
                game_id: SkyrimAdapter.id().to_string(),
                name: created.name,
                instance_id: created.instance_id,
                instance_name: created.instance_name,
                mod_count: i64::try_from(imported).unwrap_or(i64::MAX),
                channel: "stable".to_string(),
                revision: 0,
                mc_loader: None,
                mc_game_version: None,
                mc_loader_version: None,
            },
            imported,
        })
    })
    .await
}

/// Where a Minecraft command should act: the human's override from
/// Réglages → Jeux if there is one, else the adapter's own detection.
/// Refuses rather than falling back to a guess — writing jars into the
/// wrong folder is not a mistake worth being clever about.
fn minecraft_root(core: &Core) -> Result<PathBuf, String> {
    resolve_slot(core, &MinecraftAdapter, "root").ok_or_else(|| {
        "Minecraft introuvable — renseigne son dossier dans Réglages → Jeux".to_string()
    })
}

/// Import an existing modded instance's `mods/` folder into a new loadout.
/// Read-only on the game's side: not a jar is moved. Same shape as
/// `mo2_import`, and like it the real work lives in the adapter so the CLI
/// and the shell cannot drift apart.
#[tauri::command]
pub(crate) fn minecraft_import_instance(
    state: State<AppState>,
    name: String,
) -> Result<Mo2ImportResultDto, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("Donne un nom à l'instance.".into());
    }
    let core = state.core.lock().map_err(|e| e.to_string())?;
    let root = minecraft_root(&core)?;
    let (created, imported) = modlab_game_minecraft::instance::import_instance(&core, &root, name)?;
    // The import reads the launcher's mods/. The new instance still gets its
    // own folder; jars already in the global .minecraft are not moved.
    modlab_core::instance_root::ensure_instance_root(
        &core,
        &state.data_root,
        created.instance_id,
        MinecraftAdapter.instance_root_policy(),
    )
    .map_err(|e| e.to_string())?;
    Ok(Mo2ImportResultDto {
        loadout: LoadoutDto {
            id: created.id,
            game_id: MinecraftAdapter.id().to_string(),
            name: created.name,
            instance_id: created.instance_id,
            instance_name: created.instance_name,
            mod_count: i64::try_from(imported).unwrap_or(i64::MAX),
            channel: "stable".to_string(),
            revision: 0,
            mc_loader: None,
            mc_game_version: None,
            mc_loader_version: None,
        },
        imported,
    })
}

/// Install one downloaded `.jar` into the instance's `mods/` folder and
/// register it in the loadout. A real write into the game's own folder —
/// see `instance::install_jar` for what it does to stay safe (staged copy,
/// refuses to overwrite, removes the file again if the database write
/// fails). Mutation réversible tier (§7.2): the client confirms first.
#[tauri::command]
pub(crate) fn minecraft_install_jar(
    state: State<AppState>,
    loadout_id: i64,
    jar_path: String,
) -> Result<Vec<ModRowDto>, String> {
    let core = state.core.lock().map_err(|e| e.to_string())?;
    let target = crate::paths::install_target_for_loadout(&core, &state.data_root, loadout_id)?
        .ok_or_else(|| "Cette instance n'a pas de dossier mods/.".to_string())?;
    modlab_game_minecraft::instance::install_jar(
        &core,
        loadout_id,
        Path::new(jar_path.trim()),
        &target.mods_dir,
        &state.data_root,
    )?;
    entries_dto(&core, loadout_id)
}

/// Register jars already present in the instance `mods/` folder that the
/// loadout does not list yet. No copy — same idea as importing an instance,
/// for an existing loadout. Each new row is `source_host = manual`.
#[tauri::command]
pub(crate) fn minecraft_sync_mods(
    state: State<AppState>,
    loadout_id: i64,
) -> Result<MinecraftSyncResultDto, String> {
    let core = state.core.lock().map_err(|e| e.to_string())?;
    let target = crate::paths::install_target_for_loadout(&core, &state.data_root, loadout_id)?
        .ok_or_else(|| "Cette instance n'a pas de dossier mods/.".to_string())?;
    let added =
        modlab_game_minecraft::instance::sync_mods_dir(&core, loadout_id, &target.mods_dir)?;
    Ok(MinecraftSyncResultDto {
        added,
        entries: entries_dto(&core, loadout_id)?,
    })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MinecraftSyncResultDto {
    pub added: usize,
    pub entries: Vec<ModRowDto>,
}

/// Add one path the human picked (file or folder) to the active loadout as a
/// manual mod. Minecraft: `.jar` is copied into `mods/` (or registered in
/// place when it already lives there), then the folder is synced. Skyrim: a
/// folder is registered as installed; an archive is filed as archived with
/// `source_host = manual`.
#[tauri::command]
pub(crate) fn import_manual_mod(
    state: State<AppState>,
    loadout_id: i64,
    path: String,
) -> Result<ImportManualResultDto, String> {
    let path = PathBuf::from(path.trim());
    if path.as_os_str().is_empty() {
        return Err("Choisis un fichier ou un dossier.".into());
    }
    let core = state.core.lock().map_err(|e| e.to_string())?;
    let game_id = loadout::list(&core)
        .map_err(|e| e.to_string())?
        .into_iter()
        .find(|row| row.id == loadout_id)
        .map(|row| row.game_id)
        .ok_or_else(|| "Instance introuvable.".to_string())?;

    let added = if game_id == MinecraftAdapter.id() {
        import_manual_minecraft(&core, &state.data_root, loadout_id, &path)?
    } else if game_id == SkyrimAdapter.id() || game_id.starts_with("skyrim") {
        import_manual_skyrim(&core, loadout_id, &path)?
    } else {
        return Err(format!(
            "L'ajout manuel n'est pas encore branché pour « {game_id} »."
        ));
    };
    Ok(ImportManualResultDto {
        added,
        entries: entries_dto(&core, loadout_id)?,
    })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportManualResultDto {
    pub added: usize,
    pub entries: Vec<ModRowDto>,
}

fn import_manual_minecraft(
    core: &Core,
    data_root: &Path,
    loadout_id: i64,
    path: &Path,
) -> Result<usize, String> {
    let target = crate::paths::install_target_for_loadout(core, data_root, loadout_id)?
        .ok_or_else(|| "Cette instance n'a pas de dossier mods/.".to_string())?;
    let mut added = 0;
    if path.is_file() {
        let under_mods = path
            .parent()
            .is_some_and(|parent| same_dir(parent, &target.mods_dir));
        if under_mods {
            if modlab_game_minecraft::instance::register_existing_jar(core, loadout_id, path)?
                .is_some()
            {
                added += 1;
            }
        } else {
            modlab_game_minecraft::instance::install_jar(
                core,
                loadout_id,
                path,
                &target.mods_dir,
                data_root,
            )?;
            added += 1;
        }
    } else if path.is_dir() {
        // Treating a folder as the mods/ root: register every loose jar.
        added += modlab_game_minecraft::instance::sync_mods_dir(core, loadout_id, path)?;
    } else {
        return Err(format!("chemin introuvable : {}", path.display()));
    }
    // Always pick up siblings the human may have dropped in mods/ beside
    // the file they just chose.
    added += modlab_game_minecraft::instance::sync_mods_dir(core, loadout_id, &target.mods_dir)?;
    Ok(added)
}

fn import_manual_skyrim(core: &Core, loadout_id: i64, path: &Path) -> Result<usize, String> {
    if path.is_dir() {
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().trim().to_string())
            .filter(|n| !n.is_empty())
            .ok_or_else(|| "dossier sans nom".to_string())?;
        loadout::import_mods(
            core,
            loadout_id,
            &[loadout::ImportEntry {
                name,
                source_path: path.to_path_buf(),
                enabled: true,
                version: None,
            }],
        )
        .map_err(|e| e.to_string())?;
        let want = path_key(path);
        let mod_id = loadout::entries(core, loadout_id)
            .map_err(|e| e.to_string())?
            .into_iter()
            .rev()
            .find(|row| path_key(&row.source_path) == want)
            .map(|row| row.mod_id)
            .ok_or_else(|| "mod introuvable après import".to_string())?;
        loadout::mark_manual_source(core, mod_id).map_err(|e| e.to_string())?;
        return Ok(1);
    }
    if path.is_file() {
        let file_name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .ok_or_else(|| "fichier sans nom".to_string())?;
        let size = std::fs::metadata(path).ok().map(|m| m.len() as i64);
        let origin = download::DownloadOrigin {
            source_host: loadout::SOURCE_HOST_MANUAL.to_string(),
            page_url: None,
        };
        let outcome = download::capture_with_origin(
            core,
            "manual://local",
            &file_name,
            path,
            size,
            Some(&origin),
        )
        .map_err(|e| e.to_string())?;
        download::attach_to_loadout(core, outcome.id(), loadout_id, None)
            .map_err(|e| e.to_string())?;
        return Ok(1);
    }
    Err(format!("chemin introuvable : {}", path.display()))
}

fn same_dir(a: &Path, b: &Path) -> bool {
    path_key(a) == path_key(b)
}

fn path_key(path: &Path) -> String {
    path.display()
        .to_string()
        .replace('\\', "/")
        .to_ascii_lowercase()
}

/// What a Minecraft instance holds besides its mods — what the Packs and
/// Worlds views list. Plain directory listings, nothing parsed.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MinecraftContentDto {
    pub resource_packs: Vec<String>,
    pub shader_packs: Vec<String>,
    pub worlds: Vec<String>,
    /// Whether `servers.dat` exists. Its contents are **not** read: that
    /// file is binary NBT, and ModLab has no NBT parser — the Servers view
    /// says so out loud rather than showing an empty list that would read
    /// as "you have no servers".
    pub servers_dat_present: bool,
}

/// List an instance's resource packs, shader packs and worlds. Read-only
/// (Lecture tier, §7.2). A missing folder is an empty list, not an error —
/// a fresh instance simply has none yet.
#[tauri::command]
pub(crate) fn minecraft_content(state: State<AppState>) -> Result<MinecraftContentDto, String> {
    let core = state.core.lock().map_err(|e| e.to_string())?;
    let loadout_id = state
        .active_loadout
        .lock()
        .map_err(|e| e.to_string())?
        .unwrap_or(0);
    let instance_id = loadout::list(&core)
        .map_err(|e| e.to_string())?
        .into_iter()
        .find(|branch| branch.id == loadout_id)
        .map(|branch| branch.instance_id);
    let root = instance_id.and_then(|instance_id| {
        let stored = modlab_core::instance_root::stored_instance_root(&core, instance_id).ok()?;
        let override_path = core
            .instance_path_override(instance_id, modlab_core::game_adapter::INSTANCE_ROOT_SLOT)
            .ok()
            .flatten()
            .map(PathBuf::from);
        fuse_path(stored, override_path).effective
    });
    let Some(root) = root else {
        return Ok(MinecraftContentDto {
            resource_packs: Vec::new(),
            shader_packs: Vec::new(),
            worlds: Vec::new(),
            servers_dat_present: false,
        });
    };
    Ok(MinecraftContentDto {
        resource_packs: list_dir_names(&root.join("resourcepacks")),
        shader_packs: list_dir_names(&root.join("shaderpacks")),
        worlds: list_dir_names(&root.join("saves")),
        servers_dat_present: root.join("servers.dat").is_file(),
    })
}

/// Entry names directly inside `dir`, sorted, hidden entries skipped.
/// Unreadable or missing directory → empty list: these feed views that
/// should show "nothing here" rather than an error nobody can act on.
fn list_dir_names(dir: &Path) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut names: Vec<String> = entries
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .filter(|n| !n.starts_with('.'))
        .collect();
    names.sort_by_key(|n| n.to_lowercase());
    names
}

/// The journal the transactions panel lists — what happened, when, and the
/// steps it took. Newest first, as users read it.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransactionDto {
    pub id: i64,
    pub occurred_at: String,
    pub kind: String,
    pub steps: Vec<String>,
    /// Which loadout this is about — only populated for snapshot/rollback
    /// entries, whose payload structurally carries it. `null` for everything
    /// else; a client filtering "this loadout's revisions" must expect gaps.
    pub loadout_id: Option<i64>,
}

#[tauri::command]
pub(crate) fn get_transactions(
    state: State<AppState>,
    limit: Option<usize>,
) -> Result<Vec<TransactionDto>, String> {
    let core = state.core.lock().map_err(|e| e.to_string())?;
    let records =
        transaction::list(&core, limit.unwrap_or(50).clamp(1, 200)).map_err(|e| e.to_string())?;
    Ok(records
        .into_iter()
        .map(|r| TransactionDto {
            id: r.id,
            occurred_at: r.occurred_at,
            kind: r.kind,
            steps: r.steps,
            loadout_id: r.loadout_id,
        })
        .collect())
}

/// Capture the active loadout's configuration (§5 snapshot step). Returns
/// the event id the panel can point at.
#[tauri::command]
pub(crate) fn snapshot_loadout(state: State<AppState>, loadout_id: i64) -> Result<i64, String> {
    let core = state.core.lock().map_err(|e| e.to_string())?;
    transaction::snapshot_loadout(&core, loadout_id).map_err(|e| e.to_string())
}

/// Restore the loadout to its most recent snapshot and hand back the fresh
/// entry list — the panel refreshes the Mods table from this directly.
#[tauri::command]
pub(crate) fn rollback_loadout(
    state: State<AppState>,
    loadout_id: i64,
) -> Result<Vec<ModRowDto>, String> {
    let core = state.core.lock().map_err(|e| e.to_string())?;
    transaction::rollback_loadout(&core, loadout_id).map_err(|e| e.to_string())?;
    entries_dto(&core, loadout_id)
}
