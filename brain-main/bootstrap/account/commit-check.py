# -*- coding: utf-8 -*-
"""Hook PreToolUse (matcher Bash|PowerShell) de 2brain : relecture des README avant un `git commit`.

Lit l'événement sur l'entrée standard. Ne dit rien (code 0, aucune sortie) si la commande n'est pas un
`git commit`, si le dossier n'est pas un dépôt Git ou n'a pas de `brain-main/`. Sinon, pour chaque fichier du
commit (indexé ; avec `-a` ou `git add` plus haut dans la commande, aussi les fichiers modifiés), il retient le
README le plus proche en remontant les dossiers (le README racine seulement pour un fichier de la racine).
La première fois que ce contenu est présenté, l'appel est refusé avec la liste des README à relire ; l'empreinte
du contenu contrôlé est gardée dans le dossier Git (jamais dans un fichier suivi) : la même tentative passe
ensuite. Les fichiers README ne comptent pas dans l'empreinte, donc corriger un README ne redéclenche rien.
Toute erreur laisse passer. Aucun journal, aucun accès réseau, code de sortie toujours 0.
"""

import hashlib
import json
import os
import subprocess
import sys
from pathlib import Path

STATE_NAME = "2brain-readme-check.json"
SHOW_MAX = 12
EMPTY_TREE = "4b825dc642cb6eb9a060e54bf8d69288fbee4904"
# options de `git` (avant le sous-commande) qui prennent une valeur séparée
GIT_VALUE_OPTS = {"-C", "-c", "--git-dir", "--work-tree", "--namespace", "--exec-path", "--config-env"}
# options courtes / longues de `git commit` qui prennent une valeur séparée
COMMIT_SHORT_VALUE = set("mFCctu")
COMMIT_LONG_VALUE = {"--message", "--file", "--author", "--date", "--template", "--cleanup", "--reuse-message",
                     "--reedit-message", "--squash", "--fixup", "--pathspec-from-file", "--trailer",
                     "--gpg-sign", "--untracked-files"}
COMMIT_SHORT_OPTIONAL = {"u", "S"}  # valeur collée seulement


# ---------------------------------------------------------------- analyse de la commande

def split_segments(command):
    """Découpe une commande shell en segments de mots (guillemets respectés, antislash conservé sauf devant espace ou guillemet)."""
    segments, words, cur, has = [], [], [], False
    quote = None
    i, n = 0, len(command)

    def end_word():
        nonlocal cur, has
        if has:
            words.append("".join(cur))
        cur, has = [], False

    def end_seg():
        nonlocal words
        end_word()
        if words:
            segments.append(words)
        words = []

    while i < n:
        c = command[i]
        if quote:
            if c == quote:
                quote = None
            elif c == "\\" and quote == '"' and i + 1 < n and command[i + 1] in '"\\$`':
                i += 1
                cur.append(command[i])
            else:
                cur.append(c)
            i += 1
            continue
        if c in "'\"":
            quote, has = c, True
        elif c == "\\" and i + 1 < n and command[i + 1] in " \t'\"\\":
            i += 1
            cur.append(command[i])
            has = True
        elif c in " \t\r":
            end_word()
        elif c in "\n;":
            end_seg()
        elif c in "&|":
            end_seg()
            if i + 1 < n and command[i + 1] == c:
                i += 1
        elif c in "()":
            end_seg()
        else:
            cur.append(c)
            has = True
        i += 1
    end_seg()
    return segments


def is_env_assign(word):
    if "=" not in word:
        return False
    name = word.split("=", 1)[0]
    return bool(name) and name.replace("_", "a").isalnum() and not name[0].isdigit()


def join_dir(base, path):
    """Dossier `path` (éventuellement relatif) résolu depuis `base`, avec les chemins de style Git Bash (/c/...) sous Windows."""
    if os.name == "nt" and len(path) >= 3 and path[0] == "/" and path[1].isalpha() and path[2] == "/":
        path = path[1] + ":" + path[2:]
    elif path == "~" or path.startswith("~/"):
        path = str(Path.home()) + path[1:]
    return str(Path(base, path))


def parse_git(words, cwd):
    """Si ces mots sont un `git ... commit`, renvoie (dossier, options de commit) sinon None."""
    i = 0
    while i < len(words) and is_env_assign(words[i]):
        i += 1
    if i >= len(words) or os.path.basename(words[i].replace("\\", "/")).lower() not in ("git", "git.exe"):
        return None
    i += 1
    where = cwd
    while i < len(words):
        w = words[i]
        if w == "-C" and i + 1 < len(words):
            where = join_dir(where, words[i + 1])
            i += 2
        elif w in GIT_VALUE_OPTS:
            i += 2
        elif w.startswith("-"):
            i += 1
        else:
            break
    if i >= len(words) or words[i] != "commit":
        return None
    opts = {"all": False, "amend": False, "dry": False, "paths": []}
    rest = words[i + 1:]
    j = 0
    only_paths = False
    while j < len(rest):
        w = rest[j]
        j += 1
        if only_paths:
            opts["paths"].append(w)
        elif w == "--":
            only_paths = True
        elif w.startswith("--"):
            name = w.split("=", 1)[0]
            if name == "--all":
                opts["all"] = True
            elif name == "--amend":
                opts["amend"] = True
            elif name == "--dry-run":
                opts["dry"] = True
            elif name in COMMIT_LONG_VALUE and "=" not in w and name not in ("--gpg-sign", "--untracked-files"):
                j += 1
        elif w.startswith("-") and len(w) > 1:
            letters = w[1:]
            for k, ch in enumerate(letters):
                if ch == "a":
                    opts["all"] = True
                if ch in COMMIT_SHORT_OPTIONAL:
                    break
                if ch in COMMIT_SHORT_VALUE:
                    if k == len(letters) - 1:
                        j += 1
                    break
        else:
            opts["paths"].append(w)
    return where, opts


def add_scope(words):
    """Si ces mots sont un `git ... add`, renvoie (tout, mis_a_jour_seulement, chemins) sinon None."""
    i = 0
    while i < len(words) and is_env_assign(words[i]):
        i += 1
    if i >= len(words) or os.path.basename(words[i].replace("\\", "/")).lower() not in ("git", "git.exe"):
        return None
    i += 1
    while i < len(words):
        if words[i] == "-C" and i + 1 < len(words):
            i += 2
        elif words[i] in GIT_VALUE_OPTS:
            i += 2
        elif words[i].startswith("-"):
            i += 1
        else:
            break
    if i >= len(words) or words[i] != "add":
        return None
    everything, update, paths, only_paths = False, False, [], False
    for w in words[i + 1:]:
        if only_paths:
            paths.append(w)
        elif w == "--":
            only_paths = True
        elif w in ("-A", "--all"):
            everything = True
        elif w in ("-u", "--update"):
            update = True
        elif w.startswith("--"):
            continue
        elif w.startswith("-") and len(w) > 1:
            if "A" in w[1:]:
                everything = True
            if "u" in w[1:]:
                update = True
        else:
            paths.append(w)
    return everything, update, paths


CD_NAMES = ("cd", "chdir", "set-location", "sl", "push-location", "pushd")


def cd_target(args):
    """Dossier visé par cd / Set-Location / Push-Location (options -Path et -LiteralPath comprises), sinon None."""
    i = 0
    while i < len(args):
        a = args[i]
        if a.lower() in ("-path", "-literalpath") and i + 1 < len(args):
            return args[i + 1]
        if a.startswith("-") and a not in ("-",):
            i += 1
            continue
        return a
    return None


# ---------------------------------------------------------------- Git

def git_run(top, args):
    env = dict(os.environ, GIT_OPTIONAL_LOCKS="0")
    p = subprocess.run(["git", "-c", "core.quotepath=off", "-C", top] + list(args), capture_output=True,
                       timeout=20, stdin=subprocess.DEVNULL, env=env)
    return p.returncode, p.stdout


def nul_list(data):
    return [x.decode("utf-8", "surrogateescape") for x in data.split(b"\0") if x]


def hash_worktree(top, paths):
    """Empreinte Git du contenu du répertoire de travail pour chaque chemin ('supprimé' si absent)."""
    out = {}
    existing = []
    for p in paths:
        if os.path.isfile(os.path.join(top, p)):
            existing.append(p)
        else:
            out[p] = "supprimé"
    for k in range(0, len(existing), 50):
        chunk = existing[k:k + 50]
        rc, data = git_run(top, ["hash-object", "--"] + chunk)
        lines = data.decode("utf-8", "replace").split()
        if rc == 0 and len(lines) == len(chunk):
            out.update(zip(chunk, lines))
        else:
            for p in chunk:
                out[p] = "inconnu"
    return out


def scoped(path, specs):
    path = path.replace("\\", "/")
    for s in specs:
        s = s.replace("\\", "/")
        s = "" if s in (".", "./") else (s[2:] if s.startswith("./") else s)
        if not s or any(ch in s for ch in "*?[") or path == s.rstrip("/") or path.startswith(s.rstrip("/") + "/"):
            return True
    return False


def collect(top, opts, adds):
    """Renvoie {chemin: identifiant du contenu qui sera validé}."""
    rc, head = git_run(top, ["rev-parse", "--verify", "-q", "HEAD"])
    head_sha = head.decode().strip() if rc == 0 else None
    base = "HEAD" if head_sha else EMPTY_TREE
    if opts["amend"] and head_sha:
        rc2, _ = git_run(top, ["rev-parse", "--verify", "-q", "HEAD^"])
        base = "HEAD^" if rc2 == 0 else EMPTY_TREE
    files = {}
    rc, staged = git_run(top, ["diff", "--cached", "--name-only", "--no-renames", "-z", base])
    staged_files = nul_list(staged) if rc == 0 else []
    rc, lsfiles = git_run(top, ["ls-files", "-s", "-z"])
    index = {}
    if rc == 0:
        for rec in lsfiles.split(b"\0"):
            if rec and b"\t" in rec:
                meta, name = rec.split(b"\t", 1)
                parts = meta.split()
                if len(parts) == 3:
                    index[name.decode("utf-8", "surrogateescape")] = parts[1].decode()
    live = set()
    if opts["all"] or opts["paths"]:
        rc, mod = git_run(top, ["diff", "--name-only", "--no-renames", "-z"])
        for f in nul_list(mod) if rc == 0 else []:
            if opts["all"] or scoped(f, opts["paths"]):
                live.add(f)
    for everything, update, paths in adds:
        rc, mod = git_run(top, ["diff", "--name-only", "--no-renames", "-z"])
        cand = set(nul_list(mod) if rc == 0 else [])
        if not update:
            rc, oth = git_run(top, ["ls-files", "-o", "--exclude-standard", "-z"])
            cand |= set(nul_list(oth) if rc == 0 else [])
        for f in cand:
            if everything or not paths or scoped(f, paths):
                live.add(f)
    for f in staged_files:
        files[f] = index.get(f, "supprimé")
    hashed = hash_worktree(top, sorted(live))
    for f in live:
        files[f] = hashed.get(f, "inconnu")
    return files, (head_sha or "aucun")


def is_readme(path):
    return path.rsplit("/", 1)[-1].lower().startswith("readme")


def nearest_readme(top, rel_file, cache):
    parts = rel_file.split("/")[:-1]
    dirs = ["/".join(parts[:k]) for k in range(len(parts), 0, -1)]
    if not parts:
        dirs = [""]
    for d in dirs:
        if d not in cache:
            found = None
            try:
                names = sorted(os.listdir(os.path.join(top, d) if d else top))
            except OSError:
                names = []
            for n in names:
                if n.lower().startswith("readme") and os.path.isfile(os.path.join(top, d, n)):
                    found = (d + "/" + n) if d else n
                    if n.lower() == "readme.md":
                        break
            cache[d] = found
        if cache[d]:
            return cache[d]
    return None


def build_reason(pending):
    lines = ["Contrôle des README avant commit (2brain). Les fichiers de ce commit relèvent de ces README, à relire :"]
    for readme, files in sorted(pending.items()):
        shown = ", ".join(sorted(files)[:SHOW_MAX])
        if len(files) > SHOW_MAX:
            shown += " et %d autre(s)" % (len(files) - SHOW_MAX)
        lines.append("- %s : %s" % (readme, shown))
    lines += [
        "Consigne : relire ces README ; corriger seulement ce qui est devenu faux, ne rien réécrire d'autre ; "
        "indexer les corrections éventuelles (git add) ; relancer le même commit ; "
        "dire dans le rapport quels README ont été relus et lesquels corrigés.",
    ]
    return "\n".join(lines)


def main():
    for _stream in (sys.stdout, sys.stderr):
        if hasattr(_stream, "reconfigure"):
            _stream.reconfigure(encoding="utf-8")
    if hasattr(sys.stdin, "reconfigure"):
        sys.stdin.reconfigure(encoding="utf-8")
    data = json.loads(sys.stdin.read() or "{}")
    if not isinstance(data, dict) or data.get("tool_name") not in ("Bash", "PowerShell"):
        return
    tool_input = data.get("tool_input")
    command = tool_input.get("command") if isinstance(tool_input, dict) else None
    if not isinstance(command, str) or "commit" not in command:
        return
    cwd = str(data.get("cwd") or os.getcwd())
    where, found, adds = cwd, None, []
    for words in split_segments(command):
        if words and words[0].lower() in CD_NAMES:
            target = cd_target(words[1:])
            if target:
                where = join_dir(where, target)
            continue
        add = add_scope(words)
        if add:
            adds.append(add)
            continue
        parsed = parse_git(words, where)
        if parsed:
            found = parsed
            break
    if not found:
        return
    folder, opts = found
    if opts["dry"]:
        return
    rc, top = git_run(folder, ["rev-parse", "--show-toplevel"])
    if rc != 0:
        return
    top = top.decode("utf-8", "surrogateescape").strip()
    if not top or not os.path.isdir(os.path.join(top, "brain-main")):
        return
    rc, gitdir = git_run(top, ["rev-parse", "--absolute-git-dir"])
    if rc != 0:
        return
    state_file = os.path.join(gitdir.decode("utf-8", "surrogateescape").strip(), STATE_NAME)

    files, head = collect(top, opts, adds)
    content = {f: c for f, c in files.items() if not is_readme(f)}
    if not content:
        return
    fingerprint = hashlib.sha256(json.dumps(sorted(content.items()), ensure_ascii=True).encode("ascii")).hexdigest()
    previous = {}
    try:
        with open(state_file, encoding="utf-8") as fh:
            state = json.load(fh)
        if isinstance(state, dict) and state.get("head") == head:
            previous = state
    except Exception:
        pass
    if previous.get("fingerprint") == fingerprint:
        return
    seen = previous.get("files") if isinstance(previous.get("files"), dict) else {}
    cache, pending = {}, {}
    for f, c in content.items():
        if seen.get(f) == c:
            continue
        readme = nearest_readme(top, f, cache)
        if readme:
            pending.setdefault(readme, []).append(f)
    try:
        with open(state_file, "w", encoding="utf-8", newline="\n") as fh:
            json.dump({"head": head, "fingerprint": fingerprint, "files": content}, fh, ensure_ascii=True)
    except Exception:
        return  # sans mémoire, on ne bloque pas : un refus répété serait une boucle
    if not pending:
        return
    out = {"hookSpecificOutput": {"hookEventName": "PreToolUse", "permissionDecision": "deny",
                                  "permissionDecisionReason": build_reason(pending)}}
    sys.stdout.write(json.dumps(out, ensure_ascii=True))


try:
    main()
except BaseException:
    pass
sys.exit(0)
