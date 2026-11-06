# -*- coding: utf-8 -*-
"""Review du registre Brain en rapport seulement (/brain -r). Aucun fichier écrit.

Usage :
  python -I review-registry.py [--project DOSSIER] [--registry FICHIER] [--limit N] [--json]

Le rapport est imprimé sur la sortie standard. Le registre n'est ouvert qu'en lecture : aucun
changement de statut, aucune validation, aucune suppression, aucune réécriture n'est proposée.
Chaque élément est un CANDIDAT À EXAMINER (ids et raison), jamais une conclusion.

Sections :
  doublons        titres identiques après normalisation (casse, accents, ponctuation) ; contenus proches
                  (similarité des mots d'au moins JACCARD_MIN puis difflib d'au moins RATIO_MIN)
  contradictions  relations conflicts_with / contradicts / contradicted_by ; positions opposées sur un
                  même titre ; entrée validée contestée
  relations       cibles absentes, auto-références, relations inconnues, réciprocité attendue manquante
                  (TAXONOMY §7) ; statistiques du graphe
  manques         questions sans réponse reliée, approbation incohérente, referentiel sans validation,
                  locators vides, champs obligatoires absents, URL du contenu absente de refs, mapping
                  intention-type incohérent, identifiants dupliqués

Méthodes et limites : détection mécanique sur les champs du registre, sans interprétation du sens.
Elle peut manquer des doublons reformulés, des contradictions de fond et des relations implicites, et
signaler des cas sans importance (registres historiques). Le jugement de fond est une lecture humaine ;
Claude le présente comme une proposition à valider.

--limit borne le nombre d'éléments affichés par type (les comptes restent complets ; --json donne tout).
Codes : 0 ok ; 2 usage ou fichier introuvable ; 3 fichier qui n'est pas un JSONL.
"""

import argparse
import difflib
import json
import re
import sys
import unicodedata
from pathlib import Path

JACCARD_MIN = 0.70
RATIO_MIN = 0.80
MIN_TOKENS = 8
DEFAULT_REGISTRY = Path("brain") / "registry" / "entries.jsonl"

KNOWN_REL = {"related", "clarifies", "clarified_by", "answers", "answered_by", "evolves_from", "evolves_to",
             "conflicts_with", "contradicts", "contradicted_by", "resolves", "supersedes_candidate",
             "parent_of", "child_of"}
RECIPROCAL = {"clarifies": "clarified_by", "clarified_by": "clarifies", "answers": "answered_by",
              "answered_by": "answers", "evolves_from": "evolves_to", "evolves_to": "evolves_from",
              "contradicts": "contradicted_by", "contradicted_by": "contradicts", "parent_of": "child_of",
              "child_of": "parent_of", "conflicts_with": "conflicts_with"}
OFTEN_ONLY = {"conflicts_with"}  # TAXONOMY §7 : « souvent symétrique »
CONFLICT_RELS = {"conflicts_with", "contradicts", "contradicted_by"}
FAVORABLE = {"favorable", "endorsed", "selected", "confirmed"}
REQUIRED = ("id", "type", "status", "space", "validation", "author", "approved_by", "approval_scope", "title",
            "content", "evidence", "confidence", "origin", "project", "created_at", "storage")
INTENTION_TYPE = {"idea": "idea", "suggestion": "proposition", "preference": "observation",
                  "orientation": "proposition", "question": "open_question", "decision": "decision",
                  "requirement": "observation", "convention": "convention", "task": "task",
                  "milestone": "task", "observation": "observation", "risk": "observation", "feedback": "observation"}
ID_RE = re.compile(r"^CAP-(\d+)$")
URL_RE = re.compile(r"https?://[^\s\"'<>)\]]+")

METHODE = ("Détection mécanique sur les champs du registre, sans interprétation du sens. Chaque élément est un "
           "candidat à examiner. Aucune suppression ni réécriture n'est proposée ; toute décision reste humaine.")
LIMITES = ("Peut manquer des doublons reformulés, des contradictions de fond et des relations implicites ; peut "
           "signaler des cas sans importance dans un registre historique. À compléter par une lecture humaine.")


def norm(text):
    if not isinstance(text, str):
        return ""
    text = unicodedata.normalize("NFKD", text)
    text = "".join(ch for ch in text if not unicodedata.combining(ch)).casefold()
    return " ".join(re.sub(r"[^a-z0-9]+", " ", text).split())


def read_registry(path):
    with open(path, "rb") as fh:
        data = fh.read()
    try:
        text = data.decode("utf-8")
    except UnicodeDecodeError:
        return None, [{"ligne": 0, "erreur": "pas de l'UTF-8"}], 0
    entries, invalid, non_empty = [], [], 0
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


def num(eid):
    m = ID_RE.match(str(eid))
    return int(m.group(1)) if m else 10 ** 9


class Report:
    def __init__(self):
        self.items = {"doublons": [], "contradictions": [], "relations": [], "manques": []}

    def add(self, section, kind, ids, raison):
        self.items[section].append({"type": kind, "ids": sorted(set(ids), key=num), "raison": raison})


def analyse(entries, rep):
    by_id, dup_ids = {}, {}
    for e in entries:
        eid = e.get("id")
        if not isinstance(eid, str):
            rep.add("manques", "id absent", [], "entrée sans id texte (titre : %s)" % str(e.get("title"))[:60])
            continue
        if eid in by_id:
            dup_ids.setdefault(eid, 1)
            dup_ids[eid] += 1
        else:
            by_id[eid] = e
    for eid, n in dup_ids.items():
        rep.add("manques", "identifiant dupliqué", [eid], "%s apparaît %d fois dans le fichier" % (eid, n))

    rels = []
    for e in entries:
        src = e.get("id")
        lst = e.get("relations")
        if lst is None:
            continue
        if not isinstance(lst, list):
            rep.add("relations", "relations mal formées", [src], "relations n'est pas une liste")
            continue
        for r in lst:
            if isinstance(r, dict) and isinstance(r.get("rel"), str) and isinstance(r.get("to"), str):
                rels.append((src, r["rel"], r["to"]))
            else:
                rep.add("relations", "relation mal formée", [src], "élément sans rel/to texte : %s" % json.dumps(r, ensure_ascii=False)[:80])
    rel_set = set(rels)

    # ---- doublons
    groups = {}
    for eid, e in by_id.items():
        t = norm(e.get("title"))
        if t:
            groups.setdefault(t, []).append(eid)
    same_title = set()
    for t, ids in groups.items():
        if len(ids) > 1:
            rep.add("doublons", "titres identiques", ids, "même titre après normalisation : « %s »" % (by_id[ids[0]].get("title")))
            same_title.update(frozenset((a, b)) for i, a in enumerate(ids) for b in ids[i + 1:])
    toks = {}
    for eid, e in by_id.items():
        words = {w for w in norm(e.get("content")).split() if len(w) >= 3}
        if len(words) >= MIN_TOKENS:
            toks[eid] = words
    ordered = sorted(toks, key=num)
    for i, a in enumerate(ordered):
        ta = toks[a]
        for b in ordered[i + 1:]:
            tb = toks[b]
            inter = len(ta & tb)
            if inter == 0 or inter / len(ta | tb) < JACCARD_MIN:
                continue
            ca, cb = norm(by_id[a].get("content"))[:2000], norm(by_id[b].get("content"))[:2000]
            ratio = difflib.SequenceMatcher(None, ca, cb, autojunk=False).ratio()
            if ratio < RATIO_MIN:
                continue
            linked = sorted({r for (s, r, d) in rels if {s, d} == {a, b}})
            extra = " ; déjà reliées par : %s" % ", ".join(linked) if linked else ""
            if frozenset((a, b)) in same_title:
                extra += " ; titres identiques aussi"
            rep.add("doublons", "contenus proches", [a, b],
                    "mots communs %.2f (seuil %.2f), ressemblance difflib %.2f (seuil %.2f)%s"
                    % (inter / len(ta | tb), JACCARD_MIN, ratio, RATIO_MIN, extra))

    # ---- contradictions
    resolved = {x for (s, r, d) in rels if r == "resolves" for x in (s, d)}
    pairs = {}
    for (s, r, d) in rels:
        if r in CONFLICT_RELS:
            pairs.setdefault(frozenset((s, d)), []).append("%s %s %s" % (s, r, d))
    for pair, decl in pairs.items():
        ids = sorted(pair, key=num)
        closed = bool(set(ids) & resolved)
        rep.add("contradictions", "relation déclarée", ids,
                "%s ; %s" % (" / ".join(decl), "une relation resolves touche l'une des entrées" if closed
                             else "aucune relation resolves trouvée (tension non close selon TAXONOMY §7)"))
        validated = [i for i in ids if i in by_id and by_id[i].get("validation") == "explicit_user_approval"]
        if validated:
            rep.add("contradictions", "entrée validée contestée", ids,
                    "%s a validation=explicit_user_approval et figure dans une relation de tension" % ", ".join(validated))
    for t, ids in groups.items():
        positions = {by_id[i].get("position") for i in ids}
        if len(ids) > 1 and "opposed" in positions and positions & FAVORABLE:
            rep.add("contradictions", "positions opposées", ids, "même titre « %s » avec position opposed et %s"
                    % (by_id[ids[0]].get("title"), "/".join(sorted(positions & FAVORABLE))))

    # ---- relations
    linked_nodes, edges = set(), set()
    for (s, r, d) in rels:
        if r not in KNOWN_REL:
            rep.add("relations", "relation inconnue", [s], "%s %s %s : valeur absente de TAXONOMY §7" % (s, r, d))
        if s == d:
            rep.add("relations", "auto-référence", [s], "%s %s elle-même" % (s, r))
            continue
        if d not in by_id:
            rep.add("relations", "cible absente", [s], "%s %s %s : %s n'existe pas dans ce fichier" % (s, r, d, d))
            continue
        if s not in by_id:
            continue
        linked_nodes.update((s, d))
        edges.add(frozenset((s, d)))
        back = RECIPROCAL.get(r)
        if back and (d, back, s) not in rel_set:
            soft = r in OFTEN_ONLY
            rep.add("relations", "réciproque facultative absente" if soft else "réciproque attendue absente", [s, d],
                    "%s %s %s : %s %s %s est %s (TAXONOMY §7)" % (s, r, d, d, back, s, "souvent attendue" if soft else "recommandée"))
    parent = {i: i for i in by_id}

    def find(x):
        while parent[x] != x:
            parent[x] = parent[parent[x]]
            x = parent[x]
        return x

    for e in edges:
        a, b = tuple(e)
        parent[find(a)] = find(b)
    comps = {}
    for i in by_id:
        comps.setdefault(find(i), []).append(i)
    sizes = sorted((len(v) for v in comps.values()), reverse=True)
    isolated = sorted((i for i in by_id if i not in linked_nodes), key=num)
    stats = {
        "entrees": len(by_id), "relations_declarees": len(rels), "liens_distincts": len(edges),
        "entrees_reliees": len(linked_nodes), "entrees_isolees": len(isolated),
        "composantes": len(comps), "composantes_de_2_et_plus": sum(1 for s in sizes if s >= 2),
        "plus_grande_composante": sizes[0] if sizes else 0, "ids_isoles": isolated,
    }

    # ---- manques
    answered = {d for (s, r, d) in rels if r in ("answers", "resolves")} | {s for (s, r, d) in rels if r == "answered_by"}
    for eid, e in by_id.items():
        if (e.get("intention") == "question" or (e.get("intention") is None and e.get("type") == "open_question")) \
                and eid not in answered:
            rep.add("manques", "question sans réponse reliée", [eid], "aucune relation answers/answered_by/resolves vers %s" % eid)
        val, by, scope = e.get("validation"), e.get("approved_by"), e.get("approval_scope")
        if val == "explicit_user_approval" and (by != "user" or scope not in ("item", "set")):
            rep.add("manques", "approbation incohérente", [eid], "explicit_user_approval avec approved_by=%r approval_scope=%r (SCHEMA : user + item|set)" % (by, scope))
        if val in ("none", "implicit") and (by is not None or scope is not None):
            rep.add("manques", "approbation incohérente", [eid], "validation=%s mais approved_by=%r approval_scope=%r (attendu : null)" % (val, by, scope))
        if e.get("space") == "referentiel" and val == "none":
            ev = e.get("evidence")
            rep.add("manques", "referentiel sans validation", [eid],
                    "space=referentiel avec validation=none (TAXONOMY §6 règle 4) ; evidence %s" % ("renseigné" if isinstance(ev, str) and ev.strip() else "vide : rien ne documente le statut établi"))
        origin = e.get("origin")
        if not isinstance(origin, dict):
            rep.add("manques", "provenance absente", [eid], "origin absent ou mal formé")
        else:
            if not isinstance(origin.get("source"), str) or not origin["source"].strip():
                rep.add("manques", "provenance absente", [eid], "origin.source vide")
            loc = origin.get("locators")
            if not isinstance(loc, list) or not loc:
                rep.add("manques", "locators vides", [eid], "origin.locators absent ou vide")
        missing = [f for f in REQUIRED if f not in e]
        if missing:
            rep.add("manques", "champs obligatoires absents", [eid], "absents : %s" % ", ".join(missing))
        content = e.get("content") if isinstance(e.get("content"), str) else ""
        refs = [r for r in (e.get("refs") or []) if isinstance(r, str)]
        lost = [u for u in URL_RE.findall(content) if not any(u.rstrip(".,;") in r or r in u for r in refs)]
        if lost:
            rep.add("manques", "URL absente de refs", [eid], "URL du contenu non reprise dans refs : %s" % lost[0][:100])
        it, ty = e.get("intention"), e.get("type")
        if it in INTENTION_TYPE and ty != INTENTION_TYPE[it]:
            rep.add("manques", "mapping intention-type incohérent", [eid], "intention=%s attend type=%s, trouvé %s (entrée historique, conservée telle quelle)" % (it, INTENTION_TYPE[it], ty))
        if e.get("status") not in (None, "extracted"):
            rep.add("manques", "status inattendu", [eid], "status=%r (Capture écrit seulement extracted)" % e.get("status"))
    return stats


def summarize(rep):
    out = {}
    for section, items in rep.items.items():
        kinds = {}
        for it in items:
            kinds[it["type"]] = kinds.get(it["type"], 0) + 1
        out[section] = {"total": len(items), "par_type": dict(sorted(kinds.items())), "elements": items}
    return out


def as_text(rep_json, limit):
    lines = ["REVIEW EN RAPPORT SEULEMENT (aucun fichier écrit)",
             "Registre : %s (%d entrées, %d ligne(s) invalide(s))" % (rep_json["registre"], rep_json["entrees"], len(rep_json["lignes_invalides"])),
             "Méthode : " + METHODE, "Limites : " + LIMITES,
             "Seuils : mots communs >= %.2f, difflib >= %.2f, au moins %d mots" % (JACCARD_MIN, RATIO_MIN, MIN_TOKENS)]
    for inv in rep_json["lignes_invalides"]:
        lines.append("ATTENTION ligne %s ignorée : %s" % (inv["ligne"], inv["erreur"]))
    for section, data in rep_json["sections"].items():
        lines += ["", "== %s : %d candidat(s) ==" % (section, data["total"])]
        if section == "relations":
            st = rep_json["graphe"]
            lines.append("graphe : %d entrées, %d relations déclarées, %d liens distincts, %d reliées, %d isolées, %d composantes (%d de 2 et plus, la plus grande : %d)" % (
                st["entrees"], st["relations_declarees"], st["liens_distincts"], st["entrees_reliees"], st["entrees_isolees"],
                st["composantes"], st["composantes_de_2_et_plus"], st["plus_grande_composante"]))
            if st["ids_isoles"]:
                shown = st["ids_isoles"][:limit]
                lines.append("isolées : %s%s" % (", ".join(shown), " … (+%d)" % (len(st["ids_isoles"]) - len(shown)) if len(st["ids_isoles"]) > len(shown) else ""))
        for kind, count in data["par_type"].items():
            lines.append("-- %s : %d" % (kind, count))
            shown = 0
            for it in data["elements"]:
                if it["type"] != kind:
                    continue
                if shown >= limit:
                    lines.append("   … (+%d, voir --json)" % (count - shown))
                    break
                lines.append("   [%s] %s" % (", ".join(it["ids"]) or "-", it["raison"]))
                shown += 1
    lines += ["", "Rappel : candidats à examiner, pas des conclusions. Le jugement de fond est une proposition à valider par l'humain."]
    return "\n".join(lines)


def main():
    for _stream in (sys.stdout, sys.stderr):
        if hasattr(_stream, "reconfigure"):
            _stream.reconfigure(encoding="utf-8")
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--project", default=".", help="racine du projet (défaut : dossier courant)")
    ap.add_argument("--registry", help="registre à lire (défaut : brain/registry/entries.jsonl du projet)")
    ap.add_argument("--limit", type=int, default=15, help="éléments affichés par type (défaut 15)")
    ap.add_argument("--json", action="store_true")
    sys.stdout.reconfigure(encoding="utf-8")
    sys.stderr.reconfigure(encoding="utf-8")
    args = ap.parse_args()
    if args.limit < 0:
        sys.stderr.write("--limit doit être positif\n")
        return 2
    path = Path(args.registry) if args.registry else Path(args.project) / DEFAULT_REGISTRY
    if not path.is_file():
        sys.stderr.write("registre introuvable : %s\n" % path)
        return 2
    entries, invalid, non_empty = read_registry(path)
    if entries is None or (non_empty > 0 and not entries):
        sys.stderr.write("refus : %s n'est pas un JSONL de registre\n" % path)
        return 3
    rep = Report()
    stats = analyse(entries, rep)
    out = {"ok": True, "registre": str(path), "entrees": len(entries), "methode": METHODE, "limites": LIMITES,
           "seuils": {"jaccard_min": JACCARD_MIN, "ratio_difflib_min": RATIO_MIN, "mots_min": MIN_TOKENS},
           "lignes_invalides": invalid, "graphe": stats, "sections": summarize(rep)}
    out["compteurs"] = {k: v["total"] for k, v in out["sections"].items()}
    sys.stdout.write((json.dumps(out, ensure_ascii=False, indent=2) if args.json else as_text(out, args.limit)) + "\n")
    return 0


if __name__ == "__main__":
    sys.exit(main())
