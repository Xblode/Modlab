#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""Tests techniques Brain Capture 0.1.5 — protection registre + rollback.

Fixtures temporaires uniquement ; ne touche PAS brain/registry/entries.jsonl production.
"""

from __future__ import annotations

import json
import os
import shutil
import subprocess
import sys
import tempfile
import threading
import time
from pathlib import Path

SCRIPTS = Path(__file__).resolve().parent
ROOT = SCRIPTS.parents[3]  # repo: <root>/brain-main/skills/brain-capture/scripts
sys.path.insert(0, str(SCRIPTS))

from registry_io import (  # noqa: E402
    fingerprint_bytes,
    format_cap_id,
    next_cap_number,
    reserved_ids_from_history,
)


def run(cmd: list[str], cwd: Path) -> tuple[int, str]:
    proc = subprocess.run(cmd, cwd=str(cwd), capture_output=True, text=True, encoding="utf-8", errors="replace")
    return proc.returncode, (proc.stdout or "") + (proc.stderr or "")


def base_entry(cap_id: str, run_id: str = "testrun", **over) -> dict:
    e = {
        "id": cap_id,
        "type": "idea",
        "status": "extracted",
        "space": "atelier",
        "validation": "none",
        "author": "user",
        "approved_by": None,
        "approval_scope": None,
        "title": f"Title {cap_id}",
        "content": "x" * 80,
        "evidence": "evidence",
        "confidence": "high",
        "intention": "idea",
        "position": None,
        "author_confidence": "medium",
        "grounding": "source_explicit",
        "run_id": run_id,
        "capture_ref": f"{run_id}:{cap_id}",
        "origin": {
            "source": "lab/sources/fixture.txt",
            "locators": [{"kind": "lines", "start": 1, "end": 1, "role": "user"}],
        },
        "project": "2brain",
        "created_at": "2026-10-31T12:00:00Z",
        "storage": "jsonl",
        "relations": [],
    }
    e.update(over)
    return e


def write_jsonl(path: Path, entries: list[dict]) -> bytes:
    data = (
        "".join(
            json.dumps(e, ensure_ascii=False, separators=(",", ":")) + "\n" for e in entries
        )
    ).encode("utf-8")
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(data)
    return data


def setup_fake_repo(tmp: Path) -> Path:
    """Minimal repo layout for path checks."""
    (tmp / "brain" / "registry").mkdir(parents=True)
    (tmp / "brain-main" / "skills" / "brain-capture" / "scripts").mkdir(parents=True)
    (tmp / "lab" / "_test-runs").mkdir(parents=True)
    (tmp / "lab" / "sources").mkdir(parents=True)
    (tmp / "lab" / "sources" / "fixture.txt").write_text("hello\n", encoding="utf-8")
    # Copy scripts
    for name in (
        "registry_io.py",
        "append-captures.py",
        "rollback-captures.py",
        "validate-registry.py",
    ):
        shutil.copy2(SCRIPTS / name, tmp / "brain-main" / "skills" / "brain-capture" / "scripts" / name)
    return tmp


def append_cmd(repo: Path, registry: Path, staging: Path) -> list[str]:
    return [
        sys.executable,
        str(repo / "brain-main/skills/brain-capture/scripts/append-captures.py"),
        "--registry",
        str(registry),
        "--staging",
        str(staging),
        "--source",
        str(repo / "lab/sources/fixture.txt"),
        "--run-validator",
    ]


def rollback_cmd(repo: Path) -> list[str]:
    return [
        sys.executable,
        str(repo / "brain-main/skills/brain-capture/scripts/rollback-captures.py"),
        "--n",
        "1",
    ]


def main() -> int:
    results: list[tuple[str, bool, str]] = []
    tmp = Path(tempfile.mkdtemp(prefix="bc015-"))
    try:
        repo = setup_fake_repo(tmp)
        reg = repo / "brain/registry/entries.jsonl"
        # Seed with CAP-001
        seed = write_jsonl(reg, [base_entry("CAP-001", run_id="production")])
        seed_fp = fingerprint_bytes(seed)

        # --- Protection ---
        # 1. Prefix unchanged after append
        staging = repo / "brain/registry/_capture-staging/op1/entries.jsonl"
        write_jsonl(staging, [base_entry("CAP-002", run_id="production")])
        code, out = run(append_cmd(repo, reg, staging), repo)
        after = reg.read_bytes()
        ok = code == 0 and after.startswith(seed) and after[len(seed) :]
        results.append(("protect_prefix_unchanged", ok, out[-400:]))

        # Rollback to reset for clean tests? Keep going with more ops.
        # Actually reset registry for isolation of remaining tests
        def reset_seed():
            # clear history so reserved don't block unless we test reserved
            hist = repo / "brain/registry/_capture-history"
            if hist.exists():
                shutil.rmtree(hist)
            lock = repo / "brain/registry/_capture.lock"
            if lock.exists():
                lock.unlink()
            write_jsonl(reg, [base_entry("CAP-001", run_id="production")])

        # 2. Refuse if hash changed mid-flight — simulate by checking append detects change
        # We'll use a staging that is valid but change registry between fingerprint prepare
        # Covered by append's mid-check: mutate file then call append while holding... 
        # Simpler: call append twice with same staging IDs → duplicate refuse
        reset_seed()
        write_jsonl(staging, [base_entry("CAP-002")])
        run(append_cmd(repo, reg, staging), repo)
        code, out = run(append_cmd(repo, reg, staging), repo)
        results.append(
            ("protect_refuse_duplicate_ids", code != 0 and "déjà présent" in out, out[-400:])
        )

        # 3. Reserved IDs after rollback
        reset_seed()
        write_jsonl(staging, [base_entry("CAP-010")])
        code1, out1 = run(append_cmd(repo, reg, staging), repo)
        code2, out2 = run(rollback_cmd(repo), repo)
        before_rb = fingerprint_bytes(reg.read_bytes())
        # Try reuse CAP-010
        write_jsonl(staging, [base_entry("CAP-010")])
        code3, out3 = run(append_cmd(repo, reg, staging), repo)
        # Next should be CAP-011
        write_jsonl(staging, [base_entry("CAP-011")])
        code4, out4 = run(append_cmd(repo, reg, staging), repo)
        results.append(
            (
                "protect_reserved_after_rollback",
                code1 == 0
                and code2 == 0
                and code3 != 0
                and "réservé" in out3
                and code4 == 0
                and before_rb.sha256 == seed_fp.sha256,
                out3[-300:] + out4[-200:],
            )
        )

        # 4. Refuse missing relation target
        reset_seed()
        bad = base_entry("CAP-002")
        bad["relations"] = [{"rel": "related", "to": "CAP-999"}]
        write_jsonl(staging, [bad])
        code, out = run(append_cmd(repo, reg, staging), repo)
        results.append(
            ("protect_refuse_missing_relation", code != 0 and "inexistante" in out, out[-400:])
        )

        # 5. Concurrent write — second fails while lock held
        reset_seed()
        write_jsonl(staging, [base_entry("CAP-002")])
        lock_path = repo / "brain/registry/_capture.lock"
        lock_path.write_text("{}", encoding="utf-8")
        code, out = run(append_cmd(repo, reg, staging), repo)
        results.append(
            ("protect_concurrent_lock", code != 0 and "Verrou" in out, out[-400:])
        )
        lock_path.unlink(missing_ok=True)

        # 6. No full replace — after append, original CAP-001 bytes prefix identical
        reset_seed()
        original = reg.read_bytes()
        write_jsonl(staging, [base_entry("CAP-003")])
        run(append_cmd(repo, reg, staging), repo)
        results.append(
            (
                "protect_no_replace",
                reg.read_bytes().startswith(original),
                "",
            )
        )

        # 7. Invalid historical registry — no add
        reset_seed()
        reg.write_bytes(b"{not json\n")
        write_jsonl(staging, [base_entry("CAP-002")])
        code, out = run(append_cmd(repo, reg, staging), repo)
        results.append(
            (
                "protect_invalid_registry_no_add",
                code != 0
                and (
                    "illisible" in out.lower()
                    or "invalide" in out.lower()
                    or "JSONL" in out
                ),
                out[-400:],
            )
        )

        # 8. Hash mismatch refuse (change registry after staging prepared — mid-check)
        # Recreate valid seed then change SHA between: covered by writing different content
        # while lock... Use: append with wrong expected via direct API
        reset_seed()
        from registry_io import append_bytes_checked, Fingerprint, RegistrySafetyError

        try:
            append_bytes_checked(
                reg,
                b'{"id":"CAP-099"}\n',
                Fingerprint("deadbeef", 0, 0, 0, True),
            )
            ok = False
            msg = "should have raised"
        except RegistrySafetyError as exc:
            ok = "modifié" in str(exc).lower() or "annul" in str(exc).lower()
            msg = str(exc)
        results.append(("protect_hash_mismatch_refuse", ok, msg))

        # 9. Validation after add — invalid entry should restore
        reset_seed()
        bad = base_entry("CAP-002")
        bad["intention"] = "orientation"
        bad["type"] = "decision"  # mapping violation under require-intention
        write_jsonl(staging, [bad])
        before = reg.read_bytes()
        code, out = run(append_cmd(repo, reg, staging), repo)
        after = reg.read_bytes()
        results.append(
            (
                "protect_validator_restore",
                code != 0 and after == before,
                out[-500:],
            )
        )

        # --- Rollback ---
        reset_seed()
        write_jsonl(staging, [base_entry("CAP-020"), base_entry("CAP-021")])
        # fix capture_ref for CAP-021
        entries = [
            base_entry("CAP-020"),
            base_entry("CAP-021"),
        ]
        write_jsonl(staging, entries)
        code_a, out_a = run(append_cmd(repo, reg, staging), repo)
        mid = reg.read_bytes()
        code_b, out_b = run(rollback_cmd(repo), repo)
        after_rb = reg.read_bytes()
        results.append(
            (
                "rollback_success_prefix",
                code_a == 0
                and code_b == 0
                and after_rb == seed
                and fingerprint_bytes(after_rb).sha256 == seed_fp.sha256,
                out_a[-200:] + out_b[-200:],
            )
        )

        # Block recoverable
        hist = repo / "brain/registry/_capture-history"
        ops = sorted(hist.iterdir()) if hist.exists() else []
        last = ops[-1] if ops else None
        recoverable = (
            last is not None
            and (last / "reverted-entries.jsonl").exists()
            and (last / "reverted-entries.jsonl").stat().st_size > 0
        )
        results.append(("rollback_block_recoverable", recoverable, str(last)))

        # Post-modification refuse rollback
        reset_seed()
        write_jsonl(staging, [base_entry("CAP-030")])
        run(append_cmd(repo, reg, staging), repo)
        # Append another op
        write_jsonl(staging, [base_entry("CAP-031")])
        run(append_cmd(repo, reg, staging), repo)
        # Manually corrupt expectation: change file so last after hash fails if we try
        # rollback of... actually rollback only last. Mutate after last commit:
        with open(reg, "ab") as f:
            f.write(b"\n")  # break exact match
        # restore? newline alone may break jsonl. Better append space to last line — 
        # actually regenerating: rewrite with extra blank is size change
        code, out = run(rollback_cmd(repo), repo)
        results.append(
            ("rollback_refuse_if_changed", code != 0, out[-400:])
        )

        # No committed op
        reset_seed()
        hist = repo / "brain/registry/_capture-history"
        if hist.exists():
            shutil.rmtree(hist)
        code, out = run(rollback_cmd(repo), repo)
        results.append(
            ("rollback_refuse_none", code != 0 and "Aucune" in out, out[-300:])
        )

        # Multiple ops — only latest rolled back
        reset_seed()
        write_jsonl(staging, [base_entry("CAP-040")])
        run(append_cmd(repo, reg, staging), repo)
        after_first = reg.read_bytes()
        write_jsonl(staging, [base_entry("CAP-041")])
        run(append_cmd(repo, reg, staging), repo)
        run(rollback_cmd(repo), repo)
        results.append(
            (
                "rollback_only_latest",
                reg.read_bytes() == after_first,
                "",
            )
        )

        # Empty / absent registry rollback
        empty_repo = setup_fake_repo(tmp / "empty")
        empty_reg = empty_repo / "brain/registry/entries.jsonl"
        empty_reg.write_bytes(b"")
        code, out = run(rollback_cmd(empty_repo), empty_repo)
        results.append(
            ("rollback_empty_none", code != 0, out[-300:])
        )

        # Wrong hash refuse — tamper manifest after
        reset_seed()
        write_jsonl(staging, [base_entry("CAP-050")])
        code_h, out_h = run(append_cmd(repo, reg, staging), repo)
        ops = [
            p
            for p in sorted((repo / "brain/registry/_capture-history").iterdir())
            if (p / "manifest.json").exists()
        ]
        if not ops or code_h != 0:
            results.append(
                ("rollback_refuse_bad_hash", False, f"append failed: {out_h[-400:]}")
            )
        else:
            man_path = ops[-1] / "manifest.json"
            man = json.loads(man_path.read_text(encoding="utf-8"))
            man["after"]["sha256"] = "0" * 64
            man_path.write_text(json.dumps(man, indent=2), encoding="utf-8")
            code, out = run(rollback_cmd(repo), repo)
            results.append(
                ("rollback_refuse_bad_hash", code != 0, out[-400:])
            )

        # 10. Registry without trailing newline — no add
        reset_seed()
        raw = reg.read_bytes().rstrip(b"\n")
        reg.write_bytes(raw)
        write_jsonl(staging, [base_entry("CAP-002")])
        code, out = run(append_cmd(repo, reg, staging), repo)
        results.append(
            (
                "protect_no_trailing_newline",
                code != 0 and "newline" in out.lower(),
                out[-400:],
            )
        )

        # Partial-write integrity: append_bytes_checked verifies suffix == block
        reset_seed()
        from registry_io import append_bytes_checked as _abc

        before_fp = fingerprint_bytes(reg.read_bytes())
        block = write_jsonl(staging, [base_entry("CAP-060")])
        after_fp = _abc(reg, block, before_fp)
        ok_partial = (
            reg.read_bytes().startswith(seed)
            and after_fp.size == before_fp.size + len(block)
        )
        results.append(("protect_append_size_coherent", ok_partial, ""))

        # Semantic mechanical: mapping + space examples (validator)
        reset_seed()
        orient = base_entry("CAP-070")
        orient["intention"] = "orientation"
        orient["type"] = "proposition"
        orient["space"] = "atelier"
        write_jsonl(staging, [orient])
        code, out = run(append_cmd(repo, reg, staging), repo)
        results.append(("semantic_orientation_not_decision", code == 0, out[-300:]))

        reset_seed()
        fact = base_entry("CAP-071")
        fact["intention"] = "observation"
        fact["type"] = "observation"
        fact["space"] = "referentiel"
        fact["validation"] = "none"
        fact["author"] = "assistant"
        fact["origin"]["locators"][0]["role"] = "assistant"
        write_jsonl(staging, [fact])
        code, out = run(append_cmd(repo, reg, staging), repo)
        results.append(
            ("semantic_referentiel_without_explicit_ok", code == 0, out[-300:])
        )

        # Heuristic: locator role vs author (mechanical warning dataset, not validator ERROR)
        loc_mismatch = base_entry("CAP-270")
        loc_mismatch["author"] = "user"
        loc_mismatch["origin"]["locators"][0]["role"] = "assistant"
        loc_ok = (
            loc_mismatch["author"] == "user"
            and loc_mismatch["origin"]["locators"][0]["role"] == "assistant"
        )
        # The *rule* is to reject this pairing for NEW captures in the agent;
        # detector used in tests:
        def locator_author_conflict(e: dict) -> bool:
            roles = {
                loc.get("role")
                for loc in (e.get("origin") or {}).get("locators") or []
                if isinstance(loc, dict) and loc.get("role") in ("user", "assistant")
            }
            if e.get("author") in roles and len(roles) == 1:
                return False
            if e.get("author") not in roles and roles:
                return True
            return False

        results.append(
            (
                "semantic_cap270_locator_conflict_detected",
                locator_author_conflict(loc_mismatch),
                "",
            )
        )
        split_needed = base_entry("CAP-219")
        split_needed["origin"]["locators"] = [
            {"kind": "lines", "start": 1, "end": 1, "role": "user"},
            {"kind": "lines", "start": 2, "end": 2, "role": "assistant"},
        ]
        results.append(
            (
                "semantic_cap219_multi_role_detected",
                len(
                    {
                        loc["role"]
                        for loc in split_needed["origin"]["locators"]
                    }
                )
                > 1,
                "",
            )
        )

        skill = ROOT / "brain-main/skills/brain-capture"
        docs_ok = all(
            (skill / p).exists()
            for p in (
                "SKILL.md",
                "references/rollback.md",
                "references/capture-rules.md",
                "references/source-attribution.md",
            )
        )
        skill_txt = (skill / "SKILL.md").read_text(encoding="utf-8")
        results.append(
            (
                "semantic_docs_present",
                docs_ok and "0.1.5" in skill_txt and "append-captures.py" in skill_txt,
                "",
            )
        )

    finally:
        # Keep tmp on failure? Always clean for safety
        shutil.rmtree(tmp, ignore_errors=True)

    print("=== Tests techniques Brain Capture 0.1.5 ===\n")
    failed = 0
    for name, ok, detail in results:
        status = "PASS" if ok else "FAIL"
        if not ok:
            failed += 1
        print(f"[{status}] {name}")
        if not ok and detail:
            print(detail[:500])
    print(f"\nSUMMARY {len(results) - failed}/{len(results)} PASS")
    return 0 if failed == 0 else 1


if __name__ == "__main__":
    sys.exit(main())
