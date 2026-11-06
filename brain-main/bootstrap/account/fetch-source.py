# -*- coding: utf-8 -*-
"""Récupère brain-main/ depuis Xblode/2brain (ou une copie locale) vers un dossier vide.

Usage : python -I fetch-source.py --dest DOSSIER [--source-url URL | --source-dir DOSSIER]
`--source-dir` désigne un dossier contenant `brain-main/` (racine d'un dépôt).
N'extrait que brain-main/. Refuse les chemins sortant de la destination et les
liens. N'exécute rien. Sortie JSON sur la sortie standard.
Codes : 0 ok, 2 arguments, 3 Bootstrap absent de la source, 4 archive refusée, 5 échec réseau/lecture.
"""

import argparse
import io
import json
import shutil
import subprocess
import sys
import tarfile
import urllib.request
from pathlib import Path, PurePosixPath

DEFAULT_URL = "https://codeload.github.com/Xblode/2brain/tar.gz/refs/heads/master"
MARKER = Path("brain-main") / "bootstrap" / "scripts" / "prepare-install.py"
MAX_BYTES = 100 * 1024 * 1024
SKIP_NAMES = {"__pycache__"}


class Refused(Exception):
    pass


def emit(obj, code):
    sys.stdout.reconfigure(encoding="utf-8")
    sys.stdout.write(json.dumps(obj, ensure_ascii=False, indent=2) + "\n")
    return code


def check_member_name(name):
    if not name or "\\" in name or name.startswith("/") or ":" in name.split("/")[0]:
        raise Refused("chemin refusé dans l'archive : %r" % name)
    parts = PurePosixPath(name).parts
    if ".." in parts or any(p.endswith(":") or (len(p) == 2 and p[1] == ":") for p in parts):
        raise Refused("chemin refusé dans l'archive : %r" % name)
    return parts


def extract_archive(data, dest):
    commit = "inconnu"
    try:
        tf = tarfile.open(fileobj=io.BytesIO(data), mode="r:*")
    except Exception as exc:
        raise Refused("archive illisible : %s" % exc)
    with tf:
        members = tf.getmembers()
        gh = getattr(tf, "pax_headers", None) or {}
        if gh.get("comment"):
            commit = str(gh["comment"]).strip()
        # Contrôle de tous les chemins avant toute écriture.
        selected = []
        for m in members:
            parts = check_member_name(m.name)
            if len(parts) >= 2 and parts[1] == "brain-main":
                if m.issym() or m.islnk():
                    raise Refused("lien refusé dans l'archive : %r" % m.name)
                if not (m.isfile() or m.isdir()):
                    raise Refused("type de fichier refusé dans l'archive : %r" % m.name)
                selected.append((m, PurePosixPath(*parts[1:])))
        if not selected:
            raise Refused("l'archive ne contient pas brain-main/")
        total = 0
        dest_root = dest.resolve()
        for m, rel in selected:
            target = (dest_root / Path(*rel.parts)).resolve()
            if dest_root not in target.parents and target != dest_root:
                raise Refused("chemin hors destination : %r" % m.name)
            if m.isdir():
                target.mkdir(parents=True, exist_ok=True)
                continue
            total += m.size
            if total > MAX_BYTES:
                raise Refused("archive trop volumineuse")
            target.parent.mkdir(parents=True, exist_ok=True)
            src = tf.extractfile(m)
            with open(target, "wb") as out:
                shutil.copyfileobj(src, out)
    return commit


def copy_dir(src_root, dest):
    src = src_root / "brain-main"
    if not src.is_dir():
        raise Refused("%s ne contient pas brain-main/" % src_root)
    base = dest.resolve()
    for path in sorted(src.rglob("*")):
        if any(p in SKIP_NAMES for p in path.parts) or path.suffix == ".pyc":
            continue
        if path.is_symlink():
            raise Refused("lien refusé dans la source : %s" % path)
        rel = path.relative_to(src_root)
        target = base / rel
        if path.is_dir():
            target.mkdir(parents=True, exist_ok=True)
        else:
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(path, target)
    try:
        out = subprocess.run(["git", "-C", str(src_root), "rev-parse", "HEAD"], capture_output=True,
                             text=True, timeout=10, stdin=subprocess.DEVNULL)
        sha = out.stdout.strip()
        if out.returncode == 0 and len(sha) == 40:
            return sha
    except Exception:
        pass
    return "inconnu"


def main():
    for _stream in (sys.stdout, sys.stderr):
        if hasattr(_stream, "reconfigure"):
            _stream.reconfigure(encoding="utf-8")
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--dest", required=True)
    ap.add_argument("--source-url", default=None)
    ap.add_argument("--source-dir", default=None)
    args = ap.parse_args()
    if args.source_url and args.source_dir:
        return emit({"ok": False, "erreur": "--source-url et --source-dir sont exclusifs"}, 2)
    dest = Path(args.dest)
    if dest.exists() and (not dest.is_dir() or any(dest.iterdir())):
        return emit({"ok": False, "erreur": "la destination doit être un dossier vide : %s" % dest}, 2)
    dest.mkdir(parents=True, exist_ok=True)
    try:
        if args.source_dir:
            commit = copy_dir(Path(args.source_dir), dest)
        else:
            url = args.source_url or DEFAULT_URL
            if not url.startswith(("https://", "file:")):
                return emit({"ok": False, "erreur": "seules les URL https:// et file: sont acceptées"}, 2)
            try:
                with urllib.request.urlopen(url, timeout=30) as resp:
                    data = resp.read(MAX_BYTES + 1)
            except Exception as exc:
                return emit({"ok": False, "erreur": "téléchargement impossible : %s" % exc}, 5)
            if len(data) > MAX_BYTES:
                raise Refused("archive trop volumineuse")
            commit = extract_archive(data, dest)
    except Refused as exc:
        return emit({"ok": False, "erreur": str(exc), "dest": str(dest)}, 4)
    except Exception as exc:
        return emit({"ok": False, "erreur": "échec : %s" % exc, "dest": str(dest)}, 5)
    (dest / "source-commit.txt").write_text(commit + "\n", encoding="utf-8")
    if not (dest / MARKER).is_file():
        return emit({
            "ok": False,
            "erreur": "Le dépôt source ne contient pas encore le Bootstrap : "
                      "brain-main/bootstrap/scripts/prepare-install.py est absent après extraction. "
                      "Il sera disponible une fois le Bootstrap poussé sur Xblode/2brain.",
            "dest": str(dest),
            "source_commit": commit,
        }, 3)
    return emit({"ok": True, "dest": str(dest), "brain_main": str(dest / "brain-main"), "source_commit": commit}, 0)


if __name__ == "__main__":
    sys.exit(main())
