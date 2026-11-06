# -*- coding: utf-8 -*-
"""Hook SessionStart de 2brain : propose l'installation dans un dossier vide.

Lit l'événement sur l'entrée standard (JSON avec `cwd` et `source`). Ne parle
que si la session démarre (`source == "startup"`) dans un dossier sans aucune
entrée, ou contenant seulement `.git`. Dans tous les autres cas, il n'écrit
rien. Aucun journal, aucun accès réseau, code de sortie toujours 0.
"""

import json
import sys
from pathlib import Path

MESSAGE = (
    "Ce dossier est vide et 2brain n'y est pas installé. "
    "Dans cette situation, la première réponse de la session commence par la question : "
    "« Avant de commencer, souhaites-tu installer 2brain dans ce projet ? ». "
    "Si la réponse est oui, elle indique de taper /brain -i. "
    "Si la réponse est non, la session continue normalement et la question n'est pas reposée."
)


def main():
    for _stream in (sys.stdout, sys.stderr):
        if hasattr(_stream, "reconfigure"):
            _stream.reconfigure(encoding="utf-8")
    if hasattr(sys.stdin, "reconfigure"):
        sys.stdin.reconfigure(encoding="utf-8")
    data = json.loads(sys.stdin.read() or "{}")
    if not isinstance(data, dict) or data.get("source") != "startup":
        return
    cwd = Path(str(data.get("cwd", "")))
    if not str(data.get("cwd", "")) or not cwd.is_dir():
        return
    if any(entry.name != ".git" for entry in cwd.iterdir()):
        return
    out = {"hookSpecificOutput": {"hookEventName": "SessionStart", "additionalContext": MESSAGE}}
    sys.stdout.write(json.dumps(out, ensure_ascii=True))


try:
    main()
except BaseException:
    pass
sys.exit(0)
