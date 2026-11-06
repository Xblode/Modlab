# -*- coding: utf-8 -*-
"""Annule la prévisualisation : supprime EMPLACEMENT/ seulement si son marqueur existe.

Usage : python -I cancel-install.py --project DOSSIER
Ne fait rien d'autre. Sortie JSON. Codes : 0 ok (ou rien à annuler), 2 refus.
"""

import argparse
import json
import os
import shutil
import stat
import sys
from pathlib import Path

EMPLACEMENT = "EMPLACEMENT"
MARKER = ".2brain-emplacement"


def rmtree(path):
    def onerror(func, p, _exc):
        os.chmod(p, stat.S_IWRITE)
        func(p)

    shutil.rmtree(path, onerror=onerror)


def out_json(obj, code):
    sys.stdout.reconfigure(encoding="utf-8")
    sys.stdout.write(json.dumps(obj, ensure_ascii=False, indent=2) + "\n")
    return code


def main():
    for _stream in (sys.stdout, sys.stderr):
        if hasattr(_stream, "reconfigure"):
            _stream.reconfigure(encoding="utf-8")
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--project", required=True)
    args = ap.parse_args()
    project = Path(args.project).resolve()
    if not project.is_dir():
        return out_json({"ok": False, "erreur": "projet introuvable : %s" % project}, 2)
    emp = project / EMPLACEMENT
    if not os.path.lexists(emp):
        return out_json({"ok": True, "supprime": False, "note": "rien à annuler"}, 0)
    if emp.is_symlink() or not emp.is_dir() or not (emp / MARKER).is_file():
        return out_json({
            "ok": False,
            "erreur": "%s/ existe mais sans son marqueur %s : refus, rien n'est supprimé" % (EMPLACEMENT, MARKER),
        }, 2)
    rmtree(emp)
    return out_json({"ok": True, "supprime": True}, 0)


if __name__ == "__main__":
    sys.exit(main())
