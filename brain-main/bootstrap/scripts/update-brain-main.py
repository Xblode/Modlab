# -*- coding: utf-8 -*-
"""Met à jour brain-main/ (seulement) par comparaison à trois états : manifeste, local, source.

Usage :
  python -I update-brain-main.py --source DOSSIER_BRAIN_MAIN --project DOSSIER            (plan, sans écrire)
  python -I update-brain-main.py --source ... --project ... --apply --plan-sha256 EMPREINTE

N'écrit jamais en dehors de brain-main/ (`.claude/` n'est jamais modifié : un changement de l'adaptateur est seulement signalé) et n'écrase jamais une modification locale en conflit.
Sortie JSON. Codes : 0 ok, 2 refus (dépôt source, manifeste absent ou invalide, usage),
3 le plan recalculé ne correspond pas à l'empreinte validée.
"""

import argparse
import hashlib
import json
import os
import subprocess
import sys
from datetime import date
from pathlib import Path, PurePosixPath

MANIFEST = "manifest.json"
TEMPLATE_PREFIXES = ("bootstrap/templates/",)
ADAPTER_PREFIX = "adapters/claude-code/"


class Refused(Exception):
    def __init__(self, msg, code=2):
        super().__init__(msg)
        self.code = code


def sha256_file(path):
    h = hashlib.sha256()
    with open(path, "rb") as fh:
        for chunk in iter(lambda: fh.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


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


def safe_rel(rel):
    p = PurePosixPath(rel)
    if (not rel or p.is_absolute() or ".." in p.parts or "\\" in rel or ":" in rel.split("/")[0]
            or rel in (".", MANIFEST) or "." in p.parts):
        raise Refused("chemin hors de brain-main/ refusé : %r" % rel)


def scan(root):
    """Empreintes des fichiers sous root, hors manifeste et caches. Refuse les liens."""
    out = {}
    for dirpath, dirnames, filenames in os.walk(root):
        dirnames[:] = sorted(d for d in dirnames if d != "__pycache__")
        for name in sorted(filenames):
            p = Path(dirpath) / name
            rel = p.relative_to(root).as_posix()
            if name.endswith(".pyc") or rel == MANIFEST:
                continue
            if p.is_symlink():
                raise Refused("lien refusé : %s" % p)
            out[rel] = sha256_file(p)
        for d in dirnames:
            if (Path(dirpath) / d).is_symlink():
                raise Refused("lien refusé : %s" % (Path(dirpath) / d))
    return out


def read_version(root):
    try:
        return (root / "bootstrap" / "VERSION").read_text(encoding="utf-8").strip()
    except Exception:
        return "inconnue"


def build_plan(source, project):
    bm = project / "brain-main"
    mpath = bm / MANIFEST
    if not mpath.is_file():
        raise Refused("manifeste d'installation absent (%s) : brain-main/ n'a pas été installé par 2brain, refus" % mpath)
    try:
        manifest = json.loads(mpath.read_text(encoding="utf-8"))
        mfiles = manifest["files"]
        if not isinstance(mfiles, dict):
            raise ValueError("files")
    except Exception as exc:
        raise Refused("manifeste illisible : %s" % exc)
    for rel in mfiles:
        safe_rel(rel)
    local = scan(bm)
    src = scan(source)
    for rel in src:
        safe_rel(rel)
    plan = {"update": [], "add": [], "delete": [], "conflicts": [], "kept": [],
            "kept_deleted": [], "converged": [], "local_only": []}
    for rel in sorted(set(mfiles) | set(local) | set(src)):
        m, l, s = mfiles.get(rel), local.get(rel), src.get(rel)
        if m is not None:
            if l == m:
                if s == m:
                    continue
                if s is None:
                    plan["delete"].append({"path": rel, "from": m})
                else:
                    plan["update"].append({"path": rel, "from": m, "to": s})
            elif l is None:
                if s == m:
                    plan["kept_deleted"].append(rel)
                elif s is None:
                    continue
                else:
                    plan["conflicts"].append({"path": rel, "reason": "supprimé localement, modifié dans la source"})
            else:
                if s == m:
                    plan["kept"].append(rel)
                elif s == l:
                    plan["converged"].append({"path": rel, "to": s})
                else:
                    reason = "modifié localement, supprimé dans la source" if s is None else \
                        "modifié localement et dans la source, différemment"
                    plan["conflicts"].append({"path": rel, "reason": reason})
        else:
            if l is None and s is not None:
                plan["add"].append({"path": rel, "to": s})
            elif l is not None and s is None:
                plan["local_only"].append(rel)
            elif l is not None and s is not None:
                if l == s:
                    plan["converged"].append({"path": rel, "to": s})
                else:
                    plan["conflicts"].append({"path": rel, "reason": "présent hors manifeste avec un autre contenu que la source"})
    touched = [e["path"] for k in ("update", "add", "delete") for e in plan[k]]
    adaptations = [p for p in touched if p.startswith(TEMPLATE_PREFIXES)]
    plan["adaptations_brain"] = [
        {"path": p, "note": "le modèle de brain/ a changé : adaptation éventuelle de brain/ à proposer à part, rien n'est modifié"}
        for p in adaptations
    ]
    plan["adaptations_claude"] = [
        {"path": p, "note": "l'adaptateur Claude Code a changé : .claude/ n'est pas modifié, rien n'est écrit ; "
                            "la copie de .claude/ (agents, settings.json, README.md) est à comparer et à reprendre à part"}
        for p in touched if p.startswith(ADAPTER_PREFIX)
    ]
    plan["versions"] = {
        "installee": manifest.get("version", "inconnue"),
        "source": read_version(source),
        "commit_installe": manifest.get("source_commit", "inconnu"),
    }
    commit_file = source.parent / "source-commit.txt"
    plan["versions"]["commit_source"] = commit_file.read_text(encoding="utf-8").strip() if commit_file.is_file() else "inconnu"
    body = json.dumps(plan, ensure_ascii=False, sort_keys=True, separators=(",", ":"))
    plan["plan_sha256"] = hashlib.sha256(body.encode("utf-8")).hexdigest()
    return plan, manifest


def apply_plan(plan, manifest, source, project):
    bm = (project / "brain-main").resolve()
    files = dict(manifest["files"])
    done = {"updated": [], "added": [], "deleted": []}
    for entry in plan["update"] + plan["add"]:
        rel = entry["path"]
        target = (bm / rel).resolve()
        if bm not in target.parents:
            raise Refused("chemin hors de brain-main/ refusé : %r" % rel)
        target.parent.mkdir(parents=True, exist_ok=True)
        data = (source / rel).read_bytes()
        if hashlib.sha256(data).hexdigest() != entry["to"]:
            raise Refused("la source a changé pendant l'application : %s" % rel, 3)
        tmp = target.with_name(target.name + ".tmp-2brain")
        tmp.write_bytes(data)
        os.replace(tmp, target)
        files[rel] = entry["to"]
        done["updated" if entry in plan["update"] else "added"].append(rel)
    for entry in plan["delete"]:
        rel = entry["path"]
        target = (bm / rel).resolve()
        if bm not in target.parents:
            raise Refused("chemin hors de brain-main/ refusé : %r" % rel)
        if target.is_file():
            target.unlink()
            parent = target.parent
            while parent != bm and parent.is_dir() and not any(parent.iterdir()):
                parent.rmdir()
                parent = parent.parent
        files.pop(rel, None)
        done["deleted"].append(rel)
    for entry in plan["converged"]:
        files[entry["path"]] = entry["to"]
    manifest = dict(manifest)
    manifest["files"] = dict(sorted(files.items()))
    manifest["version"] = plan["versions"]["source"]
    manifest["source_commit"] = plan["versions"]["commit_source"]
    manifest["updated"] = date.today().isoformat()
    tmp = bm / (MANIFEST + ".tmp-2brain")
    tmp.write_text(json.dumps(manifest, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    os.replace(tmp, bm / MANIFEST)
    return done


def out_json(obj, code):
    sys.stdout.reconfigure(encoding="utf-8")
    sys.stdout.write(json.dumps(obj, ensure_ascii=False, indent=2) + "\n")
    return code


def main():
    for _stream in (sys.stdout, sys.stderr):
        if hasattr(_stream, "reconfigure"):
            _stream.reconfigure(encoding="utf-8")
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--source", required=True)
    ap.add_argument("--project", required=True)
    ap.add_argument("--apply", action="store_true")
    ap.add_argument("--plan-sha256", default=None)
    args = ap.parse_args()
    try:
        project = Path(args.project).resolve()
        source = Path(args.source).resolve()
        if not project.is_dir() or not source.is_dir():
            raise Refused("projet ou source introuvable")
        if is_source_repo(project):
            raise Refused("refus : ce dossier est le dépôt source 2brain (la mise à jour n'y a pas de sens)")
        if not (project / "brain-main").is_dir():
            raise Refused("brain-main/ absent : rien à mettre à jour (utiliser /brain -i)")
        plan, manifest = build_plan(source, project)
        if not args.apply:
            plan["ok"] = True
            plan["ecrit"] = False
            return out_json(plan, 0)
        if not args.plan_sha256:
            raise Refused("--apply demande --plan-sha256")
        if plan["plan_sha256"] != args.plan_sha256.strip().lower():
            raise Refused("le plan recalculé diffère de celui validé : rien n'est écrit, nouvelle validation nécessaire", 3)
        done = apply_plan(plan, manifest, source, project)
        return out_json({
            "ok": True, "ecrit": True, **done,
            "conflits_non_ecrits": plan["conflicts"],
            "conserves": plan["kept"], "supprimes_localement_conserves": plan["kept_deleted"],
            "adaptations_brain": plan["adaptations_brain"],
            "adaptations_claude": plan["adaptations_claude"],
            "version": plan["versions"]["source"],
        }, 0)
    except Refused as exc:
        return out_json({"ok": False, "erreur": str(exc)}, exc.code)
    except Exception as exc:
        return out_json({"ok": False, "erreur": "échec : %r" % exc}, 2)


if __name__ == "__main__":
    sys.exit(main())
