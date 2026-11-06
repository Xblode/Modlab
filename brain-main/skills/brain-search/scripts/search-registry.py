# -*- coding: utf-8 -*-
"""Recherche dans le registre Brain (/brain -s). Lecture seule stricte.

Usage :
  python -I search-registry.py [MOT ...] [--any] [filtres] [--project DOSSIER] [--registry FICHIER]
                               [--limit N] [--json]

Les mots sont cherchés dans title, content, evidence, tags et refs, sans tenir compte de la casse
ni des accents. Par défaut tous les mots doivent être présents ; --any en exige un seul.
Filtres (valeurs multiples séparées par des virgules = OU ; filtres différents = ET) :
  --intention --type --position (« null » pour aucune) --space --status --validation --author
  --tag (répétable ou séparé par des virgules ; tous requis) --project-name --since --until
  --id (CAP-007 ou 7) --relation REL[:CAP-NNN] (relations sortantes de l'entrée)
Au moins un mot ou un filtre est requis.

Classement : titre (100 par mot), puis tags (40), contenu (10), evidence (3), refs (3).
Chaque résultat affiche toujours l'id et la validation : une proposition ne doit pas passer pour
une décision (validation différente de explicit_user_approval = non validé).

Le registre n'est ouvert qu'en lecture (mode rb). Une ligne invalide est signalée sans arrêter la lecture ;
un fichier sans aucune ligne JSON valide (objet) est refusé.
Codes : 0 ok (même sans résultat) ; 2 usage ou fichier introuvable ; 3 fichier qui n'est pas un JSONL.
"""

import argparse
import json
import re
import sys
import unicodedata
from pathlib import Path

FIELDS = ("title", "content", "evidence", "tags", "refs")
WEIGHTS = {"title": 100, "tags": 40, "content": 10, "evidence": 3, "refs": 3}
DEFAULT_REGISTRY = Path("brain") / "registry" / "entries.jsonl"
ID_RE = re.compile(r"^(?:CAP-)?(\d+)$", re.I)


def norm(value):
    if value is None:
        return ""
    if isinstance(value, (list, tuple)):
        return " ".join(norm(v) for v in value)
    text = unicodedata.normalize("NFKD", str(value))
    return "".join(ch for ch in text if not unicodedata.combining(ch)).casefold()


def field_text(entry, name):
    value = entry.get(name)
    if isinstance(value, (list, tuple)):
        return norm([v for v in value if isinstance(v, str)])
    return norm(value if isinstance(value, str) else None)


def split_values(raw):
    out = []
    for item in raw or []:
        out.extend(v.strip() for v in str(item).split(",") if v.strip())
    return out


def canon_id(raw):
    m = ID_RE.match(raw.strip())
    if not m:
        return raw.strip().upper()
    n = int(m.group(1))
    return "CAP-%0*d" % (max(3, len(str(n))), n)


def read_registry(path):
    entries, invalid = [], []
    with open(path, "rb") as fh:  # lecture seule
        data = fh.read()
    try:
        text = data.decode("utf-8")
    except UnicodeDecodeError:
        return None, [{"ligne": 0, "erreur": "le fichier n'est pas de l'UTF-8 (pas un JSONL)"}], 0
    non_empty = 0
    for n, line in enumerate(text.splitlines(), 1):
        if not line.strip():
            continue
        non_empty += 1
        try:
            obj = json.loads(line)
        except ValueError as exc:
            invalid.append({"ligne": n, "erreur": "JSON invalide : %s" % str(exc)[:80]})
            continue
        if not isinstance(obj, dict):
            invalid.append({"ligne": n, "erreur": "objet JSON attendu"})
            continue
        entries.append(obj)
    return entries, invalid, non_empty


def date_part(entry):
    value = entry.get("created_at")
    return value[:10] if isinstance(value, str) else ""


def matches_filters(e, args, ids, tags_req, rel_req):
    def one(field, wanted):
        if not wanted:
            return True
        v = e.get(field)
        return ("null" if v is None else str(v)) in wanted

    for field, wanted in (("intention", args.intention), ("type", args.type), ("position", args.position),
                          ("space", args.space), ("status", args.status), ("validation", args.validation),
                          ("author", args.author)):
        if not one(field, wanted):
            return False
    if args.project_name and str(e.get("project")) not in args.project_name_list:
        return False
    if ids and e.get("id") not in ids:
        return False
    if tags_req:
        have = {norm(t) for t in e.get("tags") or [] if isinstance(t, str)}
        if not all(t in have for t in tags_req):
            return False
    d = date_part(e)
    if args.since and (not d or d < args.since):
        return False
    if args.until and (not d or d > args.until):
        return False
    if rel_req:
        rel, target = rel_req
        ok = False
        for r in e.get("relations") or []:
            if isinstance(r, dict) and r.get("rel") == rel and (target is None or r.get("to") == target):
                ok = True
        if not ok:
            return False
    return True


def score_entry(e, words, any_mode):
    texts = {f: field_text(e, f) for f in FIELDS}
    score, present = 0, 0
    for w in words:
        hit = False
        for f in FIELDS:
            if w in texts[f]:
                score += WEIGHTS[f]
                hit = True
        present += hit
    if not words:
        return 0, True
    return score, (present >= 1 if any_mode else present == len(words))


def excerpt(e, words, size=160):
    content = e.get("content") if isinstance(e.get("content"), str) else ""
    flat = " ".join(content.split())
    if not flat:
        return ""
    low = norm(flat)
    # norm() peut changer la longueur (NFKD) ; on cherche par mot sur le texte normalisé caractère par caractère
    pos = -1
    if len(low) == len(flat):
        for w in words:
            pos = low.find(w)
            if pos >= 0:
                break
    start = max(0, pos - 50) if pos >= 0 else 0
    piece = flat[start:start + size]
    return ("…" if start > 0 else "") + piece + ("…" if start + size < len(flat) else "")


def to_result(e, score, words):
    origin = e.get("origin") if isinstance(e.get("origin"), dict) else {}
    return {
        "id": e.get("id"), "titre": e.get("title"), "intention": e.get("intention"), "type": e.get("type"),
        "position": e.get("position"), "validation": e.get("validation"),
        "valide": e.get("validation") == "explicit_user_approval", "space": e.get("space"),
        "auteur": e.get("author"), "date": e.get("created_at"), "extrait": excerpt(e, words),
        "source": origin.get("source"), "score": score,
    }


def id_key(r):
    m = ID_RE.match(str(r["id"] or ""))
    return int(m.group(1)) if m else 10 ** 9


def as_text(report):
    lines = ["Registre : %s (%d entrées lues)" % (report["registre"], report["entrees_lues"])]
    if report["mots"]:
        lines.append("Mots : %s (%s)" % (", ".join(report["mots"]), "au moins un" if report["mode"] == "any" else "tous"))
    if report["filtres"]:
        lines.append("Filtres : " + "; ".join("%s=%s" % kv for kv in report["filtres"].items()))
    for inv in report["lignes_invalides"]:
        lines.append("ATTENTION ligne %s ignorée : %s" % (inv["ligne"], inv["erreur"]))
    lines.append("%d résultat(s), %d affiché(s)." % (report["total"], len(report["resultats"])))
    for r in report["resultats"]:
        lines.append("")
        lines.append("%s  [%s / %s]  validation=%s  space=%s  auteur=%s  %s  score=%s" % (
            r["id"], r["intention"], r["position"], r["validation"], r["space"], r["auteur"], r["date"], r["score"]))
        lines.append("  " + str(r["titre"]))
        if not r["valide"]:
            lines.append("  (non validé : ne pas lire comme une décision)")
        if r["extrait"]:
            lines.append("  " + r["extrait"])
        lines.append("  source : %s" % r["source"])
    return "\n".join(lines)


def main():
    for _stream in (sys.stdout, sys.stderr):
        if hasattr(_stream, "reconfigure"):
            _stream.reconfigure(encoding="utf-8")
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("mots", nargs="*")
    ap.add_argument("--any", action="store_true", help="un seul mot suffit")
    ap.add_argument("--project", default=".", help="racine du projet (défaut : dossier courant)")
    ap.add_argument("--registry", help="registre à lire (défaut : brain/registry/entries.jsonl du projet)")
    ap.add_argument("--limit", type=int, default=20)
    ap.add_argument("--json", action="store_true")
    for name in ("intention", "type", "position", "space", "status", "validation", "author"):
        ap.add_argument("--" + name, action="append")
    ap.add_argument("--tag", action="append")
    ap.add_argument("--project-name", action="append", dest="project_name")
    ap.add_argument("--since")
    ap.add_argument("--until")
    ap.add_argument("--id", action="append", dest="id_")
    ap.add_argument("--relation")
    sys.stdout.reconfigure(encoding="utf-8")
    sys.stderr.reconfigure(encoding="utf-8")
    args = ap.parse_args()

    for name in ("intention", "type", "position", "space", "status", "validation", "author"):
        setattr(args, name, split_values(getattr(args, name)))
    args.project_name_list = split_values(args.project_name)
    ids = {canon_id(v) for v in split_values(args.id_)}
    tags_req = [norm(t) for t in split_values(args.tag)]
    rel_req = None
    if args.relation:
        rel, _, target = args.relation.partition(":")
        rel_req = (rel.strip(), canon_id(target) if target.strip() else None)
    for label, value in (("--since", args.since), ("--until", args.until)):
        if value and not re.match(r"^\d{4}-\d{2}-\d{2}$", value):
            sys.stderr.write("%s attend une date AAAA-MM-JJ\n" % label)
            return 2
    words = [w for w in (norm(m) for m in args.mots) if w]
    filtres = {k: v for k, v in (
        ("intention", args.intention), ("type", args.type), ("position", args.position), ("space", args.space),
        ("status", args.status), ("validation", args.validation), ("author", args.author),
        ("project-name", args.project_name_list), ("tag", split_values(args.tag)), ("id", sorted(ids)),
        ("since", args.since), ("until", args.until), ("relation", args.relation)) if v}
    if not words and not filtres:
        sys.stderr.write("rien à chercher : donner au moins un mot ou un filtre\n")
        return 2
    path = Path(args.registry) if args.registry else Path(args.project) / DEFAULT_REGISTRY
    if not path.is_file():
        sys.stderr.write("registre introuvable : %s\n" % path)
        return 2
    entries, invalid, non_empty = read_registry(path)
    if entries is None or (non_empty > 0 and not entries):
        sys.stdout.write(json.dumps({"ok": False, "erreur": "ce fichier n'est pas un JSONL de registre", "registre": str(path),
                                     "lignes_invalides": invalid[:5]}, ensure_ascii=False, indent=2) + "\n")
        sys.stderr.write("refus : %s n'est pas un JSONL (aucune ligne objet JSON valide)\n" % path)
        return 3
    found = []
    for e in entries:
        if not matches_filters(e, args, ids, tags_req, rel_req):
            continue
        score, ok = score_entry(e, words, args.any)
        if ok:
            found.append((score, e))
    results = [to_result(e, s, words) for s, e in found]
    results.sort(key=lambda r: (-r["score"], id_key(r)))
    report = {
        "ok": True, "registre": str(path), "entrees_lues": len(entries), "mots": words,
        "mode": "any" if args.any else "all", "filtres": filtres, "total": len(results),
        "resultats": results[:max(0, args.limit)], "lignes_invalides": invalid,
    }
    if invalid:
        sys.stderr.write("%d ligne(s) invalide(s) ignorée(s)\n" % len(invalid))
    sys.stdout.write((json.dumps(report, ensure_ascii=False, indent=2) if args.json else as_text(report)) + "\n")
    return 0


if __name__ == "__main__":
    sys.exit(main())
