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
    let report = conflicts::report_for_entries(&virtually_deployed_rows(&rows));
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
pub(crate) fn ai_ask(
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

#[cfg(test)]
mod tests {
    use super::*;
    use modlab_core::loadout::ImportEntry;
    use std::fs;

    /// This crate has no `tempfile` dependency, and adding one just for a test
    /// would be a `Cargo.toml` change nobody asked for — so the scratch folder
    /// is made by hand and removed by the test that made it.
    fn scratch(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("modlab-ui-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

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
                    version: None,
                },
                ImportEntry {
                    name: "Mod B".into(),
                    source_path: dir_b.clone(),
                    enabled: true,
                    version: None,
                },
                ImportEntry {
                    name: "Mod Off".into(),
                    source_path: dir_a.join("nope").join("deeper"),
                    enabled: false,
                    version: None,
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
}
