# -*- coding: utf-8 -*-
"""Règle le nombre maximum de README d'un projet (`/brain -m <nombre>`).

Usage : python -I set-readme-max.py --project DOSSIER --value N [--dry-run]
N'écrit que <projet>/brain/settings.json (clé `readme_max`, entier supérieur ou égal à 1). Les autres clés et le
format existant (indentation, fin de ligne, retour final, ordre des clés) sont conservés. Aucun README n'est
touché ni supprimé. Sortie JSON. Codes : 0 ok (y compris « rien à écrire »), 2 refus ou erreur (rien n'est écrit).
"""

import argparse
import json
import os
import re
import subprocess
import sys
from pathlib import Path

DEFAULT = 8
KEY = "readme_max"


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


def parse_value(text):
    if not re.fullmatch(r"[0-9]+", text.strip()):
        raise Refused("valeur invalide « %s » : un entier supérieur ou égal à 1 est attendu (exemple : /brain -m 12)" % text)
    value = int(text.strip())
    if value < 1:
        raise Refused("valeur invalide « %s » : un entier supérieur ou égal à 1 est attendu (exemple : /brain -m 12)" % text)
    return value


def run(project, value_text, dry_run):
    value = parse_value(value_text)
    if is_source_repo(project):
        raise Refused("dépôt source 2brain : refus, son `brain/` est le projet lui-même ; ce réglage s'écrit dans un projet installé")
    if not (project / "brain").is_dir():
        raise Refused("`brain/` est absent : faire `/brain -i` d'abord. Rien n'est écrit.")
    settings = project / "brain" / "settings.json"
    if os.path.lexists(settings) and (settings.is_symlink() or not settings.is_file()):
        raise Refused("`brain/settings.json` n'est pas un fichier ordinaire, rien n'est écrit")
    exists = settings.is_file()
    data = {}
    if exists:
        try:
            data = json.loads(settings.read_bytes().decode("utf-8"))
        except Exception as exc:
            raise Refused("`brain/settings.json` n'est pas du JSON valide, rien n'est écrit : %s" % exc)
        if not isinstance(data, dict):
            raise Refused("`brain/settings.json` doit contenir un objet JSON, rien n'est écrit")
    old = data.get(KEY)
    old_valid = isinstance(old, int) and not isinstance(old, bool) and old >= 1
    out = {"ok": True, "fichier": "brain/settings.json", "readme_max": value, "ancien": old if old_valid else None,
           "ancien_effectif": old if old_valid else DEFAULT, "dry_run": dry_run, "ecrit": False}
    if old_valid and old == value:
        out.update(action="aucune", message="Le maximum de README est déjà %d : rien à écrire." % value)
        return out
    fmt = detect_format(settings)
    data[KEY] = value
    out["action"] = "création" if not exists else ("modification de `%s`" % KEY if KEY in data and old is not None else "ajout de `%s`" % KEY)
    if not dry_run:
        write_settings(settings, data, fmt)
        out["ecrit"] = True
    out["message"] = ("Simulation : rien n'est écrit. " if dry_run else "") + \
        "Maximum de README : %d (avant : %s). Aucun README n'est supprimé, même si ce nombre est inférieur au nombre existant." % (
            value, out["ancien_effectif"])
    return out


def main():
    for stream in (sys.stdout, sys.stderr):
        if hasattr(stream, "reconfigure"):
            stream.reconfigure(encoding="utf-8")
    ap = argparse.ArgumentParser(description="Règle le nombre maximum de README (/brain -m <nombre>).")
    ap.add_argument("--project", required=True)
    ap.add_argument("--value", required=True)
    ap.add_argument("--dry-run", action="store_true")
    args = ap.parse_args()
    try:
        out = run(Path(args.project).resolve(), args.value, args.dry_run)
        code = 0
    except Refused as exc:
        out, code = {"ok": False, "ecrit": False, "erreur": str(exc)}, 2
    sys.stdout.write(json.dumps(out, ensure_ascii=False, indent=2) + "\n")
    return code


if __name__ == "__main__":
    sys.exit(main())
