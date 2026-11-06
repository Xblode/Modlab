#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""Rollback de la dernière opération Capture committed (Brain Capture 0.1.5).

Usage:
  python rollback-captures.py [--n 1]

Seul n=1 est supporté : annule la dernière exécution réussie non déjà annulée.
"""

from __future__ import annotations

import argparse
import json
import sys
from datetime import datetime, timezone
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

from registry_io import (
    HISTORY_REL,
    LOCK_REL,
    FileLock,
    RegistrySafetyError,
    atomic_write_bytes,
    eprint,
    find_repo_root,
    fingerprint_bytes,
    fingerprint_file,
    read_binary,
    resolve_allowed_registry,
)


def list_operations(history_root: Path) -> list[Path]:
    if not history_root.exists():
        return []
    ops = [p for p in history_root.iterdir() if p.is_dir() and (p / "manifest.json").exists()]
    # Sort by directory name (UTC timestamp with microseconds, then pid):
    # alphabetical order follows chronological order for generated ids.
    return sorted(ops, key=lambda p: p.name)


def load_manifest(op_dir: Path) -> dict:
    return json.loads((op_dir / "manifest.json").read_text(encoding="utf-8"))


def find_last_committed(history_root: Path) -> Path | None:
    committed: list[Path] = []
    for op in list_operations(history_root):
        try:
            m = load_manifest(op)
        except (OSError, json.JSONDecodeError):
            continue
        if m.get("status") == "committed":
            committed.append(op)
    return committed[-1] if committed else None


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Rollback Capture 0.1.5 (n=1 seulement)")
    parser.add_argument(
        "--n",
        type=int,
        default=1,
        help="Nombre d'exécutions à annuler (seul 1 supporté)",
    )
    parser.add_argument(
        "--registry",
        default=None,
        help="Override registre (défaut: celui du manifeste)",
    )
    args = parser.parse_args(argv)

    if args.n != 1:
        eprint("[ABORT] Seul --n 1 est supporté en 0.1.5.")
        return 2

    try:
        root = find_repo_root()
        history_root = root / HISTORY_REL
        op_dir = find_last_committed(history_root)
        if op_dir is None:
            raise RegistrySafetyError(
                "Aucune opération committed à annuler — rollback refusé."
            )

        manifest = load_manifest(op_dir)
        if manifest.get("status") == "rolled_back":
            raise RegistrySafetyError("Opération déjà annulée.")
        if manifest.get("status") != "committed":
            raise RegistrySafetyError(
                f"Statut opération non rollbackable: {manifest.get('status')!r}"
            )

        rel_registry = manifest.get("registry")
        if not rel_registry:
            raise RegistrySafetyError("Manifeste sans chemin registry.")
        registry = resolve_allowed_registry(
            Path(args.registry) if args.registry else root / rel_registry,
            root,
        )

        before_path = op_dir / "before-registry.jsonl"
        appended_path = op_dir / "appended-entries.jsonl"
        if not before_path.exists() or not appended_path.exists():
            raise RegistrySafetyError("Snapshot ou bloc appended manquant — refus.")

        before_bytes = read_binary(before_path)
        appended_bytes = read_binary(appended_path)
        before_fp_manifest = manifest.get("before") or {}
        after_fp_manifest = manifest.get("after") or {}

        snap_fp = fingerprint_bytes(before_bytes)
        if before_fp_manifest.get("sha256") and snap_fp.sha256 != before_fp_manifest["sha256"]:
            raise RegistrySafetyError(
                "Snapshot before-registry.jsonl SHA ≠ manifeste — refus."
            )
        if before_fp_manifest.get("size") is not None and snap_fp.size != int(
            before_fp_manifest["size"]
        ):
            raise RegistrySafetyError("Snapshot taille ≠ manifeste — refus.")

        expected_current = before_bytes + appended_bytes
        expected_fp = fingerprint_bytes(expected_current)
        if after_fp_manifest.get("sha256") and expected_fp.sha256 != after_fp_manifest["sha256"]:
            raise RegistrySafetyError(
                "before+appended ≠ SHA after du manifeste — refus."
            )

        lock = FileLock(root / LOCK_REL, timeout_s=0.0)
        with lock:
            current = read_binary(registry)
            cur_fp = fingerprint_bytes(current)

            if after_fp_manifest.get("sha256") and cur_fp.sha256 != after_fp_manifest["sha256"]:
                raise RegistrySafetyError(
                    "Registre actif SHA ≠ after opération — modification postérieure, refus."
                )
            if after_fp_manifest.get("size") is not None and cur_fp.size != int(
                after_fp_manifest["size"]
            ):
                raise RegistrySafetyError(
                    "Registre actif taille ≠ after opération — refus."
                )
            if current != expected_current:
                raise RegistrySafetyError(
                    "Registre actif ≠ before+appended exact — refus (ambiguïté)."
                )
            # Last block must be exactly appended
            if not current.endswith(appended_bytes):
                raise RegistrySafetyError(
                    "Le bloc ajouté n'est plus exactement le suffixe du registre — refus."
                )
            if current[: len(before_bytes)] != before_bytes:
                raise RegistrySafetyError("Préfixe ≠ snapshot — refus.")

            # Archive reverted entries
            reverted = op_dir / "reverted-entries.jsonl"
            reverted.write_bytes(appended_bytes)

            atomic_write_bytes(registry, before_bytes)
            restored = read_binary(registry)
            restored_fp = fingerprint_bytes(restored)
            if restored != before_bytes or restored_fp.sha256 != snap_fp.sha256:
                raise RegistrySafetyError(
                    "Échec restauration atomique — intervention humaine."
                )

            manifest["status"] = "rolled_back"
            manifest["rolled_back_at"] = datetime.now(timezone.utc).isoformat()
            manifest["paths"]["reverted_entries"] = "reverted-entries.jsonl"
            manifest["restored"] = restored_fp.to_dict()
            (op_dir / "manifest.json").write_text(
                json.dumps(manifest, ensure_ascii=False, indent=2) + "\n",
                encoding="utf-8",
            )

            print(
                json.dumps(
                    {
                        "ok": True,
                        "rolled_back": manifest["operation_id"],
                        "ids_reverted": manifest.get("ids_added"),
                        "registry_sha256": restored_fp.sha256,
                        "note": "IDs restent réservés ; prochain ID = max(présent, réservés)+1",
                    },
                    ensure_ascii=False,
                    indent=2,
                )
            )
            return 0

    except RegistrySafetyError as exc:
        eprint(f"[ABORT] {exc}")
        return 2
    except Exception as exc:  # noqa: BLE001
        eprint(f"[ABORT unexpected] {exc}")
        return 3


if __name__ == "__main__":
    for _stream in (sys.stdout, sys.stderr):
        if hasattr(_stream, "reconfigure"):
            _stream.reconfigure(encoding="utf-8", errors="backslashreplace")
    sys.exit(main())
