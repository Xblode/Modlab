# -*- coding: utf-8 -*-
"""Installe (ou retire) le skill /brain et ses deux hooks (démarrage, contrôle des README au commit) sur le compte Claude Code.

Usage : python -I install-account.py [--home DOSSIER] [--dry-run] [--uninstall] [--replace-probe]
Écrit seulement sous <home>/.claude : skills/brain/, settings.json et
2brain-bootstrap/backups/. Sauvegarde settings.json avant toute écriture.
Sortie JSON. Codes : 0 ok, 2 refus ou erreur (rien n'est écrit dans le cas d'un refus).
"""

import argparse
import json
import os
import shutil
import stat
import sys
from datetime import datetime
from pathlib import Path

ACCOUNT_FILES = ("SKILL.md", "session-start.py", "commit-check.py", "status.py", "fetch-source.py")
PROBE_SCRIPT_TAIL = "2brain-sonde/session_start.py"
OWN_SCRIPT_TAIL = "skills/brain/session-start.py"
COMMIT_SCRIPT_TAIL = "skills/brain/commit-check.py"


class Refused(Exception):
    pass


def norm(text):
    return str(text).replace("\\", "/").lower()


def group_points_to(group, tail):
    """Vrai si un hook du groupe référence un script se terminant par `tail`."""
    if not isinstance(group, dict):
        return False
    for hook in group.get("hooks", []) or []:
        if not isinstance(hook, dict):
            continue
        parts = [hook.get("command", "")] + list(hook.get("args", []) or [])
        if any(norm(p).endswith(norm(tail)) for p in parts if isinstance(p, str)):
            return True
    return False


def load_settings(path):
    if not path.exists():
        return {}
    try:
        data = json.loads(path.read_text(encoding="utf-8"))
    except Exception as exc:
        raise Refused("settings.json n'est pas du JSON valide, rien n'est écrit : %s" % exc)
    if not isinstance(data, dict):
        raise Refused("settings.json doit contenir un objet JSON, rien n'est écrit")
    hooks = data.get("hooks")
    if hooks is not None:
        if not isinstance(hooks, dict):
            raise Refused("settings.json : « hooks » n'est pas un objet, rien n'est écrit")
        for event in ("SessionStart", "PreToolUse"):
            if event in hooks and not isinstance(hooks[event], list):
                raise Refused("settings.json : « hooks.%s » n'est pas une liste, rien n'est écrit" % event)
    return data


def session_groups(data, create=False, event="SessionStart"):
    hooks = data.get("hooks")
    if hooks is None:
        if not create:
            return []
        hooks = data["hooks"] = {}
    if event not in hooks:
        if not create:
            return []
        hooks[event] = []
    return hooks[event]


def prune_empty(data):
    hooks = data.get("hooks")
    if isinstance(hooks, dict):
        for event in ("SessionStart", "PreToolUse"):
            if hooks.get(event) == []:
                del hooks[event]
        if not hooks:
            del data["hooks"]


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


def backup_probe_settings(sonde_dir, backups, dry_run, actions):
    """Copie à l'identique la seule sauvegarde d'avant la sonde, sans rien écraser."""
    src = sonde_dir / "settings.json.avant-sonde"
    if not src.is_file():
        return
    target = backups / "settings.avant-sonde.json"
    n = 1
    while target.exists():
        target = backups / ("settings.avant-sonde-%d.json" % n)
        n += 1
    actions.append("copie de %s vers %s" % (src, target))
    if not dry_run:
        backups.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(src, target)


def backup(settings, backups):
    if not settings.exists():
        return None
    backups.mkdir(parents=True, exist_ok=True)
    stamp = datetime.now().strftime("%Y%m%d-%H%M%S")
    target = backups / ("settings.%s.json" % stamp)
    n = 1
    while target.exists():
        target = backups / ("settings.%s-%d.json" % (stamp, n))
        n += 1
    shutil.copyfile(settings, target)
    return target


def rmtree(path):
    def onerror(func, p, _exc):
        os.chmod(p, stat.S_IWRITE)
        func(p)

    shutil.rmtree(path, onerror=onerror)


def inside(child, parent):
    return parent.resolve() in child.resolve().parents


def report(ok, dry_run, actions, error=None):
    out = {"ok": ok, "dry_run": dry_run, "actions": actions}
    if error:
        out["erreur"] = error
    sys.stdout.reconfigure(encoding="utf-8")
    sys.stdout.write(json.dumps(out, ensure_ascii=False, indent=2) + "\n")
    return 0 if ok else 2


def main():
    for _stream in (sys.stdout, sys.stderr):
        if hasattr(_stream, "reconfigure"):
            _stream.reconfigure(encoding="utf-8")
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--home", default=None)
    ap.add_argument("--dry-run", action="store_true")
    ap.add_argument("--uninstall", action="store_true")
    ap.add_argument("--replace-probe", action="store_true")
    args = ap.parse_args()
    home = Path(args.home) if args.home else Path.home()
    claude = home / ".claude"
    settings = claude / "settings.json"
    skill_dir = claude / "skills" / "brain"
    sonde_dir = claude / "2brain-sonde"
    backups = claude / "2brain-bootstrap" / "backups"
    hook_script = skill_dir / "session-start.py"
    actions = []
    try:
        data = load_settings(settings)
        fmt = detect_format(settings)
        groups = session_groups(data)
        ours_installed = (skill_dir / "VERSION").is_file()

        if args.uninstall:
            if args.replace_probe:
                raise Refused("--uninstall et --replace-probe sont incompatibles")
            kept = [g for g in groups if not group_points_to(g, OWN_SCRIPT_TAIL)]
            pre_groups = session_groups(data, event="PreToolUse")
            pre_kept = [g for g in pre_groups if not group_points_to(g, COMMIT_SCRIPT_TAIL)]
            write_needed = len(kept) != len(groups) or len(pre_kept) != len(pre_groups)
            if len(kept) != len(groups):
                actions.append("retrait du groupe hooks.SessionStart pointant vers skills/brain/session-start.py")
                data["hooks"]["SessionStart"] = kept
            if len(pre_kept) != len(pre_groups):
                actions.append("retrait du groupe hooks.PreToolUse pointant vers skills/brain/commit-check.py")
                data["hooks"]["PreToolUse"] = pre_kept
            if write_needed:
                prune_empty(data)
            if ours_installed:
                actions.append("suppression de %s" % skill_dir)
            elif skill_dir.exists():
                actions.append("%s conservé : pas posé par cet installateur (VERSION absent)" % skill_dir)
            if not actions:
                actions.append("rien à retirer")
            if not args.dry_run:
                if write_needed:
                    actions.append("sauvegarde : %s" % backup(settings, backups))
                    write_settings(settings, data, fmt)
                if ours_installed and inside(skill_dir, claude):
                    rmtree(skill_dir)
            return report(True, args.dry_run, actions)

        root = Path(__file__).resolve().parent.parent
        src = root / "account"
        for name in ACCOUNT_FILES:
            if not (src / name).is_file():
                raise Refused("fichier manquant dans account/ : %s" % name)
        version = (root / "VERSION").read_text(encoding="utf-8").strip()

        remove_probe = False
        if skill_dir.exists() and not ours_installed:
            if not args.replace_probe:
                raise Refused(
                    "%s existe et n'a pas été posé par cet installateur (VERSION absent). "
                    "Rien n'est écrit. S'il s'agit de la sonde de test, relancer avec --replace-probe." % skill_dir
                )
            remove_probe = True
        if args.replace_probe:
            if any(group_points_to(g, PROBE_SCRIPT_TAIL) for g in groups):
                data["hooks"]["SessionStart"] = [g for g in groups if not group_points_to(g, PROBE_SCRIPT_TAIL)]
                actions.append("retrait du groupe hooks.SessionStart de la sonde")
            if remove_probe:
                actions.append("suppression de %s (sonde)" % skill_dir)
            if sonde_dir.exists():
                backup_probe_settings(sonde_dir, backups, args.dry_run, actions)
                actions.append("suppression de %s" % sonde_dir)

        entry = {"type": "command", "command": sys.executable, "args": [str(hook_script)]}
        new_group = {"matcher": "startup", "hooks": [entry]}
        groups = session_groups(data, create=True)
        replaced = False
        result = []
        for g in groups:
            if group_points_to(g, OWN_SCRIPT_TAIL):
                if not replaced:
                    result.append(new_group)
                    replaced = True
                continue
            result.append(g)
        if not replaced:
            result.append(new_group)
        data["hooks"]["SessionStart"] = result
        actions.append("hook SessionStart %s (matcher startup, sans shell)" % ("mis à jour" if replaced else "ajouté"))
        commit_entry = {"type": "command", "command": sys.executable, "args": [str(skill_dir / "commit-check.py")]}
        commit_group = {"matcher": "Bash|PowerShell", "hooks": [commit_entry]}
        pre_result, pre_replaced = [], False
        for g in session_groups(data, create=True, event="PreToolUse"):
            if group_points_to(g, COMMIT_SCRIPT_TAIL):
                if not pre_replaced:
                    pre_result.append(commit_group)
                    pre_replaced = True
                continue
            pre_result.append(g)
        if not pre_replaced:
            pre_result.append(commit_group)
        data["hooks"]["PreToolUse"] = pre_result
        actions.append("hook PreToolUse %s (matcher Bash|PowerShell, sans shell : contrôle des README avant git commit)" % ("mis à jour" if pre_replaced else "ajouté"))
        actions.append("skill copié vers %s (VERSION %s)" % (skill_dir, version))

        if not args.dry_run:
            claude.mkdir(parents=True, exist_ok=True)
            if settings.exists():
                actions.append("sauvegarde : %s" % backup(settings, backups))
            if remove_probe or ours_installed:
                rmtree(skill_dir)
            if args.replace_probe and sonde_dir.exists():
                rmtree(sonde_dir)
            skill_dir.mkdir(parents=True, exist_ok=True)
            for name in ACCOUNT_FILES:
                text = (src / name).read_text(encoding="utf-8")
                if name == "SKILL.md":
                    text = text.replace("__PYTHON__", Path(sys.executable).as_posix())
                (skill_dir / name).write_text(text, encoding="utf-8", newline="\n")
            (skill_dir / "VERSION").write_text(version + "\n", encoding="utf-8", newline="\n")
            write_settings(settings, data, fmt)
        return report(True, args.dry_run, actions)
    except Refused as exc:
        return report(False, args.dry_run, actions=[], error=str(exc))


if __name__ == "__main__":
    sys.exit(main())
