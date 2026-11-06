# -*- coding: utf-8 -*-
"""Active ou désactive le routeur Claude Code d'un projet (`/brain -d on|off`).

Usage : python -I set-router.py --project DOSSIER --state on|off [--dry-run]
N'écrit que <projet>/.claude/settings.json (clé `agent` = `orchestrator`). Format existant conservé
(indentation, fin de ligne, retour final, ordre des clés). Sortie JSON.
Codes : 0 ok (y compris « rien à écrire »), 2 refus ou erreur (rien n'est écrit).
Le changement s'applique à la conversation suivante, jamais à la conversation en cours.
"""

import argparse
import json
import os
import subprocess
import sys
from pathlib import Path

AGENT_VALUE = "orchestrator"
AGENT_FILES = ("orchestrator.md", "worker-sonnet.md", "worker-haiku.md")
NEXT = "Le changement s'applique à la conversation suivante, jamais à la conversation en cours : ouvrir une nouvelle conversation dans ce dossier."


class Refused(Exception):
    pass


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


def detect_format(path):
    """Format du fichier d'origine : indentation, fin de ligne, retour final (défaut : 2, LF, retour final)."""
    fmt = {"indent": 2, "eol": "\n", "final": True}
    if not path.exists():
        return fmt
    raw = path.read_bytes().decode("utf-8")
    fmt["eol"] = "\r\n" if "\r\n" in raw else "\n"
    fmt["final"] = raw.endswith("\n")
    for line in raw.splitlines()[1:]:
        stripped = line.lstrip(" \t")
        if stripped and len(stripped) != len(line):
            lead = line[: len(line) - len(stripped)]
            fmt["indent"] = "\t" if lead.startswith("\t") else len(lead)
            break
    return fmt


def write_settings(path, data, fmt):
    tmp = path.with_name(path.name + ".tmp-2brain")
    text = json.dumps(data, ensure_ascii=False, indent=fmt["indent"])
    if fmt["eol"] != "\n":
        text = text.replace("\n", fmt["eol"])
    if fmt["final"]:
        text += fmt["eol"]
    tmp.write_bytes(text.encode("utf-8"))
    os.replace(tmp, path)


def load(path):
    try:
        data = json.loads(path.read_bytes().decode("utf-8"))
    except Exception as exc:
        raise Refused("`.claude/settings.json` n'est pas du JSON valide, rien n'est écrit : %s" % exc)
    if not isinstance(data, dict):
        raise Refused("`.claude/settings.json` doit contenir un objet JSON, rien n'est écrit")
    return data


def run(project, state, dry_run):
    if is_source_repo(project):
        raise Refused("dépôt source 2brain : refus, le routeur de ce dépôt ne se change pas par cette commande")
    settings = project / ".claude" / "settings.json"
    if os.path.lexists(settings) and (settings.is_symlink() or not settings.is_file()):
        raise Refused("`.claude/settings.json` n'est pas un fichier ordinaire, rien n'est écrit")
    exists = settings.is_file()
    data = load(settings) if exists else {}
    value = data.get("agent") if "agent" in data else None
    if "agent" in data and value != AGENT_VALUE:
        raise Refused("`.claude/settings.json` définit `agent` = %s (autre valeur que `%s`) : rien n'est modifié"
                      % (json.dumps(value, ensure_ascii=False), AGENT_VALUE))
    active = data.get("agent") == AGENT_VALUE
    out = {"ok": True, "etat_demande": state, "dry_run": dry_run, "fichier": ".claude/settings.json", "ecrit": False}
    if state == "on":
        missing = [n for n in AGENT_FILES if not (project / ".claude" / "agents" / n).is_file()]
        if missing:
            raise Refused("agents absents de `.claude/agents/` : %s. Copier l'adaptateur depuis `brain-main/adapters/claude-code/` "
                          "ou refaire l'installation (`/brain -i`). Rien n'est écrit." % ", ".join(missing))
        if active:
            out.update(action="aucune", message="Le routeur est déjà activé (`agent` = `orchestrator`) : rien à écrire.")
            return out
        fmt = detect_format(settings)
        data["agent"] = AGENT_VALUE
        out["action"] = "creation" if not exists else "ajout de la clé `agent`"
    else:
        if not active:
            out.update(action="aucune", message="Le routeur est déjà désactivé (clé `agent` absente) : rien à écrire.")
            return out
        fmt = detect_format(settings)
        del data["agent"]
        out["action"] = "retrait de la clé `agent`"
    if not dry_run:
        settings.parent.mkdir(parents=True, exist_ok=True)
        write_settings(settings, data, fmt)
        out["ecrit"] = True
    out["message"] = ("Simulation : rien n'est écrit. " if dry_run else "") + \
        ("Routeur activé." if state == "on" else "Routeur désactivé.")
    out["conversation_suivante"] = NEXT
    return out


def main():
    for stream in (sys.stdout, sys.stderr):
        if hasattr(stream, "reconfigure"):
            stream.reconfigure(encoding="utf-8")
    ap = argparse.ArgumentParser(description="Active ou désactive le routeur (/brain -d on|off).")
    ap.add_argument("--project", required=True)
    ap.add_argument("--state", required=True, choices=("on", "off"))
    ap.add_argument("--dry-run", action="store_true")
    args = ap.parse_args()
    try:
        out = run(Path(args.project).resolve(), args.state, args.dry_run)
        code = 0
    except Refused as exc:
        out, code = {"ok": False, "ecrit": False, "erreur": str(exc)}, 2
    if code == 0 and "conversation_suivante" not in out:
        out["conversation_suivante"] = NEXT
    sys.stdout.write(json.dumps(out, ensure_ascii=False, indent=2) + "\n")
    return code


if __name__ == "__main__":
    sys.exit(main())
