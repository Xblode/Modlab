# -*- coding: utf-8 -*-
"""Installe 2brain depuis EMPLACEMENT/, après validation humaine du plan.

Usage : python -I apply-install.py --project DOSSIER --plan-sha256 EMPREINTE
Codes de sortie :
  0 installation réussie ; 2 refus (usage, dépôt source, pas de prévisualisation) ;
  3 plan ou arborescence de EMPLACEMENT/ ne correspondent pas à l'empreinte ;
  4 conflit apparu depuis la prévisualisation (rien n'est écrit, nouvelle validation à demander) ;
  5 échec en cours de route (rien n'est détruit, le JSON dit ce qui a été posé).
"""

import argparse
import hashlib
import json
import os
import shutil
import stat
import subprocess
import sys
from datetime import date
from pathlib import Path, PurePosixPath

EMPLACEMENT = "EMPLACEMENT"
MARKER = ".2brain-emplacement"
PLAN = "plan.json"
MANIFEST = "manifest.json"
SKIP_TOP = {".git", "node_modules"}
AGENT_NAMES = ("orchestrator", "worker-sonnet", "worker-haiku")
ACTIVE = "actif à la prochaine conversation"
INACTIVE = "non activé"
CLAUDE_FILES = {".claude/settings.json", ".claude/README.md"} | {".claude/agents/%s.md" % n for n in AGENT_NAMES}


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


def rmtree(path):
    def onerror(func, p, _exc):
        os.chmod(p, stat.S_IWRITE)
        func(p)

    shutil.rmtree(path, onerror=onerror)


def tree_hashes(root, exclude_top=()):
    """Empreintes de tous les fichiers sous root (hors dossiers de premier niveau exclus)."""
    out = {}
    for dirpath, dirnames, filenames in os.walk(root):
        rel_dir = Path(dirpath).relative_to(root)
        if rel_dir == Path("."):
            dirnames[:] = [d for d in dirnames if d not in exclude_top]
        dirnames.sort()
        for name in sorted(filenames):
            p = Path(dirpath) / name
            rel = (rel_dir / name).as_posix() if rel_dir != Path(".") else name
            if p.is_symlink():
                out[rel] = "lien"
            else:
                out[rel] = sha256_file(p)
    return out


def safe_rel(rel):
    p = PurePosixPath(rel)
    if p.is_absolute() or ".." in p.parts or "\\" in rel or ":" in rel.split("/")[0]:
        raise Refused("chemin dangereux dans le plan : %r" % rel, 3)
    if p.parts[0] not in ("brain-main", "brain", "AGENTS.md", "CLAUDE.md", ".gitignore", ".claude"):
        raise Refused("chemin inattendu dans le plan : %r" % rel, 3)
    if p.parts[0] == ".claude" and rel not in CLAUDE_FILES:
        raise Refused("chemin inattendu dans .claude/ : %r" % rel, 3)


def router_report(project):
    """État du routeur après l'installation, d'après `.claude/settings.json` et les trois agents."""
    settings = project / ".claude" / "settings.json"
    try:
        data = json.loads(settings.read_text(encoding="utf-8"))
        agent = data.get("agent") if isinstance(data, dict) else None
        readable = isinstance(data, dict)
    except Exception:
        agent, readable = None, False
    missing = [n for n in AGENT_NAMES if not (project / ".claude" / "agents" / (n + ".md")).is_file()]
    if missing:
        return {"etat": INACTIVE, "raison": "agents absents : %s" % ", ".join(missing), "nouvelle_conversation": False}
    if agent == "orchestrator":
        return {"etat": ACTIVE, "raison": "`.claude/settings.json` contient \"agent\": \"orchestrator\" et les trois agents sont présents",
                "nouvelle_conversation": True}
    if not readable:
        raison = "`.claude/settings.json` absent ou illisible, non modifié : la clé `agent` n'y est pas"
    elif agent is None:
        raison = "`.claude/settings.json` existait déjà et n'est pas modifié : la clé `\"agent\": \"orchestrator\"` n'y est pas, à ajouter à la main"
    else:
        raison = "`.claude/settings.json` définit déjà `agent` = %s : conflit, non modifié" % json.dumps(agent, ensure_ascii=False)
    return {"etat": INACTIVE, "raison": raison, "nouvelle_conversation": True}


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
    ap.add_argument("--plan-sha256", required=True)
    args = ap.parse_args()
    posed = []
    try:
        project = Path(args.project).resolve()
        if not project.is_dir():
            raise Refused("projet introuvable : %s" % project)
        if is_source_repo(project):
            raise Refused("refus : ce dossier est le dépôt source 2brain")
        emp = project / EMPLACEMENT
        if emp.is_symlink() or not (emp / MARKER).is_file():
            raise Refused("pas de prévisualisation valide (%s/ ou son marqueur absent) : relancer /brain -i" % EMPLACEMENT)
        plan_bytes = (emp / PLAN).read_bytes() if (emp / PLAN).is_file() else b""
        if hashlib.sha256(plan_bytes).hexdigest() != args.plan_sha256.strip().lower():
            raise Refused("le plan ne correspond pas à l'empreinte validée : rien n'est écrit", 3)
        plan = json.loads(plan_bytes.decode("utf-8"))
        files = plan.get("fichiers")
        if not isinstance(files, dict) or not files:
            raise Refused("plan.json sans liste de fichiers", 3)
        for rel in files:
            safe_rel(rel)
        actual = tree_hashes(emp)
        actual.pop(MARKER, None)
        actual.pop(PLAN, None)
        if actual != files:
            diff = sorted(set(actual) ^ set(files) | {k for k in actual if k in files and actual[k] != files[k]})
            raise Refused("l'arborescence de %s/ ne correspond pas au plan (%s) : rien n'est écrit" % (EMPLACEMENT, ", ".join(diff[:5])), 3)

        # Conflits apparus depuis la prévisualisation.
        conflicts = [rel for rel in files if os.path.lexists(project / rel)]
        for top in ("brain-main", "brain"):
            if os.path.lexists(project / top) and not any(r.startswith(top + "/") for r in conflicts):
                conflicts.append(top)
        for root_file in ("AGENTS.md", "CLAUDE.md", ".gitignore"):
            if os.path.lexists(project / root_file) and root_file in files and root_file not in conflicts:
                conflicts.append(root_file)
        if conflicts:
            return out_json({
                "ok": False,
                "erreur": "conflit apparu depuis la prévisualisation : rien n'est écrit, nouvelle validation nécessaire",
                "conflits": sorted(conflicts),
            }, 4)

        before = tree_hashes(project, exclude_top=SKIP_TOP | {EMPLACEMENT})
        order = sorted(files, key=lambda r: (0 if r.startswith("brain-main/") else 1 if r.startswith("brain/") else 2, r))
        try:
            for rel in order:
                target = project / rel
                target.parent.mkdir(parents=True, exist_ok=True)
                shutil.copyfile(emp / rel, target)
                posed.append(rel)
            bm_files = {
                rel[len("brain-main/"):]: sha for rel, sha in files.items()
                if rel.startswith("brain-main/") and rel != "brain-main/" + MANIFEST
            }
            manifest = {
                "manifest_version": 1,
                "name": "brain-main",
                "version": plan.get("bootstrap_version", "inconnue"),
                "source_commit": plan.get("source_commit", "inconnu"),
                "installed": date.today().isoformat(),
                "files": dict(sorted(bm_files.items())),
            }
            mpath = project / "brain-main" / MANIFEST
            mpath.write_text(json.dumps(manifest, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
            posed.append("brain-main/" + MANIFEST)

            # Contrôles de cohérence.
            for rel, sha in files.items():
                if sha256_file(project / rel) != sha:
                    raise RuntimeError("empreinte installée différente du plan : %s" % rel)
            validator = project / "brain-main" / "skills" / "brain-capture" / "scripts" / "validate-registry.py"
            registry = project / "brain" / "registry" / "entries.jsonl"
            if not validator.is_file():
                raise RuntimeError("validateur absent : %s" % validator)
            res = subprocess.run([sys.executable, "-I", "-B", str(validator), str(registry)],
                                 capture_output=True, text=True, encoding="utf-8", errors="replace",
                                 timeout=60, stdin=subprocess.DEVNULL)
            if res.returncode != 0:
                raise RuntimeError("le validateur de registre a échoué (code %d) : %s" % (res.returncode, res.stdout[-300:]))
            after = tree_hashes(project, exclude_top=SKIP_TOP | {EMPLACEMENT})
            expected_new = set(posed)
            changed = [k for k in before if after.get(k) != before[k]]
            extra = sorted(set(after) - set(before) - expected_new)
            if changed or extra:
                raise RuntimeError("des fichiers hors plan ont changé : %s" % ", ".join((changed + extra)[:5]))
        except Exception as exc:
            return out_json({
                "ok": False,
                "erreur": "échec en cours d'installation : %s" % exc,
                "fichiers_poses": posed,
                "note": "rien n'a été détruit ; %s/ est conservé" % EMPLACEMENT,
            }, 5)

        rmtree(emp)
        routeur = router_report(project)
        return out_json({
            "ok": True,
            "bilan": {
                "brain_main": sum(1 for r in posed if r.startswith("brain-main/")),
                "brain": sorted(r for r in posed if r.startswith("brain/")),
                "agents_md": "AGENTS.md" in posed,
                "claude_md": "CLAUDE.md" in posed,
                "gitignore": ".gitignore" in posed,
                "claude": sorted(r for r in posed if r.startswith(".claude/")),
                "claude_deja_presents": plan.get("claude_deja_presents", []),
                "routeur": routeur,
                "message": ("Ouvre une nouvelle conversation pour que le contexte se charge et que le routeur s'applique."
                            if routeur["etat"] == ACTIVE else
                            "Le routeur n'est pas activé (%s). Ouvre une nouvelle conversation pour charger le contexte." % routeur["raison"]),
                "manifeste": "brain-main/" + MANIFEST,
                "validateur_registre": "ok",
                "autres_fichiers_inchanges": len(before),
                "emplacement_supprime": True,
                "adaptations": plan.get("adaptations", []),
            },
        }, 0)
    except Refused as exc:
        return out_json({"ok": False, "erreur": str(exc)}, exc.code)
    except Exception as exc:
        return out_json({"ok": False, "erreur": "échec : %r" % exc, "fichiers_poses": posed}, 5)


if __name__ == "__main__":
    sys.exit(main())
