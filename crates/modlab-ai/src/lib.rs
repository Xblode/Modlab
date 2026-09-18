//! ModLab's local AI layer — `docs/01-architecture-decidee.md` §6.
//!
//! The split is deliberate and non-negotiable: the **rules engine** produces
//! the diagnosis from Core facts, and the LLM (when present) only *phrases*
//! what the rules engine already determined. This crate is therefore a pure
//! consumer of facts — it never talks to the Core, never reads the
//! filesystem, and never decides anything a rule didn't say first. The
//! anti-hallucination contract: every sentence a rules answer contains can be
//! traced to a field of `LoadoutFacts` or a `Diagnostic`.
//!
//! The llama.cpp backend (`llama.rs`, behind the `local-llm` feature) receives
//! the same facts and diagnostics as prompt material and is instructed not to
//! add any. A GBNF-constrained output format arrives with the MCP wiring
//! (§7) — the prompt-level instruction is the first slice, not the end state.

use std::path::Path;

#[cfg(feature = "local-llm")]
pub mod llama;

/// One mod's share of the loadout's conflict scan — mirrored from
/// `modlab_core::conflicts`, so this crate never needs a Core dependency.
#[derive(Debug, Clone)]
pub struct ModFact {
    pub name: String,
    pub enabled: bool,
    /// Distinct file paths this mod wins (is the last link for).
    pub wins: usize,
    /// Distinct file paths this mod loses to a later mod.
    pub loses: usize,
}

/// Everything the AI layer is allowed to know about a loadout. Built by the
/// host (Tauri shell, CLI, MCP) from Core queries — never by this crate.
#[derive(Debug, Clone, Default)]
pub struct LoadoutFacts {
    pub game_id: String,
    pub loadout_name: String,
    pub mods: Vec<ModFact>,
    /// Enabled mods whose folder could not be read, as `id: reason`.
    pub unreadable: Vec<String>,
}

impl LoadoutFacts {
    pub fn enabled_count(&self) -> usize {
        self.mods.iter().filter(|m| m.enabled).count()
    }

    pub fn total(&self) -> usize {
        self.mods.len()
    }

    /// Mods that lose at least one file conflict, worst first.
    fn conflict_losers(&self) -> Vec<&ModFact> {
        let mut losing: Vec<&ModFact> = self.mods.iter().filter(|m| m.loses > 0).collect();
        losing.sort_by(|a, b| b.loses.cmp(&a.loses).then(a.name.cmp(&b.name)));
        losing
    }

    /// Mods that win at least one file conflict, biggest winner first.
    fn conflict_winners(&self) -> Vec<&ModFact> {
        let mut winning: Vec<&ModFact> = self.mods.iter().filter(|m| m.wins > 0).collect();
        winning.sort_by(|a, b| b.wins.cmp(&a.wins).then(a.name.cmp(&b.name)));
        winning
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Info,
    Warn,
    /// Something the user must act on — e.g. a mod folder that vanished.
    Blocker,
}

#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub severity: Severity,
    pub title: String,
    pub detail: String,
}

/// The rules engine's whole output for a loadout. Deliberately small: these
/// are the facts the conflict scan and the entry table already established —
/// restated, never inferred.
pub fn diagnose(facts: &LoadoutFacts) -> Vec<Diagnostic> {
    let mut out = Vec::new();

    if facts.total() == 0 {
        out.push(Diagnostic {
            severity: Severity::Warn,
            title: "Instance vide".into(),
            detail: "Cette instance ne contient aucun mod.".into(),
        });
        return out;
    }

    if facts.enabled_count() == 0 {
        out.push(Diagnostic {
            severity: Severity::Warn,
            title: "Tous les mods sont désactivés".into(),
            detail: format!(
                "{} mods dans l'instance, aucun actif : le déploiement ne montera rien.",
                facts.total()
            ),
        });
    }

    for m in facts.conflict_losers() {
        out.push(Diagnostic {
            severity: Severity::Warn,
            title: format!("« {} » est écrasé", m.name),
            detail: format!(
                "Ce mod est écrasé sur {n} fichier(s) par un mod placé plus bas dans l'ordre de priorité.",
                n = m.loses
            ),
        });
    }

    for m in facts.conflict_winners() {
        out.push(Diagnostic {
            severity: Severity::Info,
            title: format!("{} écrase des fichiers", m.name),
            detail: format!(
                "Ce mod écrase {n} fichier(s) appartenant aussi à un autre mod (« dernier lié gagne », ADR 0002).",
                n = m.wins
            ),
        });
    }

    if !facts.unreadable.is_empty() {
        out.push(Diagnostic {
            severity: Severity::Blocker,
            title: "Dossiers de mods illisibles".into(),
            detail: format!(
                "{} mod(s) activé(s) pointent vers un dossier absent ou illisible : {}",
                facts.unreadable.len(),
                facts.unreadable.join(" ; ")
            ),
        });
    }

    out
}

/// The default phrasing: a rules-only answer in French, built strictly from
/// the facts. Used as-is when no LLM is available, and as the factual
/// skeleton an LLM is asked to rephrase.
pub fn rules_answer(question: &str, facts: &LoadoutFacts, diags: &[Diagnostic]) -> String {
    let mut parts: Vec<String> = Vec::new();

    parts.push(format!(
        "Instance « {} » ({} · {} mods, {} activé(s)).",
        facts.loadout_name,
        facts.game_id,
        facts.total(),
        facts.enabled_count()
    ));

    let q = question.to_lowercase();
    let asks_health = q.contains("santé")
        || q.contains("diagnos")
        || q.contains("problème")
        || q.contains("conflit")
        || q.contains("écrase")
        || q.contains("état");

    if asks_health || diags.iter().any(|d| d.severity != Severity::Info) {
        for d in diags {
            let tag = match d.severity {
                Severity::Blocker => "Bloquant",
                Severity::Warn => "À vérifier",
                Severity::Info => "Info",
            };
            parts.push(format!("{} — {} : {}", tag, d.title, d.detail));
        }
    }

    if parts.len() == 1 {
        parts.push(
            "Le moteur de règles n'a rien de particulier à signaler sur cette \
             instance : aucun conflit de fichier détecté, tous les dossiers \
             lisibles."
                .into(),
        );
    }

    parts.join("\n\n")
}

/// Build the exact prompt an LLM backend receives. Ungated so it is testable
/// without the llama.cpp toolchain: the instruction pins the model to the
/// provided facts and forbids adding any (§6 anti-hallucination design).
pub fn llm_prompt(question: &str, facts: &LoadoutFacts, diags: &[Diagnostic]) -> String {
    let mut facts_text = format!(
        "- instance « {} » du jeu {} : {} mods, {} activé(s)\n",
        facts.loadout_name,
        facts.game_id,
        facts.total(),
        facts.enabled_count()
    );
    for m in &facts.mods {
        let state = if m.enabled { "activé" } else { "désactivé" };
        facts_text.push_str(&format!(
            "- mod « {} » ({}, {} conflit(s) gagné(s), {} perdu(s))\n",
            m.name, state, m.wins, m.loses
        ));
    }
    if !facts.unreadable.is_empty() {
        facts_text.push_str(&format!(
            "- dossiers illisibles : {}\n",
            facts.unreadable.join(" ; ")
        ));
    }
    let diags_text = diags
        .iter()
        .map(|d| {
            format!(
                "  - [{}] {} : {}",
                severity_label(d.severity),
                d.title,
                d.detail
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    let diags_block = if diags.is_empty() {
        "  (aucun problème signalé)".to_string()
    } else {
        diags_text
    };

    format!(
        "Tu es l'assistant local de ModLab, un gestionnaire de mods. \
Tu dois reformuler en français clair UNIQUEMENT les faits listés ci-dessous. \
Interdiction absolue d'ajouter un fait qui n'y figure pas : ne suggère pas de \
mod, ne cite pas de chemin absent de la liste, ne devine rien. Si l'utilisateur \
demande quelque chose qui n'est pas couvert par ces faits, dis-le.\n\n\
FAITS DU CORE :\n\
{facts_text}\n\
DIAGNOSTIC DU MOTEUR DE RÈGLES :\n\
{diags_block}\n\n\
QUESTION DE L'UTILISATEUR : {question}\n\n\
RÉPONSE :",
        facts_text = facts_text,
        diags_block = diags_block,
        question = question,
    )
}

fn severity_label(severity: Severity) -> &'static str {
    match severity {
        Severity::Blocker => "bloquant",
        Severity::Warn => "à vérifier",
        Severity::Info => "info",
    }
}

/// Try to phrase `llm_prompt(question, facts, diags)` through the local
/// llama.cpp backend. Without the `local-llm` feature this errs with an
/// explicit reason — the host falls back to the rules answer and says so.
pub fn llm_phrase(
    model_path: &Path,
    question: &str,
    facts: &LoadoutFacts,
    diags: &[Diagnostic],
    max_tokens: i32,
) -> Result<String, String> {
    #[cfg(feature = "local-llm")]
    {
        let prompt = llm_prompt(question, facts, diags);
        crate::llama::generate(model_path, &prompt, max_tokens)
    }
    #[cfg(not(feature = "local-llm"))]
    {
        let _ = (model_path, question, facts, diags, max_tokens);
        Err(
            "cette version de ModLab a été compilée sans le moteur d'inférence \
             local (feature `local-llm`)"
                .into(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn facts() -> LoadoutFacts {
        LoadoutFacts {
            game_id: "skyrimse".into(),
            loadout_name: "Main 2026".into(),
            mods: vec![
                ModFact {
                    name: "SkyUI".into(),
                    enabled: true,
                    wins: 2,
                    loses: 0,
                },
                ModFact {
                    name: "Lux".into(),
                    enabled: true,
                    wins: 0,
                    loses: 3,
                },
                ModFact {
                    name: "Disabled Overhaul".into(),
                    enabled: false,
                    wins: 0,
                    loses: 0,
                },
            ],
            unreadable: vec![],
        }
    }

    #[test]
    fn diagnose_flags_losers_winners_and_counts() {
        let f = facts();
        let diags = diagnose(&f);

        assert!(
            diags
                .iter()
                .any(|d| d.title.contains("Lux") && d.severity == Severity::Warn),
            "the losing mod must be flagged: {diags:?}"
        );
        assert!(
            diags
                .iter()
                .any(|d| d.title.contains("SkyUI") && d.severity == Severity::Info),
            "the winning mod is informational: {diags:?}"
        );
        // The disabled mod takes part in nothing — no diagnosis may name it.
        assert!(
            !diags.iter().any(|d| d.title.contains("Disabled Overhaul")),
            "disabled mods are outside every rule: {diags:?}"
        );
    }

    #[test]
    fn diagnose_flags_an_empty_loadout() {
        let diags = diagnose(&LoadoutFacts::default());
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].severity, Severity::Warn);
        assert!(diags[0].title.contains("vide"));
    }

    #[test]
    fn diagnose_flags_unreadable_folders_as_blockers() {
        let mut f = facts();
        f.unreadable = vec!["7: le dossier a été renommé".into()];
        let diags = diagnose(&f);
        assert!(
            diags.iter().any(|d| d.severity == Severity::Blocker),
            "a vanished folder blocks: {diags:?}"
        );
    }

    #[test]
    fn rules_answer_stays_within_the_facts() {
        let f = facts();
        let diags = diagnose(&f);
        let a = rules_answer("Qui a gagné la coupe du monde 1998 ?", &f, &diags);
        assert!(
            a.contains("Main 2026"),
            "the answer states its instance: {a}"
        );
        assert!(
            !a.to_lowercase().contains("1998"),
            "the rules engine must not answer what no fact covers: {a}"
        );
    }

    #[test]
    fn rules_answer_lists_diagnostics_when_asked_about_health() {
        let f = facts();
        let diags = diagnose(&f);
        let a = rules_answer("Y a-t-il des conflits ?", &f, &diags);
        assert!(
            a.contains("Lux"),
            "conflicts are part of a health answer: {a}"
        );
    }

    #[test]
    fn llm_prompt_carries_facts_and_the_no_invention_rule() {
        let f = facts();
        let diags = diagnose(&f);
        let p = llm_prompt("Y a-t-il des conflits ?", &f, &diags);
        assert!(p.contains("Interdiction absolue d'ajouter un fait"));
        assert!(p.contains("Lux"));
        assert!(p.contains("3 perdu(s)"));

        let mut empty = LoadoutFacts::default();
        empty.loadout_name = "Vide".into();
        let p = llm_prompt("vois quelque chose ?", &empty, &[]);
        assert!(p.contains("aucun problème signalé"));
    }
}
