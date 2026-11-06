#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""Tests techniques Brain Capture 0.1.4 (mécaniques uniquement).

N'exécute aucune capture. Ne fabrique pas de résultats métier.
"""

from __future__ import annotations

import json
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]  # repo root (brain-main/skills/brain-capture/scripts -> +3)
# parents: scripts=0, brain-capture=1, skills=2, brain-main=3 — wrong
# Path: repo/brain-main/skills/brain-capture/scripts/this.py
# parents[0]=scripts, [1]=brain-capture, [2]=skills, [3]=brain-main, [4]=repo
ROOT = Path(__file__).resolve().parents[4]
VALIDATOR = Path(__file__).resolve().parent / "validate-registry.py"
FIXTURES = Path(__file__).resolve().parent / "fixtures"


def base_entry(**overrides):
    e = {
        "id": "CAP-001",
        "run_id": "fixture",
        "capture_ref": "fixture:CAP-001",
        "type": "proposition",
        "intention": "orientation",
        "position": "selected",
        "status": "extracted",
        "space": "atelier",
        "validation": "none",
        "author": "user",
        "author_confidence": "high",
        "approved_by": None,
        "approval_scope": None,
        "title": "Orientation B ou C",
        "content": "Mode quotidien B ou C, non tranché définitivement.",
        "evidence": "B ou C",
        "confidence": "high",
        "grounding": "source_explicit",
        "origin": {
            "source": "fixtures/sample.txt",
            "url": None,
            "locators": [{"kind": "lines", "start": 1, "end": 2, "role": "user"}],
        },
        "project": "2brain",
        "created_at": "2026-10-31T00:00:00Z",
        "storage": "fixtures/entries.jsonl",
        "relations": [],
        "clickup_ref": None,
        "feeds_doc": None,
        "tags": ["ambiguous"],
        "refs": [],
    }
    e.update(overrides)
    if "id" in overrides or "run_id" in overrides:
        e["capture_ref"] = f"{e['run_id']}:{e['id']}"
    return e


def write_jsonl(path: Path, entries: list[dict]) -> None:
    path.write_text(
        "".join(json.dumps(x, ensure_ascii=False) + "\n" for x in entries),
        encoding="utf-8",
    )


def run_validator(path: Path, require: bool = True) -> tuple[int, str]:
    cmd = [sys.executable, str(VALIDATOR), str(path)]
    if require:
        cmd.append("--require-intention")
    proc = subprocess.run(cmd, capture_output=True, text=True, encoding="utf-8")
    return proc.returncode, proc.stdout + proc.stderr


def main() -> int:
    FIXTURES.mkdir(parents=True, exist_ok=True)
    results: list[tuple[str, bool, str]] = []

    with tempfile.TemporaryDirectory() as td:
        tdir = Path(td)

        # 1. mapping OK
        p = tdir / "map_ok.jsonl"
        write_jsonl(p, [base_entry()])
        code, out = run_validator(p)
        results.append(("mapping_ok", code == 0, out))

        # 2. orientation must not be decision
        p = tdir / "map_bad.jsonl"
        write_jsonl(p, [base_entry(type="decision", intention="orientation")])
        code, out = run_validator(p)
        results.append(
            (
                "mapping_orientation_not_decision",
                code != 0 and "mapping intention→type" in out,
                out,
            )
        )

        # 3. suggestion not decision
        p = tdir / "sug_bad.jsonl"
        write_jsonl(
            p,
            [
                base_entry(
                    type="decision",
                    intention="suggestion",
                    title="Suggestion",
                    content="Proposition assistant non tranchée.",
                    position="proposed",
                    author="assistant",
                    tags=[],
                )
            ],
        )
        code, out = run_validator(p)
        results.append(
            ("mapping_suggestion_not_decision", code != 0 and "mapping" in out, out)
        )

        # 4. question → open_question
        p = tdir / "q_ok.jsonl"
        write_jsonl(
            p,
            [
                base_entry(
                    type="open_question",
                    intention="question",
                    title="Question init",
                    content="brain init une fois ou par session ?",
                    position="null",
                    tags=[],
                )
            ],
        )
        # position null as JSON null
        entries = [
            base_entry(
                type="open_question",
                intention="question",
                title="Question init",
                content="brain init une fois ou par session ?",
                position=None,
                tags=[],
            )
        ]
        write_jsonl(p, entries)
        code, out = run_validator(p)
        results.append(("mapping_question_ok", code == 0, out))

        # 5. position selected + validation none OK
        p = tdir / "pos_val.jsonl"
        write_jsonl(
            p,
            [
                base_entry(
                    position="selected",
                    validation="none",
                    approved_by=None,
                    approval_scope=None,
                )
            ],
        )
        code, out = run_validator(p)
        results.append(("position_selected_without_explicit", code == 0, out))

        # 6. space atelier for orientation
        p = tdir / "space_ok.jsonl"
        write_jsonl(p, [base_entry(space="atelier")])
        code, out = run_validator(p)
        ok = code == 0 and "préférer atelier" not in out
        results.append(("space_atelier_orientation", ok, out))

        # 7. space referentiel + orientation → REVIEW (still OK exit)
        p = tdir / "space_rev.jsonl"
        write_jsonl(p, [base_entry(space="referentiel")])
        code, out = run_validator(p)
        results.append(
            (
                "space_referentiel_review",
                code == 0 and "préférer atelier" in out,
                out,
            )
        )

        # 8. multi URLs in refs
        url1 = "https://example.com/a?x=1&y=2#frag"
        url2 = "https://app.clickup.com/90151633729/v/f/1201110000005979/90156746497"
        p = tdir / "urls_ok.jsonl"
        write_jsonl(
            p,
            [
                base_entry(
                    title="Folder ClickUp",
                    content=f"Dossier : {url1} et {url2}",
                    evidence=url2,
                    refs=[url1, url2],
                    intention="decision",
                    type="decision",
                    position="confirmed",
                    validation="explicit_user_approval",
                    approved_by="user",
                    approval_scope="item",
                    space="pilotage",
                    tags=[],
                )
            ],
        )
        code, out = run_validator(p)
        results.append(("refs_multi_urls_ok", code == 0, out))

        # 9. lien fourni without URL → REVIEW
        p = tdir / "lien_fourni.jsonl"
        write_jsonl(
            p,
            [
                base_entry(
                    content="Lien fourni vers le folder ClickUp ; ne rien ajouter.",
                    refs=[],
                    intention="decision",
                    type="decision",
                    validation="explicit_user_approval",
                    approved_by="user",
                    approval_scope="item",
                    space="pilotage",
                    tags=[],
                )
            ],
        )
        code, out = run_validator(p)
        results.append(
            (
                "lien_fourni_review",
                code == 0 and "sans URL littérale" in out,
                out,
            )
        )

        # 10. refs bad type
        p = tdir / "refs_bad.jsonl"
        write_jsonl(p, [base_entry(refs="https://x.com")])  # type: ignore
        code, out = run_validator(p)
        results.append(("refs_must_be_list", code != 0 and "refs doit être une liste" in out, out))

        # 11. relations answers reciprocity warn
        p = tdir / "rel.jsonl"
        write_jsonl(
            p,
            [
                base_entry(
                    id="CAP-001",
                    intention="question",
                    type="open_question",
                    title="Q",
                    content="Question ouverte sur init ?",
                    position=None,
                    tags=[],
                    relations=[],
                ),
                base_entry(
                    id="CAP-002",
                    intention="orientation",
                    type="proposition",
                    title="R",
                    content="Réponse orientation.",
                    relations=[{"rel": "answers", "to": "CAP-001"}],
                    tags=[],
                ),
            ],
        )
        code, out = run_validator(p)
        results.append(
            (
                "relations_answers_reciprocity_warn",
                code == 0 and "sans réciproque" in out,
                out,
            )
        )

        # 12. empty file require-intention ERROR
        p = tdir / "empty.jsonl"
        p.write_text("", encoding="utf-8")
        code, out = run_validator(p, require=True)
        results.append(("empty_require_error", code != 0, out))

        # 13. historical without intention OK without flag
        p = tdir / "hist.jsonl"
        hist = base_entry()
        del hist["intention"]
        del hist["position"]
        del hist["author_confidence"]
        del hist["grounding"]
        del hist["run_id"]
        del hist["capture_ref"]
        del hist["refs"]
        hist["origin"] = {"source": "old.txt"}  # no locators
        write_jsonl(p, [hist])
        code, out = run_validator(p, require=False)
        results.append(("historical_compat_no_flag", code == 0, out))

        # 14. append integrity: two entries unique ids
        p = tdir / "append.jsonl"
        write_jsonl(
            p,
            [
                base_entry(id="CAP-001"),
                base_entry(
                    id="CAP-002",
                    intention="observation",
                    type="observation",
                    title="Fait",
                    content="Observation factuelle ClickUp a deux espaces.",
                    position=None,
                    tags=[],
                ),
            ],
        )
        code, out = run_validator(p)
        results.append(("unique_ids_append", code == 0, out))

    # Living registry without --require-intention (compat)
    living = ROOT / "brain" / "registry" / "entries.jsonl"
    if living.exists():
        code, out = run_validator(living, require=False)
        results.append(("living_no_rewrite_compat", code == 0, out))
    else:
        results.append(("living_no_rewrite_compat", True, "absent"))

    print("=== Tests techniques Brain Capture 0.1.4 ===\n")
    failed = 0
    for name, ok, out in results:
        status = "PASS" if ok else "FAIL"
        if not ok:
            failed += 1
        print(f"[{status}] {name}")
        if not ok:
            print(out[:800])
            print("---")

    print(f"\nTotal: {len(results)} | FAIL: {failed}")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
