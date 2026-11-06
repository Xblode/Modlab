#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""Seul mécanisme autorisé pour ajouter des CAP au registre (Brain Capture 0.1.5).

Usage:
  python append-captures.py --registry PATH --staging PATH \\
    --source PATH [--operation-id ID] [--run-validator]

Ne reconstruit jamais le registre. Append binaire + empreintes + verrou + historique.
"""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
from datetime import datetime, timezone
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

from registry_io import (
    HISTORY_REL,
    LOCK_REL,
    SKILL_VERSION,
    STAGING_REL,
    FileLock,
    RegistrySafetyError,
    append_bytes_checked,
    copy_byte_for_byte,
    eprint,
    find_repo_root,
    fingerprint_bytes,
    fingerprint_file,
    new_operation_id,
    parse_jsonl_bytes,
    read_binary,
    resolve_allowed_registry,
    validate_staging_lot,
)


def run_validator(registry: Path, json_out: Path | None) -> tuple[int, str]:
    script = Path(__file__).resolve().parent / "validate-registry.py"
    cmd = [sys.executable, str(script), str(registry), "--require-intention"]
    if json_out is not None:
        cmd.extend(["--json-out", str(json_out)])
    proc = subprocess.run(
        cmd, capture_output=True, text=True, encoding="utf-8", errors="replace"
    )
    out = (proc.stdout or "") + (proc.stderr or "")
    return proc.returncode, out


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Append sécurisé de captures Brain 0.1.5")
    parser.add_argument("--registry", required=True, help="Chemin entries.jsonl cible")
    parser.add_argument(
        "--staging",
        required=True,
        help="Fichier JSONL ne contenant QUE les nouvelles lignes",
    )
    parser.add_argument("--source", required=True, help="Chemin source conversation")
    parser.add_argument("--operation-id", default=None)
    parser.add_argument(
        "--run-validator",
        action="store_true",
        help="Valider le registre après append (recommandé)",
    )
    parser.add_argument(
        "--validate-json-out",
        default=None,
        help="Destination rapport validateur (hors registre)",
    )
    parser.add_argument(
        "--allow-empty-new",
        action="store_true",
        help="Autoriser création entries.jsonl neuf (test-run seulement)",
    )
    args = parser.parse_args(argv)

    try:
        root = find_repo_root()
        registry = resolve_allowed_registry(
            Path(args.registry), root, allow_empty_new=args.allow_empty_new
        )
        staging_path = Path(args.staging).resolve()
        source_path = Path(args.source)

        if not staging_path.exists():
            raise RegistrySafetyError(f"Staging introuvable: {staging_path}")
        if not source_path.exists():
            raise RegistrySafetyError(f"Source introuvable: {source_path}")

        history_root = root / HISTORY_REL
        history_root.mkdir(parents=True, exist_ok=True)
        op_id = args.operation_id or new_operation_id()
        op_dir = history_root / op_id
        if op_dir.exists():
            raise RegistrySafetyError(
                f"Dossier d'opération déjà existant (ne pas écraser): {op_dir}"
            )

        # Create empty registry for brand-new test-run if requested
        if args.allow_empty_new and not registry.exists():
            registry.parent.mkdir(parents=True, exist_ok=True)
            registry.write_bytes(b"")

        lock = FileLock(root / LOCK_REL, timeout_s=0.0)
        with lock:
            before_bytes = read_binary(registry)
            before_fp = fingerprint_bytes(before_bytes)
            if before_bytes and not before_fp.ends_with_newline:
                raise RegistrySafetyError(
                    "Registre sans newline finale — ARRÊT, aucun ajout."
                )
            # Structural sanity: every non-empty line must parse
            try:
                parse_jsonl_bytes(before_bytes)
            except RegistrySafetyError:
                raise RegistrySafetyError(
                    "Registre historique illisible/invalide — ARRÊT, aucun ajout."
                )

            staging_bytes = read_binary(staging_path)
            entries = validate_staging_lot(
                staging_bytes, before_bytes, history_root, require_fields=True
            )
            ids_added = [e["id"] for e in entries]

            # Créer le dossier d'opération seulement après validations pré-écriture
            op_dir.mkdir(parents=False)

            # Snapshot byte-for-byte BEFORE any mutation
            before_snap = op_dir / "before-registry.jsonl"
            copy_byte_for_byte(registry, before_snap)
            appended_path = op_dir / "appended-entries.jsonl"
            appended_path.write_bytes(staging_bytes)
            from registry_io import fsync_file

            fsync_file(appended_path)

            # Re-check fingerprint immediately before write
            mid_bytes = read_binary(registry)
            mid_fp = fingerprint_bytes(mid_bytes)
            if mid_fp.sha256 != before_fp.sha256 or mid_fp.size != before_fp.size:
                raise RegistrySafetyError(
                    "Registre changé avant écriture — opération annulée sans écriture."
                )

            manifest = {
                "operation_id": op_id,
                "skill_version": SKILL_VERSION,
                "status": "started",
                "created_at": datetime.now(timezone.utc).isoformat(),
                "source": str(source_path.as_posix()),
                "registry": str(registry.relative_to(root)).replace("\\", "/"),
                "ids_added": ids_added,
                "ids_reserved_numeric": [
                    int(i.split("-")[1]) for i in ids_added if "-" in i
                ],
                "entry_count": len(ids_added),
                "before": before_fp.to_dict(),
                "after": None,
                "paths": {
                    "before_registry": "before-registry.jsonl",
                    "appended_entries": "appended-entries.jsonl",
                    "reverted_entries": None,
                },
            }
            (op_dir / "manifest.json").write_text(
                json.dumps(manifest, ensure_ascii=False, indent=2) + "\n",
                encoding="utf-8",
            )

            after_fp = append_bytes_checked(registry, staging_bytes, before_fp)

            # Post-write: prefix + block already checked inside append_bytes_checked
            after_bytes = read_binary(registry)
            expected = before_bytes + staging_bytes
            if after_bytes != expected:
                raise RegistrySafetyError(
                    "INTÉGRITÉ FATALE: contenu final ≠ before+staging — "
                    "intervention humaine requise."
                )

            validator_ok = None
            validator_out = ""
            if args.run_validator:
                v_out = (
                    Path(args.validate_json_out)
                    if args.validate_json_out
                    else op_dir / "validate.json"
                )
                code, validator_out = run_validator(registry, v_out)
                validator_ok = code == 0
                if not validator_ok:
                    # Attempt safe restore ONLY if prefix intact and block exact
                    current = read_binary(registry)
                    if current == before_bytes + staging_bytes:
                        from registry_io import atomic_write_bytes

                        atomic_write_bytes(registry, before_bytes)
                        restored = read_binary(registry)
                        if fingerprint_bytes(restored).sha256 != before_fp.sha256:
                            raise RegistrySafetyError(
                                "Validation KO et restauration SHA mismatch — "
                                "intervention humaine."
                            )
                        manifest["status"] = "failed"
                        manifest["failure"] = "validator_failed_restored"
                        manifest["validator_excerpt"] = validator_out[-2000:]
                        (op_dir / "manifest.json").write_text(
                            json.dumps(manifest, ensure_ascii=False, indent=2) + "\n",
                            encoding="utf-8",
                        )
                        eprint("Validation échouée — registre restauré au snapshot.")
                        eprint(validator_out[-1500:])
                        return 1
                    raise RegistrySafetyError(
                        "Validation KO et état ambigu — aucune restauration auto."
                    )

            manifest["status"] = "committed"
            manifest["after"] = after_fp.to_dict()
            manifest["committed_at"] = datetime.now(timezone.utc).isoformat()
            manifest["validator_ok"] = validator_ok
            (op_dir / "manifest.json").write_text(
                json.dumps(manifest, ensure_ascii=False, indent=2) + "\n",
                encoding="utf-8",
            )

            # Mirror staging under _capture-staging for traceability
            staging_mirror = root / STAGING_REL / op_id
            staging_mirror.mkdir(parents=True, exist_ok=True)
            (staging_mirror / "entries.jsonl").write_bytes(staging_bytes)

            print(
                json.dumps(
                    {
                        "ok": True,
                        "operation_id": op_id,
                        "ids_added": ids_added,
                        "entry_count": len(ids_added),
                        "before_sha256": before_fp.sha256,
                        "after_sha256": after_fp.sha256,
                        "history": str(op_dir.relative_to(root)).replace("\\", "/"),
                    },
                    ensure_ascii=False,
                    indent=2,
                )
            )
            return 0

    except RegistrySafetyError as exc:
        eprint(f"[ABORT] {exc}")
        return 2
    except Exception as exc:  # noqa: BLE001 — surface unexpected errors safely
        eprint(f"[ABORT unexpected] {exc}")
        return 3


if __name__ == "__main__":
    for _stream in (sys.stdout, sys.stderr):
        if hasattr(_stream, "reconfigure"):
            _stream.reconfigure(encoding="utf-8", errors="backslashreplace")
    sys.exit(main())
