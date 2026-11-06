#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""Helpers I/O sécurisés pour Brain Capture 0.1.5 (empreintes, verrous, IDs)."""

from __future__ import annotations

import hashlib
import json
import os
import re
import sys
import time
from dataclasses import dataclass
from pathlib import Path
from typing import Any

ID_RE = re.compile(r"^CAP-(\d+)$")
CAP_IN_JSON_RE = re.compile(rb'"id"\s*:\s*"CAP-(\d+)"')

REPO_ROOT_MARKERS = ("brain", "brain-main")
PROD_REGISTRY_REL = Path("brain") / "registry" / "entries.jsonl"
HISTORY_REL = Path("brain") / "registry" / "_capture-history"
STAGING_REL = Path("brain") / "registry" / "_capture-staging"
LOCK_REL = Path("brain") / "registry" / "_capture.lock"
TEST_RUNS_REL = Path("lab") / "_test-runs"

SKILL_VERSION = "0.1.5"


class RegistrySafetyError(Exception):
    """Erreur bloquante de sécurité registre (aucune écriture / rollback refusée)."""


@dataclass
class Fingerprint:
    sha256: str
    size: int
    line_count: int
    max_cap: int
    ends_with_newline: bool

    def to_dict(self) -> dict[str, Any]:
        return {
            "sha256": self.sha256,
            "size": self.size,
            "line_count": self.line_count,
            "max_cap": self.max_cap,
            "ends_with_newline": self.ends_with_newline,
        }

    @classmethod
    def from_dict(cls, d: dict[str, Any]) -> "Fingerprint":
        return cls(
            sha256=str(d["sha256"]),
            size=int(d["size"]),
            line_count=int(d["line_count"]),
            max_cap=int(d["max_cap"]),
            ends_with_newline=bool(d["ends_with_newline"]),
        )


def find_repo_root(start: Path | None = None) -> Path:
    cur = (start or Path.cwd()).resolve()
    for candidate in [cur, *cur.parents]:
        if all((candidate / m).exists() for m in REPO_ROOT_MARKERS):
            return candidate
    raise RegistrySafetyError(f"Racine dépôt introuvable depuis {cur}")


def sha256_bytes(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def read_binary(path: Path) -> bytes:
    if not path.exists():
        raise RegistrySafetyError(f"Fichier introuvable: {path}")
    return path.read_bytes()


def fingerprint_bytes(data: bytes) -> Fingerprint:
    ids = [int(m.group(1)) for m in CAP_IN_JSON_RE.finditer(data)]
    # Count non-empty lines for JSONL semantics; empty file = 0
    if not data:
        lines = 0
    else:
        lines = data.count(b"\n")
        if not data.endswith(b"\n") and data.strip():
            lines += 1
        # trailing empty after final newline not counted as entry
        # line_count = number of newline-terminated or last partial line with content
        # Prefer: number of non-empty lines
        lines = sum(1 for ln in data.splitlines() if ln.strip())
    return Fingerprint(
        sha256=sha256_bytes(data),
        size=len(data),
        line_count=lines,
        max_cap=max(ids) if ids else 0,
        ends_with_newline=(len(data) == 0) or data.endswith(b"\n"),
    )


def fingerprint_file(path: Path) -> Fingerprint:
    return fingerprint_bytes(read_binary(path))


def resolve_allowed_registry(path: Path, root: Path, *, allow_empty_new: bool = False) -> Path:
    """Refuse chemins hors production / test-run autorisés ; refuse symlink escape."""
    resolved = path.resolve()
    root = root.resolve()
    prod = (root / PROD_REGISTRY_REL).resolve()
    test_runs = (root / TEST_RUNS_REL).resolve()

    if resolved == prod:
        if resolved.is_symlink() or path.is_symlink():
            # production path itself shouldn't be a symlink redirecting elsewhere
            target = resolved
            if not str(target).startswith(str((root / "brain" / "registry").resolve())):
                raise RegistrySafetyError("Registre production via symlink hors limite.")
        return resolved

    try:
        resolved.relative_to(test_runs)
    except ValueError as exc:
        raise RegistrySafetyError(
            f"Destination hors périmètre autorisé: {resolved} "
            f"(attendu production ou sous {test_runs})"
        ) from exc

    if resolved.name != "entries.jsonl":
        raise RegistrySafetyError(f"Cible test-run doit s'appeler entries.jsonl: {resolved}")
    if resolved.is_symlink() or path.is_symlink():
        raise RegistrySafetyError(f"Symlink interdit pour registre test-run: {path}")
    if not allow_empty_new and not resolved.exists():
        raise RegistrySafetyError(f"Registre test-run absent: {resolved}")
    return resolved


def reserved_ids_from_history(history_dir: Path) -> set[int]:
    reserved: set[int] = set()
    if not history_dir.exists():
        return reserved
    for manifest in sorted(history_dir.glob("*/manifest.json")):
        try:
            data = json.loads(manifest.read_text(encoding="utf-8"))
        except (OSError, json.JSONDecodeError):
            continue
        for cap_id in data.get("ids_added") or []:
            if isinstance(cap_id, str):
                m = ID_RE.match(cap_id)
                if m:
                    reserved.add(int(m.group(1)))
        for n in data.get("ids_reserved_numeric") or []:
            if isinstance(n, int):
                reserved.add(n)
    return reserved


def next_cap_number(registry_bytes: bytes, history_dir: Path) -> int:
    fp = fingerprint_bytes(registry_bytes)
    reserved = reserved_ids_from_history(history_dir)
    floor = max([fp.max_cap, *reserved], default=0)
    return floor + 1


def format_cap_id(n: int) -> str:
    width = max(3, len(str(n)))
    return f"CAP-{n:0{width}d}"


def parse_jsonl_bytes(data: bytes) -> list[dict[str, Any]]:
    entries: list[dict[str, Any]] = []
    if not data.strip():
        return entries
    text = data.decode("utf-8")
    for i, line in enumerate(text.splitlines(), 1):
        if not line.strip():
            continue
        try:
            obj = json.loads(line)
        except json.JSONDecodeError as exc:
            raise RegistrySafetyError(f"JSONL invalide ligne {i}: {exc}") from exc
        if not isinstance(obj, dict):
            raise RegistrySafetyError(f"JSONL ligne {i}: objet attendu.")
        entries.append(obj)
    return entries


def entries_to_jsonl_bytes(entries: list[dict[str, Any]]) -> bytes:
    lines = [
        json.dumps(e, ensure_ascii=False, separators=(",", ":")) + "\n" for e in entries
    ]
    return "".join(lines).encode("utf-8")


def validate_staging_lot(
    staging_bytes: bytes,
    registry_bytes: bytes,
    history_dir: Path,
    *,
    require_fields: bool = True,
) -> list[dict[str, Any]]:
    if not staging_bytes:
        raise RegistrySafetyError("Lot staging vide.")
    if not staging_bytes.endswith(b"\n"):
        raise RegistrySafetyError("Lot staging doit se terminer par une nouvelle ligne.")
    entries = parse_jsonl_bytes(staging_bytes)
    if not entries:
        raise RegistrySafetyError("Lot staging sans entrées.")

    existing_ids = {
        e["id"]
        for e in parse_jsonl_bytes(registry_bytes)
        if isinstance(e.get("id"), str)
    }
    reserved_nums = reserved_ids_from_history(history_dir)
    reserved_ids = {format_cap_id(n) for n in reserved_nums} | {
        f"CAP-{n}" for n in reserved_nums
    }
    # Also match zero-padded variants present in history ids_added
    for n in reserved_nums:
        reserved_ids.add(format_cap_id(n))

    lot_ids: set[str] = set()
    lot_refs: set[str] = set()
    for e in entries:
        eid = e.get("id")
        if not isinstance(eid, str) or not ID_RE.match(eid):
            raise RegistrySafetyError(f"ID invalide dans staging: {eid!r}")
        if eid in lot_ids:
            raise RegistrySafetyError(f"ID dupliqué dans le lot: {eid}")
        if eid in existing_ids:
            raise RegistrySafetyError(f"ID déjà présent dans le registre: {eid}")
        m = ID_RE.match(eid)
        assert m
        n = int(m.group(1))
        if n in reserved_nums:
            raise RegistrySafetyError(f"ID déjà réservé (historique opérations): {eid}")
        lot_ids.add(eid)

        cref = e.get("capture_ref")
        if require_fields:
            for field in (
                "intention",
                "position",
                "author_confidence",
                "grounding",
                "run_id",
                "capture_ref",
                "origin",
            ):
                if field not in e:
                    raise RegistrySafetyError(f"{eid}: champ obligatoire manquant: {field}")
        if isinstance(cref, str):
            if cref in lot_refs:
                raise RegistrySafetyError(f"capture_ref dupliqué dans le lot: {cref}")
            lot_refs.add(cref)

    # Relations: targets must exist in registry or lot
    known = existing_ids | lot_ids
    for e in entries:
        eid = e.get("id", "?")
        for idx, rel in enumerate(e.get("relations") or []):
            if not isinstance(rel, dict):
                raise RegistrySafetyError(f"{eid}: relations[{idx}] invalide.")
            to = rel.get("to")
            if isinstance(to, str) and ID_RE.match(to) and to not in known:
                raise RegistrySafetyError(
                    f"{eid}: relation vers capture inexistante {to}."
                )
    return entries


class FileLock:
    """Verrou exclusif compatible Windows/Unix via create-exclusive lock file."""

    def __init__(self, lock_path: Path, *, timeout_s: float = 0.0, stale_s: float = 3600.0):
        self.lock_path = lock_path
        self.timeout_s = timeout_s
        self.stale_s = stale_s
        self._held = False

    def acquire(self) -> None:
        self.lock_path.parent.mkdir(parents=True, exist_ok=True)
        deadline = time.time() + self.timeout_s
        while True:
            try:
                fd = os.open(
                    str(self.lock_path),
                    os.O_CREAT | os.O_EXCL | os.O_WRONLY,
                )
                payload = json.dumps(
                    {
                        "pid": os.getpid(),
                        "created_at": time.time(),
                        "host": os.environ.get("COMPUTERNAME")
                        or os.environ.get("HOSTNAME")
                        or "",
                    }
                ).encode("utf-8")
                os.write(fd, payload)
                os.close(fd)
                self._held = True
                return
            except FileExistsError:
                # Uncertain stale lock → secure stop (no auto-delete without proof)
                if self.lock_path.exists():
                    age = time.time() - self.lock_path.stat().st_mtime
                    if age > self.stale_s:
                        raise RegistrySafetyError(
                            f"Verrou ancien présent ({age:.0f}s) à {self.lock_path} — "
                            f"arrêt sécurisé (ne pas supprimer automatiquement)."
                        )
                if time.time() >= deadline:
                    raise RegistrySafetyError(
                        f"Verrou d'écriture déjà tenu: {self.lock_path}"
                    )
                time.sleep(0.05)

    def release(self) -> None:
        if self._held and self.lock_path.exists():
            try:
                self.lock_path.unlink()
            except OSError:
                pass
            self._held = False

    def __enter__(self) -> "FileLock":
        self.acquire()
        return self

    def __exit__(self, *args: object) -> None:
        self.release()


def fsync_file(path: Path) -> None:
    # Windows: fsync on a read-only handle can raise EBADF (errno 9).
    fd = os.open(str(path), os.O_RDWR)
    try:
        os.fsync(fd)
    finally:
        os.close(fd)
    if os.name == "nt":
        return
    try:
        dir_fd = os.open(str(path.parent), os.O_RDONLY)
        try:
            os.fsync(dir_fd)
        finally:
            os.close(dir_fd)
    except OSError:
        pass


def atomic_write_bytes(path: Path, data: bytes) -> None:
    """Écriture atomique (temp + replace) pour restauration rollback uniquement."""
    path.parent.mkdir(parents=True, exist_ok=True)
    tmp = path.with_name(path.name + f".tmp-{os.getpid()}")
    with open(tmp, "wb") as f:
        f.write(data)
        f.flush()
        os.fsync(f.fileno())
    os.replace(tmp, path)
    fsync_file(path)


def append_bytes_checked(
    registry_path: Path,
    block: bytes,
    expected_before: Fingerprint,
) -> Fingerprint:
    """Append binaire exclusif ; vérifie préfixe avant et après."""
    before = read_binary(registry_path)
    fp_before = fingerprint_bytes(before)
    if (
        fp_before.sha256 != expected_before.sha256
        or fp_before.size != expected_before.size
    ):
        raise RegistrySafetyError(
            "Registre modifié depuis le début de l'opération — écriture annulée."
        )
    if before and not before.endswith(b"\n"):
        raise RegistrySafetyError(
            "Registre ne se termine pas par une nouvelle ligne — aucun ajout."
        )
    if not block.endswith(b"\n"):
        raise RegistrySafetyError("Bloc à ajouter doit se terminer par une nouvelle ligne.")

    with open(registry_path, "ab", buffering=0) as f:
        f.write(block)
        f.flush()
        os.fsync(f.fileno())

    after = read_binary(registry_path)
    if not after.startswith(before):
        raise RegistrySafetyError(
            "INTÉGRITÉ: préfixe historique altéré après écriture — intervention humaine."
        )
    if after[len(before) :] != block:
        raise RegistrySafetyError(
            "INTÉGRITÉ: bloc ajouté ≠ staging — intervention humaine."
        )
    expected_size = len(before) + len(block)
    if len(after) != expected_size:
        raise RegistrySafetyError(
            f"INTÉGRITÉ: taille finale {len(after)} ≠ attendu {expected_size}."
        )
    return fingerprint_bytes(after)


def copy_byte_for_byte(src: Path, dst: Path) -> Fingerprint:
    data = read_binary(src)
    dst.parent.mkdir(parents=True, exist_ok=True)
    dst.write_bytes(data)
    fsync_file(dst)
    fp = fingerprint_bytes(data)
    if fingerprint_file(dst).sha256 != fp.sha256:
        raise RegistrySafetyError(f"Snapshot SHA mismatch: {dst}")
    return fp


def new_operation_id() -> str:
    # UTC compact with microseconds, then zero-padded pid: lexicographic order
    # follows chronological order (pid only breaks same-microsecond ties).
    from datetime import datetime, timezone

    ts = datetime.now(timezone.utc).strftime("%Y%m%d-%H%M%S-%f")
    return f"{ts}-{os.getpid():08d}"


def eprint(*args: object) -> None:
    print(*args, file=sys.stderr)
