# -*- coding: utf-8 -*-
"""État d'un projet vis-à-vis de 2brain. Lecture seule.

Usage : python -I status.py [--project DOSSIER] [--json] [--offline] [--home DOSSIER]
Codes de sortie : 0 normal, 2 dossier invalide.
"""

import argparse
import json
import subprocess
import sys
import urllib.request
from pathlib import Path

REMOTE_VERSION_URL = "https://raw.githubusercontent.com/Xblode/2brain/master/brain-main/bootstrap/VERSION"
MANIFEST = "manifest.json"
DOCS = ("vision.md", "project.md", "requirements.md", "architecture.md", "workflow.md")
# Sixième document standard (décision de l'humain, 2026-11-06). Son absence ne rend pas `brain/` partiel : les projets installés avant lui restent complets.
DOCS_OPTIONAL = ("cadrage.md",)
BRAIN_MAIN_REQUIRED = ("README.md", "conventions", "skills", "bootstrap")
TECH_HINTS = (
    "package.json", "pnpm-lock.yaml", "yarn.lock", "tsconfig.json", "pyproject.toml",
    "requirements.txt", "setup.py", "Cargo.toml", "go.mod", "pom.xml", "build.gradle",
    "Gemfile", "composer.json", "Dockerfile", "docker-compose.yml", "Makefile",
    "next.config.js", "vite.config.ts", "README.md", "AGENTS.md", "CLAUDE.md",
)


def is_source_repo(project):
    """Dépôt source 2brain : remote origin Xblode/2brain, lab/ et le registre présents."""
    try:
        out = subprocess.run(
            ["git", "-C", str(project), "remote", "get-url", "origin"],
            capture_output=True, text=True, timeout=10, stdin=subprocess.DEVNULL,
        )
        if out.returncode != 0 or "Xblode/2brain" not in out.stdout:
            return False
    except Exception:
        return False
    return (project / "lab").is_dir() and (project / "brain" / "registry" / "entries.jsonl").is_file()


def parse_version(text):
    try:
        return tuple(int(p) for p in text.strip().split("."))
    except Exception:
        return None


def read_text(path):
    try:
        return path.read_text(encoding="utf-8").strip()
    except Exception:
        return None


def brain_main_state(project):
    root = project / "brain-main"
    if not root.exists():
        return {"etat": "absent", "manifeste": False, "version_installee": None}
    missing = [n for n in BRAIN_MAIN_REQUIRED if not (root / n).exists()]
    version = None
    manifest = (root / MANIFEST).is_file()
    if manifest:
        try:
            version = json.loads((root / MANIFEST).read_text(encoding="utf-8")).get("version")
        except Exception:
            version = None
    return {
        "etat": "complet" if not missing else "partiel",
        "manquants": missing,
        "manifeste": manifest,
        "version_installee": version,
    }


TODO_MARK = "À compléter"
SYSTEM_ENTRIES = (".git", "brain", "brain-main", ".claude", "AGENTS.md", "CLAUDE.md", ".gitignore", "EMPLACEMENT")
AGENT_NAMES = ("orchestrator", "worker-sonnet", "worker-haiku")


def router_state(project):
    """Routeur Claude Code : `configuré` (clé agent = orchestrator et trois agents ; la configuration est lue, pas la session en cours), `désactivé` (agents présents, clé `agent` absente), `absent` (ni l'un ni l'autre), sinon `partiel`."""
    try:
        data = json.loads((project / ".claude" / "settings.json").read_text(encoding="utf-8"))
        key = isinstance(data, dict) and data.get("agent") == "orchestrator"
        other = data.get("agent") if isinstance(data, dict) and "agent" in data and not key else None
        has_key = isinstance(data, dict) and "agent" in data
        readable = isinstance(data, dict)
    except FileNotFoundError:
        key, other, has_key, readable = False, None, False, True
    except Exception:
        key, other, has_key, readable = False, None, False, False
    present = [n for n in AGENT_NAMES if (project / ".claude" / "agents" / (n + ".md")).is_file()]
    if key and len(present) == len(AGENT_NAMES):
        return {"etat": "configuré", "raison": "`.claude/settings.json` contient \"agent\": \"orchestrator\" et les trois agents sont présents ; s'applique aux conversations ouvertes après l'installation, sans dire si la session courante est routée"}
    if readable and not has_key and len(present) == len(AGENT_NAMES):
        return {"etat": "désactivé", "raison": "les trois agents sont présents mais `.claude/settings.json` n'a pas de clé `agent` : `/brain -d on` active le routeur (à la conversation suivante)"}
    if not key and not present:
        return {"etat": "absent", "raison": "ni la clé `agent` ni les agents dans `.claude/`"}
    parts = []
    parts.append("clé `agent` = orchestrator présente" if key else
                 "clé `agent` = %s dans `.claude/settings.json` (autre valeur que orchestrator)" % json.dumps(other, ensure_ascii=False) if has_key else
                 "clé `agent` = orchestrator absente de `.claude/settings.json`" if readable else
                 "`.claude/settings.json` illisible ou n'est pas un objet JSON")
    missing = [n for n in AGENT_NAMES if n not in present]
    parts.append("agents : %s" % ("complets" if not missing else "manquants : " + ", ".join(missing)))
    return {"etat": "partiel", "raison": "; ".join(parts)}


def docs_to_complete(docs_dir):
    """Documents de brain/docs/ contenant encore « À compléter » (noms triés)."""
    out = []
    if docs_dir.is_dir():
        for p in sorted(docs_dir.glob("*.md")):
            text = read_text(p)
            if text and TODO_MARK in text:
                out.append(p.name)
    return out


def brain_state(project):
    root = project / "brain"
    if not root.exists():
        return {"etat": "absent", "entrees_registre": 0, "documents_a_completer": 0, "documents_a_completer_noms": []}
    docs_dir = root / "docs"
    present = {p.name.lower() for p in docs_dir.iterdir()} if docs_dir.is_dir() else set()
    missing = [d for d in DOCS if d not in present]
    todo = docs_to_complete(docs_dir)
    registry = root / "registry" / "entries.jsonl"
    count = 0
    if registry.is_file():
        try:
            with registry.open(encoding="utf-8") as fh:
                count = sum(1 for line in fh if line.strip())
        except Exception:
            count = 0
    else:
        missing.append("registry/entries.jsonl")
    to_create = [d for d in DOCS_OPTIONAL if d not in present] if docs_dir.is_dir() else []
    return {
        "etat": "complet" if not missing else "partiel",
        "manquants": missing,
        "documents_a_creer": to_create,
        "entrees_registre": count,
        "documents_a_completer": len(todo),
        "documents_a_completer_noms": todo,
    }


def last_audit(project):
    """Nom du dernier rapport de brain/sources/audits/ (ordre des noms AAAAMMJJ-HHMM.md), sinon None."""
    d = project / "brain" / "sources" / "audits"
    try:
        names = sorted(p.name for p in d.glob("*.md") if p.is_file())
    except OSError:
        return None
    return names[-1] if names else None


def readme_max(project):
    """(valeur, détail) du réglage `readme_max` de brain/settings.json ; 8 par défaut si le fichier est absent ou invalide."""
    path = project / "brain" / "settings.json"
    if not path.is_file():
        return 8, "brain/settings.json absent : valeur par défaut 8"
    try:
        data = json.loads(path.read_bytes().decode("utf-8"))
    except Exception:
        return 8, "brain/settings.json invalide (JSON illisible) : valeur par défaut 8"
    value = data.get("readme_max") if isinstance(data, dict) else None
    if isinstance(value, bool) or not isinstance(value, int) or value < 1:
        return 8, "brain/settings.json sans `readme_max` valide (entier supérieur ou égal à 1) : valeur par défaut 8"
    return value, "lu dans brain/settings.json"


def has_project_files(entries):
    """Vrai si le premier niveau contient autre chose que les fichiers et dossiers de 2brain."""
    return any(e not in SYSTEM_ENTRIES for e in entries)


def remote_version(offline):
    if offline:
        return None
    try:
        with urllib.request.urlopen(REMOTE_VERSION_URL, timeout=3) as resp:
            if resp.status != 200:
                return None
            text = resp.read(64).decode("utf-8").strip()
        return text if parse_version(text) else None
    except Exception:
        return None


def build(project, home, offline):
    entries = sorted(p.name for p in project.iterdir())
    account = read_text(home / ".claude" / "skills" / "brain" / "VERSION")
    in_project = read_text(project / "brain-main" / "bootstrap" / "VERSION")
    remote = remote_version(offline)
    notes = []
    a, r, p = parse_version(account or ""), parse_version(remote or ""), parse_version(in_project or "")
    if remote is None:
        notes.append("Version distante : non vérifié.")
    elif a is None:
        notes.append("Version du Bootstrap sur le compte inconnue.")
    elif r and r > a:
        notes.append(
            "Une version plus récente du Bootstrap existe (%s > %s). Relance l'installation "
            "sur le compte (install-account.py) : /brain -u ne met pas à jour le compte." % (remote, account)
        )
    if a and p and p > a:
        notes.append(
            "Une version plus récente du Bootstrap existe : le projet embarque %s, le compte a %s. "
            "Relance install-account.py (brain-main/bootstrap/scripts/) pour mettre à jour le compte." % (in_project, account)
        )
    brain = brain_state(project)
    if brain["etat"] != "absent" and brain["documents_a_completer"] > 0:
        notes.append(
            "%d document(s) de brain/docs/ contiennent encore « %s » : /brain -o propose de les remplir (rien n'est écrit sans validation)."
            % (brain["documents_a_completer"], TODO_MARK)
        )
    if brain["etat"] != "absent" and brain.get("documents_a_creer"):
        notes.append("Document standard absent de brain/docs/ : %s. /brain -o propose de le créer, après validation (brain/ reste complet ; /brain -u n'écrit pas dans brain/)." % ", ".join(brain["documents_a_creer"]))
    audit = last_audit(project)
    with_files = has_project_files(entries)
    if brain["etat"] != "absent" and with_files and audit is None:
        notes.append("Le projet a des fichiers et aucun rapport d'audit : /brain -a propose un audit en profondeur (rien n'est écrit sans validation).")
    readme_value, readme_detail = readme_max(project)
    router = router_state(project)
    if router["etat"] == "partiel":
        notes.append("Routeur partiel (%s) : il ne sera pas appliqué." % router["raison"])
    return {
        "racine": str(project),
        "premier_niveau": entries,
        "dossier_vide": all(e == ".git" for e in entries),
        "brain_main": brain_main_state(project),
        "brain": brain,
        "projet_avec_fichiers": with_files,
        "audit_dernier": audit,
        "readme_max": readme_value,
        "readme_max_detail": readme_detail,
        "depot_source": is_source_repo(project),
        "routeur": router["etat"],
        "routeur_raison": router["raison"],
        "technologies": [h for h in TECH_HINTS if (project / h).exists()],
        "bootstrap": {
            "compte": account or "inconnue",
            "projet": in_project or "absent",
            "distante": remote or "non vérifié",
        },
        "notes": notes,
    }


def as_text(st):
    bm, br = st["brain_main"], st["brain"]
    lines = [
        "Projet : %s" % st["racine"],
        "Dossier vide : %s" % ("oui" if st["dossier_vide"] else "non"),
        "Premier niveau : %s" % (", ".join(st["premier_niveau"]) or "(rien)"),
        "brain-main : %s%s" % (bm["etat"], " (version %s)" % bm["version_installee"] if bm.get("version_installee") else ""),
        "brain : %s (%d entrées au registre)" % (br["etat"], br["entrees_registre"]),
        "Documents contenant encore « %s » : %d" % (TODO_MARK, br["documents_a_completer"]),
        "Dernier audit : %s" % (st["audit_dernier"] or "aucun"),
        "Maximum de README : %d (%s)" % (st["readme_max"], st["readme_max_detail"]),
        "Dépôt source 2brain : %s" % ("oui" if st["depot_source"] else "non"),
        "Routeur (orchestrateur et workers) : %s (%s)" % (st["routeur"], st["routeur_raison"]),
        "Technologies (indices) : %s" % (", ".join(st["technologies"]) or "aucun"),
        "Bootstrap : compte %s, projet %s, distante %s" % (
            st["bootstrap"]["compte"], st["bootstrap"]["projet"], st["bootstrap"]["distante"]),
    ]
    lines += st["notes"]
    return "\n".join(lines)


def main():
    for _stream in (sys.stdout, sys.stderr):
        if hasattr(_stream, "reconfigure"):
            _stream.reconfigure(encoding="utf-8")
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--project", default=".")
    ap.add_argument("--json", action="store_true")
    ap.add_argument("--offline", action="store_true", help="ne pas interroger la version distante")
    ap.add_argument("--home", default=None, help="home à lire pour la version du compte (défaut : réel)")
    args = ap.parse_args()
    project = Path(args.project).resolve()
    if not project.is_dir():
        sys.stderr.write("Dossier invalide : %s\n" % project)
        return 2
    home = Path(args.home) if args.home else Path.home()
    st = build(project, home, args.offline)
    if args.json:
        sys.stdout.write(json.dumps(st, ensure_ascii=False, indent=2) + "\n")
    else:
        sys.stdout.write(as_text(st) + "\n")
    return 0


if __name__ == "__main__":
    sys.exit(main())
