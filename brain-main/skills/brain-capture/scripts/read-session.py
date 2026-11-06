# -*- coding: utf-8 -*-
"""Extrait lisible et traçable d'une session Claude Code, pour la capture (/brain -c).

Ce script ne capture rien : il transforme une transcription JSONL en un fichier texte
dont les numéros de ligne servent ensuite d'origin.locators (kind "lines").

Usage :
  python -I read-session.py --session-id ID --project DOSSIER [--out FICHIER | --out-dir DOSSIER]
  python -I read-session.py --transcript CHEMIN.jsonl [--out FICHIER | --out-dir DOSSIER]

Sans --out ni --out-dir, rien n'est écrit : seul un résumé JSON est imprimé.
--out refuse d'écraser un fichier existant. --out-dir écrit DOSSIER/AAAAMMJJ-HHMM-<8 premiers
caractères de l'id>.txt (heure locale de l'extraction) et crée le dossier si besoin.

Localisation (--session-id) : <config>/projects/<dossier encodé>/<id>.jsonl, où <config> est
--config-dir, sinon CLAUDE_CONFIG_DIR, sinon ~/.claude, et où le dossier encodé est le chemin
absolu du projet dont tout caractère non alphanumérique est remplacé par « - » (CLAUDE_CODE_PROJECT_DIR_NAME
est respecté quand CLAUDE_CONFIG_DIR est défini).

Attribution : seul [USER] est un message humain. Le raisonnement interne (thinking) et les
conversations de sous-agents (isSidechain) sont exclus. Les rappels système, notifications,
messages d'autres agents et sorties d'outils ne sont jamais étiquetés [USER].

Format interne non documenté et instable : validation souple. Un type d'entrée inconnu est compté
et listé (nom seulement). Si aucun tour n'est reconnu ou si la part d'entrées non reconnues dépasse
UNKNOWN_MAX_SHARE, rien n'est écrit (code 3).

Le masquage des secrets (clés, jetons, clés privées, en-têtes Authorization) couvre des formes
évidentes. Ce n'est pas une garantie : relire l'extrait avant de le conserver ou de le commiter.

Codes de sortie : 0 ok ; 2 usage, fichier introuvable ou sortie déjà existante ; 3 format non reconnu.
Aucun accès réseau. Aucun appel à /compact.
"""

import argparse
import json
import os
import re
import sys
from datetime import datetime
from pathlib import Path

SCRIPT_VERSION = "1"
UNKNOWN_MAX_SHARE = 0.20  # au-delà : format non reconnu, code 3
MASK = "[MASQUÉ]"
AGENT_MAX_LINES = 80
AGENT_MAX_CHARS = 8000
PARAM_MAX_CHARS = 200
PARAMS_TOTAL_MAX = 800

# Types d'entrée connus qui ne sont pas des tours de conversation (métadonnées de Claude Code).
KNOWN_IGNORED = {
    "attachment", "queue-operation", "system", "custom-title", "last-prompt", "atis-latch",
    "bridge-session", "mode", "agent-name", "agent-setting", "api-request", "api-request-blob",
    "api-request-shape", "file-history-snapshot", "file-history-delta", "frame-link", "tools-used",
    "relocated", "artifact-comment-monitor", "artifact-autoreact-ledger", "cost-state",
    "session-owner", "permission-mode", "summary", "progress", "ai-title", "pr-link",
}
# Balises d'un message user qui n'est pas une parole humaine.
SYSTEM_TAGS = {
    "system-reminder", "local-command-caveat", "local-command-stdout", "local-command-stderr",
    "task-notification", "bash-stdout", "bash-stderr", "user-prompt-submit-hook",
}
# Balises produites par une action humaine (commande tapée, contenu collé).
HUMAN_TAGS = {"command-name", "command-message", "command-args", "bash-input", "pasted_content"}
AGENT_KINDS = {"peer", "coordinator"}

SECRET_PATTERNS = [
    (re.compile(r"-----BEGIN [A-Z ]*PRIVATE KEY-----.*?(?:-----END [A-Z ]*PRIVATE KEY-----|\Z)", re.S), MASK),
    (re.compile(r"(?i)\b(Authorization\s*[:=]\s*)[^\r\n]+"), r"\1" + MASK),
    (re.compile(r"(?i)\b(Bearer\s+)[A-Za-z0-9._~+/=-]{12,}"), r"\1" + MASK),
    (re.compile(
        r"(?i)\b([a-z0-9_.-]*(?:api[_-]?key|apikey|secret|token|passwd|password|pwd|access[_-]?key|private[_-]?key)"
        r"[\"']?\s*[:=]\s*[\"']?)(?!\[MASQUÉ\])[^\s\"',;&)}\]]{6,}"), r"\1" + MASK),
    (re.compile(r"(?<=://)[^/\s:@]+:[^/\s@]+(?=@)"), MASK),
    (re.compile(r"\bsk-[A-Za-z0-9_-]{16,}"), MASK),
    (re.compile(r"\b(?:sk|rk|pk)_(?:live|test)_[A-Za-z0-9]{16,}"), MASK),
    (re.compile(r"\bgh[pousr]_[A-Za-z0-9]{20,}"), MASK),
    (re.compile(r"\bgithub_pat_[A-Za-z0-9_]{20,}"), MASK),
    (re.compile(r"\bglpat-[A-Za-z0-9_-]{20,}"), MASK),
    (re.compile(r"\bAKIA[0-9A-Z]{16}\b"), MASK),
    (re.compile(r"\bAIza[0-9A-Za-z_-]{30,}"), MASK),
    (re.compile(r"\bxox[abprs]-[A-Za-z0-9-]{10,}"), MASK),
    (re.compile(r"\bnpm_[A-Za-z0-9]{30,}"), MASK),
    (re.compile(r"\bhf_[A-Za-z0-9]{30,}"), MASK),
    (re.compile(r"\beyJ[A-Za-z0-9_-]{8,}\.[A-Za-z0-9_-]{8,}\.[A-Za-z0-9_-]{8,}"), MASK),
]
LINE_BREAKERS = dict.fromkeys(map(ord, "\x0b\x0c\x1c\x1d\x1e\x85  "), " ")
REMINDER_RE = re.compile(r"<system-reminder>(.*?)</system-reminder>", re.S)
TAG_RE = re.compile(r"\s*<([A-Za-z_][\w:-]*)")
ID_RE = re.compile(r"^[A-Za-z0-9][A-Za-z0-9_-]{0,127}$")


class Stop(Exception):
    def __init__(self, code, msg, extra=None):
        super().__init__(msg)
        self.code = code
        self.msg = msg
        self.extra = extra or {}


# --------------------------------------------------------------------------- utilitaires


def mask(text, counter):
    for pattern, repl in SECRET_PATTERNS:
        text, n = pattern.subn(repl, text)
        counter[0] += n
    return text


def clean(text):
    return text.replace("\r\n", "\n").replace("\r", "\n").translate(LINE_BREAKERS)


def truncate(text, max_lines, max_chars):
    """Retourne (texte, lignes_retirées, caractères_retirés). Les retraits sont dits dans le texte."""
    lines = text.split("\n")
    kept, used = [], 0
    for ln in lines:
        if len(kept) >= max_lines or used >= max_chars:
            break
        room = max_chars - used
        if len(ln) > room:
            kept.append(ln[:room])
            used = max_chars
            break
        kept.append(ln)
        used += len(ln) + 1
    kept_text = "\n".join(kept)
    removed_chars = len(text) - len(kept_text)
    removed_lines = len(lines) - len(kept)
    if removed_chars <= 0:
        return text, 0, 0
    note = "[… tronqué : %d ligne(s) retirée(s), %d caractère(s) retiré(s)]" % (removed_lines, removed_chars)
    return kept_text + "\n" + note, removed_lines, removed_chars


def safe_name(name):
    name = re.sub(r"[^A-Za-z0-9_.:-]", "_", str(name))[:60]
    return name or "?"


def parse_ts(value):
    if not isinstance(value, str):
        return None
    try:
        return datetime.fromisoformat(value.replace("Z", "+00:00"))
    except ValueError:
        return None


def encode_project(path):
    return re.sub(r"[^A-Za-z0-9]", "-", str(path))


def locate(session_id, project, config_dir):
    if not ID_RE.match(session_id):
        raise Stop(2, "identifiant de session invalide")
    cfg = Path(config_dir or os.environ.get("CLAUDE_CONFIG_DIR") or (Path.home() / ".claude"))
    projects = cfg / "projects"
    pinned = os.environ.get("CLAUDE_CODE_PROJECT_DIR_NAME") if os.environ.get("CLAUDE_CONFIG_DIR") or config_dir else None
    name = pinned or encode_project(Path(project).resolve())
    candidates = [projects / name]
    if projects.is_dir() and not (projects / name).is_dir():
        # variante de casse (Windows), puis nom tronqué à 200 caractères suivi d'un hachage
        for d in sorted(projects.iterdir()):
            if d.is_dir() and (d.name.lower() == name.lower() or (len(name) > 200 and d.name.startswith(name[:200]))):
                candidates.append(d)
    for d in candidates:
        f = d / (session_id + ".jsonl")
        if f.is_file():
            return f
    raise Stop(2, "transcription introuvable : %s (dossier projet attendu : %s). "
                  "Vérifier l'identifiant, le dossier du projet et CLAUDE_CONFIG_DIR." % (session_id + ".jsonl", projects / name))


# --------------------------------------------------------------------------- lecture


class Reader:
    def __init__(self, max_lines, max_chars):
        self.max_lines, self.max_chars = max_lines, max_chars
        self.turns = []  # {role, label, ts, body}
        self.masked = [0]
        self.counts = {"lignes_non_vides": 0, "lignes_invalides": 0, "malformees": 0, "sidechain_exclus": 0,
                       "thinking_exclus": 0, "doublons_ignores": 0, "tool_results_tronques": 0,
                       "lignes_retirees": 0, "caracteres_retires": 0, "user_origine_inferee": 0,
                       "entrees_ignorees_connues": 0}
        self.unknown_types, self.unknown_blocks, self.unknown_origins = {}, {}, {}
        self.warnings = []
        self.session_ids, self.versions = {}, {}
        self.first_ts = self.last_ts = None
        self.seen_uuid, self.tool_names = set(), {}
        self.lost_looking = {}

    # -- fabrication d'un tour
    def add(self, role, label, ts, text, limits=None, note=None, premasked=False):
        text = clean(text).strip("\n")
        if not text.strip():
            return
        if not premasked:
            text = mask(text, self.masked)
        if limits:
            text, rl, rc = truncate(text, *limits)
            if rc:
                self.counts["lignes_retirees"] += rl
                self.counts["caracteres_retires"] += rc
                if role == "tool":
                    self.counts["tool_results_tronques"] += 1
        self.turns.append({"role": role, "label": label, "ts": ts, "body": text.split("\n"), "note": note})

    def user_text(self, obj, ts, text, kind, forced=None):
        """Classe un message de rôle user (hors résultats d'outils) et ajoute le ou les tours."""
        tag = (TAG_RE.match(text) or [None, None])[1]
        sys_limits = (self.max_lines, self.max_chars)
        if obj.get("isCompactSummary") is True:
            return self.add("system", "SYSTEM", ts, text, sys_limits, "résumé de compactage")
        if kind in AGENT_KINDS:
            who = (obj.get("origin") or {}).get("from")
            note = ("de " + safe_name(who)) if isinstance(who, str) else None
            return self.add("agent_message", "AGENT-MESSAGE", ts, text, (AGENT_MAX_LINES, AGENT_MAX_CHARS), note)
        if obj.get("isMeta") is True:
            return self.add("system", "SYSTEM", ts, text, sys_limits)
        if kind == "human" or kind is None:
            if kind is None:
                if text.lstrip().startswith("[Request interrupted"):
                    return self.add("system", "SYSTEM", ts, text, sys_limits)
                if tag and tag not in HUMAN_TAGS:
                    return self.add("system", "SYSTEM", ts, text, sys_limits)
            elif tag in SYSTEM_TAGS and tag != "system-reminder":
                return self.add("system", "SYSTEM", ts, text, sys_limits)
            # rappels système insérés dans un message humain : séparés, jamais en [USER]
            pos, parts = 0, []
            for m in REMINDER_RE.finditer(text):
                parts.append(("user", text[pos:m.start()]))
                parts.append(("system", m.group(1)))
                pos = m.end()
            parts.append(("user", text[pos:]))
            for who, chunk in parts:
                if not chunk.strip():
                    continue
                if who == "system":
                    self.add("system", "SYSTEM", ts, chunk, sys_limits, "rappel inséré dans un message user")
                else:
                    if kind is None:
                        self.counts["user_origine_inferee"] += 1
                    self.add("user", "USER", ts, chunk)
            return
        # origine déclarée mais autre (task-notification, inconnue...) : système, jamais [USER]
        if kind != "task-notification":
            self.unknown_origins[str(kind)[:40]] = self.unknown_origins.get(str(kind)[:40], 0) + 1
        return self.add("system", "SYSTEM", ts, text, sys_limits)

    def tool_result(self, block, ts):
        name = self.tool_names.get(block.get("tool_use_id"), "?")
        content = block.get("content")
        if isinstance(content, str):
            text = content
        elif isinstance(content, list):
            pieces = []
            for b in content:
                if isinstance(b, dict) and b.get("type") == "text" and isinstance(b.get("text"), str):
                    pieces.append(b["text"])
                elif isinstance(b, dict):
                    pieces.append("[bloc %s omis]" % safe_name(b.get("type", "?")))
            text = "\n".join(pieces)
        else:
            text = ""
        if not text.strip():
            text = "(vide)"
        note = "erreur" if block.get("is_error") is True else None
        self.add("tool", "TOOL-RESULT:" + safe_name(name), ts, text, (self.max_lines, self.max_chars), note)

    def tool_use(self, block, ts):
        name = safe_name(block.get("name", "?"))
        if isinstance(block.get("id"), str):
            self.tool_names[block["id"]] = name
        inp = block.get("input")
        if isinstance(inp, dict):
            parts = []
            for k, v in inp.items():
                s = v if isinstance(v, str) else json.dumps(v, ensure_ascii=False)
                s = mask(clean(s), self.masked).replace("\n", "↵")  # masquer avant de couper
                if len(s) > PARAM_MAX_CHARS:
                    s = s[:PARAM_MAX_CHARS] + "…(+%d car.)" % (len(s) - PARAM_MAX_CHARS)
                parts.append("%s=%s" % (k, s))
            text = "paramètres : " + (", ".join(parts) if parts else "(aucun)")
        else:
            text = "paramètres : " + mask(clean(str(inp)), self.masked)[:PARAM_MAX_CHARS]
        if len(text) > PARAMS_TOTAL_MAX:
            text = text[:PARAMS_TOTAL_MAX] + "…"
        self.add("tool", "TOOL:" + name, ts, text, premasked=True)

    # -- entrées
    def feed(self, obj):
        t = obj.get("type")
        ts = obj.get("timestamp") if isinstance(obj.get("timestamp"), str) else None
        if obj.get("isSidechain") is True:
            self.counts["sidechain_exclus"] += 1
            return
        if t in ("user", "assistant"):
            uid = obj.get("uuid")
            if isinstance(uid, str):
                if uid in self.seen_uuid:
                    self.counts["doublons_ignores"] += 1
                    return
                self.seen_uuid.add(uid)
            msg = obj.get("message")
            if not isinstance(msg, dict) or not isinstance(msg.get("content"), (str, list)):
                self.counts["malformees"] += 1
                return
            content = msg["content"]
            blocks = [{"type": "text", "text": content}] if isinstance(content, str) else content
            if t == "assistant":
                synthetic = msg.get("model") == "<synthetic>"
                for b in blocks:
                    if not isinstance(b, dict):
                        self.unknown_blocks["assistant:?"] = self.unknown_blocks.get("assistant:?", 0) + 1
                        continue
                    bt = b.get("type")
                    if bt == "text" and isinstance(b.get("text"), str):
                        if synthetic:
                            self.add("system", "SYSTEM", ts, b["text"], (self.max_lines, self.max_chars), "message synthétique")
                        else:
                            self.add("assistant", "ASSISTANT", ts, b["text"])
                    elif bt in ("thinking", "redacted_thinking"):
                        self.counts["thinking_exclus"] += 1
                    elif bt == "tool_use":
                        self.tool_use(b, ts)
                    else:
                        k = "assistant:" + safe_name(bt)
                        self.unknown_blocks[k] = self.unknown_blocks.get(k, 0) + 1
                return
            texts = []
            for b in blocks:
                if not isinstance(b, dict):
                    self.unknown_blocks["user:?"] = self.unknown_blocks.get("user:?", 0) + 1
                    continue
                bt = b.get("type")
                if bt == "tool_result":
                    self.tool_result(b, ts)
                elif bt == "text" and isinstance(b.get("text"), str):
                    texts.append(b["text"])
                elif bt == "image":
                    texts.append("[image omise]")
                else:
                    k = "user:" + safe_name(bt)
                    self.unknown_blocks[k] = self.unknown_blocks.get(k, 0) + 1
            if texts:
                org = obj.get("origin")
                kind = org.get("kind") if isinstance(org, dict) else (org if isinstance(org, str) else None)
                self.user_text(obj, ts, clean("\n".join(texts)), kind)
            return
        if t == "attachment":
            att = obj.get("attachment")
            org = att.get("origin") if isinstance(att, dict) else None
            kind = org.get("kind") if isinstance(org, dict) else None
            if isinstance(att, dict) and att.get("type") == "queued_command" and kind == "human":
                p = att.get("prompt")
                if isinstance(p, list):
                    p = "\n".join(b.get("text", "") for b in p if isinstance(b, dict) and b.get("type") == "text")
                if isinstance(p, str):
                    self.user_text({}, att.get("timestamp") if isinstance(att.get("timestamp"), str) else ts, clean(p), "human")
                    return
            self.counts["entrees_ignorees_connues"] += 1
            return
        if t in KNOWN_IGNORED:
            self.counts["entrees_ignorees_connues"] += 1
            return
        key = str(t)[:40] if isinstance(t, str) else "<sans type>"
        self.unknown_types[key] = self.unknown_types.get(key, 0) + 1
        if isinstance(obj.get("message"), dict) and "role" in obj["message"]:
            self.lost_looking[key] = self.lost_looking.get(key, 0) + 1

    def read(self, path):
        with open(path, "r", encoding="utf-8", errors="replace") as fh:
            for raw in fh:
                raw = raw.strip()
                if not raw:
                    continue
                self.counts["lignes_non_vides"] += 1
                try:
                    obj = json.loads(raw)
                except ValueError:
                    self.counts["lignes_invalides"] += 1
                    continue
                if not isinstance(obj, dict):
                    self.counts["lignes_invalides"] += 1
                    continue
                dt = parse_ts(obj.get("timestamp"))
                if dt is not None:
                    if self.first_ts is None or dt < self.first_ts[0]:
                        self.first_ts = (dt, obj["timestamp"])
                    if self.last_ts is None or dt > self.last_ts[0]:
                        self.last_ts = (dt, obj["timestamp"])
                sid = obj.get("sessionId")
                if isinstance(sid, str):
                    self.session_ids[sid] = self.session_ids.get(sid, 0) + 1
                if isinstance(obj.get("version"), str):
                    self.versions[obj["version"]] = self.versions.get(obj["version"], 0) + 1
                self.feed(obj)


# --------------------------------------------------------------------------- rendu


def build(reader, session_id, origin_file, extracted_at):
    ranges = {"user": [], "assistant": [], "tool": [], "system": [], "agent_message": []}
    n_header = 15
    body, line_no = [], n_header
    for turn in reader.turns:
        marker = "[%s]" % turn["label"] + (" " + turn["ts"] if turn["ts"] else " (horodatage absent)")
        if turn["note"]:
            marker += " (" + turn["note"] + ")"
        lines = [marker] + ["  " + ln for ln in turn["body"]]
        start = line_no + 1
        ranges[turn["role"]].append((start, start + len(lines) - 1))
        body.extend(lines + [""])
        line_no += len(lines) + 1
    first = reader.first_ts[1] if reader.first_ts else "inconnu"
    last = reader.last_ts[1] if reader.last_ts else "inconnu"
    header = [
        "# EXTRAIT DE SESSION CLAUDE CODE (read-session.py v%s)" % SCRIPT_VERSION,
        "# session_id: %s" % session_id,
        "# fichier_origine: %s" % origin_file,
        "# premier_horodatage: %s" % first,
        "# dernier_horodatage: %s" % last,
        "# extrait_le: %s" % extracted_at,
        "# role: source de capture traçable ; extrait filtré, pas la conversation intégrale. Les numéros de ligne de ce fichier servent d'origin.locators (kind lines).",
        "# marqueurs: [USER] message humain ; [ASSISTANT] réponse de l'assistant ; [TOOL:nom] appel d'outil ; [TOOL-RESULT:nom] sortie d'outil tronquée ; [SYSTEM] rappel, injection ou notification système ; [AGENT-MESSAGE] message d'un autre agent ou d'un sous-agent.",
        "# attribution: seul [USER] est un message humain. Un contenu collé par l'utilisateur reste dans [USER] : coller n'est pas approuver. Ni [SYSTEM], ni [AGENT-MESSAGE], ni une sortie d'outil n'est une parole de l'utilisateur.",
        "# exclus: raisonnement interne (thinking) ; conversations de sous-agents (isSidechain).",
        "# masquage: formes évidentes de secrets remplacées par %s ; ce n'est pas une garantie, relire avant de conserver ou de commiter." % MASK,
        "# fraicheur: la transcription est écrite en continu ; elle peut être en retard sur la conversation en cours.",
        "# format_corps: chaque ligne de contenu est indentée de deux espaces ; seules les lignes de marqueur commencent à la colonne 0.",
        "# source_format: transcription JSONL de Claude Code, format interne non documenté et instable.",
        "# ---",
    ]
    assert len(header) == n_header, len(header)
    text = "\n".join(header + body)
    if not text.endswith("\n"):
        text += "\n"
    return text, ranges


def fmt_ranges(rs):
    return ", ".join("%d-%d" % r for r in rs)


def main():
    for _stream in (sys.stdout, sys.stderr):
        if hasattr(_stream, "reconfigure"):
            _stream.reconfigure(encoding="utf-8")
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--session-id")
    ap.add_argument("--project", help="dossier de travail de la session")
    ap.add_argument("--transcript", help="chemin direct d'une transcription .jsonl")
    ap.add_argument("--config-dir", help="dossier de configuration Claude Code (défaut : CLAUDE_CONFIG_DIR, sinon ~/.claude)")
    ap.add_argument("--out", help="fichier d'extrait à créer (refuse d'écraser)")
    ap.add_argument("--out-dir", help="dossier où créer AAAAMMJJ-HHMM-<id8>.txt")
    ap.add_argument("--max-tool-lines", type=int, default=20)
    ap.add_argument("--max-tool-chars", type=int, default=2000)
    sys.stdout.reconfigure(encoding="utf-8")
    sys.stderr.reconfigure(encoding="utf-8")
    args = ap.parse_args()
    try:
        if args.out and args.out_dir:
            raise Stop(2, "--out et --out-dir sont exclusifs")
        if bool(args.transcript) == bool(args.session_id):
            raise Stop(2, "donner soit --session-id avec --project, soit --transcript")
        if args.session_id and not args.project:
            raise Stop(2, "--session-id demande --project")
        if args.max_tool_lines < 1 or args.max_tool_chars < 1:
            raise Stop(2, "limites d'outils invalides")
        if args.out and os.path.lexists(args.out):
            raise Stop(2, "refus d'écraser un fichier existant : %s" % args.out)
        if args.transcript:
            path = Path(args.transcript)
            if not path.is_file():
                raise Stop(2, "transcription introuvable : %s" % path)
        else:
            path = locate(args.session_id, args.project, args.config_dir)
        reader = Reader(args.max_tool_lines, args.max_tool_chars)
        reader.read(path)
        c = reader.counts
        sid = args.session_id
        if not sid and reader.session_ids:
            sid = max(reader.session_ids, key=reader.session_ids.get)
        if not sid and ID_RE.match(path.stem):
            sid = path.stem
        sid = sid or "inconnu"
        unknown_entries = sum(reader.unknown_types.values()) + c["lignes_invalides"] + c["malformees"]
        share = unknown_entries / c["lignes_non_vides"] if c["lignes_non_vides"] else 1.0
        mtime = datetime.fromtimestamp(path.stat().st_mtime).astimezone().isoformat(timespec="seconds")
        now = datetime.now().astimezone()
        summary = {
            "ok": True, "session_id": sid, "transcript": str(path), "fichier_modifie_le": mtime,
            "premier_horodatage": reader.first_ts[1] if reader.first_ts else None,
            "dernier_horodatage": reader.last_ts[1] if reader.last_ts else None,
            "versions_claude_code": sorted(reader.versions), "session_ids_dans_le_fichier": len(reader.session_ids),
            "lignes_lues": c["lignes_non_vides"], "types_inconnus": reader.unknown_types,
            "blocs_inconnus": reader.unknown_blocks, "origines_inconnues": reader.unknown_origins,
            "lignes_invalides": c["lignes_invalides"], "entrees_malformees": c["malformees"],
            "part_inconnue": round(share, 3), "seuil_part_inconnue": UNKNOWN_MAX_SHARE,
            "thinking_exclus": c["thinking_exclus"], "sidechain_exclus": c["sidechain_exclus"],
            "doublons_ignores": c["doublons_ignores"], "entrees_ignorees_connues": c["entrees_ignorees_connues"],
            "user_origine_inferee": c["user_origine_inferee"],
            "tool_results_tronques": c["tool_results_tronques"], "lignes_retirees": c["lignes_retirees"],
            "caracteres_retires": c["caracteres_retires"], "masquages": reader.masked[0],
            "avertissements": [
                "La transcription est écrite en continu : elle peut être en retard sur la conversation (dernier horodatage lu ci-dessus).",
                "Le masquage des secrets n'est pas une garantie : relire l'extrait avant de le conserver ou de le commiter.",
            ],
        }
        if reader.lost_looking:
            summary["avertissements"].append(
                "Types inconnus contenant un message (tours peut-être non lus) : %s." % ", ".join(sorted(reader.lost_looking)))
        if c["lignes_invalides"]:
            summary["avertissements"].append(
                "%d ligne(s) JSON invalide(s) ignorée(s) (la dernière peut être en cours d'écriture)." % c["lignes_invalides"])
        if c["user_origine_inferee"]:
            summary["avertissements"].append(
                "%d message(s) user sans origine déclarée classés [USER] d'après leur forme seulement." % c["user_origine_inferee"])
        if not reader.turns or share > UNKNOWN_MAX_SHARE:
            why = "aucun tour reconnu" if not reader.turns else "part d'entrées non reconnues %.0f %% > seuil %.0f %%" % (
                share * 100, UNKNOWN_MAX_SHARE * 100)
            summary["ok"] = False
            summary["ecrit"] = False
            summary["erreur"] = ("format de transcription non reconnu (%s). Le format interne de Claude Code peut avoir changé : "
                                 "aucun extrait n'a été écrit." % why)
            raise Stop(3, summary["erreur"], summary)
        text, ranges = build(reader, sid, str(path), now.isoformat(timespec="seconds"))
        out = None
        if args.out:
            out = Path(args.out)
        elif args.out_dir:
            out = Path(args.out_dir) / ("%s-%s.txt" % (now.strftime("%Y%m%d-%H%M"), sid[:8]))
        summary["ecrit"] = False
        summary["out"] = out.as_posix() if out else None
        if out:
            if os.path.lexists(out):
                raise Stop(2, "refus d'écraser un fichier existant : %s" % out.as_posix())
            out.parent.mkdir(parents=True, exist_ok=True)
            try:
                with open(out, "x", encoding="utf-8", newline="\n") as fh:
                    fh.write(text)
            except FileExistsError:
                raise Stop(2, "refus d'écraser un fichier existant : %s" % out.as_posix())
            summary["ecrit"] = True
        summary["lignes_extrait"] = len(text.split("\n")) - 1
        summary["tours"] = {k: len(v) for k, v in ranges.items()}
        summary["tours"]["tool_use"] = sum(1 for t in reader.turns if t["label"].startswith("TOOL:"))
        summary["tours"]["tool_result"] = sum(1 for t in reader.turns if t["label"].startswith("TOOL-RESULT:"))
        summary["plages_par_role"] = {k: fmt_ranges(v) for k, v in ranges.items()}
        summary["role_locator"] = {
            "user": "user", "assistant": "assistant",
            "tool": "unknown (note: sortie ou appel d'outil)", "system": "unknown (note: message système)",
            "agent_message": "unknown (note: message d'agent)",
        }
        sys.stdout.write(json.dumps(summary, ensure_ascii=False, indent=2) + "\n")
        return 0
    except Stop as exc:
        payload = exc.extra or {"ok": False, "erreur": exc.msg}
        sys.stdout.write(json.dumps(payload, ensure_ascii=False, indent=2) + "\n")
        sys.stderr.write(exc.msg + "\n")
        return exc.code


if __name__ == "__main__":
    sys.exit(main())
