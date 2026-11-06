#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""Validateur déterministe du registre Brain (lecture seule, mécanique uniquement).

Contrat schéma : 0.1.5 (rétrocompat 0.1.x)
- Hors --require-intention : rétrocompat (intention/position/locators/refs optionnels).
- Avec --require-intention : générations neuves ≥0.1.3/0.1.4 (vide = ERROR ; champs exigés ;
  origin+locators ; mapping intention→type ; refs structurels si présents).

Couche 1 seulement (validation technique). Ne juge pas la fidélité sémantique.
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from datetime import datetime
from pathlib import Path
from typing import Any

ALLOWED_TYPES = {
    "idea",
    "observation",
    "proposition",
    "decision",
    "convention",
    "task",
    "open_question",
    "event",
}
ALLOWED_INTENTION = {
    "idea",
    "suggestion",
    "preference",
    "orientation",
    "question",
    "decision",
    "requirement",
    "convention",
    "task",
    "milestone",
    "observation",
    "risk",
    "feedback",
}
ALLOWED_POSITION = {
    "proposed",
    "exploring",
    "favorable",
    "endorsed",
    "opposed",
    "selected",
    "confirmed",
    "uncertain",
    None,
}
ALLOWED_STATUS = {"extracted"}
ALLOWED_SPACES = {"atelier", "referentiel", "journal", "pilotage"}
ALLOWED_VALIDATION = {"none", "implicit", "explicit_user_approval"}
ALLOWED_AUTHOR = {"user", "assistant"}
ALLOWED_APPROVED_BY = {"user", None}
ALLOWED_APPROVAL_SCOPE = {"item", "set", None}
ALLOWED_CONFIDENCE = {"high", "medium", "low"}
ALLOWED_AUTHOR_CONFIDENCE = {"high", "medium", "low", "uncertain"}
ALLOWED_GROUNDING = {
    "source_explicit",
    "source_contextual",
    "source_interpreted",
    "uncertain",
}
ALLOWED_LOCATOR_KIND = {"lines", "window", "offset"}
ALLOWED_LOCATOR_ROLE = {"user", "assistant", "unknown"}
ALLOWED_REL = {
    "related",
    "evolves_from",
    "evolves_to",
    "answers",
    "answered_by",
    "conflicts_with",
    "resolves",
    "supersedes_candidate",
    "parent_of",
    "child_of",
    "clarifies",
    "clarified_by",
    "contradicts",
    "contradicted_by",
}
FORBIDDEN_REL = {"feeds_doc", "refers_clickup"}
REQUIRED_FIELDS = {
    "id",
    "type",
    "status",
    "space",
    "validation",
    "author",
    "approved_by",
    "approval_scope",
    "title",
    "content",
    "evidence",
    "confidence",
    "origin",
    "project",
    "created_at",
    "storage",
}
KNOWN_FIELDS = REQUIRED_FIELDS | {
    "intention",
    "position",
    "author_confidence",
    "grounding",
    "run_id",
    "capture_ref",
    "uid",
    "relations",
    "clickup_ref",
    "feeds_doc",
    "tags",
    "refs",
}
ID_RE = re.compile(r"^CAP-\d{3,}$")
CAP_TARGET_RE = re.compile(r"^CAP-\d{3,}$")
URL_IN_TEXT_RE = re.compile(r"https?://[^\s\]\)\"<>]+", re.I)
LIEN_FOURNE_RE = re.compile(
    r"lien\s+fourn|link\s+provid|url\s+clickup(?!\s*:?\s*https?://)", re.I
)
EXPRESS_NOT_APPROVE_TYPES = {"observation", "event", "idea"}
META_NOISE_TAGS = {"test-noise", "meta-test", "simulation"}
AMBIGUOUS_TAGS = {"ambiguous", "ambiguous_orientation", "ambiguous-approval"}
ATELIER_DEFAULT_INTENTIONS = {
    "idea",
    "suggestion",
    "orientation",
    "question",
    "preference",
    "feedback",
}
# Mapping déterministe intention → type (taxonomy 0.1.4)
INTENTION_TO_TYPE = {
    "idea": "idea",
    "suggestion": "proposition",
    "preference": "observation",
    "orientation": "proposition",
    "question": "open_question",
    "decision": "decision",
    "requirement": "observation",
    "convention": "convention",
    "task": "task",
    "milestone": "task",
    "observation": "observation",
    "risk": "observation",
    "feedback": "observation",
}


def is_iso_datetime(value: str) -> bool:
    if not isinstance(value, str) or not value:
        return False
    candidates = [value]
    if value.endswith("Z"):
        candidates.append(value[:-1] + "+00:00")
    for c in candidates:
        try:
            datetime.fromisoformat(c)
            return True
        except ValueError:
            continue
    return False


class Report:
    def __init__(self) -> None:
        self.errors: list[str] = []
        self.warnings: list[str] = []
        self.human_review: list[str] = []

    def error(self, msg: str) -> None:
        self.errors.append(msg)

    def warn(self, msg: str) -> None:
        self.warnings.append(msg)

    def review(self, msg: str) -> None:
        self.human_review.append(msg)

    @property
    def ok(self) -> bool:
        return not self.errors

    def to_dict(self) -> dict[str, Any]:
        return {
            "ok": self.ok,
            "schema_contract": "0.1.4",
            "layer": "technical_validation",
            "errors": self.errors,
            "warnings": self.warnings,
            "human_review": self.human_review,
        }


def load_entries(
    path: Path, report: Report, require_intention: bool
) -> list[dict[str, Any]]:
    entries: list[dict[str, Any]] = []
    text = path.read_text(encoding="utf-8")
    if not text.strip():
        if require_intention:
            report.error(
                "Fichier vide (0 entrée) — interdit sous --require-intention (génération 0.1.3)."
            )
        else:
            report.warn("Fichier vide.")
        return entries
    for i, line in enumerate(text.splitlines(), 1):
        if not line.strip():
            report.warn(f"Ligne {i}: ligne vide ignorée.")
            continue
        try:
            obj = json.loads(line)
        except json.JSONDecodeError as exc:
            report.error(f"Ligne {i}: JSON invalide ({exc}).")
            continue
        if not isinstance(obj, dict):
            report.error(f"Ligne {i}: l'entrée doit être un objet JSON.")
            continue
        entries.append(obj)
    if require_intention and not entries and not report.errors:
        report.error(
            "0 entrée valide — interdit sous --require-intention (génération 0.1.3)."
        )
    return entries


def _is_int(value: Any) -> bool:
    return isinstance(value, int) and not isinstance(value, bool)


def _in_allowed(value: Any, allowed: set[Any]) -> bool:
    """Membership safe for JSON malformé (objet/liste non hashable → False, pas d'exception)."""
    try:
        return value in allowed
    except TypeError:
        return False


def validate_locator(eid: str, idx: int, loc: Any, report: Report) -> None:
    prefix = f"{eid}: origin.locators[{idx}]"
    if not isinstance(loc, dict):
        report.error(f"{prefix}: doit être un objet.")
        return
    kind = loc.get("kind")
    if not _in_allowed(kind, ALLOWED_LOCATOR_KIND):
        report.error(f"{prefix}: kind={kind!r} non autorisé (lines|window|offset).")
        return
    if kind == "lines":
        # 1-based inclusive ; start/end entiers positifs ; end >= start
        start, end = loc.get("start"), loc.get("end")
        start_ok = _is_int(start)
        end_ok = _is_int(end)
        if not start_ok:
            report.error(f"{prefix}: start (int) obligatoire pour kind=lines.")
        elif start < 1:
            report.error(f"{prefix}: start ({start}) doit être >= 1 (lines, 1-based).")
        if not end_ok:
            report.error(f"{prefix}: end (int) obligatoire pour kind=lines.")
        elif end < 1:
            report.error(f"{prefix}: end ({end}) doit être >= 1 (lines, 1-based).")
        if start_ok and end_ok and start >= 1 and end >= 1 and end < start:
            report.error(f"{prefix}: end ({end}) < start ({start}).")
    elif kind == "offset":
        # positions Unicode ; 0-based ; start inclusif, end exclusif ; start >= 0 ; end >= start
        start, end = loc.get("start"), loc.get("end")
        start_ok = _is_int(start)
        end_ok = _is_int(end)
        if not start_ok:
            report.error(f"{prefix}: start (int) obligatoire pour kind=offset.")
        elif start < 0:
            report.error(f"{prefix}: start ({start}) doit être >= 0 (offset, 0-based).")
        if not end_ok:
            report.error(f"{prefix}: end (int) obligatoire pour kind=offset.")
        elif end < 0:
            report.error(f"{prefix}: end ({end}) doit être >= 0 (offset, 0-based).")
        if start_ok and end_ok and start >= 0 and end >= 0 and end < start:
            report.error(f"{prefix}: end ({end}) < start ({start}).")
    elif kind == "window":
        wid = loc.get("window_id")
        if not isinstance(wid, str) or not wid.strip():
            report.error(
                f"{prefix}: window_id (string non vide) obligatoire pour kind=window."
            )
    role = loc.get("role")
    if role is not None and not _in_allowed(role, ALLOWED_LOCATOR_ROLE):
        report.error(f"{prefix}: role={role!r} non autorisé (user|assistant|unknown).")
    note = loc.get("note")
    if note is not None and not isinstance(note, str):
        report.error(f"{prefix}: note doit être string ou absent.")


def validate_origin(
    eid: str, origin: Any, report: Report, require_intention: bool
) -> None:
    """Valide origin / locators.

    Avec --require-intention : origin objet, source non vide, locators[] non vide
    et chaque locator schema-valide ; null / type incorrect / incomplet = ERROR.
    Sans flag : rétrocompat (locators optionnels ; origin null/non-objet = ERROR
    car champ obligatoire).
    """
    if not isinstance(origin, dict):
        report.error(
            f"{eid}: origin doit être un objet JSON"
            f"{' (obligatoire en génération 0.1.3, --require-intention)' if require_intention else ''}."
        )
        return

    source = origin.get("source")
    if require_intention:
        if not isinstance(source, str) or not source.strip():
            report.error(
                f"{eid}: origin.source (string non vide) obligatoire "
                f"en génération 0.1.3 (--require-intention)."
            )
    else:
        if "source" not in origin or not isinstance(source, str):
            report.error(f"{eid}: origin.source (string) obligatoire.")

    if "url" in origin and origin["url"] is not None and not isinstance(
        origin["url"], str
    ):
        report.error(f"{eid}: origin.url doit être string ou absent.")
    if "locator" in origin:
        report.error(
            f"{eid}: origin.locator (singulier) interdit — utiliser origin.locators[]."
        )

    locators = origin.get("locators")
    if require_intention:
        if not isinstance(locators, list) or len(locators) == 0:
            report.error(
                f"{eid}: origin.locators (tableau non vide) obligatoire "
                f"en génération 0.1.3 (--require-intention)."
            )
        else:
            for i, loc in enumerate(locators):
                validate_locator(eid, i, loc, report)
    elif locators is not None:
        if not isinstance(locators, list):
            report.error(f"{eid}: origin.locators doit être une liste.")
        else:
            for i, loc in enumerate(locators):
                validate_locator(eid, i, loc, report)


def validate_entry(
    entry: dict[str, Any],
    line_no: int,
    ids: set[str],
    report: Report,
    require_intention: bool,
) -> None:
    eid = entry.get("id", f"<ligne {line_no}>")

    unknown = set(entry.keys()) - KNOWN_FIELDS
    if unknown:
        report.error(f"{eid}: champs inconnus: {sorted(unknown)}")

    for field in REQUIRED_FIELDS:
        if field not in entry:
            report.error(f"{eid}: champ obligatoire manquant: {field}")

    if require_intention:
        for field in (
            "intention",
            "position",
            "author_confidence",
            "grounding",
            "run_id",
            "capture_ref",
        ):
            if field not in entry:
                report.error(
                    f"{eid}: {field} obligatoire en génération 0.1.3 (--require-intention)."
                )

    if "id" in entry:
        if not isinstance(entry["id"], str) or not ID_RE.match(entry["id"]):
            report.error(f"{eid}: id doit matcher CAP-NNN.")
        elif entry["id"] in ids:
            report.error(f"{eid}: id dupliqué.")
        else:
            ids.add(entry["id"])

    def check_enum(field: str, allowed: set[Any]) -> None:
        if field not in entry:
            return
        if not _in_allowed(entry[field], allowed):
            report.error(f"{eid}: {field}={entry[field]!r} non autorisé.")

    check_enum("type", ALLOWED_TYPES)
    check_enum("status", ALLOWED_STATUS)
    check_enum("space", ALLOWED_SPACES)
    check_enum("validation", ALLOWED_VALIDATION)
    check_enum("author", ALLOWED_AUTHOR)
    check_enum("confidence", ALLOWED_CONFIDENCE)
    check_enum("intention", ALLOWED_INTENTION)
    check_enum("author_confidence", ALLOWED_AUTHOR_CONFIDENCE)
    check_enum("grounding", ALLOWED_GROUNDING)
    if "position" in entry and not _in_allowed(entry["position"], ALLOWED_POSITION):
        report.error(f"{eid}: position={entry['position']!r} non autorisé.")

    if "approved_by" in entry and not _in_allowed(
        entry["approved_by"], ALLOWED_APPROVED_BY
    ):
        report.error(f"{eid}: approved_by={entry['approved_by']!r} non autorisé (user|null).")
    if "approval_scope" in entry and not _in_allowed(
        entry["approval_scope"], ALLOWED_APPROVAL_SCOPE
    ):
        report.error(
            f"{eid}: approval_scope={entry['approval_scope']!r} non autorisé (item|set|null)."
        )

    if "run_id" in entry:
        rid = entry["run_id"]
        if not isinstance(rid, str) or not rid.strip():
            report.error(f"{eid}: run_id doit être une string non vide.")

    if "capture_ref" in entry:
        cref = entry["capture_ref"]
        if not isinstance(cref, str) or not cref.strip():
            report.error(f"{eid}: capture_ref doit être une string non vide.")
        else:
            rid = entry.get("run_id")
            iid = entry.get("id")
            if isinstance(rid, str) and isinstance(iid, str):
                expected = f"{rid}:{iid}"
                if cref != expected:
                    report.error(
                        f"{eid}: capture_ref={cref!r} ≠ attendu {expected!r}."
                    )

    if "uid" in entry and entry["uid"] is not None and not isinstance(entry["uid"], str):
        report.error(f"{eid}: uid doit être string ou absent.")

    # refs[] — littéraux URL/chemin/id (0.1.4)
    if "refs" in entry and entry["refs"] is not None:
        refs = entry["refs"]
        if not isinstance(refs, list):
            report.error(f"{eid}: refs doit être une liste ou absent.")
        else:
            for i, ref in enumerate(refs):
                if not isinstance(ref, str) or not ref.strip():
                    report.error(
                        f"{eid}: refs[{i}] doit être une string non vide (littéral exact)."
                    )

    for text_field in ("title", "content", "evidence", "project", "storage"):
        if text_field in entry and not isinstance(entry[text_field], str):
            report.error(f"{eid}: {text_field} doit être une string.")
        elif text_field in entry and not entry[text_field].strip():
            report.error(f"{eid}: {text_field} ne doit pas être vide.")

    content = entry.get("content")
    if isinstance(content, str) and 0 < len(content.strip()) < 80:
        report.review(
            f"{eid}: content court ({len(content.strip())} car.) — vérifier l'autonomie."
        )

    if entry.get("status") != "extracted":
        report.error(f"{eid}: status doit être 'extracted' pour une capture.")

    if "created_at" in entry and not is_iso_datetime(entry["created_at"]):
        report.error(f"{eid}: created_at n'est pas une date ISO valide.")

    if "origin" in entry:
        validate_origin(eid, entry.get("origin"), report, require_intention)

    validation = entry.get("validation")
    approved_by = entry.get("approved_by")
    approval_scope = entry.get("approval_scope")
    position = entry.get("position") if "position" in entry else None
    intention = entry.get("intention")

    if validation == "explicit_user_approval":
        if approved_by != "user":
            report.error(
                f"{eid}: validation=explicit_user_approval exige approved_by=user "
                f"(reçu {approved_by!r})."
            )
        if approval_scope not in ("item", "set"):
            report.error(
                f"{eid}: validation=explicit_user_approval exige "
                f"approval_scope item|set (reçu {approval_scope!r})."
            )
    if validation in ("none", "implicit"):
        if approved_by not in (None,):
            report.error(
                f"{eid}: validation={validation} exige approved_by=null (reçu {approved_by!r})."
            )
        if approval_scope is not None:
            report.error(
                f"{eid}: validation={validation} exige approval_scope=null "
                f"(reçu {approval_scope!r})."
            )
    if approved_by == "user" and approval_scope not in ("item", "set"):
        report.error(
            f"{eid}: approved_by=user exige approval_scope item|set "
            f"(reçu {approval_scope!r})."
        )
    if validation == "implicit":
        report.review(
            f"{eid}: validation=implicit (historique) — réexaminer la preuve avant conversion."
        )

    if position in ("selected", "confirmed") and validation == "explicit_user_approval":
        report.review(
            f"{eid}: position={position} + explicit — vérifier que l'approbation formelle "
            f"est distincte de la seule posture."
        )

    # Soft REVIEW: orientation ambiguë / partial + explicit (anti-faux-explicit)
    raw_tags = entry.get("tags") if isinstance(entry.get("tags"), list) else []
    tags = [t for t in raw_tags if isinstance(t, str)]
    if any(not isinstance(t, str) for t in raw_tags):
        report.error(f"{eid}: tags[] doit contenir uniquement des strings.")
    if intention == "orientation" and validation == "explicit_user_approval":
        report.review(
            f"{eid}: intention=orientation + explicit — vérifier anti-faux-explicit "
            f"(orientation souvent non tranchée → none)."
        )
    if any(t in AMBIGUOUS_TAGS for t in tags) and validation == "explicit_user_approval":
        report.review(
            f"{eid}: tag ambigu + explicit_user_approval — suspect anti-faux-explicit."
        )
    if "approval_global_partial" in tags and validation == "explicit_user_approval":
        report.review(
            f"{eid}: approval_global_partial + explicit — partial doit rester validation none."
        )
    if (
        entry.get("grounding") == "source_explicit"
        and validation == "explicit_user_approval"
        and intention in ("orientation", "suggestion", "idea")
    ):
        report.review(
            f"{eid}: grounding=source_explicit + explicit sur intention={intention} — "
            f"vérifier que l'approbation formelle est bien dans la source."
        )

    etype = entry.get("type")
    author = entry.get("author")

    # Mapping intention → type (ERROR sous --require-intention pour générations neuves)
    if (
        require_intention
        and isinstance(intention, str)
        and intention in INTENTION_TO_TYPE
        and isinstance(etype, str)
    ):
        expected_type = INTENTION_TO_TYPE[intention]
        if etype != expected_type:
            report.error(
                f"{eid}: mapping intention→type : intention={intention!r} exige "
                f"type={expected_type!r} (reçu {etype!r})."
            )
    elif (
        isinstance(intention, str)
        and intention in INTENTION_TO_TYPE
        and isinstance(etype, str)
        and etype != INTENTION_TO_TYPE[intention]
    ):
        report.warn(
            f"{eid}: intention={intention!r} / type={etype!r} hors mapping 0.1.4 "
            f"(attendu {INTENTION_TO_TYPE[intention]!r}) — historique toléré hors "
            f"--require-intention."
        )

    if (
        author == "user"
        and etype in EXPRESS_NOT_APPROVE_TYPES
        and validation == "explicit_user_approval"
    ):
        report.warn(
            f"{eid}: type={etype} + author=user avec explicit_user_approval "
            f"(exprimer ≠ approuver — vérifier)."
        )

    # Heuristique mécanique : périphrase de lien sans URL littérale
    blob = " ".join(
        x
        for x in (
            content if isinstance(content, str) else "",
            entry.get("evidence") if isinstance(entry.get("evidence"), str) else "",
            entry.get("title") if isinstance(entry.get("title"), str) else "",
        )
        if x
    )
    refs_list = entry.get("refs") if isinstance(entry.get("refs"), list) else []
    refs_text = " ".join(r for r in refs_list if isinstance(r, str))
    has_url = bool(URL_IN_TEXT_RE.search(blob) or URL_IN_TEXT_RE.search(refs_text))
    if LIEN_FOURNE_RE.search(blob) and not has_url:
        report.review(
            f"{eid}: formulation de lien sans URL littérale dans content/evidence/refs "
            f"— vérifier préservation URL (0.1.4)."
        )
    if has_url:
        missing_refs = not isinstance(entry.get("refs"), list)
        for m in URL_IN_TEXT_RE.findall(blob):
            url = m.rstrip(".,);]")
            if url and (missing_refs or url not in refs_text):
                report.review(
                    f"{eid}: URL présente dans le texte mais absente de refs[] — "
                    f"ajouter le littéral exact dans refs."
                )
                break

    if etype == "decision" and isinstance(content, str):
        low = content.lower()
        if any(
            x in low
            for x in (
                "préfère",
                "prefere",
                "b ou c",
                "pourrait",
                "peut-être",
                "peut etre",
            )
        ):
            if validation == "explicit_user_approval":
                report.review(
                    f"{eid}: decision+explicit avec formulation préférentielle/ambiguë — vérifier."
                )
            if require_intention and intention in (
                "orientation",
                "suggestion",
                "preference",
            ):
                report.error(
                    f"{eid}: intention={intention} incompatible avec type=decision "
                    f"(mapping 0.1.4)."
                )

    if isinstance(tags, list):
        if "approval_global_partial" in tags:
            report.review(
                f"{eid}: tag approval_global_partial — accord global/partiel ; "
                f"vérifier qu'aucune approbation item/set n'a été forcée."
            )
        if any(t in META_NOISE_TAGS for t in tags):
            report.review(f"{eid}: tag bruit/méta de test — vérifier exclusion métier.")

    if etype == "task" and entry.get("space") not in ("pilotage", "atelier"):
        report.warn(f"{eid}: type=task habituellement dans pilotage ou atelier.")
    if intention in ("task", "milestone") and entry.get("space") not in (
        "pilotage",
        "atelier",
    ):
        report.warn(
            f"{eid}: intention={intention} habituellement dans pilotage ou atelier."
        )
    if entry.get("space") == "referentiel" and validation != "explicit_user_approval":
        report.review(
            f"{eid}: space=referentiel sans explicit_user_approval — vérifier le placement."
        )
    if (
        entry.get("space") == "referentiel"
        and intention in ATELIER_DEFAULT_INTENTIONS
        and validation == "none"
    ):
        report.review(
            f"{eid}: intention={intention} en space=referentiel avec validation=none — "
            f"préférer atelier (0.1.4)."
        )

    if "relations" in entry and entry["relations"] is not None and not isinstance(
        entry["relations"], list
    ):
        report.error(f"{eid}: relations doit être une liste.")
    if "tags" in entry and entry["tags"] is not None and not isinstance(entry["tags"], list):
        report.error(f"{eid}: tags doit être une liste.")
    if "clickup_ref" in entry and entry["clickup_ref"] is not None and not isinstance(
        entry["clickup_ref"], str
    ):
        report.error(f"{eid}: clickup_ref doit être string ou null.")
    if "feeds_doc" in entry and entry["feeds_doc"] is not None and not isinstance(
        entry["feeds_doc"], str
    ):
        report.error(f"{eid}: feeds_doc doit être string ou null.")


def _pair_key(a: str, b: str) -> tuple[str, str]:
    return (a, b) if a <= b else (b, a)


def validate_relations(entries: list[dict[str, Any]], report: Report) -> None:
    by_id = {e["id"]: e for e in entries if isinstance(e.get("id"), str)}
    all_ids = set(by_id)
    evolves_pairs: set[tuple[str, str]] = set()
    supersedes_pairs: set[tuple[str, str]] = set()
    resolves_edges: set[tuple[str, str]] = set()
    conflicts_edges: set[tuple[str, str]] = set()

    for entry in entries:
        eid = entry.get("id", "?")
        relations = entry.get("relations") or []
        if not isinstance(relations, list):
            continue
        for idx, rel in enumerate(relations):
            if not isinstance(rel, dict):
                report.error(f"{eid}: relations[{idx}] doit être un objet.")
                continue
            r = rel.get("rel")
            to = rel.get("to")
            if _in_allowed(r, FORBIDDEN_REL):
                report.error(
                    f"{eid}: rel={r!r} interdit dans relations "
                    f"(utiliser feeds_doc / clickup_ref)."
                )
            elif not _in_allowed(r, ALLOWED_REL):
                report.error(f"{eid}: rel={r!r} non autorisé.")
            if not isinstance(to, str):
                report.error(f"{eid}: relations[{idx}].to doit être une string.")
                continue
            if not isinstance(eid, str):
                continue
            if not CAP_TARGET_RE.match(to):
                report.error(
                    f"{eid}: cible de relation non-capture {to!r} "
                    f"(documents/ClickUp interdits dans relations)."
                )
            elif to not in all_ids:
                report.error(f"{eid}: relation vers capture inexistante {to}.")
            else:
                if r in ("evolves_from", "evolves_to"):
                    evolves_pairs.add(_pair_key(eid, to))
                if r == "supersedes_candidate":
                    supersedes_pairs.add(_pair_key(eid, to))
                if r == "resolves":
                    resolves_edges.add((eid, to))
                if r == "conflicts_with":
                    conflicts_edges.add((eid, to))

    for pair in evolves_pairs & supersedes_pairs:
        report.error(
            f"{pair[0]}↔{pair[1]}: evolves_* et supersedes_candidate sur la même paire "
            f"(choisir une seule famille)."
        )

    for src, dst in resolves_edges:
        if (src, dst) in conflicts_edges or (dst, src) in conflicts_edges:
            report.warn(
                f"{src}: resolves → {dst} alors qu'un conflicts_with existe sur la paire "
                f"(préférer resolves seul si la tension est close)."
            )

    def has_rel(src: str, rel: str, dst: str) -> bool:
        e = by_id.get(src)
        if not e:
            return False
        for r in e.get("relations") or []:
            if isinstance(r, dict) and r.get("rel") == rel and r.get("to") == dst:
                return True
        return False

    pairs = [
        ("answered_by", "answers"),
        ("answers", "answered_by"),
        ("evolves_from", "evolves_to"),
        ("evolves_to", "evolves_from"),
        ("parent_of", "child_of"),
        ("child_of", "parent_of"),
        ("clarifies", "clarified_by"),
        ("clarified_by", "clarifies"),
        ("contradicts", "contradicted_by"),
        ("contradicted_by", "contradicts"),
    ]
    seen: set[tuple[str, str, str]] = set()
    for entry in entries:
        eid = entry.get("id")
        if not eid:
            continue
        for r in entry.get("relations") or []:
            if not isinstance(r, dict):
                continue
            rel, to = r.get("rel"), r.get("to")
            if rel == "conflicts_with" and isinstance(to, str) and to in all_ids:
                if not has_rel(to, "conflicts_with", eid):
                    report.warn(
                        f"{eid}: conflicts_with → {to} sans réciproque "
                        f"(recommandé mais non bloquant)."
                    )
            for a, b in pairs:
                if rel == a and isinstance(to, str) and to in all_ids:
                    key = (min(eid, to), max(eid, to), a)
                    if key in seen:
                        continue
                    seen.add(key)
                    if not has_rel(to, b, eid):
                        report.warn(
                            f"{eid}: {a} → {to} sans réciproque {b} "
                            f"(recommandé si les deux côtés sont connus)."
                        )


def validate_registry(path: Path, require_intention: bool = False) -> Report:
    report = Report()
    if not path.exists():
        report.error(f"Fichier introuvable: {path}")
        return report
    entries = load_entries(path, report, require_intention)
    ids: set[str] = set()
    for i, entry in enumerate(entries, 1):
        validate_entry(entry, i, ids, report, require_intention)
    validate_relations(entries, report)
    report.warn(
        f"Résumé: {len(entries)} entrées, {len(ids)} ids uniques"
        f"{' [require-intention]' if require_intention else ''}."
    )
    return report


def print_report(report: Report, as_json: bool) -> None:
    if as_json:
        print(json.dumps(report.to_dict(), ensure_ascii=False, indent=2))
        return
    print("=== Validateur registre Brain (lecture seule, mécanique) ===")
    print(f"Contrat: 0.1.4 | Couche: validation technique uniquement")
    print(f"OK bloquant: {'oui' if report.ok else 'non'}")
    print(f"\nErreurs bloquantes ({len(report.errors)}):")
    for m in report.errors:
        print(f"  [ERROR] {m}")
    if not report.errors:
        print("  (aucune)")
    print(f"\nAvertissements ({len(report.warnings)}):")
    for m in report.warnings:
        print(f"  [WARN] {m}")
    if not report.warnings:
        print("  (aucun)")
    print(f"\nVérification humaine ({len(report.human_review)}):")
    for m in report.human_review:
        print(f"  [REVIEW] {m}")
    if not report.human_review:
        print("  (aucun)")


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description=(
            "Valide entries.jsonl sans modification (mode lecture seule, mécanique). "
            "OK technique ≠ qualité sémantique."
        )
    )
    parser.add_argument(
        "path",
        nargs="?",
        default="brain/registry/entries.jsonl",
        help="Chemin vers entries.jsonl",
    )
    parser.add_argument("--json", action="store_true", help="Sortie JSON sur stdout")
    parser.add_argument(
        "--json-out",
        metavar="FILE",
        help="Écrit le rapport JSON dans FILE (artefact validate.json)",
    )
    parser.add_argument(
        "--require-intention",
        action="store_true",
        help=(
            "Exige champs ≥0.1.3/0.1.4 (intention, position, author_confidence, grounding, "
            "run_id, capture_ref) ; origin+locators ; mapping intention→type ; "
            "fichier vide = ERROR"
        ),
    )
    args = parser.parse_args(argv)
    report = validate_registry(Path(args.path), require_intention=args.require_intention)
    if args.json_out:
        out_path = Path(args.json_out)
        out_path.parent.mkdir(parents=True, exist_ok=True)
        out_path.write_text(
            json.dumps(report.to_dict(), ensure_ascii=False, indent=2) + "\n",
            encoding="utf-8",
        )
    print_report(report, args.json)
    return 0 if report.ok else 1


if __name__ == "__main__":
    for _stream in (sys.stdout, sys.stderr):
        if hasattr(_stream, "reconfigure"):
            _stream.reconfigure(encoding="utf-8", errors="backslashreplace")
    sys.exit(main())
