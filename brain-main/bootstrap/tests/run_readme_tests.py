# -*- coding: utf-8 -*-
"""Tests autonomes du contrôle des README au commit (hook), du réglage `readme_max` et de `/brain -m`.

Usage : python brain-main/bootstrap/tests/run_readme_tests.py
Bibliothèque standard uniquement. Tout se passe dans un dossier créé sous README_TEST_TMP s'il est défini, sinon
par tempfile.mkdtemp, hors du dépôt. De vrais dépôts Git temporaires sont utilisés. Rien n'est écrit dans ~/.claude :
l'installation sur le compte utilise --home avec un dossier temporaire.
Sortie : une ligne PASS/FAIL par contrôle. Code de sortie 0 si tout passe, 1 sinon.
"""

import hashlib
import json
import os
import shutil
import stat
import subprocess
import sys
import tempfile
import traceback
from pathlib import Path

BOOT = Path(__file__).resolve().parent.parent
REPO = BOOT.parent.parent
SCRIPTS = BOOT / "scripts"
ACCOUNT = BOOT / "account"
HOOK = ACCOUNT / "commit-check.py"
SCAN = REPO / "brain-main/skills/brain-audit/scripts/scan-project.py"
SETMAX = SCRIPTS / "set-readme-max.py"
RESULTS = []
TMP = None
ANSWERS = {"nature": "Site", "objectif": "Test", "modules": [], "outils": "x", "identite": "T"}


def check(name, cond, detail=""):
    RESULTS.append((name, bool(cond)))
    print(("PASS " if cond else "FAIL ") + name + ("" if cond else "  :: " + str(detail)[:500]))


def rmtree(path):
    def onerror(func, p, _exc):
        os.chmod(p, stat.S_IWRITE)
        func(p)

    shutil.rmtree(path, onerror=onerror)


def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def tree(root):
    out = {}
    for p in sorted(Path(root).rglob("*")):
        if ".git" in p.relative_to(root).parts:
            continue
        out[p.relative_to(root).as_posix()] = "dir" if p.is_dir() else sha(p)
    return out


def new_dir(name):
    d = Path(TMP) / name
    n = 0
    while d.exists():
        n += 1
        d = Path(TMP) / ("%s-%d" % (name, n))
    d.mkdir(parents=True)
    return d


def mk(path, files):
    path = Path(path)
    path.mkdir(parents=True, exist_ok=True)
    for rel, content in files.items():
        p = path / rel
        p.parent.mkdir(parents=True, exist_ok=True)
        p.write_text(content, encoding="utf-8", newline="\n")
    return path


def run(script, *args, inp=None, env=None):
    p = subprocess.run([sys.executable, "-I", "-B", str(script), *[str(a) for a in args]], capture_output=True,
                       text=True, encoding="utf-8", input=inp, timeout=120, env=env)
    return p.returncode, p.stdout, p.stderr


def runj(script, *args, **kw):
    rc, out, err = run(script, *args, **kw)
    try:
        return rc, json.loads(out)
    except Exception:
        return rc, {"_raw": out, "_err": err}


def git(*args, cwd):
    p = subprocess.run(["git", "-c", "user.name=t", "-c", "user.email=t@t", "-c", "commit.gpgsign=false", *args],
                       cwd=cwd, capture_output=True, stdin=subprocess.DEVNULL)
    return p.returncode, p.stdout.decode("utf-8", "replace")


def repo(name, files, brain_main=True, commit=True):
    d = new_dir(name)
    git("init", "-q", cwd=d)
    mk(d, files)
    if brain_main:
        mk(d, {"brain-main/README.md": "x\n"})
    if commit:
        git("add", "-A", cwd=d)
        git("commit", "-q", "-m", "init", cwd=d)
    return d


def hook(command, cwd, raw=None, env=None, tool="Bash"):
    event = raw if raw is not None else json.dumps({"hook_event_name": "PreToolUse", "tool_name": tool,
                                                    "tool_input": {"command": command}, "cwd": str(cwd)})
    p = subprocess.run([sys.executable, str(HOOK)], input=event.encode("utf-8"), capture_output=True, timeout=120, env=env)
    return p.returncode, p.stdout.decode("utf-8"), p.stderr.decode("utf-8")


def denial(out):
    try:
        h = json.loads(out)["hookSpecificOutput"]
        return h["permissionDecisionReason"] if h["hookEventName"] == "PreToolUse" and h["permissionDecision"] == "deny" else None
    except Exception:
        return None


BASE = {"docs/README.md": "# docs\n", "docs/a.md": "a\n", "docs/b.md": "b\n", "src/x.py": "x=1\n", "README.md": "# racine\n",
        "top.py": "t=1\n", "lib/y.py": "y=1\n"}


def test_hook():
    d = repo("h-nobm", BASE, brain_main=False)
    (d / "docs/a.md").write_text("a2\n", encoding="utf-8")
    git("add", "-A", cwd=d)
    rc, out, err = hook("git commit -m x", d)
    check("hook: sans brain-main/, aucun effet (code 0, aucune sortie)", rc == 0 and out == "" and err == "", (rc, out, err))

    d = repo("h-basic", BASE)
    (d / "docs/a.md").write_text("a2\n", encoding="utf-8")
    git("add", "docs/a.md", cwd=d)
    for label, cmd in (("git status", "git status"), ("echo", "echo git commit"), ("git log", "git log --oneline"),
                       ("add seul", "git add docs/a.md"), ("message citant commit", 'git log -1 --grep="git commit"')):
        rc, out, err = hook(cmd, d)
        check("hook: commande autre qu'un commit (%s), aucun effet" % label, rc == 0 and out == "" and err == "", (rc, out, err))
    rc, out, err = hook("git commit --dry-run -m x", d)
    check("hook: git commit --dry-run ne commit rien, aucun effet", rc == 0 and out == "" and err == "", (out, err))
    rc, out, err = hook("ls", d, raw="pas du json")
    check("hook: entrée illisible, aucun effet", rc == 0 and out == "" and err == "", (out, err))
    rc, out, err = hook("git commit -m x", d, raw=json.dumps({"tool_name": "Read", "tool_input": {"file_path": "x"}}))
    check("hook: autre outil que Bash, aucun effet", rc == 0 and out == "" and err == "")

    # fichier sans README concerné : lib/y.py (aucun README dans lib/, le README racine n'est retenu que pour la racine)
    d = repo("h-none", BASE)
    (d / "lib/y.py").write_text("y=2\n", encoding="utf-8")
    git("add", "-A", cwd=d)
    rc, out, err = hook("git commit -m x", d)
    check("hook: fichier sans README dans son dossier ni au-dessus (hors racine), aucun effet", rc == 0 and out == "" and err == "", (out, err))

    # refus avec la bonne liste
    d = repo("h-deny", BASE)
    (d / "docs/a.md").write_text("a2\n", encoding="utf-8")
    (d / "src/x.py").write_text("x=2\n", encoding="utf-8")
    (d / "top.py").write_text("t=2\n", encoding="utf-8")
    (d / "lib/y.py").write_text("y=2\n", encoding="utf-8")
    git("add", "-A", cwd=d)
    rc, out, err = hook("git commit -m x", d)
    reason = denial(out)
    check("hook: refus au format PreToolUse (hookEventName, permissionDecision deny, raison), code 0, stderr vide",
          rc == 0 and reason and err == "", (rc, out, err))
    lines = (reason or "").splitlines()
    check("hook: la liste donne docs/README.md avec docs/a.md, le README racine avec top.py seulement, rien pour src/x.py ni lib/y.py",
          any(l.startswith("- docs/README.md : ") and "docs/a.md" in l and "src/x.py" not in l for l in lines)
          and any(l.startswith("- README.md : ") and "top.py" in l and "docs/a.md" not in l for l in lines)
          and "src/x.py" not in (reason or "") and "lib/y.py" not in (reason or ""), reason)
    check("hook: la raison est en français et donne la consigne complète (relire, corriger seulement le faux, indexer, relancer, rapporter)",
          all(w in (reason or "") for w in ("relire ces README", "corriger seulement ce qui est devenu faux", "ne rien réécrire d'autre",
                                           "indexer les corrections", "relancer le même commit", "quels README ont été relus et lesquels corrigés")), reason)
    st = git("status", "--porcelain", cwd=d)[1]
    gd = git("rev-parse", "--absolute-git-dir", cwd=d)[1].strip()
    check("hook: mémoire dans le dossier Git, aucun fichier suivi ni non suivi ajouté au dépôt",
          (Path(gd) / "2brain-readme-check.json").is_file() and "2brain" not in st and "??" not in st, st)
    rc, out, err = hook("git commit -m x", d)
    check("hook: second essai avec le même contenu indexé, il passe (aucune sortie)", rc == 0 and out == "" and err == "", (out, err))
    rc, out, err = hook("git commit -m autre-message --no-verify", d)
    check("hook: un autre message sur le même contenu passe aussi", rc == 0 and out == "", out)

    # contenu changé : redéclenche et ne liste que le nouveau
    (d / "docs/b.md").write_text("b2\n", encoding="utf-8")
    git("add", "docs/b.md", cwd=d)
    rc, out, err = hook("git commit -m x", d)
    reason = denial(out) or ""
    check("hook: contenu indexé changé, nouveau refus qui liste seulement ce qui reste à relire (b.md, pas a.md ni top.py)",
          "docs/b.md" in reason and "docs/a.md" not in reason and "top.py" not in reason, reason)
    rc, out, err = hook("git commit -m x", d)
    check("hook: puis il passe", rc == 0 and out == "", out)

    # README corrigés : pas de boucle
    d = repo("h-loop", BASE)
    (d / "docs/a.md").write_text("a2\n", encoding="utf-8")
    git("add", "-A", cwd=d)
    hook("git commit -m x", d)
    (d / "docs/README.md").write_text("# docs corrigé\n", encoding="utf-8")
    git("add", "docs/README.md", cwd=d)
    rc, out, err = hook("git commit -m x", d)
    check("hook: README corrigé et indexé après le refus, le même commit passe (pas de boucle)", rc == 0 and out == "", out)
    d = repo("h-readme-only", BASE)
    (d / "docs/README.md").write_text("# docs v2\n", encoding="utf-8")
    (d / "README.md").write_text("# racine v2\n", encoding="utf-8")
    git("add", "-A", cwd=d)
    rc, out, err = hook("git commit -m x", d)
    check("hook: commit de README seuls, aucun effet", rc == 0 and out == "" and err == "", (out, err))

    # après un vrai commit, la mémoire ne vaut plus
    d = repo("h-after", BASE)
    (d / "docs/a.md").write_text("a2\n", encoding="utf-8")
    git("add", "-A", cwd=d)
    hook("git commit -m x", d)
    git("commit", "-q", "-m", "ok", cwd=d)
    (d / "docs/a.md").write_text("a3\n", encoding="utf-8")
    git("add", "-A", cwd=d)
    rc, out, err = hook("git commit -m x", d)
    check("hook: après un commit réel, un nouveau changement du même fichier est contrôlé de nouveau", denial(out) is not None, out)
    git("reset", "-q", "--soft", "HEAD~1", cwd=d)
    (d / "docs/a.md").write_text("a3\n", encoding="utf-8")

    # -a, -am, git -C, cd, git add && git commit
    d = repo("h-am", BASE)
    (d / "docs/a.md").write_text("a2\n", encoding="utf-8")
    rc, out, err = hook("git commit -m x", d)
    check("hook: modification non indexée sans -a, rien à contrôler", rc == 0 and out == "", out)
    for flag in ("-am 'msg'", "-a -m msg", "--all -m msg", "-qam msg"):
        d2 = repo("h-am2", BASE)
        (d2 / "docs/a.md").write_text("a2\n", encoding="utf-8")
        rc, out, err = hook("git commit %s" % flag, d2)
        reason = denial(out) or ""
        check("hook: `git commit %s` couvre les fichiers suivis modifiés" % flag, "docs/README.md" in reason and "docs/a.md" in reason, (out, err))
    d3 = repo("h-newfile", BASE)
    (d3 / "docs/c.md").write_text("c\n", encoding="utf-8")
    rc, out, err = hook("git add -A && git commit -m x", d3)
    check("hook: `git add -A && git commit` : le fichier nouveau pas encore indexé est couvert", "docs/c.md" in (denial(out) or ""), out)
    rc, out, err = hook("git add -A && git commit -m x", d3)
    check("hook: même enchaînement relancé, il passe", rc == 0 and out == "", out)
    d4 = repo("h-chain", BASE)
    (d4 / "docs/a.md").write_text("a2\n", encoding="utf-8")
    git("add", "-A", cwd=d4)
    elsewhere = new_dir("h-elsewhere")
    rc, out, err = hook('git -C "%s" commit -m x' % d4, elsewhere)
    check("hook: `git -C <dossier> commit` depuis un autre dossier, contrôlé dans le bon dépôt", "docs/a.md" in (denial(out) or ""), (out, err))
    d5 = repo("h-cd", BASE)
    (d5 / "docs/a.md").write_text("a2\n", encoding="utf-8")
    git("add", "-A", cwd=d5)
    rc, out, err = hook("cd %s && git commit -m x" % d5.as_posix(), elsewhere)
    check("hook: `cd <dossier> && git commit`", "docs/a.md" in (denial(out) or ""), (out, err))
    d6 = repo("h-sub", BASE)
    (d6 / "docs/a.md").write_text("a2\n", encoding="utf-8")
    git("add", "-A", cwd=d6)
    rc, out, err = hook("git commit -m x", d6 / "src")
    check("hook: lancé depuis un sous-dossier du dépôt (cwd), contrôlé à la racine du dépôt", "docs/README.md" in (denial(out) or ""), (out, err))
    d7 = repo("h-msg", BASE)
    (d7 / "docs/a.md").write_text("a2\n", encoding="utf-8")
    git("add", "-A", cwd=d7)
    msg = 'git commit -m "$(cat <<\'EOF\'\nCorrection\n\ngit add && echo oops\nEOF\n)"'
    rc, out, err = hook(msg, d7)
    check("hook: message multiligne (heredoc) avec guillemets, contrôlé normalement", "docs/a.md" in (denial(out) or ""), (out, err))

    # chemin accentué, sous un environnement cp1252
    da = repo("dépôt é accentué", BASE)
    (da / "docs/a.md").write_text("a2\n", encoding="utf-8")
    git("add", "-A", cwd=da)
    env = dict(os.environ, PYTHONIOENCODING="cp1252")
    env.pop("PYTHONUTF8", None)
    rc, out, err = hook('git -C "%s" commit -m "été"' % da, new_dir("h-ailleurs-é"), env=env)
    check("hook: chemin accentué (dépôt et dossier de lancement), refus en JSON valide, sortie ASCII, stderr vide",
          denial(out) and "docs/a.md" in denial(out) and out.isascii() and err == "", (out, err))
    rc, out, err = hook('git -C "%s" commit -m x' % da, new_dir("h-ailleurs-é"), env=env)
    check("hook: chemin accentué, second essai passe", rc == 0 and out == "", out)

    # outil PowerShell
    dp = repo("h-ps", BASE)
    (dp / "docs/a.md").write_text("a2\n", encoding="utf-8")
    git("add", "-A", cwd=dp)
    other = new_dir("h-ps-ailleurs")
    ps_cmds = (("git commit", "git commit -m x", dp), ("git.exe", "git.exe commit -m x", dp),
               ("git -C", 'git -C "%s" commit -m x' % dp, other), ("Set-Location ;", "Set-Location '%s'; git commit -m x" % dp, other),
               ("cd ;", 'cd "%s"; git commit -m x' % dp, other), ("Set-Location -Path", 'Set-Location -Path "%s"; git commit -m x' % dp, other),
               ("&& (PowerShell 7)", 'cd "%s" && git commit -m x' % dp, other), ("opérateur &", "& git commit -m x", dp))
    for label, cmd, cwd in ps_cmds:
        (Path(git("rev-parse", "--absolute-git-dir", cwd=dp)[1].strip()) / "2brain-readme-check.json").unlink(missing_ok=True)
        rc, out, err = hook(cmd, cwd, tool="PowerShell")
        check("hook PowerShell: %s, contrôlé" % label, "docs/a.md" in (denial(out) or "") and rc == 0, (out, err))
    rc, out, err = hook("git commit -m x", dp, tool="PowerShell")
    check("hook PowerShell: second essai passe", rc == 0 and out == "", out)
    rc, out, err = hook("git status; Get-ChildItem", dp, tool="PowerShell")
    check("hook PowerShell: commande autre qu'un commit, aucun effet", rc == 0 and out == "" and err == "", (out, err))
    rc, out, err = hook("git commit -m x", dp, tool="Read")
    check("hook: tool_name autre que Bash et PowerShell, aucun effet", rc == 0 and out == "" and err == "")

    # dépôt cassé
    db = repo("h-broken", BASE)
    (db / "docs/a.md").write_text("a2\n", encoding="utf-8")
    git("add", "-A", cwd=db)
    (db / ".git/HEAD").write_text("garbage\n", encoding="utf-8")
    rc, out, err = hook("git commit -m x", db)
    check("hook: dépôt cassé (HEAD illisible), le commit passe (code 0, aucune sortie)", rc == 0 and out == "" and err == "", (rc, out, err))
    dn = new_dir("h-notrepo")
    mk(dn, {"brain-main/README.md": "x\n", "docs/README.md": "r\n", "docs/a.md": "a\n"})
    rc, out, err = hook("git commit -m x", dn)
    check("hook: dossier qui n'est pas un dépôt, aucun effet", rc == 0 and out == "" and err == "", (rc, out, err))
    rc, out, err = hook("git commit -m x", Path(TMP) / "inexistant")
    check("hook: dossier inexistant, aucun effet", rc == 0 and out == "" and err == "", (rc, out, err))
    dg = repo("h-nomemo", BASE)
    (dg / "docs/a.md").write_text("a2\n", encoding="utf-8")
    git("add", "-A", cwd=dg)
    memo = Path(git("rev-parse", "--absolute-git-dir", cwd=dg)[1].strip()) / "2brain-readme-check.json"
    memo.mkdir()
    rc, out, err = hook("git commit -m x", dg)
    check("hook: mémoire impossible à écrire, le commit passe (pas de boucle de refus)", rc == 0 and out == "" and err == "", (out, err))
    src = HOOK.read_text(encoding="utf-8")
    check("hook: aucun accès réseau ni journal dans le script", not any(w in src for w in ("urllib", "socket", "http.client", "logging", "print(")))


def installed(name):
    home = new_dir(name)
    return home


def settings_fixture(home, extra_pre=True):
    data = {
        "permissions": {"allow": ["mcp__x"]},
        "hooks": {
            "SessionStart": [{"hooks": [{"type": "command", "command": "echo", "args": ["autre"]}]}],
            "PreToolUse": [{"matcher": "Edit", "hooks": [{"type": "command", "command": "true"}]}] if extra_pre else [],
        },
    }
    p = home / ".claude" / "settings.json"
    p.parent.mkdir(parents=True, exist_ok=True)
    p.write_text(json.dumps(data, indent=4), encoding="utf-8")
    return data


def ours(settings):
    return [g for g in settings.get("hooks", {}).get("PreToolUse", [])
            if any(str(a).replace("\\", "/").endswith("skills/brain/commit-check.py") for h in g["hooks"] for a in h.get("args", []))]


def test_account():
    inst = SCRIPTS / "install-account.py"
    home = new_dir("home-pre")
    orig = settings_fixture(home)
    sp = home / ".claude/settings.json"
    h0 = sha(sp)
    rc, out = runj(inst, "--home", home, "--dry-run")
    check("compte: --dry-run annonce le hook PreToolUse sans rien écrire", rc == 0 and any("PreToolUse" in a for a in out["actions"])
          and sha(sp) == h0 and not (home / ".claude/skills").exists(), out)
    rc, out = runj(inst, "--home", home)
    s = json.loads(sp.read_text(encoding="utf-8"))
    g = ours(s)
    check("compte: un groupe PreToolUse, matcher Bash|PowerShell, forme sans shell (command=python, args=[commit-check.py]), sans champ shell",
          rc == 0 and len(g) == 1 and g[0]["matcher"] == "Bash|PowerShell" and g[0]["hooks"][0]["type"] == "command"
          and g[0]["hooks"][0]["command"] == sys.executable and len(g[0]["hooks"][0]["args"]) == 1 and "shell" not in g[0]["hooks"][0], (out, s))
    check("compte: autres hooks (PreToolUse Edit, SessionStart) et autres réglages conservés",
          s["hooks"]["PreToolUse"][0] == orig["hooks"]["PreToolUse"][0] and s["permissions"] == orig["permissions"]
          and s["hooks"]["SessionStart"][0] == orig["hooks"]["SessionStart"][0] and len(s["hooks"]["SessionStart"]) == 2, s["hooks"])
    sd = home / ".claude/skills/brain"
    check("compte: commit-check.py posé, au chemin enregistré", (sd / "commit-check.py").is_file() and Path(g[0]["hooks"][0]["args"][0]).is_file())
    d = repo("acc-hook", BASE)
    (d / "docs/a.md").write_text("a2\n", encoding="utf-8")
    git("add", "-A", cwd=d)
    ev = json.dumps({"tool_name": "Bash", "tool_input": {"command": "git commit -m x"}, "cwd": str(d)})
    p = subprocess.run([g[0]["hooks"][0]["command"], g[0]["hooks"][0]["args"][0]], input=ev.encode(), capture_output=True)
    check("compte: le hook installé refuse bien un commit à relire", denial(p.stdout.decode()) is not None and p.returncode == 0, p.stdout)
    old = json.loads(sp.read_text(encoding="utf-8"))
    for grp in old["hooks"]["PreToolUse"]:
        if grp["matcher"] == "Bash|PowerShell":
            grp["matcher"] = "Bash"
    sp.write_text(json.dumps(old, indent=4), encoding="utf-8")
    rc, out = runj(inst, "--home", home)
    s2 = json.loads(sp.read_text(encoding="utf-8"))
    check("compte: réinstallation depuis l'ancienne forme (matcher Bash) : un seul groupe, matcher Bash|PowerShell, pas de doublon",
          len(ours(s2)) == 1 and ours(s2)[0]["matcher"] == "Bash|PowerShell", s2["hooks"])
    check("compte: réinstallation sans doublon", rc == 0 and len(ours(s2)) == 1 and len(s2["hooks"]["PreToolUse"]) == 2, s2["hooks"])
    rc, out = runj(inst, "--home", home, "--uninstall", "--dry-run")
    check("compte: --uninstall --dry-run n'écrit rien", rc == 0 and sha(sp) != "" and (sd / "commit-check.py").is_file() and len(ours(json.loads(sp.read_text(encoding="utf-8")))) == 1, out)
    rc, out = runj(inst, "--home", home, "--uninstall")
    s3 = json.loads(sp.read_text(encoding="utf-8"))
    check("compte: --uninstall retire le groupe PreToolUse et le dossier, garde les autres hooks",
          rc == 0 and not sd.exists() and not ours(s3) and s3["hooks"]["PreToolUse"] == orig["hooks"]["PreToolUse"]
          and s3["hooks"]["SessionStart"] == orig["hooks"]["SessionStart"] and s3["permissions"] == orig["permissions"], (out, s3))
    home2 = new_dir("home-pre-empty")
    mk(home2 / ".claude", {"settings.json": json.dumps({"model": "x"})})
    runj(inst, "--home", home2)
    s4 = json.loads((home2 / ".claude/settings.json").read_text(encoding="utf-8"))
    check("compte: sans autre hook, les deux groupes sont posés", len(ours(s4)) == 1 and len(s4["hooks"]["SessionStart"]) == 1, s4)
    runj(inst, "--home", home2, "--uninstall")
    s5 = json.loads((home2 / ".claude/settings.json").read_text(encoding="utf-8"))
    check("compte: --uninstall remet settings.json comme avant (aucune clé hooks vide)", s5 == {"model": "x"}, s5)
    home3 = new_dir("home-pre-bad")
    mk(home3 / ".claude", {"settings.json": json.dumps({"hooks": {"PreToolUse": "oops"}})})
    before = tree(home3)
    rc, out = runj(inst, "--home", home3)
    check("compte: hooks.PreToolUse mal formé, refus sans rien écrire", rc == 2 and tree(home3) == before, out)


def installed_project(name):
    proj = mk(new_dir(name), {"README.md": "# P\n", "src/app.py": "print(1)\n"})
    root = new_dir(name + "-src")
    shutil.copytree(REPO / "brain-main", root / "brain-main", ignore=shutil.ignore_patterns("__pycache__", "*.pyc"))
    rc, prep = runj(SCRIPTS / "prepare-install.py", "--source", root / "brain-main", "--project", proj, "--answers", json.dumps(ANSWERS))
    rc, data = runj(SCRIPTS / "apply-install.py", "--project", proj, "--plan-sha256", prep["plan_sha256"])
    assert rc == 0, data
    return proj, root / "brain-main"


def status(project, home):
    return runj(ACCOUNT / "status.py", "--project", project, "--home", home, "--json", "--offline")


def masters(base, n_with_readme, n_without, files_each=3):
    """Projet avec des dossiers maîtres (src/m1..), les n premiers avec README."""
    files = {}
    for i in range(n_with_readme + n_without):
        for j in range(files_each + i):
            files["m%02d/f%d.py" % (i, j)] = "x\n"
        if i < n_with_readme:
            files["m%02d/README.md" % i] = "# m%02d\n" % i
    return mk(new_dir(base), files)


def test_setting():
    tpl = BOOT / "templates/brain/settings.json"
    check("modèle: templates/brain/settings.json contient exactement readme_max 8",
          tpl.is_file() and json.loads(tpl.read_text(encoding="utf-8")) == {"readme_max": 8})
    proj, src = installed_project("set-inst")
    sj = proj / "brain/settings.json"
    check("installation: brain/settings.json créé avec readme_max 8, sans trace du système",
          sj.is_file() and json.loads(sj.read_text(encoding="utf-8")) == {"readme_max": 8}
          and not any(w in sj.read_text(encoding="utf-8").lower() for w in ("2brain", "brain-main", "bootstrap")))
    home = new_dir("set-home")
    rc, st = status(proj, home)
    check("status.py: readme_max 8 lu dans le réglage", st["readme_max"] == 8 and "lu dans" in st["readme_max_detail"], st)
    sj.unlink()
    rc, st = status(proj, home)
    check("status.py: réglage absent, 8 par défaut et le dit", st["readme_max"] == 8 and "absent" in st["readme_max_detail"] and "défaut" in st["readme_max_detail"], st)
    for label, raw in (("JSON invalide", "{ pas du json"), ("non-objet", "[1]"), ("clé absente", "{}"), ("zéro", '{"readme_max": 0}'),
                       ("négatif", '{"readme_max": -2}'), ("texte", '{"readme_max": "12"}'), ("booléen", '{"readme_max": true}'),
                       ("décimal", '{"readme_max": 1.5}')):
        sj.write_text(raw, encoding="utf-8")
        rc, st = status(proj, home)
        rc2, scan = runj(SCAN, "--project", proj, "--json")
        check("réglage invalide (%s): status.py et scan-project.py donnent 8 par défaut et le disent" % label,
              st["readme_max"] == 8 and "défaut" in st["readme_max_detail"]
              and scan["dossiers_maitres"]["plafond_readme"] == 8 and scan["dossiers_maitres"]["reglage"]["etat"] == "défaut"
              and "défaut" in scan["dossiers_maitres"]["reglage"]["detail"], (st, scan["dossiers_maitres"]["reglage"]))
    sj.write_text('{"readme_max": 12}\n', encoding="utf-8")
    rc, st = status(proj, home)
    rc2, scan = runj(SCAN, "--project", proj, "--json")
    check("réglage modifié (12): status.py et scan-project.py le lisent", st["readme_max"] == 12 and scan["dossiers_maitres"]["plafond_readme"] == 12
          and scan["dossiers_maitres"]["reglage"]["etat"] == "réglage", (st["readme_max"], scan["dossiers_maitres"]))
    rc, scan = runj(SCAN, "--project", proj, "--json", "--plafond", "3")
    check("scan-project.py: --plafond l'emporte sur le réglage", scan["dossiers_maitres"]["plafond_readme"] == 3 and scan["dossiers_maitres"]["reglage"]["etat"] == "paramètre", scan["dossiers_maitres"])
    rc, txt, err = run(SCAN, "--project", proj)
    check("scan-project.py: la sortie texte donne le plafond, les README existants et restants", "Plafond : 12" in txt and "README existants" in txt, txt[-600:])

    # -u ne touche pas au réglage
    sj.write_text('{"readme_max": 12, "autre": true}\n', encoding="utf-8")
    s2 = new_dir("set-src2")
    shutil.copytree(src, s2 / "brain-main")
    (s2 / "brain-main/bootstrap/templates/brain/settings.json").write_text('{\n  "readme_max": 99\n}\n', encoding="utf-8")
    (s2 / "brain-main/bootstrap/VERSION").write_text("9.9.9\n", encoding="utf-8")
    before = tree(proj / "brain")
    outside = {k: v for k, v in tree(proj).items() if not k.startswith("brain-main/")}
    upd = SCRIPTS / "update-brain-main.py"
    rc, plan = runj(upd, "--source", s2 / "brain-main", "--project", proj)
    rc, done = runj(upd, "--source", s2 / "brain-main", "--project", proj, "--apply", "--plan-sha256", plan["plan_sha256"])
    outside2 = {k: v for k, v in tree(proj).items() if not k.startswith("brain-main/")}
    check("-u: brain/settings.json intact (12 et autres clés), brain/ et tout hors brain-main/ inchangés, modèle changé signalé seulement",
          rc == 0 and done.get("ok") and tree(proj / "brain") == before and outside == outside2
          and json.loads(sj.read_text(encoding="utf-8")) == {"readme_max": 12, "autre": True}
          and any(a["path"].endswith("templates/brain/settings.json") for a in plan["adaptations_brain"]), (plan.get("adaptations_brain"), done))

    # set-readme-max.py
    def go(d, value, *extra):
        return runj(SETMAX, "--project", d, "--value", value, *extra)

    def raw(d):
        f = d / "brain/settings.json"
        return f.read_bytes() if f.is_file() else None

    d = mk(new_dir("smax-new"), {"brain/docs/x.md": "x\n"})
    rc, out = go(d, "12")
    check("set-readme-max: crée brain/settings.json (2 espaces, LF, retour final) avec readme_max",
          rc == 0 and out["ecrit"] is True and raw(d) == b'{\n  "readme_max": 12\n}\n' and out["ancien_effectif"] == 8, (out, raw(d)))
    check("set-readme-max: dit que rien n'est supprimé", "Aucun README n'est supprimé" in out["message"], out)
    before = raw(d)
    rc, out = go(d, "12")
    check("set-readme-max: même valeur, rien à écrire, dit", rc == 0 and out["ecrit"] is False and out["action"] == "aucune" and raw(d) == before, out)
    orig = b'{\r\n\t"autre": {"a": 1},\r\n\t"readme_max": 4,\r\n\t"z": [1]\r\n}'
    d = mk(new_dir("smax-keep"), {"brain/docs/x.md": "x\n"})
    (d / "brain/settings.json").write_bytes(orig)
    rc, out = go(d, "20")
    new = json.loads(raw(d))
    check("set-readme-max: conserve les autres clés, leur ordre, tabulations, CRLF et l'absence de retour final",
          rc == 0 and new == {"autre": {"a": 1}, "readme_max": 20, "z": [1]} and list(new) == ["autre", "readme_max", "z"]
          and b"\r\n\t" in raw(d) and not raw(d).endswith(b"\n") and out["ancien"] == 4, (out, raw(d)))
    for bad in ("0", "-1", "-3", "abc", "1.5", "", " ", "8x", "1e3", "٣"):
        d = mk(new_dir("smax-bad"), {"brain/docs/x.md": "x\n", "brain/settings.json": '{"readme_max": 8}\n'})
        t0 = tree(d)
        rc, out = go(d, bad)
        check("set-readme-max: valeur hors limites %r refusée, rien écrit" % bad, rc == 2 and out["ok"] is False and tree(d) == t0 and "entier supérieur ou égal à 1" in out["erreur"], (rc, out))
    for label, content in (("JSON invalide", b"{ pas du json"), ("non-objet", b"[1, 2]")):
        d = mk(new_dir("smax-refus"), {"brain/docs/x.md": "x\n"})
        (d / "brain/settings.json").write_bytes(content)
        rc, out = go(d, "9")
        check("set-readme-max: réglage existant %s, refus sans changement à l'octet" % label, rc == 2 and raw(d) == content, (rc, out))
    d = mk(new_dir("smax-nobrain"), {"README.md": "x\n"})
    t0 = tree(d)
    rc, out = go(d, "9")
    check("set-readme-max: brain/ absent, refus qui renvoie à /brain -i, rien créé", rc == 2 and "/brain -i" in out["erreur"] and tree(d) == t0, out)
    d = new_dir("smax-source")
    git("init", "-q", cwd=d)
    git("remote", "add", "origin", "https://github.com/Xblode/2brain.git", cwd=d)
    mk(d, {"lab/x.txt": "x", "brain/registry/entries.jsonl": ""})
    t0 = tree(d)
    rc, out = go(d, "9")
    check("set-readme-max: dépôt source simulé, refus sans écrire", rc == 2 and "dépôt source" in out["erreur"] and tree(d) == t0, out)
    d = mk(new_dir("smax-dry"), {"brain/docs/x.md": "x\n"})
    rc, out = go(d, "9", "--dry-run")
    d2 = mk(new_dir("smax-dry2"), {"brain/settings.json": '{"readme_max": 3}\n'})
    b2 = raw(d2)
    rc2, out2 = go(d2, "9", "--dry-run")
    check("set-readme-max: --dry-run n'écrit rien (fichier absent non créé, existant intact)",
          rc == 0 and rc2 == 0 and raw(d) is None and raw(d2) == b2 and out["dry_run"] is True and out["ecrit"] is False and out2["ecrit"] is False, (out, out2))
    d = mk(new_dir("smax-only"), {"brain/docs/x.md": "x\n", "README.md": "r\n", "m1/a.py": "a\n"})
    t0 = tree(d)
    go(d, "5")
    t1 = tree(d)
    check("set-readme-max: seul brain/settings.json change", sorted(k for k in set(t0) | set(t1) if t0.get(k) != t1.get(k)) == ["brain/settings.json"])

    # inférieur à l'existant : rien n'est supprimé, le scan le montre
    d = masters("smax-less", 3, 2)
    mk(d, {"brain/docs/x.md": "x\n"})
    readmes = tree(d)
    rc, out = go(d, "2")
    rc2, scan = runj(SCAN, "--project", d, "--json")
    dm = scan["dossiers_maitres"]
    after = tree(d)
    check("-m <nombre> inférieur à l'existant (2 pour 3 README) : réglage écrit, aucun README supprimé ni modifié",
          rc == 0 and all(after.get(k) == v for k, v in readmes.items()) and json.loads((d / "brain/settings.json").read_text(encoding="utf-8")) == {"readme_max": 2}, (out, after.keys()))
    check("-m : le scan donne 3 README existants pour un plafond de 2, aucune place restante",
          dm["plafond_readme"] == 2 and dm["readme_existants"] == 3 and dm["readme_restants"] == 0 and dm["candidats_total"] == 5, dm)
    d = masters("smax-room", 2, 4)
    mk(d, {"brain/docs/x.md": "x\n"})
    go(d, "5")
    rc, scan = runj(SCAN, "--project", d, "--json")
    dm = scan["dossiers_maitres"]
    names = [c["chemin"] for c in dm["candidats"] if c["readme"]]
    check("-m : inventaire de 2 README existants (noms et chemins), 3 places restantes sur 5, 2 dossiers maîtres au-delà",
          dm["readme_existants"] == 2 and dm["readme_restants"] == 3 and sorted(names) == ["m00", "m01"]
          and all(c["readme_noms"] == ["README.md"] for c in dm["candidats"] if c["readme"]) and dm["au_dela_du_plafond"] == 1, dm)
    check("-m : le réglage ne dépend pas des dossiers détectés (plafond 5 avec 6 candidats)", dm["plafond_readme"] == 5 and dm["candidats_total"] == 6, dm)


def test_cadrage_and_search():
    import re
    tpl = BOOT / "templates/brain/docs/CADRAGE.md"
    text = tpl.read_text(encoding="utf-8")
    check("modèle CADRAGE.md: même forme que les autres (front matter, 6 sections, « À compléter »), sans donnée de 2brain",
          text.startswith("---\nmetadata_version: 1\n") and text.count("\n## ") == 6 and "À compléter" in text and "{{date}}" in text
          and not re.search(r"2brain|brain-main|bootstrap|capture|cap-|claude|CAP-\d", text, re.I), text[:200])
    proj, src = installed_project("cad-inst")
    cad = proj / "brain/docs/CADRAGE.md"
    check("installation: brain/docs/CADRAGE.md créé depuis le modèle, date substituée, aucune donnée de 2brain",
          cad.is_file() and "{{" not in cad.read_text(encoding="utf-8") and "À compléter" in cad.read_text(encoding="utf-8")
          and "2brain" not in cad.read_text(encoding="utf-8"))
    home = new_dir("cad-home")
    rc, st = status(proj, home)
    check("status.py: projet neuf avec CADRAGE.md, complet, rien à créer", st["brain"]["etat"] == "complet" and st["brain"]["documents_a_creer"] == [], st["brain"])

    # projet installé avant CADRAGE.md
    cad.unlink()
    man_path = proj / "brain-main/manifest.json"
    man = json.loads(man_path.read_text(encoding="utf-8"))
    key = "bootstrap/templates/brain/docs/CADRAGE.md"
    man["files"].pop(key, None)
    man_path.write_text(json.dumps(man, ensure_ascii=False, indent=2), encoding="utf-8")
    (proj / "brain-main" / key).unlink()
    rc, st = status(proj, home)
    check("projet ancien sans CADRAGE.md: brain reste « complet » (pas partiel), documents_a_creer et note qui renvoient à /brain -o",
          st["brain"]["etat"] == "complet" and st["brain"]["manquants"] == [] and st["brain"]["documents_a_creer"] == ["cadrage.md"]
          and any("cadrage.md" in n and "/brain -o" in n for n in st["notes"]), st)
    rc, txt, _ = run(ACCOUNT / "status.py", "--project", proj, "--home", home, "--offline")
    brain_line = [l for l in txt.splitlines() if l.startswith("brain :")][0]
    check("projet ancien sans CADRAGE.md: la sortie texte n'est pas cassée, « brain : complet » et la note", rc == 0 and "complet" in brain_line and "cadrage.md" in txt, txt)
    before_brain = tree(proj / "brain")
    outside = {k: v for k, v in tree(proj).items() if not k.startswith("brain-main/")}
    upd = SCRIPTS / "update-brain-main.py"
    rc, plan = runj(upd, "--source", src, "--project", proj)
    rc2, done = runj(upd, "--source", src, "--project", proj, "--apply", "--plan-sha256", plan["plan_sha256"])
    check("-u sur un projet sans CADRAGE.md: le modèle est ajouté à brain-main/, signalé dans adaptations_brain, brain/ et le reste du projet intacts",
          rc == 0 and rc2 == 0 and (proj / "brain-main" / key).is_file() and any(a["path"] == key for a in plan["adaptations_brain"])
          and not cad.exists() and tree(proj / "brain") == before_brain
          and outside == {k: v for k, v in tree(proj).items() if not k.startswith("brain-main/")}, (plan.get("adaptations_brain"), done))
    rc, st = status(proj, home)
    check("après -u, toujours « complet » et CADRAGE.md toujours à créer (par /brain -o, après validation)", st["brain"]["etat"] == "complet" and st["brain"]["documents_a_creer"] == ["cadrage.md"], st["brain"])
    onboarding = (REPO / "brain-main/skills/brain-onboarding/SKILL.md").read_text(encoding="utf-8")
    flat = " ".join(onboarding.split())
    check("skill -o: ordre VISION, CADRAGE, PROJECT, REQUIREMENTS, ARCHITECTURE, WORKFLOW, création de CADRAGE.md proposée après validation (documents_a_creer), au plus 80 lignes",
          "VISION, CADRAGE, PROJECT, REQUIREMENTS, ARCHITECTURE, WORKFLOW" in flat and "documents_a_creer" in flat and "ne rien créer sans validation" in flat
          and lines(REPO / "brain-main/skills/brain-onboarding/SKILL.md") <= 80)

    # standard documentaire
    std = (REPO / "brain/docs/README.md").read_text(encoding="utf-8")
    std_head = std.split("---")[1]
    check("brain/docs/README.md: toujours locked: true, version 1.1.0, six documents, ligne cadrage.md, plus aucune mention « cinq »",
          "locked: true" in std_head and "version: 1.1.0" in std_head and "| `cadrage.md`" in std and "six documents" in std and "cinq" not in std.lower(), std_head)

    # recherche sans demander
    key_phrases = ("worker-haiku", "sans demander", "lecture seule")
    agents = (REPO / "AGENTS.md").read_text(encoding="utf-8")
    tplag = (BOOT / "templates/AGENTS.md").read_text(encoding="utf-8")
    guide = (REPO / "brain-main/README.md").read_text(encoding="utf-8")
    orch_a = (REPO / ".claude/agents/orchestrator.md").read_text(encoding="utf-8").replace("\r\n", "\n")
    orch_b = (REPO / "brain-main/adapters/claude-code/agents/orchestrator.md").read_text(encoding="utf-8").replace("\r\n", "\n")
    for label, doc in (("AGENTS.md", agents), ("modèle AGENTS.md", tplag), ("guide brain-main/README.md", guide), ("orchestrator.md (adaptateur)", orch_b), ("orchestrator.md (.claude)", orch_a)):
        check("règle de recherche sans demande présente dans %s" % label, all(w in doc for w in key_phrases), [w for w in key_phrases if w not in doc])
    check("AGENTS.md: la recherche est autorisée sans demande, Review et Sync restent sur demande, essai sur une copie, session non routée",
          "Recherche sans demander" in agents and "ne pas lancer Brain Review / Sync ;" in agents and "Sans cet agent (session non routée)" in agents
          and "Un essai de script se fait toujours sur une copie" in agents and "ne se lancent que sur demande claire" not in agents)
    check("modèle AGENTS.md: sans cet agent, la recherche est lancée soi-même ; Review et Sync sur demande claire ; copie pour un essai",
          "lancer soi-même la recherche" in tplag and "Review et Sync restent sur demande claire" in tplag and "sur une copie" in tplag)
    new_sentence = "ne lance ni Brain Review, ni Sync, ni commit, ni push. Avant de proposer une implémentation"
    check("orchestrator.md: « ni Search » retiré, nouvelle règle de recherche présente dans les deux copies",
          all(new_sentence in o and "ni Search" not in o and "fais chercher dans le registre sans demander, par `worker-haiku`" in o for o in (orch_a, orch_b)))
    gap = [l for l in orch_a.splitlines() if l not in orch_b.splitlines()]
    check("orchestrator.md: la phrase de recherche est identique dans les deux copies (les seuls écarts sont les trois écarts documentés de l'adaptateur)",
          len(gap) == 3 and not any("worker-haiku" in l for l in gap), gap)
    # -u signale le changement d'adaptateur sans l'appliquer
    proj2, src2 = installed_project("cad-adapt")
    s3 = new_dir("cad-adapt-src")
    shutil.copytree(src2, s3 / "brain-main")
    orch = s3 / "brain-main/adapters/claude-code/agents/orchestrator.md"
    orch.write_text(orch.read_text(encoding="utf-8") + "\nmodif de test\n", encoding="utf-8")
    claude_before = tree(proj2 / ".claude")
    rc, plan = runj(SCRIPTS / "update-brain-main.py", "--source", s3 / "brain-main", "--project", proj2)
    rc2, done = runj(SCRIPTS / "update-brain-main.py", "--source", s3 / "brain-main", "--project", proj2, "--apply", "--plan-sha256", plan["plan_sha256"])
    check("-u: un changement de l'orchestrateur de l'adaptateur est signalé (adaptations_claude), .claude/ n'est pas modifié",
          rc == 0 and rc2 == 0 and [a["path"] for a in done["adaptations_claude"]] == ["adapters/claude-code/agents/orchestrator.md"]
          and tree(proj2 / ".claude") == claude_before, done.get("adaptations_claude"))
    boot = (BOOT / "README.md").read_text(encoding="utf-8")
    check("README Bootstrap: décision de recherche sans demande (§8.3), non appliquée automatiquement à .claude/ (§9.1), CADRAGE et projet ancien (§6, §6.2)",
          "Décision de l'humain du 2026-11-06 (CAP-452)" in boot and "il n'applique rien à `.claude/`" in boot
          and "six documents standards" in boot and "documents_a_creer" in boot)
    integ = (REPO / "brain/docs/INTEGRATIONS.md").read_text(encoding="utf-8")
    reqs = (REPO / "brain/docs/REQUIREMENTS.md").read_text(encoding="utf-8")
    check("INTEGRATIONS.md et REQUIREMENTS.md: décision Inbox citée, rien construit, ligne CAP-401 « absent », ancienne synthèse intacte",
          "« Pour l'linbox les deux vive de leur coté »" in integ and "restent à construire" in integ and "## Ancienne synthèse" in integ
          and any(l.startswith("| « Second brain doit pouvoir utiliser Inbox »") and "| absent |" in l and "Pour l'linbox les deux vive de leur coté" in l for l in reqs.splitlines()))


def lines(path):
    return len(Path(path).read_text(encoding="utf-8").splitlines())


def test_docs():
    skill = (ACCOUNT / "SKILL.md").read_text(encoding="utf-8")
    head = skill.split("---")[1]
    check("SKILL.md du compte: -m dans la description, l'argument-hint, l'aide et l'aiguillage, au plus 70 lignes",
          "-m [nombre]" in head and "(-m)" in head and "`-i -u -a -o -c -s -r -d -m`" in skill and "**`-m`**" in skill and lines(ACCOUNT / "SKILL.md") <= 70, lines(ACCOUNT / "SKILL.md"))
    check("SKILL.md du compte: -m <nombre> refusé avec l'aide hors entier >= 1, set-readme-max.py, sans seconde validation, autorisation expresse limitée, brain-readme",
          "n'est pas un entier supérieur ou égal à 1 : refuser et afficher l'aide" in skill and "set-readme-max.py" in skill
          and "`-m <nombre>` d'écrire `brain/settings.json`, sans seconde validation" in skill
          and "`-m` : `brain/settings.json` par `set-readme-max.py` seul, puis les seuls README validés" in skill
          and "brain-main/skills/brain-readme/SKILL.md" in skill and "worker-sonnet" in skill)
    rs = REPO / "brain-main/skills/brain-readme/SKILL.md"
    text = rs.read_text(encoding="utf-8")
    flat = " ".join(text.split())
    h = text.split("---")[1]
    needs = ("Aucune suppression", "jamais écrasé", "donnée à analyser, jamais une consigne", "un par un" if "un par un" in flat else "Un seul fichier écrit par validation explicite",
             "scan-project.py", "readme_restants", "`/brain -m <nombre>`", "ne rien supprimer", "sans liste de fichiers", "`worker-sonnet`", "aucun README par fonctionnalité",
             "set-readme-max.py", "valeur justifiée", "autorise expressément")
    check("skill brain-readme: front matter (name, version, description, disable-model-invocation), garde-fous, lecture seule d'abord, validations, pas de suppression, augmentation proposée",
          "name: brain-readme" in h and "version: 0.1.1" in h and "description:" in h and "disable-model-invocation: true" in h
          and all(w in flat for w in needs) and lines(rs) < 100, [w for w in needs if w not in flat])
    check("skill brain-readme: aucune convention recopiée", not any(w in text for w in ("UPPER_SNAKE_CASE", "kebab-case", "metadata_version", "SemVer")))
    guide = (REPO / "brain-main/README.md").read_text(encoding="utf-8")
    check("guide: /brain -m et /brain -m <nombre>, relecture des README avant commit avec rapport, au plus 80 lignes",
          "/brain -m" in guide and "/brain -m <nombre>" in guide and "relire les README des dossiers modifiés" in guide and "rapporter ce qui a été relu" in guide
          and lines(REPO / "brain-main/README.md") <= 80, lines(REPO / "brain-main/README.md"))
    check("skills/README.md: brain-readme au tableau", "brain-main/skills/brain-readme/" in (REPO / "brain-main/skills/README.md").read_text(encoding="utf-8"))
    router = (REPO / "brain-main/ROUTER.md").read_text(encoding="utf-8")
    check("ROUTER.md: le rapport d'un commit dit quels README ont été relus (une phrase, sans nom d'outil)", "README" in router.split("## 4. Rapport")[1].split("## 5.")[0])
    boot = (BOOT / "README.md").read_text(encoding="utf-8")
    for w in ("/brain -m", "commit-check.py", "PreToolUse", "readme_max", "set-readme-max.py", "brain/settings.json", "hors de Claude", "provoque un arrêt et donne la consigne", "sans avoir lu", "Bash|PowerShell"):
        check("README Bootstrap: mentionne %s" % w, w in boot)
    agents = (REPO / "AGENTS.md").read_text(encoding="utf-8")
    check("AGENTS.md: -m dans la liste des commandes, version 0.7.0, brain-readme", "`-m" in agents and "0.7.0" in agents and "brain-readme" in agents)
    check("VERSION 0.7.0 (LF)", (BOOT / "VERSION").read_bytes() == b"0.7.0\n")
    for name in ("account/commit-check.py", "scripts/set-readme-max.py", "templates/brain/settings.json", "tests/run_readme_tests.py"):
        check("fichier présent: bootstrap/" + name, (BOOT / name).is_file())


def test_standard():
    """Standard des README de dossiers maîtres : emplacement, règle de lecture partout, copies des workers, gitignore."""
    def norm(path):
        return (REPO / path).read_bytes().decode("utf-8").replace("\r\n", "\n")

    meta = norm("brain-main/conventions/METADATA.md")
    head = meta.split("\n---\n")[0]
    check("METADATA.md: §6.9 présent, version 1.1.0, non verrouillé",
          "### 6.9 README de dossier maître" in meta and 'version: "1.1.0"' in head and "locked" not in head)
    sec = meta.split("### 6.9 README de dossier maître")[1].split("\n## 7.")[0] if "### 6.9 README de dossier maître" in meta else ""
    need = ("À quoi sert ce dossier", "Fonctionnalités", "Où aller ensuite", "Ce qu'il ne faut pas y faire", "Pas de liste de fichiers", "omise", "le plus proche",
            "`readme_max`", "Pas de README par fonctionnalité ni par sous-dossier", "jamais remplacé", "`description`")
    check("METADATA §6.9: quatre sections du modèle, pas de liste de fichiers, section vide omise, plafond, lecture du README le plus proche",
          all(w in sec for w in need), [w for w in need if w not in sec])
    check("METADATA: §6.8 et §7 non renumérotés, pas de §6.10",
          "### 6.8 Fichiers binaires et formats incompatibles" in meta and "## 7. Hiérarchie et héritage des conventions" in meta and "### 6.10" not in meta)
    check("conventions/README.md: catalogue renvoie à METADATA §6.9", "§6.9" in norm("brain-main/conventions/README.md"))
    for rel in ("brain-main/README.md", "brain-main/bootstrap/templates/AGENTS.md", "brain-main/ROUTER.md",
                "brain-main/adapters/claude-code/agents/worker-sonnet.md", "brain-main/adapters/claude-code/agents/worker-haiku.md",
                ".claude/agents/worker-sonnet.md", ".claude/agents/worker-haiku.md"):
        t = norm(rel)
        check("règle de lecture (README le plus proche, METADATA §6.9) dans " + rel,
              "Avant de travailler dans un dossier" in t and "README le plus proche" in t and "§6.9" in t, rel)
    for n in ("worker-sonnet", "worker-haiku"):
        check("copie du worker identique (.claude/agents et adaptateur) : " + n,
              norm(".claude/agents/%s.md" % n) == norm("brain-main/adapters/claude-code/agents/%s.md" % n))
    check("modèle installé: AGENTS.md du modèle porte la règle de lecture",
          "Avant de travailler dans un dossier" in (BOOT / "templates/AGENTS.md").read_text(encoding="utf-8"))
    proj, _src = installed_project("standard-inst")
    inst = (proj / "AGENTS.md").read_text(encoding="utf-8")
    check("projet installé: AGENTS.md porte la règle de lecture et METADATA §6.9 est livré",
          "Avant de travailler dans un dossier" in inst and "README le plus proche" in inst
          and "### 6.9 README de dossier maître" in (proj / "brain-main/conventions/METADATA.md").read_text(encoding="utf-8").replace("\r\n", "\n"))
    audit = norm("brain-main/skills/brain-audit/SKILL.md")
    rd = norm("brain-main/skills/brain-readme/SKILL.md")
    check("skills audit et readme: renvoient à METADATA §6.9, gardent readme_max et aucun README de sous-dossier",
          "METADATA.md` §6.9" in audit and "METADATA.md` §6.9" in rd and "`readme_max`" in audit
          and "aucun dans les sous-dossiers" in audit and "README de sous-dossier" in rd)
    boot = norm("brain-main/bootstrap/README.md")
    check("bootstrap/README.md §6.4: indique le standard", "METADATA.md` §6.9" in boot.split("### 6.4.")[1].split("\n## 7.")[0])
    line = "brain/registry/_capture-history/"
    check(".gitignore du dépôt et modèle gitignore.txt: _capture-history/ exclu de Git",
          line in norm(".gitignore").split("\n") and line in norm("brain-main/bootstrap/templates/gitignore.txt").split("\n"))
    r = subprocess.run(["git", "check-ignore", "-v", line + "x/entries.jsonl"], cwd=str(REPO), capture_output=True, text=True)
    check("git check-ignore reconnaît brain/registry/_capture-history/", r.returncode == 0, r.stdout + r.stderr)


def main():
    global TMP
    sys.stdout.reconfigure(encoding="utf-8")
    base = os.environ.get("README_TEST_TMP")
    if base:
        Path(base).mkdir(parents=True, exist_ok=True)
    TMP = tempfile.mkdtemp(prefix="2brain-readme-tests-", dir=base or None)
    real = Path.home() / ".claude" / "settings.json"
    real_before = (sha(real) if real.is_file() else None, (Path.home() / ".claude/skills/brain").exists())
    try:
        for name, fn in (("hook", test_hook), ("compte", test_account), ("réglage", test_setting), ("cadrage et recherche", test_cadrage_and_search), ("documentation", test_docs), ("standard des README", test_standard)):
            try:
                fn()
            except Exception:
                check("exception dans le groupe " + name, False, traceback.format_exc()[-700:])
        check("le vrai ~/.claude (settings.json, skills/brain) est inchangé par ces tests",
              real_before == (sha(real) if real.is_file() else None, (Path.home() / ".claude/skills/brain").exists()))
    finally:
        rmtree(TMP)
    ok = sum(1 for _, r in RESULTS if r)
    print("\nRÉSULTAT : %d PASS, %d FAIL sur %d" % (ok, len(RESULTS) - ok, len(RESULTS)))
    return 0 if ok == len(RESULTS) else 1


if __name__ == "__main__":
    sys.exit(main())
