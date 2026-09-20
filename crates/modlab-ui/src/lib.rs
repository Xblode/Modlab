//! Tauri host for the React shell. This crate is a *client* of
//! `modlab-core` — same rule as the CLI: no private state, no game rules.

use modlab_ai::{diagnose, llm_phrase, rules_answer, LoadoutFacts, ModFact};
use modlab_core::game_adapter::{GameAdapter, PathKind};
use modlab_core::{conflicts, loadout, transaction, Core};
use modlab_game_minecraft::MinecraftAdapter;
use modlab_game_skyrim::SkyrimAdapter;
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::Duration;
use tauri::State;

pub struct AppState {
    core: Mutex<Core>,
    data_root: PathBuf,
    /// §6 lifecycle guard: the local LLM is never allowed to infer while a
    /// game session runs. `launch_skse` sets this for the whole session.
    game_running: AtomicBool,
}

/// Resets `game_running` on drop — including on every early-return path.
struct GameSessionGuard<'a>(&'a AtomicBool);

impl Drop for GameSessionGuard<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DetectedGameDto {
    pub id: String,
    pub name: String,
    pub root: String,
    pub short: String,
    pub capabilities: modlab_core::game_adapter::GameCapabilities,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LoadoutDto {
    pub id: i64,
    pub game_id: String,
    pub name: String,
    /// Number of mods the Core holds for this loadout. Real count, not an
    /// estimate — the dashboard instance cards show it. Everything else the
    /// maquette puts on those cards (revision, last played) has no Core
    /// backing yet and is rendered as an explicit placeholder by the UI.
    pub mod_count: i64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModRowDto {
    pub mod_id: i64,
    pub name: String,
    pub source_path: String,
    pub enabled: bool,
    pub position: i64,
}

/// One mod's share of a loadout's file conflicts. `wins`/`loses` count the
/// distinct paths this mod is involved in — the shape the Mods table needs to
/// badge a row, without shipping every conflicting path across IPC.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModConflictsDto {
    pub mod_id: i64,
    pub name: String,
    pub wins: i64,
    pub loses: i64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LoadoutConflictsDto {
    pub mods: Vec<ModConflictsDto>,
    /// Enabled mods whose folder could not be read, as `id: reason`.
    ///
    /// An empty `mods` list plus a non-empty `unreadable` list means "not
    /// scanned", never "no conflict" — the UI must not conflate the two.
    pub unreadable: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShellSnapshot {
    pub data_root: String,
    pub games: Vec<DetectedGameDto>,
    pub loadouts: Vec<LoadoutDto>,
    pub entries: Vec<ModRowDto>,
    pub selected_loadout_id: Option<i64>,
    pub usvfs_dll: Option<String>,
}

/// Every Game Adapter this shell knows about — the one place that lists
/// them, so `detect_games` and `game_path_slots` can never drift apart on
/// which games exist.
fn adapters() -> [&'static dyn GameAdapter; 2] {
    [&SkyrimAdapter, &MinecraftAdapter]
}

/// One `PathSlot`'s effective value: a human override if the Core has one,
/// else the adapter's own live detection. Every command that actually *uses*
/// a path (launching the game, deploying mods) must go through this instead
/// of calling adapter detection directly — otherwise an override set from
/// Réglages → Jeux is silently ignored, which is exactly the bug that made
/// `launch_skse` keep reporting the DLL as missing after it was overridden.
fn resolve_slot(core: &Core, adapter: &dyn GameAdapter, slot_id: &str) -> Option<PathBuf> {
    if let Ok(Some(over)) = core.game_path_override(adapter.id(), slot_id) {
        return Some(PathBuf::from(over));
    }
    adapter
        .path_slots()
        .into_iter()
        .find(|s| s.id == slot_id)
        .and_then(|s| s.detected)
}

fn detect_games() -> Vec<DetectedGameDto> {
    adapters()
        .into_iter()
        .filter_map(|adapter| {
            let install = adapter.detect()?;
            let capabilities = adapter.capabilities();
            Some(DetectedGameDto {
                id: install.id,
                name: install.name,
                root: install.root.display().to_string(),
                short: capabilities.short_name.to_string(),
                capabilities,
            })
        })
        .collect()
}

fn resolve_data_root() -> PathBuf {
    if let Some(from_env) = std::env::var_os("MODLAB_DATA_ROOT") {
        return PathBuf::from(from_env);
    }
    let real = PathBuf::from(r"F:\ModLab");
    if real.join("db").join("modlab.db").is_file() || real.join("modlab.db").is_file() {
        return real;
    }
    let sandbox = PathBuf::from(".modlab-dev");
    if sandbox.join("modlab.db").is_file() || sandbox.join("db").join("modlab.db").is_file() {
        return sandbox;
    }
    if real.is_dir() {
        return real;
    }
    sandbox
}

fn open_core(data_root: &std::path::Path) -> Result<Core, String> {
    std::fs::create_dir_all(data_root).map_err(|e| e.to_string())?;
    let nested = data_root.join("db").join("modlab.db");
    let flat = data_root.join("modlab.db");
    let db_path = if nested.is_file() {
        nested
    } else if flat.is_file() {
        flat
    } else {
        std::fs::create_dir_all(data_root.join("db")).map_err(|e| e.to_string())?;
        nested
    };
    Core::open(db_path).map_err(|e| e.to_string())
}

fn entries_dto(core: &Core, loadout_id: i64) -> Result<Vec<ModRowDto>, String> {
    loadout::entries(core, loadout_id)
        .map_err(|e| e.to_string())
        .map(|rows| {
            rows.into_iter()
                .map(|r| ModRowDto {
                    mod_id: r.mod_id,
                    name: r.name,
                    source_path: r.source_path.display().to_string(),
                    enabled: r.enabled,
                    position: r.position,
                })
                .collect()
        })
}

#[tauri::command]
fn get_shell_snapshot(
    state: State<AppState>,
    loadout_id: Option<i64>,
) -> Result<ShellSnapshot, String> {
    let core = state.core.lock().map_err(|e| e.to_string())?;
    let loadouts: Vec<LoadoutDto> = loadout::list(&core)
        .map_err(|e| e.to_string())?
        .into_iter()
        .map(|l| {
            let mod_count = loadout::entries(&core, l.id)
                .map(|rows| rows.len() as i64)
                .unwrap_or(0);
            LoadoutDto {
                id: l.id,
                game_id: l.game_id,
                name: l.name,
                mod_count,
            }
        })
        .collect();
    let selected = loadout_id.or_else(|| loadouts.first().map(|l| l.id));
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
        usvfs_dll: resolve_slot(&core, &SkyrimAdapter, "usvfs_dll").map(|p| p.display().to_string()),
    })
}

/// One `PathSlot`, merged with any human override the Core holds for it —
/// the shape Réglages → Jeux needs to show "what ModLab found" vs "what you
/// pointed it at" without doing that merge itself.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GamePathSlotDto {
    pub id: String,
    pub label: String,
    pub kind: PathKind,
    pub detected: Option<String>,
    pub override_path: Option<String>,
    /// `detected` unless `override_path` is set, in which case the override
    /// wins — the single value a client actually uses (to launch, to deploy
    /// into, …), so it never has to re-implement this precedence itself.
    pub effective: Option<String>,
}

/// The path slots this game's adapter declares, each with whatever the Core
/// has on file as a human override. Read-only (Lecture tier, §7.2): nothing
/// is written here, so it needs no confirmation.
#[tauri::command]
fn game_path_slots(state: State<AppState>, game_id: String) -> Result<Vec<GamePathSlotDto>, String> {
    let adapter = adapters()
        .into_iter()
        .find(|a| a.id() == game_id)
        .ok_or_else(|| format!("jeu inconnu : {game_id}"))?;
    let overrides = {
        let core = state.core.lock().map_err(|e| e.to_string())?;
        core.game_path_overrides(&game_id).map_err(|e| e.to_string())?
    };
    Ok(adapter
        .path_slots()
        .into_iter()
        .map(|slot| {
            let detected = slot.detected.map(|p| p.display().to_string());
            let override_path = overrides
                .iter()
                .find(|(id, _)| id == slot.id)
                .map(|(_, path)| path.clone());
            let effective = override_path.clone().or_else(|| detected.clone());
            GamePathSlotDto {
                id: slot.id.to_string(),
                label: slot.label.to_string(),
                kind: slot.kind,
                detected,
                override_path,
                effective,
            }
        })
        .collect())
}

/// Point one of a game's path slots at a folder/file the human chose by
/// hand — Mutation réversible tier (§7.2): it only ever changes what ModLab
/// *reads from* next time, nothing on disk moves, and clearing it (see
/// below) undoes it instantly. Returns the merged slot list so the panel can
/// refresh from the Core's own answer rather than guessing what changed.
#[tauri::command]
fn set_game_path_override(
    state: State<AppState>,
    game_id: String,
    slot_id: String,
    path: String,
) -> Result<Vec<GamePathSlotDto>, String> {
    let path = path.trim();
    if path.is_empty() {
        return Err("Le chemin ne peut pas être vide.".into());
    }
    {
        let core = state.core.lock().map_err(|e| e.to_string())?;
        core.set_game_path_override(&game_id, &slot_id, path)
            .map_err(|e| e.to_string())?;
    }
    game_path_slots(state, game_id)
}

/// Drop a human override for one slot, falling back to the adapter's own
/// detection again.
#[tauri::command]
fn clear_game_path_override(
    state: State<AppState>,
    game_id: String,
    slot_id: String,
) -> Result<Vec<GamePathSlotDto>, String> {
    {
        let core = state.core.lock().map_err(|e| e.to_string())?;
        core.clear_game_path_override(&game_id, &slot_id)
            .map_err(|e| e.to_string())?;
    }
    game_path_slots(state, game_id)
}

#[tauri::command]
fn set_mod_enabled(
    state: State<AppState>,
    loadout_id: i64,
    mod_id: i64,
    enabled: bool,
) -> Result<Vec<ModRowDto>, String> {
    let core = state.core.lock().map_err(|e| e.to_string())?;
    loadout::set_enabled(&core, loadout_id, mod_id, enabled).map_err(|e| e.to_string())?;
    entries_dto(&core, loadout_id)
}

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

/// The scan itself, split out from the command so it can be tested against a
/// real (in-memory) Core plus real folders — the browser preview only ever
/// exercises the UI's handling of the payload, never this path.
fn loadout_conflicts(core: &Core, loadout_id: i64) -> Result<LoadoutConflictsDto, String> {
    let rows = loadout::entries(core, loadout_id).map_err(|e| e.to_string())?;
    let report = conflicts::report_for_entries(&rows);
    let count = |n: usize| i64::try_from(n).unwrap_or(i64::MAX);
    Ok(LoadoutConflictsDto {
        mods: report
            .mods
            .into_iter()
            .map(|m| ModConflictsDto {
                mod_id: m.mod_id,
                name: m.name,
                wins: count(m.wins),
                loses: count(m.loses),
            })
            .collect(),
        unreadable: report
            .unreadable
            .into_iter()
            .map(|(mod_id, why)| format!("{mod_id}: {why}"))
            .collect(),
    })
}

#[tauri::command]
fn get_loadout_conflicts(
    state: State<AppState>,
    loadout_id: i64,
) -> Result<LoadoutConflictsDto, String> {
    let core = state.core.lock().map_err(|e| e.to_string())?;
    loadout_conflicts(&core, loadout_id)
}

/// Create an empty loadout through the Core. The UI never writes SQLite
/// directly; it remains a client of the engine just like the CLI.
#[tauri::command]
fn create_loadout(
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
    let loadout = loadout::create(&core, &game_id, name).map_err(|e| e.to_string())?;
    Ok(LoadoutDto {
        id: loadout.id,
        game_id,
        name: loadout.name,
        mod_count: 0,
    })
}

/// Import an MO2 profile into a new Skyrim loadout. A mutation like any
/// other: journaled by the Core, and the UI stays a client of the engine.
/// The heavy lifting (profile parsing, folder verification, import order)
/// lives in `modlab_game_skyrim::mo2::import_into`, the same code the CLI
/// runs — this host only validates what the user typed and wraps the answer.
#[tauri::command]
fn mo2_import(
    state: State<AppState>,
    profile: String,
    mods_dir: String,
    name: String,
) -> Result<Mo2ImportResultDto, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("Donne un nom à l'instance.".into());
    }
    if name.chars().count() > 100 {
        return Err("Le nom de l'instance ne peut pas dépasser 100 caractères.".into());
    }
    let core = state.core.lock().map_err(|e| e.to_string())?;
    let (created, imported) = modlab_game_skyrim::mo2::import_into(
        &core,
        Path::new(&profile),
        Path::new(&mods_dir),
        name,
    )?;
    Ok(Mo2ImportResultDto {
        loadout: LoadoutDto {
            id: created.id,
            game_id: SkyrimAdapter.id().to_string(),
            name: created.name,
            mod_count: i64::try_from(imported).unwrap_or(i64::MAX),
        },
        imported,
    })
}

#[tauri::command]
fn launch_skse(state: State<AppState>, loadout_id: i64) -> Result<(), String> {
    #[cfg(not(windows))]
    {
        let _ = (state, loadout_id);
        return Err("le lancement SKSE n'est disponible que sous Windows".into());
    }
    #[cfg(windows)]
    {
        // A human override from Réglages → Jeux must win here — this is the
        // one place that actually launches the game, so it's the one place
        // an ignored override would be most visible (and most confusing).
        let (install_root, usvfs_dll, data_dir, rows) = {
            let core = state.core.lock().map_err(|e| e.to_string())?;
            let install_root = resolve_slot(&core, &SkyrimAdapter, "root")
                .ok_or_else(|| "Skyrim SE introuvable sur cette machine".to_string())?;
            let usvfs_dll = resolve_slot(&core, &SkyrimAdapter, "usvfs_dll").ok_or_else(|| {
                "usvfs_x64.dll introuvable — installe MO2, ou renseigne son chemin dans \
                 Réglages → Jeux"
                    .to_string()
            })?;
            let data_dir = resolve_slot(&core, &SkyrimAdapter, "data_dir")
                .unwrap_or_else(|| install_root.join("Data"));
            let rows = loadout::entries(&core, loadout_id).map_err(|e| e.to_string())?;
            (install_root, usvfs_dll, data_dir, rows)
        };
        let loader = install_root.join("skse64_loader.exe");
        if !loader.is_file() {
            return Err(format!(
                "skse64_loader.exe introuvable dans {}",
                install_root.display()
            ));
        }
        // The whole session counts as "game running" (§6): while it lasts,
        // `ai_ask` refuses to run the local LLM.
        state.game_running.store(true, Ordering::SeqCst);
        let _session = GameSessionGuard(&state.game_running);
        let vfs = modlab_game_skyrim::usvfs::Vfs::create(&usvfs_dll, "modlab")
            .map_err(|e| e.to_string())?;
        modlab_game_skyrim::usvfs::deploy_enabled_mods(&vfs, &rows, &data_dir)
            .map_err(|e| e.to_string())?;
        let cmd = format!("\"{}\"", loader.display());
        vfs.launch_hooked_visible(&cmd, &install_root)
            .map_err(|e| e.to_string())?;
        vfs.wait_for_session_end(Duration::from_secs(2), Duration::from_secs(60))
            .map_err(|e| e.to_string())?;
        Ok(())
    }
}

/// What the AI layer is allowed to know about a loadout — the same facts the
/// conflict scan and the entry table already carry, reshaped for
/// `modlab_ai`. Read-only; recomputed per call like the conflict scan.
fn loadout_facts(core: &Core, loadout_id: i64) -> Result<LoadoutFacts, String> {
    let (game_id, name) = loadout::list(core)
        .map_err(|e| e.to_string())?
        .into_iter()
        .find(|l| l.id == loadout_id)
        .map(|l| (l.game_id, l.name))
        .unwrap_or_else(|| ("?".to_string(), "?".to_string()));
    let rows = loadout::entries(core, loadout_id).map_err(|e| e.to_string())?;
    let report = conflicts::report_for_entries(&rows);
    let mods = rows
        .iter()
        .map(|r| {
            let share = report.mods.iter().find(|m| m.mod_id == r.mod_id);
            ModFact {
                name: r.name.clone(),
                enabled: r.enabled,
                wins: share.map(|m| m.wins).unwrap_or(0),
                loses: share.map(|m| m.loses).unwrap_or(0),
            }
        })
        .collect();
    Ok(LoadoutFacts {
        game_id,
        loadout_name: name,
        mods,
        unreadable: report
            .unreadable
            .iter()
            .map(|(id, why)| format!("{id}: {why}"))
            .collect(),
    })
}

/// Where a local GGUF lives, if one does: `MODLAB_LLM_MODEL` wins, else the
/// first `.gguf` under `<data root>/models`. No discovery beyond that —
/// model management is a later, explicit feature.
fn find_llm_model(data_root: &Path) -> Option<PathBuf> {
    if let Some(from_env) = std::env::var_os("MODLAB_LLM_MODEL") {
        let p = PathBuf::from(from_env);
        if p.is_file() {
            return Some(p);
        }
    }
    let mut entries: Vec<PathBuf> = std::fs::read_dir(models_dir(data_root))
        .ok()?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|x| x.to_str()) == Some("gguf"))
        .collect();
    entries.sort();
    entries.into_iter().next()
}

fn models_dir(data_root: &Path) -> PathBuf {
    data_root.join("models")
}

/// What the AI panel shows: the text plus *who* produced it — the rules
/// engine alone, or the local LLM phrasing the rules engine's facts (§6).
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiAnswerDto {
    pub text: String,
    /// `"rules"` or `"local-llm"` — the chat renders it as a source badge.
    pub source: String,
    pub model: Option<String>,
}

/// Answer a question about the active loadout. §6 in practice: the rules
/// engine diagnoses from Core facts; the local LLM (feature `local-llm`,
/// model found via `MODLAB_LLM_MODEL` or `<data root>/models/*.gguf`) only
/// rephrases those facts and is refused outright while a game runs.
#[tauri::command]
fn ai_ask(
    state: State<AppState>,
    loadout_id: i64,
    question: String,
) -> Result<AiAnswerDto, String> {
    let question = question.trim();
    if question.is_empty() {
        return Err("Pose ta question d'abord.".into());
    }
    if question.chars().count() > 2000 {
        return Err("La question dépasse 2000 caractères.".into());
    }
    if state.game_running.load(Ordering::SeqCst) {
        return Err(
            "Un jeu tourne : l'inférence locale est suspendue pendant la \
             session (§6). Relance ta demande une fois la partie terminée."
                .into(),
        );
    }
    let facts = {
        let core = state.core.lock().map_err(|e| e.to_string())?;
        loadout_facts(&core, loadout_id)?
    };
    let diags = diagnose(&facts);
    let rules = rules_answer(question, &facts, &diags);

    let answer = match find_llm_model(&state.data_root) {
        Some(path) => match llm_phrase(&path, question, &facts, &diags, 256) {
            Ok(text) => AiAnswerDto {
                text,
                source: "local-llm".into(),
                model: Some(path.display().to_string()),
            },
            Err(reason) => AiAnswerDto {
                text: format!("{rules}\n\n(Note : l'inférence locale n'a pas tourné — {reason}.)"),
                source: "rules".into(),
                model: None,
            },
        },
        None => AiAnswerDto {
            text: format!(
                "{rules}\n\n(Note : aucun modèle local — dépose un .gguf dans \
                 {} ou définis MODLAB_LLM_MODEL. Le moteur de règles a répondu \
                 seul.)",
                models_dir(&state.data_root).display()
            ),
            source: "rules".into(),
            model: None,
        },
    };
    Ok(answer)
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
}

#[tauri::command]
fn get_transactions(
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
        })
        .collect())
}

/// Capture the active loadout's configuration (§5 snapshot step). Returns
/// the event id the panel can point at.
#[tauri::command]
fn snapshot_loadout(state: State<AppState>, loadout_id: i64) -> Result<i64, String> {
    let core = state.core.lock().map_err(|e| e.to_string())?;
    transaction::snapshot_loadout(&core, loadout_id).map_err(|e| e.to_string())
}

/// Restore the loadout to its most recent snapshot and hand back the fresh
/// entry list — the panel refreshes the Mods table from this directly.
#[tauri::command]
fn rollback_loadout(state: State<AppState>, loadout_id: i64) -> Result<Vec<ModRowDto>, String> {
    let core = state.core.lock().map_err(|e| e.to_string())?;
    transaction::rollback_loadout(&core, loadout_id).map_err(|e| e.to_string())?;
    entries_dto(&core, loadout_id)
}

pub fn run() {
    let data_root = resolve_data_root();
    let core = open_core(&data_root).expect("failed to open the ModLab engine");
    let _ = core.log_event("ui.started", "{}");

    tauri::Builder::default()
        .manage(AppState {
            core: Mutex::new(core),
            data_root,
            game_running: AtomicBool::new(false),
        })
        .invoke_handler(tauri::generate_handler![
            get_shell_snapshot,
            game_path_slots,
            set_game_path_override,
            clear_game_path_override,
            set_mod_enabled,
            get_loadout_conflicts,
            create_loadout,
            mo2_import,
            launch_skse,
            ai_ask,
            get_transactions,
            snapshot_loadout,
            rollback_loadout
        ])
        .run(tauri::generate_context!())
        .expect("error while running ModLab");
}

#[cfg(test)]
mod tests {
    use super::*;
    use modlab_core::loadout::ImportEntry;
    use std::fs;

    /// `ai_ask` is built on these facts; they must say about the install
    /// exactly what the conflict scan says, or the AI panel and the Mods
    /// table tell two different stories.
    #[test]
    fn ai_facts_match_the_conflict_scan() {
        let core = Core::open_in_memory().unwrap();
        let lo = loadout::create(&core, "skyrimse", "Test").unwrap();

        let dir_a = scratch("ai-a");
        let dir_b = scratch("ai-b");
        fs::write(dir_a.join("config.txt"), b"a").unwrap();
        fs::write(dir_b.join("config.txt"), b"b").unwrap();

        loadout::import_mods(
            &core,
            lo.id,
            &[
                ImportEntry {
                    name: "Mod A".into(),
                    source_path: dir_a.clone(),
                    enabled: true,
                },
                ImportEntry {
                    name: "Mod B".into(),
                    source_path: dir_b.clone(),
                    enabled: true,
                },
                ImportEntry {
                    name: "Mod Off".into(),
                    source_path: dir_a.join("nope").join("deeper"),
                    enabled: false,
                },
            ],
        )
        .unwrap();

        let facts = loadout_facts(&core, lo.id).unwrap();

        assert_eq!(facts.loadout_name, "Test");
        assert_eq!(facts.game_id, "skyrimse");
        assert_eq!(facts.total(), 3);
        assert_eq!(facts.enabled_count(), 2);

        let a = facts.mods.iter().find(|m| m.name == "Mod A").unwrap();
        let b = facts.mods.iter().find(|m| m.name == "Mod B").unwrap();
        // B comes after A in the order, so B is the last link: it wins, A loses.
        assert_eq!(
            (a.wins, a.loses),
            (0, 1),
            "A is overridden: {:?}",
            (a.wins, a.loses)
        );
        assert_eq!((b.wins, b.loses), (1, 0), "B wins: {:?}", (b.wins, b.loses));

        // The disabled mod's missing folder must not leak into `unreadable`:
        // the scan never walks disabled mods.
        assert!(facts.unreadable.is_empty());

        let diags = modlab_ai::diagnose(&facts);
        assert!(
            diags.iter().any(|d| d.title.contains("Mod A")),
            "the loser shows up in the diagnosis: {diags:?}"
        );

        for dir in [dir_a, dir_b] {
            let _ = fs::remove_dir_all(dir);
        }
    }

    /// This crate has no `tempfile` dependency, and adding one just for a test
    /// would be a `Cargo.toml` change nobody asked for — so the scratch folder
    /// is made by hand and removed by the test that made it.
    fn scratch(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("modlab-ui-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// Two mods sharing `interface/config.txt`, plus a third that is disabled
    /// and also provides it. The scan must badge the enabled pair and never
    /// look at the disabled one.
    #[test]
    fn conflicts_dto_tallies_enabled_mods_and_leaves_out_disabled_ones() {
        let core = Core::open_in_memory().unwrap();
        let loadout = loadout::create(&core, "skyrimse", "Test").unwrap();

        let mut folders = Vec::new();
        for (tag, body) in [("losing", "a"), ("winning", "b"), ("disabled", "c")] {
            let dir = scratch(tag);
            fs::create_dir_all(dir.join("interface")).unwrap();
            fs::write(dir.join("interface/config.txt"), body).unwrap();
            folders.push(dir);
        }
        let [losing, winning, disabled] = folders.as_slice() else {
            unreachable!("the loop above always pushes three folders")
        };

        let imported = loadout::import_mods(
            &core,
            loadout.id,
            &[
                ImportEntry {
                    name: "Lux".into(),
                    source_path: losing.clone(),
                    enabled: true,
                },
                ImportEntry {
                    name: "Disabled Overhaul".into(),
                    source_path: disabled.clone(),
                    enabled: false,
                },
                ImportEntry {
                    name: "SkyUI".into(),
                    source_path: winning.clone(),
                    enabled: true,
                },
            ],
        )
        .unwrap();
        assert_eq!(imported, 3);

        let dto = loadout_conflicts(&core, loadout.id).unwrap();

        assert_eq!(dto.mods.len(), 2, "only enabled mods take part in a scan");
        let lux = dto.mods.iter().find(|m| m.name == "Lux").unwrap();
        let skyui = dto.mods.iter().find(|m| m.name == "SkyUI").unwrap();
        assert_eq!((lux.wins, lux.loses), (0, 1));
        assert_eq!((skyui.wins, skyui.loses), (1, 0));
        assert!(dto.unreadable.is_empty());

        for dir in folders {
            let _ = fs::remove_dir_all(dir);
        }
    }

    /// A mod folder that no longer exists must not swallow the whole answer:
    /// the readable mods are still reported, and the failure is named.
    #[test]
    fn conflicts_dto_reports_a_missing_folder_as_unreadable() {
        let core = Core::open_in_memory().unwrap();
        let loadout = loadout::create(&core, "skyrimse", "Test").unwrap();

        let present = scratch("present");
        fs::write(present.join("readme.txt"), b"x").unwrap();
        let gone = present.join("this-folder-was-renamed");

        loadout::import_mods(
            &core,
            loadout.id,
            &[
                ImportEntry {
                    name: "Present".into(),
                    source_path: present.clone(),
                    enabled: true,
                },
                ImportEntry {
                    name: "Renamed Away".into(),
                    source_path: gone,
                    enabled: true,
                },
            ],
        )
        .unwrap();

        let dto = loadout_conflicts(&core, loadout.id).unwrap();

        assert_eq!(dto.mods.len(), 1, "the readable mod is still reported");
        assert_eq!(dto.mods[0].name, "Present");
        assert_eq!(dto.unreadable.len(), 1);
        assert!(
            dto.unreadable[0].contains("this-folder-was-renamed"),
            "the payload names the folder that failed: {}",
            dto.unreadable[0]
        );

        let _ = fs::remove_dir_all(present);
    }
}
