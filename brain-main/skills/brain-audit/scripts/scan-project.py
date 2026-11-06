# -*- coding: utf-8 -*-
"""Survol d'un projet existant pour /brain -a. LECTURE SEULE STRICTE : aucune écriture, aucun réseau.

Usage : python -I scan-project.py --project DOSSIER [--json] [--max-depth N] [--limit N] [--plafond N] [--min-fichiers N]

Ce que le script fait : compter les fichiers par dossier et par extension, repérer les manifestes et la
documentation, proposer des candidats de « dossiers maîtres », signaler des candidats d'écart mécaniques
(front matter et noms des .md). Ce sont des candidats à examiner : ni erreurs, ni décisions.
Les fichiers de secrets ne sont JAMAIS ouverts : seuls leur nombre et leur nom sont donnés.
Seuls les .md non secrets, non lourds, hors dossiers exclus sont ouverts (lecture).
Le plafond de README vient de `brain/settings.json` (clé `readme_max`, lecture seule) ; absent ou invalide : 8, et le
résultat le dit (`reglage`). `--plafond N` l'emporte sur le réglage.
Codes de sortie : 0 normal, 2 dossier invalide.
"""

import argparse
import fnmatch
import json
import os
import re
import sys
from pathlib import Path

EXCLUDED_DIRS = (".git", "brain", "brain-main", ".claude", "node_modules", "dist", "build", ".next", "target",
                 "vendor", "__pycache__", ".venv", "venv", "coverage")
SECRET_PATTERNS = (".env*", "*.pem", "*.key", "*.p12", "*.pfx", "id_rsa*", "credentials*", "secrets*")
HEAVY_BYTES = 1024 * 1024
BINARY_EXT = {".png", ".jpg", ".jpeg", ".gif", ".webp", ".ico", ".bmp", ".tiff", ".pdf", ".zip", ".gz", ".tar", ".tgz",
              ".7z", ".rar", ".exe", ".dll", ".so", ".dylib", ".bin", ".class", ".jar", ".pyc", ".woff", ".woff2",
              ".ttf", ".otf", ".eot", ".mp3", ".mp4", ".mov", ".avi", ".wav", ".ogg", ".webm", ".sqlite", ".db",
              ".psd", ".ai", ".pen", ".lock"}
CONTAINERS = ("src", "apps", "packages", "libs", "services", "modules", "lib", "app", "crates", "cmd", "internal",
              "plugins", "projects")
MANIFESTS = ("package.json", "pyproject.toml", "setup.py", "requirements.txt", "Cargo.toml", "go.mod", "pom.xml",
             "build.gradle", "Gemfile", "composer.json", "tsconfig.json", "Dockerfile", "docker-compose.yml",
             "Makefile", "pnpm-workspace.yaml", "next.config.js", "vite.config.ts")
DOC_NAMES = ("AGENTS.md", "CLAUDE.md", "CHANGELOG.md", "CONTRIBUTING.md", "LICENSE")
REQUIRED_FM = ("metadata_version", "title", "type", "description", "status", "created", "updated")
TOOL_NAMES = {"README.md", "AGENTS.md", "CLAUDE.md", "SKILL.md"}
KEBAB = re.compile(r"^[a-z0-9]+(-[a-z0-9]+)*$")
UPPER = re.compile(r"^[A-Z0-9]+(_[A-Z0-9]+)*$")
REF_FM = "brain-main/conventions/METADATA.md §4.2 (propriétés obligatoires), §6.1 (front matter Markdown)"
REF_NAME = "brain-main/conventions/NAMING.md §3.1, §3.2, §3.4 (casse et extensions)"
REF_CASE = "brain-main/conventions/NAMING.md §7.4 (doublons de casse)"
RULE_MASTER = ("Candidats : sous-dossiers du premier niveau du projet, sauf les conteneurs (%s), remplacés par leurs "
               "propres sous-dossiers directs ; un conteneur sans sous-dossier retenu reste candidat lui-même. "
               "Un candidat a au moins `min_fichiers` fichiers (récursif, hors exclusions) ; dossiers cachés "
               "écartés ; ordre par nombre de fichiers décroissant ; les `plafond_readme` premiers sont retenus." % ", ".join(CONTAINERS))


def is_secret(name):
    low = name.lower()
    return any(fnmatch.fnmatch(low, pat) for pat in SECRET_PATTERNS)


def find_case_duplicates(rels):
    """Groupes de chemins du même dossier ne différant que par la casse du nom (NAMING §7.4)."""
    by_dir_lower = {}
    for rel in rels:
        d = rel.rsplit("/", 1)[0] if "/" in rel else "."
        by_dir_lower.setdefault((d, rel.rsplit("/", 1)[-1].lower()), []).append(rel)
    return [sorted(v) for v in by_dir_lower.values() if len(v) > 1]


DEFAULT_PLAFOND = 8


def read_readme_max(project):
    """(valeur, état, détail) du réglage `readme_max` de brain/settings.json ; défaut 8 si absent ou invalide."""
    path = project / "brain" / "settings.json"
    if not path.is_file():
        return DEFAULT_PLAFOND, "défaut", "fichier de réglages de brain/ absent : valeur par défaut %d" % DEFAULT_PLAFOND
    try:
        data = json.loads(path.read_bytes().decode("utf-8"))
    except Exception:
        return DEFAULT_PLAFOND, "défaut", "fichier de réglages de brain/ invalide (JSON illisible) : valeur par défaut %d" % DEFAULT_PLAFOND
    if not isinstance(data, dict) or "readme_max" not in data:
        return DEFAULT_PLAFOND, "défaut", "fichier de réglages de brain/ sans clé `readme_max` : valeur par défaut %d" % DEFAULT_PLAFOND
    v = data["readme_max"]
    if isinstance(v, bool) or not isinstance(v, int) or v < 1:
        return DEFAULT_PLAFOND, "défaut", "`readme_max` invalide (entier supérieur ou égal à 1 attendu) : valeur par défaut %d" % DEFAULT_PLAFOND
    return v, "réglage", "lu dans le fichier de réglages de brain/"


def scan(project, max_depth, limit, plafond, min_files, reglage=None):
    dir_counts = {}
    ext_counts = {}
    secrets = []
    heavy = 0
    binary = 0
    links = 0
    unreadable = 0
    excluded_found = {}
    files_total = 0
    root_files = []
    all_files = []  # (rel, size)
    dirs_seen = set()

    def onerror(_exc):
        nonlocal unreadable
        unreadable += 1

    for cur, dirs, files in os.walk(project, topdown=True, onerror=onerror, followlinks=False):
        curp = Path(cur)
        rel_dir = curp.relative_to(project).as_posix()
        keep = []
        for d in sorted(dirs):
            full = curp / d
            if d in EXCLUDED_DIRS:
                excluded_found[d] = excluded_found.get(d, 0) + 1
            elif full.is_symlink():
                links += 1
            else:
                keep.append(d)
        dirs[:] = keep
        if rel_dir != ".":
            dirs_seen.add(rel_dir)
        for f in sorted(files):
            full = curp / f
            rel = (full.relative_to(project)).as_posix()
            if is_secret(f):
                secrets.append(rel)
                continue
            if full.is_symlink():
                links += 1
                continue
            try:
                size = full.stat().st_size
            except OSError:
                unreadable += 1
                continue
            ext = full.suffix.lower() or "(sans extension)"
            ext_counts[ext] = ext_counts.get(ext, 0) + 1
            files_total += 1
            if size > HEAVY_BYTES:
                heavy += 1
            elif ext in BINARY_EXT:
                binary += 1
            all_files.append((rel, size))
            if rel_dir == ".":
                root_files.append(f)
            parts = rel.split("/")[:-1]
            for i in range(1, len(parts) + 1):
                key = "/".join(parts[:i])
                dir_counts[key] = dir_counts.get(key, 0) + 1

    # arborescence résumée
    tree = [{"chemin": ".", "profondeur": 0, "fichiers": files_total, "fichiers_directs": len(root_files)}]
    direct = {}
    for rel, _ in all_files:
        d = rel.rsplit("/", 1)[0] if "/" in rel else "."
        direct[d] = direct.get(d, 0) + 1
    for d in sorted(dirs_seen):
        depth = d.count("/") + 1
        if depth <= max_depth:
            tree.append({"chemin": d, "profondeur": depth, "fichiers": dir_counts.get(d, 0), "fichiers_directs": direct.get(d, 0)})
    tree_total = len(tree)
    tree = tree[:limit]

    # manifestes et documentation (profondeur <= max_depth)
    found = []
    for rel, _ in all_files:
        parts = rel.split("/")
        if len(parts) - 1 > max_depth:
            continue
        name = parts[-1]
        low = name.lower()
        kind = None
        if name in MANIFESTS:
            kind = "manifeste"
        elif low.startswith("readme"):
            kind = "readme"
        elif name in DOC_NAMES:
            kind = "documentation"
        elif parts[0] == "docs" and len(parts) > 1:
            kind = "docs/"
        if kind:
            found.append({"chemin": rel, "nature": kind})
    docs_dir = (project / "docs").is_dir()
    found_total = len(found)
    found = found[:limit]

    # dossiers maîtres
    def has_readme(rel):
        base = project / rel
        try:
            return sorted(n.name for n in base.iterdir() if n.is_file() and n.name.lower().startswith("readme"))
        except OSError:
            return []

    cands = {}
    top = sorted(d for d in dirs_seen if "/" not in d and not d.startswith("."))
    for d in top:
        if d in CONTAINERS:
            subs = sorted(s for s in dirs_seen if s.startswith(d + "/") and s.count("/") == 1 and not s.split("/")[1].startswith("."))
            kept = [s for s in subs if dir_counts.get(s, 0) >= min_files]
            if kept:
                for s in kept:
                    cands[s] = "sous-dossier de conteneur"
                continue
        cands[d] = "premier niveau"
    masters = []
    for rel, kind in cands.items():
        n = dir_counts.get(rel, 0)
        if n >= min_files:
            r = has_readme(rel)
            masters.append({"chemin": rel, "fichiers": n, "origine": kind, "readme": bool(r), "readme_noms": r})
    masters.sort(key=lambda m: (-m["fichiers"], m["chemin"]))
    for i, m in enumerate(masters):
        m["retenu"] = i < plafond

    # candidats d'écart mécaniques
    gaps = []
    md_count = 0
    case_dups = find_case_duplicates([rel for rel, _ in all_files])
    for rel, size in all_files:
        name = rel.rsplit("/", 1)[-1]
        if not name.lower().endswith(".md") or size > HEAVY_BYTES:
            continue
        md_count += 1
        items = []
        stem, ext = os.path.splitext(name)
        if name not in TOOL_NAMES and not (KEBAB.match(stem) or UPPER.match(stem)):
            items.append({"nature": "nom hors kebab-case et UPPER_SNAKE_CASE", "reference": REF_NAME})
        if ext != ext.lower():
            items.append({"nature": "extension en majuscules", "reference": REF_NAME})
        try:
            text = (project / rel).read_text(encoding="utf-8")
        except (OSError, UnicodeDecodeError):
            items.append({"nature": "fichier illisible en UTF-8 (non examiné)", "reference": "limite de la lecture"})
            text = None
        if text is not None:
            lines = text.lstrip("﻿").split("\n")
            if not lines or lines[0].strip() != "---":
                items.append({"nature": "pas de front matter en tête de fichier", "reference": REF_FM})
            else:
                end = next((i for i, l in enumerate(lines[1:], 1) if l.strip() == "---"), None)
                if end is None:
                    items.append({"nature": "front matter non fermé", "reference": REF_FM})
                else:
                    keys = {m.group(1) for l in lines[1:end] for m in [re.match(r"^([A-Za-z_][\w-]*)\s*:", l)] if m}
                    missing = [k for k in REQUIRED_FM if k not in keys]
                    if missing:
                        items.append({"nature": "propriétés obligatoires absentes : " + ", ".join(missing), "reference": REF_FM})
        if items:
            gaps.append({"fichier": rel, "ecarts": items, "statut": "candidat à examiner"})
    for group in case_dups:
        gaps.append({"fichier": " / ".join(group), "ecarts": [{"nature": "noms ne différant que par la casse", "reference": REF_CASE}],
                     "statut": "candidat à examiner"})
    gaps_total = len(gaps)
    gaps = gaps[:limit]

    ext_sorted = sorted(ext_counts.items(), key=lambda kv: (-kv[1], kv[0]))
    return {
        "racine": str(project),
        "parametres": {"max_depth": max_depth, "limit": limit, "plafond_readme": plafond, "min_fichiers": min_files},
        "fichiers": {"total": files_total, "par_extension": dict(ext_sorted[:limit]), "extensions_distinctes": len(ext_sorted)},
        "arborescence": {"dossiers": tree, "dossiers_total_a_cette_profondeur": tree_total},
        "exclusions": {"dossiers_exclus": list(EXCLUDED_DIRS), "dossiers_exclus_rencontres": excluded_found,
                       "fichiers_lourds_ignores": heavy, "seuil_lourd_octets": HEAVY_BYTES,
                       "fichiers_binaires_ignores": binary, "liens_symboliques_ignores": links,
                       "illisibles": unreadable},
        "secrets": {"motifs": list(SECRET_PATTERNS), "nombre": len(secrets), "noms": secrets[:limit],
                    "note": "Jamais ouverts : seuls le nombre et le nom sont connus."},
        "reperes": {"elements": found, "total": found_total, "dossier_docs": docs_dir},
        "dossiers_maitres": {"regle": RULE_MASTER, "plafond_readme": plafond, "candidats_total": len(masters),
                             "candidats": masters[:limit],
                             "au_dela_du_plafond": max(0, len(masters) - plafond),
                             "readme_existants": sum(1 for m in masters if m["readme"]),
                             "readme_restants": max(0, plafond - sum(1 for m in masters if m["readme"])),
                             "reglage": reglage or {"valeur": plafond, "etat": "paramètre", "detail": "--plafond"}},
        "ecarts_candidats": {"avertissement": "Candidats d'écart mécaniques, à examiner : ni des erreurs, ni des décisions. "
                                              "Ne pas corriger sans validation explicite.",
                             "fichiers_md_examines": md_count, "total": gaps_total, "elements": gaps},
    }


def as_text(r):
    L = ["Projet : %s" % r["racine"],
         "Fichiers (hors exclusions et secrets) : %d ; extensions distinctes : %d" % (r["fichiers"]["total"], r["fichiers"]["extensions_distinctes"]),
         "Par extension : " + ", ".join("%s=%d" % kv for kv in r["fichiers"]["par_extension"].items()),
         "", "Arborescence (dossiers, fichiers récursifs) :"]
    for d in r["arborescence"]["dossiers"]:
        L.append("%s%s : %d" % ("  " * d["profondeur"], d["chemin"], d["fichiers"]))
    ex = r["exclusions"]
    L += ["", "Exclus (dossiers) : " + (", ".join("%s x%d" % kv for kv in ex["dossiers_exclus_rencontres"].items()) or "aucun rencontré"),
          "Ignorés : %d lourds (> %d octets), %d binaires, %d liens symboliques, %d illisibles" % (
              ex["fichiers_lourds_ignores"], ex["seuil_lourd_octets"], ex["fichiers_binaires_ignores"],
              ex["liens_symboliques_ignores"], ex["illisibles"]),
          "", "Fichiers de secrets (jamais ouverts) : %d" % r["secrets"]["nombre"]]
    L += ["  " + n for n in r["secrets"]["noms"]]
    L += ["", "Manifestes et documentation (%d) :" % r["reperes"]["total"]]
    L += ["  [%s] %s" % (e["nature"], e["chemin"]) for e in r["reperes"]["elements"]]
    dm = r["dossiers_maitres"]
    L += ["", "Dossiers maîtres candidats (%d, plafond README %d, %d au-delà) :" % (dm["candidats_total"], dm["plafond_readme"], dm["au_dela_du_plafond"]),
          "  Plafond : %s (%s)" % (dm["reglage"]["valeur"], dm["reglage"]["detail"]),
          "  README existants : %d, restants avant le plafond : %d" % (dm["readme_existants"], dm["readme_restants"]),
          "  Règle : " + dm["regle"]]
    L += ["  %s %s : %d fichiers, README %s" % ("*" if c["retenu"] else "-", c["chemin"], c["fichiers"], "oui" if c["readme"] else "non") for c in dm["candidats"]]
    eg = r["ecarts_candidats"]
    L += ["", "Candidats d'écart (%d sur %d .md examinés) : %s" % (eg["total"], eg["fichiers_md_examines"], eg["avertissement"])]
    for g in eg["elements"]:
        for it in g["ecarts"]:
            L.append("  %s : %s [%s]" % (g["fichier"], it["nature"], it["reference"]))
    return "\n".join(L)


def main():
    for _stream in (sys.stdout, sys.stderr):
        if hasattr(_stream, "reconfigure"):
            _stream.reconfigure(encoding="utf-8")
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--project", required=True)
    ap.add_argument("--json", action="store_true")
    ap.add_argument("--max-depth", type=int, default=3, help="profondeur de l'arborescence et des repères (défaut 3)")
    ap.add_argument("--limit", type=int, default=200, help="éléments maximum par liste (défaut 200)")
    ap.add_argument("--plafond", type=int, default=None, help="plafond_readme : dossiers maîtres retenus (défaut : brain/settings.json readme_max, sinon 8)")
    ap.add_argument("--min-fichiers", type=int, default=3, help="seuil de fichiers d'un dossier maître (défaut 3)")
    args = ap.parse_args()
    project = Path(args.project).resolve()
    if not project.is_dir() or args.max_depth < 1 or args.limit < 1 or (args.plafond is not None and args.plafond < 1) or args.min_fichiers < 1:
        sys.stderr.write("Dossier ou paramètre invalide.\n")
        return 2
    value, state, detail = read_readme_max(project)
    reglage = {"valeur": value, "etat": state, "detail": detail}
    plafond = value
    if args.plafond is not None:
        plafond = args.plafond
        reglage = {"valeur": plafond, "etat": "paramètre", "detail": "--plafond l'emporte sur le réglage (%s)" % detail}
    res = scan(project, args.max_depth, args.limit, plafond, args.min_fichiers, reglage)
    if args.json:
        sys.stdout.write(json.dumps(res, ensure_ascii=False, indent=2) + "\n")
    else:
        sys.stdout.write(as_text(res) + "\n")
    return 0


if __name__ == "__main__":
    sys.exit(main())
