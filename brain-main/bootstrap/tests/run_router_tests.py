# -*- coding: utf-8 -*-
"""Tests autonomes des règles d'optimisation du routeur (priorité 1) : ROUTER.md, agents de l'adaptateur et de .claude/.
Bibliothèque standard seulement. Lecture seule : rien n'est écrit.

Usage : python brain-main/bootstrap/tests/run_router_tests.py
Sortie : une ligne PASS/FAIL par contrôle. Code de sortie 0 si tout passe, 1 sinon.
"""

import re
import sys
import traceback
from pathlib import Path

BOOT = Path(__file__).resolve().parent.parent
REPO = BOOT.parent.parent
ROUTER = REPO / "brain-main" / "ROUTER.md"
ADAPTER = REPO / "brain-main" / "adapters" / "claude-code"
AGENTS3 = ("orchestrator", "worker-sonnet", "worker-haiku")
# Écarts documentés (voir ADAPTER_GAPS dans run_bootstrap_tests.py) : seul l'orchestrateur diffère, de trois lignes.
GAP_LINES = 3
FORBIDDEN_KEYS = ("autoCompactWindow", "effort", "maxTurns")

RESULTS = []


def check(name, cond, detail=""):
    RESULTS.append((name, bool(cond)))
    print(("PASS " if cond else "FAIL ") + name + ("" if cond else "  :: " + str(detail)[:500]))


def lf(path):
    return Path(path).read_bytes().decode("utf-8").replace("\r\n", "\n")


def section(text, start, end):
    """Texte entre deux titres (début inclus, fin exclue)."""
    a = text.index(start)
    return text[a:text.index(end, a + len(start))]


def has_all(text, words):
    return all(w in text for w in words)


def test_router():
    r = lf(ROUTER)
    s22 = section(r, "### 2.2 Découpage", "## 3. Brief")
    check("ROUTER §2.2: signes d'une mission volumineuse (fichiers, sources, livrables, résultat dont dépend la suite)",
          has_all(s22, ("beaucoup de fichiers", "longues sources", "plusieurs livrables", "dont dépend la suite")), s22)
    check("ROUTER §2.2: sous-tâches avec périmètre et livrable, séquence si dépendance, points de contrôle, pas de découpage inutile",
          has_all(s22, ("périmètre", "livrable", "séquence", "point de contrôle", "Pas de découpage inutile")), s22)
    s3 = section(r, "## 3. Brief", "## 4. Rapport")
    check("ROUTER §3: strict nécessaire, sections plutôt que documents entiers, pas de répétition du prompt du worker",
          has_all(s3, ("strict nécessaire", "une section plutôt que le document entier", "ne répète pas ce que le prompt du worker porte déjà", "règle d'un adaptateur")), s3)
    s4 = section(r, "## 4. Rapport", "## 5. Connaissances")
    check("ROUTER §4: format court (statut, résultat, fichiers et lignes, tests, incertitudes), taille proportionnelle sans plafond chiffré",
          has_all(s4, ("décisionnel court", "statut", "résultat", "lignes", "commande, code de sortie, comptes", "incertitudes et décisions", "proportionnelle à la mission", "sans plafond chiffré")), s4)
    check("ROUTER §4: preuves à la demande (commande à rejouer ou fichier temporaire), sortie brute décisive, échec ou demandée par le brief",
          has_all(s4, ("à la demande", "commande à rejouer", "fichier temporaire", "décisive", "en cas d'échec", "si le brief la demande")), s4)
    check("ROUTER §4: partial en cas de dépassement net du brief ; la phrase sur les README d'un commit reste",
          has_all(s4, ("dépasse nettement le brief", "`partial`", "README des dossiers modifiés")), s4)
    check("ROUTER: plus de « preuves brutes » ni de « sortie brute dans le rapport » systématiques",
          "preuves brutes" not in r and "sortie brute dans le rapport" not in r)
    s6 = section(r, "## 6. Question en cours de mission", "## 7. Contrôle")
    check("ROUTER §6.1: comparaison de coût (contexte hérité relu à chaque tour / démarrage + relecture + brief)",
          "### 6.1" in s6 and has_all(s6, ("contexte hérité", "chaque tour restant", "démarrage", "relecture nécessaire", "brief")), s6)
    check("ROUTER §6.1: reprendre si blocage, question ou petite suite ; sinon handoff (fait, décisions, fichiers, reste à faire) ; pas d'interdiction",
          has_all(s6, ("blocage", "question", "petite suite", "l'historique sert vraiment", "handoff synthétique", "ce qui est fait",
                       "les décisions", "les fichiers", "le reste à faire", "Aucune interdiction de reprendre")), s6)
    s7 = section(r, "## 7. Contrôle", "## 8. Trace")
    check("ROUTER §7: contrôle sur preuves vérifiables, détail demandé en cas de doute, contre-vérification en exception",
          has_all(s7, ("preuves vérifiables", "en cas de doute", "demande le détail", "contre-vérification")), s7)
    check("ROUTER §7.1: gestion adaptative (progression, contexte, coût ; réviser, changer de profil, recadrer, arrêter ; partial)",
          "### 7.1 Gestion adaptative" in s7 and has_all(s7, ("progression", "contexte", "coût", "réviser le plan", "changer de profil", "recadrer", "arrêter", "`partial`")), s7)
    s9 = r[r.index("## 9. Contrat d'adaptateur"):]
    check("ROUTER §9: l'adaptateur dit les signaux de suivi, ce qu'on ne peut pas surveiller et l'arrêt d'un worker",
          has_all(s9, ("signaux de suivi", "ce qu'on ne peut pas surveiller", "arrêter un worker")), s9)
    check("ROUTER: numérotation §1 à §9 conservée (pas de renumérotation)",
          re.findall(r"^## (\d+)\. ", r, re.M) == [str(i) for i in range(1, 10)])


def test_no_names_no_ceiling():
    r = lf(ROUTER)
    body = r.split("\n---\n", 1)[1].replace("# Agent Router", "")  # le titre n'est pas un nom d'outil
    tools = [w for w in ("Agent", "SendMessage", "AskUserQuestion", "TaskStop", "Bash", "Read", "Grep", "Glob", "Edit", "Write",
                         "output_file", "subagent_tokens", "tool_uses") if re.search(r"\b%s\b" % w, body)]
    check("ROUTER: aucun nom d'outil", not tools, tools)
    models = [w for w in ("opus", "sonnet", "haiku", "claude", "anthropic", "gpt") if re.search(w, body, re.I)]
    check("ROUTER: aucun nom de modèle", not models, models)
    unit = r"(?:jetons?|tokens?|minutes?|secondes?|heures?|lignes?|caractères|mots|Ko|Mo|k\b|%|fois)"
    ceil = re.findall(r"\d[\d   .,]*\s*" + unit, body, re.I)
    check("ROUTER: aucun chiffre de plafond (jetons, minutes, lignes, pourcentage...)", not ceil, ceil)
    check("ROUTER: aucun seuil chiffré du type « au plus N » ou « N maximum »",
          not re.search(r"(?:au plus|maximum|max\.?|plafond de|limite de)\s*\d|\d+\s*(?:maximum|max\b)", body, re.I))


def test_agents():
    for n in AGENTS3:
        a, b = lf(ADAPTER / "agents" / (n + ".md")), lf(REPO / ".claude" / "agents" / (n + ".md"))
        la, lb = a.splitlines(), b.splitlines()
        gap = [l for l in lb if l not in la]
        want = GAP_LINES if n == "orchestrator" else 0
        check("agents: %s identique entre .claude/ et l'adaptateur (écarts documentés : %d lignes)" % (n, want),
              len(gap) == want and len(la) == len(lb), gap)
    for n in AGENTS3:
        for label, base in (("adaptateur", ADAPTER / "agents"), (".claude", REPO / ".claude" / "agents")):
            bad = [w for w in FORBIDDEN_KEYS if w in lf(base / (n + ".md"))]
            check("agents: %s (%s) sans autoCompactWindow, effort, maxTurns" % (n, label), not bad, bad)
    for label, base in (("adaptateur", ADAPTER), (".claude", REPO / ".claude")):
        bad = [w for w in FORBIDDEN_KEYS if w in lf(base / "settings.json")]
        check("settings.json (%s) sans autoCompactWindow, effort, maxTurns" % label, not bad, bad)
    for n in ("worker-sonnet", "worker-haiku"):
        t = lf(ADAPTER / "agents" / (n + ".md"))
        check("%s: rapport court (statut, résultat, fichiers, tests, incertitudes), plus de « Rapport en 5 lignes » ni de sortie brute systématique" % n,
              has_all(t, ("Rapport court et décisionnel", "Sortie brute seulement si elle est décisive", "tests : commande, code de sortie, comptes"))
              and "Rapport en 5 lignes" not in t and "cite leur sortie brute" not in t, t)
        check("%s: partial en cas de dépassement, gros volume de sortie filtré ou résumé" % n,
              has_all(t, ("rends `partial`", "Gros volume de sortie", "filtre ou résume")), t)
    h = lf(ADAPTER / "agents" / "worker-haiku.md")
    check("worker-haiku: « Créer un fichier » reste bloqué, aucune preuve par fichier temporaire",
          "Créer un fichier n'est pas de ton ressort : arrête-toi avec le statut `blocked`." in h and "fichier temporaire" not in h, h)
    o = lf(ADAPTER / "agents" / "orchestrator.md")
    for fact in ("5 minutes", "réécrit tout son contexte", "`subagent_tokens`", "taille du contexte final du worker, pas sa consommation",
                 "multiplié par 5", "100 000 jetons", "repère de coût, pas une limite", "Ne lis jamais le fichier `output_file`",
                 "jamais le paramètre `model`", "l'autre worker", "`TaskStop`", "reste sur le disque", "§1 à §8", "§6.1", "§7.1", "§2.2",
                 "demande-la explicitement dans le brief"):
        check("orchestrator: contient « %s »" % fact, fact in o)
    tools_line = re.search(r"^tools: .*$", o, re.M).group(0)
    check("orchestrator: tools = liste précédente + TaskStop (arrêt d'un worker), aucun autre outil",
          tools_line == "tools: Read, Grep, Glob, Agent(worker-sonnet, worker-haiku), SendMessage, TaskStop, AskUserQuestion", tools_line)
    o2 = lf(REPO / ".claude" / "agents" / "orchestrator.md")
    t2 = re.search(r"^tools: .*$", o2, re.M).group(0)
    check("TaskStop présent dans les deux copies de l'orchestrateur, tools identiques",
          "TaskStop" in tools_line and "TaskStop" in t2 and tools_line == t2, (tools_line, t2))


def test_readme_and_skill():
    r = lf(ADAPTER / "README.md")
    check("README adaptateur: signaux disponibles (tool_uses, durée, contexte final, rapports) et limites",
          has_all(r, ("## Signaux de suivi", "`tool_uses`", "durée", "contexte final", "ne peut pas surveiller")), r)
    check("README adaptateur: arrêt d'un worker par TaskStop, doc citée, non essayé, SendMessage au tour suivant seulement",
          has_all(r, ("possible avec `TaskStop`", "non essayé en session réelle", "https://code.claude.com/docs/en/sub-agents", "`TaskStop`", "tour suivant")), r)
    sc = r[r.index("## Scénarios à observer"):]
    check("README adaptateur: table de 5 scénarios (grosse mission, reprise, handoff, rapport court puis détail, partial)",
          has_all(sc, ("Grosse mission découpée", "Reprise utile", "Handoff", "Rapport court puis demande de détail", "Dépassement avec `partial`")), sc)
    skill = lf(BOOT / "account" / "SKILL.md")
    check("SKILL /brain: restitution de la sortie brute du worker maintenue (compatible: le brief la demande explicitement)",
          "relire sa sortie brute et la restituer sans la réécrire" in skill)


def main():
    sys.stdout.reconfigure(encoding="utf-8")
    for name, fn in (("ROUTER.md", test_router), ("noms et plafonds", test_no_names_no_ceiling), ("agents", test_agents),
                     ("README et skill", test_readme_and_skill)):
        try:
            fn()
        except Exception:
            check("exception dans le groupe " + name, False, traceback.format_exc()[-800:])
    ok = sum(1 for _, r in RESULTS if r)
    print("\nRÉSULTAT : %d PASS, %d FAIL sur %d" % (ok, len(RESULTS) - ok, len(RESULTS)))
    return 0 if ok == len(RESULTS) else 1


if __name__ == "__main__":
    sys.exit(main())
