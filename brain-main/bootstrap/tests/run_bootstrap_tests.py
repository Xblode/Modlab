# -*- coding: utf-8 -*-
"""Tests autonomes du Bootstrap /brain (lot 2). Bibliothèque standard uniquement.

Usage : python brain-main/bootstrap/tests/run_bootstrap_tests.py
Tout se passe dans un dossier créé par tempfile.mkdtemp, hors du dépôt. La source
utilisée est une copie temporaire de brain-main/ ; le brain/ réel n'est jamais
écrit (le SCHEMA.md du dépôt n'est lu que pour comparaison). Rien n'est écrit dans ~/.claude :
les tests du compte utilisent --home avec un dossier temporaire.
Sortie : une ligne PASS/FAIL par contrôle. Code de sortie 0 si tout passe, 1 sinon.
"""

import hashlib
import io
import json
import os
import re
import shutil
import stat
import subprocess
import sys
import tarfile
import tempfile
import traceback
from pathlib import Path

BOOT = Path(__file__).resolve().parent.parent
REPO = BOOT.parent.parent
SCRIPTS = BOOT / "scripts"
ACCOUNT = BOOT / "account"
EXPECTED_MESSAGE = (
    "Ce dossier est vide et 2brain n'y est pas installé. "
    "Dans cette situation, la première réponse de la session commence par la question : "
    "« Avant de commencer, souhaites-tu installer 2brain dans ce projet ? ». "
    "Si la réponse est oui, elle indique de taper /brain -i. "
    "Si la réponse est non, la session continue normalement et la question n'est pas reposée."
)
ANSWERS_FULL = {
    "nature": "Site web vitrine",
    "objectif": "Présenter l'activité de l'atelier",
    "modules": ["recherche", "revue"],
    "outils": "Next.js, Figma",
    "identite": "Atelier Test",
}
ANSWERS_NULL = {k: None for k in ANSWERS_FULL}

NL = chr(10)
RESULTS = []
TMP = None


def check(name, cond, detail=""):
    RESULTS.append((name, bool(cond)))
    print(("PASS " if cond else "FAIL ") + name + ("" if cond else "  :: " + str(detail)[:400]))


def rmtree(path):
    def onerror(func, p, _exc):
        os.chmod(p, stat.S_IWRITE)
        func(p)

    shutil.rmtree(path, onerror=onerror)


def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def tree(root, skip=()):
    out = {}
    root = Path(root)
    for p in sorted(root.rglob("*")):
        rel = p.relative_to(root).as_posix()
        if any(rel == s or rel.startswith(s + "/") for s in skip):
            continue
        out[rel] = "dir" if p.is_dir() else sha(p)
    return out


def run(script, *args, cwd=None, inp=None):
    p = subprocess.run(
        [sys.executable, "-I", "-B", str(script), *[str(a) for a in args]],
        capture_output=True, text=True, encoding="utf-8", input=inp, cwd=cwd, timeout=180,
    )
    return p.returncode, p.stdout, p.stderr


def runj(script, *args, **kw):
    rc, out, err = run(script, *args, **kw)
    try:
        data = json.loads(out)
    except Exception:
        data = {"_raw": out, "_err": err}
    return rc, data


def mk(path, files):
    path = Path(path)
    path.mkdir(parents=True, exist_ok=True)
    for rel, content in files.items():
        p = path / rel
        p.parent.mkdir(parents=True, exist_ok=True)
        p.write_text(content, encoding="utf-8")
    return path


def new_dir(name):
    d = Path(TMP) / name
    n = 0
    while d.exists():
        n += 1
        d = Path(TMP) / ("%s-%d" % (name, n))
    d.mkdir(parents=True)
    return d


def git(*args, cwd):
    subprocess.run(["git", *args], cwd=cwd, capture_output=True, check=True, stdin=subprocess.DEVNULL)


def fake_source_repo(name, with_lab=True, remote="https://github.com/Xblode/2brain.git"):
    d = new_dir(name)
    git("init", "-q", cwd=d)
    if remote:
        git("remote", "add", "origin", remote, cwd=d)
    if with_lab:
        mk(d, {"lab/x.txt": "x", "brain/registry/entries.jsonl": ""})
    return d


def copy_brain_main(dst_root):
    dst = Path(dst_root) / "brain-main"
    shutil.copytree(REPO / "brain-main", dst, ignore=shutil.ignore_patterns("__pycache__", "*.pyc"))
    return dst


def hook(stdin_obj=None, raw=None, cwd=None):
    inp = raw if raw is not None else json.dumps(stdin_obj)
    rc, out, err = run(ACCOUNT / "session-start.py", inp=inp, cwd=cwd)
    return rc, out, err


# --------------------------------------------------------------------------- groupes


def test_hook():
    empty = new_dir("hook-empty")
    gitonly = new_dir("hook-git")
    (gitonly / ".git").mkdir()
    withfile = mk(new_dir("hook-file"), {"a.txt": "x"})
    withbm = mk(new_dir("hook-bm"), {"brain-main/README.md": "x"})
    rc, out, _ = hook({"cwd": str(empty), "source": "startup"})
    ctx = json.loads(out)["hookSpecificOutput"] if out else {}
    check("hook: dossier vide -> message exact, code 0",
          rc == 0 and ctx.get("additionalContext") == EXPECTED_MESSAGE and ctx.get("hookEventName") == "SessionStart", out)
    rc, out, _ = hook({"cwd": str(gitonly), "source": "startup"})
    check("hook: seulement .git -> message", rc == 0 and "installer 2brain" in out, out)
    rc, out, _ = hook({"cwd": str(withfile), "source": "startup"})
    check("hook: dossier avec fichier -> silence", rc == 0 and out == "", out)
    rc, out, _ = hook({"cwd": str(withbm), "source": "startup"})
    check("hook: brain-main présent -> silence", rc == 0 and out == "", out)
    rc, out, _ = hook({"cwd": str(empty), "source": "resume"})
    check("hook: source resume -> silence", rc == 0 and out == "", out)
    rc, out, _ = hook({"cwd": str(empty / "inexistant"), "source": "startup"})
    check("hook: cwd invalide -> silence, code 0", rc == 0 and out == "", out)
    rc, out, _ = hook({"source": "startup"})
    check("hook: cwd absent -> silence, code 0", rc == 0 and out == "")
    rc, out, err = hook(raw="pas du json {{")
    check("hook: entrée invalide -> silence, code 0, pas de trace", rc == 0 and out == "" and err == "", err)
    rc, out, err = hook(raw="")
    check("hook: entrée vide -> silence, code 0", rc == 0 and out == "")
    check("hook: aucun fichier créé dans le dossier vide", list(empty.iterdir()) == [])


def status(project, home, *extra):
    return runj(ACCOUNT / "status.py", "--project", project, "--home", home, "--json", "--offline", *extra)


def test_status():
    home = new_dir("home-status")
    p_abs = mk(new_dir("st-abs"), {"package.json": "{}", "src/a.js": "x"})
    rc, st = status(p_abs, home)
    check("status: absent", rc == 0 and st["brain_main"]["etat"] == "absent" and st["brain"]["etat"] == "absent"
          and st["dossier_vide"] is False and "package.json" in st["technologies"] and st["depot_source"] is False, st)
    check("status: version distante non vérifiée hors ligne", st["bootstrap"]["distante"] == "non vérifié", st["bootstrap"])
    p_empty = new_dir("st-empty")
    (p_empty / ".git").mkdir()
    rc, st = status(p_empty, home)
    check("status: dossier avec seulement .git = vide", st["dossier_vide"] is True, st)

    p_part = mk(new_dir("st-part"), {"brain-main/README.md": "x", "brain/docs/vision.md": "x"})
    rc, st = status(p_part, home)
    check("status: partiel", st["brain_main"]["etat"] == "partiel" and st["brain"]["etat"] == "partiel", st)

    p_full = mk(new_dir("st-full"), {
        "brain-main/README.md": "x", "brain-main/conventions/a.md": "x", "brain-main/skills/s.md": "x",
        "brain-main/bootstrap/VERSION": "0.1.0\n",
        "brain-main/manifest.json": json.dumps({"version": "0.1.0", "files": {}}),
        "brain/docs/vision.md": "x", "brain/docs/project.md": "x", "brain/docs/requirements.md": "x",
        "brain/docs/architecture.md": "x", "brain/docs/workflow.md": "x",
        "brain/registry/entries.jsonl": '{"id":"CAP-001"}\n{"id":"CAP-002"}\n\n',
    })
    rc, st = status(p_full, home)
    check("status: complet, 2 entrées, version installée",
          st["brain_main"]["etat"] == "complet" and st["brain"]["etat"] == "complet" and st["brain"]["entrees_registre"] == 2
          and st["brain_main"]["version_installee"] == "0.1.0" and st["bootstrap"]["projet"] == "0.1.0", st)

    src = fake_source_repo("st-src")
    rc, st = status(src, home)
    check("status: dépôt source reconnu", st["depot_source"] is True, st)
    nolab = fake_source_repo("st-nolab", with_lab=False)
    rc, st = status(nolab, home)
    check("status: remote 2brain sans lab/ -> pas dépôt source", st["depot_source"] is False, st)
    norem = fake_source_repo("st-norem", remote=None)
    rc, st = status(norem, home)
    check("status: sans remote -> pas dépôt source", st["depot_source"] is False, st)
    other = fake_source_repo("st-other", remote="https://github.com/autre/projet.git")
    rc, st = status(other, home)
    check("status: autre remote -> pas dépôt source", st["depot_source"] is False, st)

    # version du compte et message de mise à jour (fonction interne, sans réseau)
    mk(home, {".claude/skills/brain/VERSION": "0.0.9\n"})
    rc, st = status(p_abs, home)
    check("status: version du compte lue", st["bootstrap"]["compte"] == "0.0.9", st["bootstrap"])

    before = tree(p_full)
    before_home = tree(home)
    run(ACCOUNT / "status.py", "--project", p_full, "--home", home, "--offline")
    run(ACCOUNT / "status.py", "--project", p_full, "--home", home, "--offline", "--json")
    check("status: écriture nulle (projet et home inchangés)", tree(p_full) == before and tree(home) == before_home)
    rc, out, err = run(ACCOUNT / "status.py", "--project", p_abs, "--home", home, "--offline")
    check("status: sortie texte", rc == 0 and "brain-main : absent" in out, out)


def make_tar(path, members, pax_comment=None):
    """members : liste de (nom, contenu_bytes|None pour dossier|('link', cible))."""
    kwargs = {}
    if pax_comment:
        kwargs = {"format": tarfile.PAX_FORMAT, "pax_headers": {"comment": pax_comment}}
    with tarfile.open(path, "w:gz", **kwargs) as tf:
        for name, content in members:
            ti = tarfile.TarInfo(name)
            if content is None:
                ti.type = tarfile.DIRTYPE
                ti.mode = 0o755
                tf.addfile(ti)
            elif isinstance(content, tuple):
                ti.type = tarfile.SYMTYPE
                ti.linkname = content[1]
                tf.addfile(ti)
            else:
                ti.size = len(content)
                tf.addfile(ti, io.BytesIO(content))


def test_fetch():
    fetch = ACCOUNT / "fetch-source.py"
    repo_root = new_dir("fetch-src")
    copy_brain_main(repo_root)
    dest = new_dir("fetch-dest")
    rc, data = runj(fetch, "--dest", dest, "--source-dir", repo_root)
    prep = dest / "brain-main" / "bootstrap" / "scripts" / "prepare-install.py"
    check("fetch: copie locale", rc == 0 and data.get("ok") and prep.is_file(), data)
    check("fetch: commit source noté (inconnu pour une copie sans git)",
          (dest / "source-commit.txt").read_text(encoding="utf-8").strip() == "inconnu")
    check("fetch: pas de __pycache__ copié", not list(dest.rglob("__pycache__")))
    rc, data = runj(fetch, "--dest", dest, "--source-dir", repo_root)
    check("fetch: destination non vide refusée", rc == 2, data)

    # archive valide, avec commit dans l'en-tête pax
    good = new_dir("fetch-tar") / "good.tar.gz"
    members = [("2brain-master/", None), ("2brain-master/README.md", b"racine\n"), ("2brain-master/brain/x.md", b"hors brain-main\n")]
    for p in sorted((repo_root / "brain-main").rglob("*")):
        rel = p.relative_to(repo_root).as_posix()
        members.append(("2brain-master/" + rel, None if p.is_dir() else p.read_bytes()))
    make_tar(good, members, pax_comment="a" * 40)
    dest = new_dir("fetch-dest")
    rc, data = runj(fetch, "--dest", dest, "--source-url", good.as_uri())
    check("fetch: archive locale valide, seul brain-main/ extrait",
          rc == 0 and (dest / "brain-main" / "bootstrap" / "VERSION").is_file()
          and not (dest / "README.md").exists() and not (dest / "brain").exists(), data)
    check("fetch: commit lu dans l'archive", data.get("source_commit") == "a" * 40, data)

    # chemin malveillant
    evil = good.parent / "evil.tar.gz"
    make_tar(evil, [("2brain-master/brain-main/ok.md", b"ok"), ("2brain-master/brain-main/../../evil.txt", b"pwned")])
    dest = new_dir("fetch-dest-evil")
    rc, data = runj(fetch, "--dest", dest, "--source-url", evil.as_uri())
    check("fetch: chemin malveillant (..) refusé, rien écrit", rc == 4 and not (dest.parent / "evil.txt").exists()
          and not (dest / "brain-main").exists(), data)
    evil2 = good.parent / "evil2.tar.gz"
    make_tar(evil2, [("/abs/evil.txt", b"x"), ("2brain-master/brain-main/ok.md", b"ok")])
    dest = new_dir("fetch-dest-evil2")
    rc, data = runj(fetch, "--dest", dest, "--source-url", evil2.as_uri())
    check("fetch: chemin absolu refusé", rc == 4 and not (dest / "brain-main").exists(), data)
    link = good.parent / "link.tar.gz"
    make_tar(link, [("2brain-master/brain-main/ok.md", b"ok"), ("2brain-master/brain-main/l", ("l", "../../x"))])
    dest = new_dir("fetch-dest-link")
    rc, data = runj(fetch, "--dest", dest, "--source-url", link.as_uri())
    check("fetch: lien refusé", rc == 4 and not (dest / "brain-main").exists(), data)

    # archive sans Bootstrap
    nob = good.parent / "nobootstrap.tar.gz"
    make_tar(nob, [("2brain-master/brain-main/README.md", b"x"), ("2brain-master/brain-main/conventions/N.md", b"x")])
    dest = new_dir("fetch-dest-nob")
    rc, data = runj(fetch, "--dest", dest, "--source-url", nob.as_uri())
    check("fetch: archive sans Bootstrap -> message clair, code 3",
          rc == 3 and "ne contient pas encore le Bootstrap" in data.get("erreur", ""), data)
    nobm = good.parent / "nobrainmain.tar.gz"
    make_tar(nobm, [("2brain-master/README.md", b"x")])
    rc, data = runj(fetch, "--dest", new_dir("fetch-dest-nobm"), "--source-url", nobm.as_uri())
    check("fetch: archive sans brain-main/ refusée", rc == 4, data)
    rc, data = runj(fetch, "--dest", new_dir("fetch-dest-http"), "--source-url", "http://example.com/x.tgz")
    check("fetch: URL http refusée", rc == 2, data)


class Env:
    """Source récupérée (copie locale) partagée par les tests d'installation."""


def setup_fetched():
    root = new_dir("fetched-src")
    copy_brain_main(root)
    dest = new_dir("fetched")
    rc, data = runj(ACCOUNT / "fetch-source.py", "--dest", dest, "--source-dir", root)
    Env.source = dest / "brain-main"
    check("préparation: source récupérée", rc == 0, data)


def prepare(project, answers=ANSWERS_FULL, source=None):
    return runj(SCRIPTS / "prepare-install.py", "--source", source or Env.source, "--project", project,
                "--answers", json.dumps(answers))


def apply(project, sha_value):
    return runj(SCRIPTS / "apply-install.py", "--project", project, "--plan-sha256", sha_value)


def project_with_files(name="proj"):
    return mk(new_dir(name), {"README.md": "# Projet\n", "src/app.py": "print(1)\n", ".gitignore": "x\n", "CLAUDE.md": "perso\n"})


def test_prepare_cancel():
    proj = project_with_files("prep")
    before = tree(proj)
    rc, data = prepare(proj)
    emp = proj / "EMPLACEMENT"
    check("prévisualisation: EMPLACEMENT créé avec marqueur, plan, brain-main, brain",
          rc == 0 and (emp / ".2brain-emplacement").is_file() and (emp / "plan.json").is_file()
          and (emp / "brain-main" / "bootstrap" / "VERSION").is_file() and (emp / "brain" / "docs" / "VISION.md").is_file(), data)
    check("prévisualisation: plan_sha256 = empreinte de plan.json", data.get("plan_sha256") == sha(emp / "plan.json"), data)
    check("prévisualisation: projet inchangé hors EMPLACEMENT", tree(proj, skip=("EMPLACEMENT",)) == before)
    check("prévisualisation: AGENTS.md prévu car absent, CLAUDE.md existant non touché",
          (emp / "AGENTS.md").is_file() and data.get("agents_md_cree") is True and (proj / "CLAUDE.md").read_text(encoding="utf-8") == "perso\n", data)
    plan = json.loads((emp / "plan.json").read_text(encoding="utf-8"))
    check("prévisualisation: plan.json contient réponses, source, commit, conflits, adaptations, empreintes",
          plan["reponses"]["nature"] == "Site web vitrine" and plan["source_commit"] and "conflits" in plan
          and "adaptations" in plan and all(len(v) == 64 for v in plan["fichiers"].values()), plan.keys())
    check("prévisualisation: pas de manifeste prévu", "brain-main/manifest.json" not in plan["fichiers"])
    # nouvelle prévisualisation par-dessus la précédente
    rc, data2 = prepare(proj)
    check("prévisualisation: refaite sur EMPLACEMENT marqué", rc == 0 and data2["plan_sha256"] == data["plan_sha256"], data2)
    rc, out = runj(SCRIPTS / "cancel-install.py", "--project", proj)
    check("annulation: EMPLACEMENT supprimé, projet strictement identique",
          rc == 0 and out.get("supprime") is True and not emp.exists() and tree(proj) == before, out)
    rc, out = runj(SCRIPTS / "cancel-install.py", "--project", proj)
    check("annulation: rien à annuler = ok", rc == 0 and out.get("supprime") is False, out)

    # EMPLACEMENT étranger
    proj2 = mk(new_dir("prep-foreign"), {"EMPLACEMENT/perso.txt": "à moi\n", "a.txt": "x"})
    before2 = tree(proj2)
    rc, data = prepare(proj2)
    check("EMPLACEMENT étranger: prévisualisation refusée sans toucher", rc == 2 and tree(proj2) == before2, data)
    rc, out = runj(SCRIPTS / "cancel-install.py", "--project", proj2)
    check("EMPLACEMENT étranger: annulation refusée, jamais supprimé", rc == 2 and tree(proj2) == before2, out)
    rc, out = runj(SCRIPTS / "apply-install.py", "--project", proj2, "--plan-sha256", "0" * 64)
    check("EMPLACEMENT étranger: installation refusée", rc == 2 and tree(proj2) == before2, out)

    # AGENTS.md existant
    proj3 = mk(new_dir("prep-agents"), {"AGENTS.md": "# à moi\n", "a.txt": "x"})
    before3 = tree(proj3)
    rc, data = prepare(proj3)
    check("AGENTS.md existant: non prévu, adaptation signalée, fichier intact",
          rc == 0 and not (proj3 / "EMPLACEMENT" / "AGENTS.md").exists() and any("AGENTS.md existe" in a for a in data["adaptations"])
          and tree(proj3, skip=("EMPLACEMENT",)) == before3, data)

    # dépôt source
    src = fake_source_repo("prep-srcrepo")
    before4 = tree(src, skip=(".git",))
    rc, data = prepare(src)
    check("dépôt source: prévisualisation refusée, rien créé", rc == 2 and tree(src, skip=(".git",)) == before4, data)
    rc, data = apply(src, "0" * 64)
    check("dépôt source: installation refusée", rc == 2, data)

    # projet déjà équipé
    for name in ("brain-main", "brain"):
        pe = mk(new_dir("prep-equipped"), {name + "/x.md": "x"})
        b = tree(pe)
        rc, data = prepare(pe)
        check("projet déjà équipé (%s/): refus, rien créé" % name, rc == 2 and tree(pe) == b, data)
    rc, data = runj(SCRIPTS / "prepare-install.py", "--source", Env.source, "--project", project_with_files("prep-badans"),
                    "--answers", "{pas du json")
    check("réponses illisibles: refus", rc == 2, data)


def doc_text(proj, name):
    return (proj / "brain" / "docs" / name).read_text(encoding="utf-8")


def test_install():
    proj = project_with_files("inst")
    others = tree(proj)
    rc, prep = prepare(proj)
    rc, data = apply(proj, prep["plan_sha256"])
    check("installation validée: code 0, bilan", rc == 0 and data.get("ok") and data["bilan"]["validateur_registre"] == "ok", data)
    check("installation: brain-main/ et brain/ créés",
          (proj / "brain-main" / "ROUTER.md").is_file() and (proj / "brain" / "registry" / "entries.jsonl").is_file()
          and all((proj / "brain" / "docs" / n).is_file() for n in ("VISION.md", "PROJECT.md", "REQUIREMENTS.md", "ARCHITECTURE.md", "WORKFLOW.md")))
    check("installation: autres fichiers intacts",
          all(tree(proj).get(k) == v for k, v in others.items()))
    check("installation: EMPLACEMENT supprimé", not (proj / "EMPLACEMENT").exists())
    check("installation: AGENTS.md créé depuis le modèle", (proj / "AGENTS.md").is_file() and "brain-main/" in (proj / "AGENTS.md").read_text(encoding="utf-8"))
    man = json.loads((proj / "brain-main" / "manifest.json").read_text(encoding="utf-8"))
    check("installation: manifeste (version, commit, date, empreintes, sans lui-même)",
          man["version"] == (BOOT / "VERSION").read_text(encoding="utf-8").strip() and man["source_commit"] and man["installed"]
          and "manifest.json" not in man["files"] and all(sha(proj / "brain-main" / k) == v for k, v in man["files"].items())
          and len(man["files"]) > 10, list(man)[:6])
    rc, out, _ = run(proj / "brain-main/skills/brain-capture/scripts/validate-registry.py", proj / "brain/registry/entries.jsonl")
    check("installation: validateur de registre OK sur le registre généré", rc == 0 and "OK bloquant: oui" in out, out)
    check("installation: registre vide", (proj / "brain/registry/entries.jsonl").read_bytes() == b"")
    check("installation: SCHEMA.md de brain-main identique à celui du dépôt source (brain/registry/SCHEMA.md)",
          sha(REPO / "brain-main/registry/SCHEMA.md") == sha(REPO / "brain/registry/SCHEMA.md"))
    check("installation: SCHEMA.md installé dans brain-main/registry/, pas dans brain/",
          sha(proj / "brain-main/registry/SCHEMA.md") == sha(REPO / "brain/registry/SCHEMA.md") and not (proj / "brain/registry/SCHEMA.md").exists())
    check("installation: aucun __pycache__", not list(proj.rglob("__pycache__")))
    vision, project_md = doc_text(proj, "VISION.md"), doc_text(proj, "PROJECT.md")
    check("installation: front matter conforme METADATA (6 champs obligatoires)",
          all(doc_text(proj, n).startswith("---\nmetadata_version: 1\n") and all(("\n%s:" % k) in doc_text(proj, n).split("\n---\n")[0]
              for k in ("title", "type", "description", "status", "created", "updated")) for n in
              ("VISION.md", "PROJECT.md", "REQUIREMENTS.md", "ARCHITECTURE.md", "WORKFLOW.md")))
    check("installation: réponses reprises (objectif, nature, identité, outils)",
          "Présenter l'activité" in vision and "Site web vitrine" in project_md and "Atelier Test" in project_md
          and "Next.js" in doc_text(proj, "ARCHITECTURE.md"))
    check("installation: modules notés comme intention, sans activation",
          "- recherche" in project_md and "rien n'est activé" in project_md)
    check("installation: aucun placeholder restant", not any("{{" in p.read_text(encoding="utf-8") for p in (proj / "brain").rglob("*.md")))
    rc, prep2 = prepare(proj)
    check("installation: projet désormais équipé -> nouvelle prévisualisation refusée", rc == 2, prep2)

    # réponses nulles
    projn = project_with_files("inst-null")
    rc, prep = prepare(projn, ANSWERS_NULL)
    rc, data = apply(projn, prep["plan_sha256"])
    texts = "".join(doc_text(projn, n) for n in ("VISION.md", "PROJECT.md", "ARCHITECTURE.md"))
    check("réponses nulles: non bloquantes, « À compléter. » partout, rien d'inventé",
          rc == 0 and texts.count("À compléter.") >= 6 and "{{" not in texts and "None" not in texts, data)
    rc, prep = runj(SCRIPTS / "prepare-install.py", "--source", Env.source, "--project", project_with_files("inst-empty-json"), "--answers", "{}")
    check("réponses vides ({}): acceptées", rc == 0, prep)

    # plan altéré
    pa = project_with_files("inst-alt")
    rc, prep = prepare(pa)
    before = tree(pa)
    plan_path = pa / "EMPLACEMENT" / "plan.json"
    plan = json.loads(plan_path.read_text(encoding="utf-8"))
    plan["reponses"]["nature"] = "altéré"
    plan_path.write_text(json.dumps(plan), encoding="utf-8")
    rc, data = apply(pa, prep["plan_sha256"])
    check("plan altéré: refus code 3, rien écrit", rc == 3 and tree(pa) == {**before, "EMPLACEMENT/plan.json": sha(plan_path)}
          and not (pa / "brain-main").exists(), data)
    rc, prep = prepare(pa)
    rc, data = apply(pa, "f" * 64)
    check("empreinte validée incorrecte: refus code 3", rc == 3 and not (pa / "brain-main").exists(), data)
    (pa / "EMPLACEMENT" / "brain" / "docs" / "VISION.md").write_text("altéré", encoding="utf-8")
    rc, data = apply(pa, prep["plan_sha256"])
    check("arborescence EMPLACEMENT altérée: refus code 3", rc == 3 and not (pa / "brain-main").exists() and not (pa / "brain").exists(), data)
    (pa / "EMPLACEMENT" / "intrus.txt").write_text("x", encoding="utf-8")
    rc, prep = prepare(pa)
    check("EMPLACEMENT marqué: refait proprement (intrus supprimé)", rc == 0 and not (pa / "EMPLACEMENT" / "intrus.txt").exists())
    (pa / "EMPLACEMENT" / "intrus.txt").write_text("x", encoding="utf-8")
    rc, data = apply(pa, prep["plan_sha256"])
    check("fichier en trop dans EMPLACEMENT: refus code 3", rc == 3 and not (pa / "brain-main").exists(), data)

    # conflit apparu après validation
    for what in ("AGENTS.md", "brain", "brain-main"):
        pc = project_with_files("inst-conflict")
        rc, prep = prepare(pc)
        if what == "AGENTS.md":
            (pc / what).write_text("apparu\n", encoding="utf-8")
        else:
            (pc / what).mkdir()
            (pc / what / "x.md").write_text("apparu\n", encoding="utf-8")
        b = tree(pc)
        rc, data = apply(pc, prep["plan_sha256"])
        check("conflit apparu après validation (%s): code 4, rien écrit, EMPLACEMENT conservé" % what,
              rc == 4 and tree(pc) == b and (pc / "EMPLACEMENT" / "plan.json").is_file() and what in " ".join(data.get("conflits", [])), data)
    rc, data = apply(project_with_files("inst-noprep"), "0" * 64)
    check("installation sans prévisualisation: refus", rc == 2, data)


IMPORTS = ["brain-main/conventions/README.md", "brain-main/conventions/TAXONOMY.md", "brain-main/README.md"]
DOCS5 = ("VISION.md", "CADRAGE.md", "PROJECT.md", "REQUIREMENTS.md", "ARCHITECTURE.md", "WORKFLOW.md")  # nom conservé ; six documents standards depuis le 2026-11-06


def import_lines(text):
    """Lignes d'import @chemin hors blocs de code (une ligne qui commence par @)."""
    out, fence = [], False
    for line in text.splitlines():
        if line.strip().startswith("```"):
            fence = not fence
            continue
        if not fence and line.strip().startswith("@"):
            out.append(line.strip()[1:])
    return out


def stray_at(text):
    """Arobases hors blocs et hors code en ligne, en dehors des lignes d'import (import involontaire possible)."""
    cleaned, fence = [], False
    for line in text.splitlines():
        if line.strip().startswith("```"):
            fence = not fence
            continue
        if not fence and not line.strip().startswith("@"):
            cleaned.append(re.sub(r"`[^`]*`", "", line))
    return re.findall(r"(?:^|\s)@[\w./-]+", "\n".join(cleaned))


def test_root_files():
    # CLAUDE.md et AGENTS.md absents : créés, affichés dans le plan
    pn = mk(new_dir("root-none"), {"README.md": "# P" + NL})
    rc, prep = prepare(pn)
    plan = json.loads((pn / "EMPLACEMENT" / "plan.json").read_text(encoding="utf-8"))
    check("racine absente: CLAUDE.md et AGENTS.md dans le plan et dans le résumé",
          rc == 0 and prep["racine_creee"] == ["AGENTS.md", "CLAUDE.md"] and "CLAUDE.md" in plan["fichiers"] and "AGENTS.md" in plan["fichiers"]
          and prep["adaptations"] == [] and prep["conflits"] == [], prep)
    rc, data = apply(pn, prep["plan_sha256"])
    claude = (pn / "CLAUDE.md").read_text(encoding="utf-8") if (pn / "CLAUDE.md").is_file() else ""
    agents = (pn / "AGENTS.md").read_text(encoding="utf-8") if (pn / "AGENTS.md").is_file() else ""
    check("racine absente: CLAUDE.md = ligne @AGENTS.md, AGENTS.md créé, bilan", rc == 0 and claude == "@AGENTS.md" + NL
          and agents.startswith("---" + NL) and data["bilan"]["claude_md"] and data["bilan"]["agents_md"], data)
    check("AGENTS.md installé: dates rendues, aucun placeholder", "{{" not in agents and "created: " in agents, agents[:300])
    check("AGENTS.md installé: les 3 imports, dans l'ordre, hors de tout bloc de code", import_lines(agents) == IMPORTS, import_lines(agents))
    check("AGENTS.md installé: chaque fichier importé existe dans le projet installé",
          all((pn / rel).is_file() for rel in import_lines(agents)), [r for r in import_lines(agents) if not (pn / r).is_file()])
    check("AGENTS.md installé: ordre répété en clair (sans arobase), consigne -o, séparation brain/brain-main",
          "dans le même ordre" in agents and "`/brain -o`" in agents and "une seule fois par session" in agents
          and "jamais dans `brain-main/`" in agents and "À compléter" in agents, agents)
    check("AGENTS.md installé: METADATA.md et NAMING.md non importés, consigne obligatoire présente (modèle et guide)",
          not any("METADATA" in x or "NAMING" in x for x in import_lines(agents))
          and all(w in agents for w in ("Obligatoire avant de créer ou modifier un document", "brain-main/conventions/NAMING.md", "brain-main/conventions/METADATA.md"))
          and "Obligatoire avant de créer ou modifier un document" in (pn / "brain-main/README.md").read_text(encoding="utf-8"))
    check("AGENTS.md installé: aucun arobase hors des imports", stray_at(agents) == [], stray_at(agents))
    check("fichiers importés: aucun arobase involontaire (pas d'import en chaîne)",
          all(stray_at((pn / rel).read_text(encoding="utf-8")) == [] for rel in IMPORTS + ["brain-main/conventions/METADATA.md", "brain-main/conventions/NAMING.md"]),
          [(r, stray_at((pn / r).read_text(encoding="utf-8"))) for r in IMPORTS])
    check("modèles: CLAUDE.md = @AGENTS.md ; AGENTS.md importe dans l'ordre",
          (BOOT / "templates/CLAUDE.md").read_text(encoding="utf-8").strip() == "@AGENTS.md"
          and import_lines((BOOT / "templates/AGENTS.md").read_text(encoding="utf-8")) == IMPORTS)

    # les deux existent : jamais modifiés, adaptation exacte
    pe = mk(new_dir("root-both"), {"CLAUDE.md": "perso claude" + NL, "AGENTS.md": "perso agents" + NL})
    before = tree(pe)
    rc, prep = prepare(pe)
    ad = prep["adaptations"]
    check("racine existante: rien créé à la racine, adaptations signalées avec les lignes exactes",
          rc == 0 and prep["racine_creee"] == [] and len(ad) == 2
          and all("@" + rel in ad[0] for rel in IMPORTS) and ad[0].index("@" + IMPORTS[0]) < ad[0].index("@" + IMPORTS[-1])
          and "/brain -o" in ad[0] and ad[1].endswith("@AGENTS.md"), ad)
    check("racine existante: la prévisualisation n'écrit que EMPLACEMENT", tree(pe, skip=("EMPLACEMENT",)) == before)
    rc, data = apply(pe, prep["plan_sha256"])
    check("racine existante: installation ok, CLAUDE.md et AGENTS.md jamais modifiés",
          rc == 0 and tree(pe, skip=("brain-main", "brain", ".claude", ".gitignore")) == before and data["bilan"]["adaptations"] == ad, data)

    # AGENTS.md existe seul : CLAUDE.md créé, AGENTS.md intact
    pa = mk(new_dir("root-agents"), {"AGENTS.md": "perso agents" + NL})
    before = tree(pa)
    rc, prep = prepare(pa)
    rc2, data = apply(pa, prep["plan_sha256"])
    check("AGENTS.md seul: CLAUDE.md créé, AGENTS.md intact, adaptation = lignes à ajouter à AGENTS.md",
          rc == 0 and rc2 == 0 and prep["racine_creee"] == ["CLAUDE.md"] and (pa / "CLAUDE.md").read_text(encoding="utf-8") == "@AGENTS.md" + NL
          and sha(pa / "AGENTS.md") == before["AGENTS.md"] and len(prep["adaptations"]) == 1 and "@" + IMPORTS[0] in prep["adaptations"][0], prep)

    # CLAUDE.md existe seul : AGENTS.md créé, CLAUDE.md intact, ligne @AGENTS.md à ajouter
    pc = mk(new_dir("root-claude"), {"CLAUDE.md": "perso claude" + NL})
    rc, prep = prepare(pc)
    rc2, data = apply(pc, prep["plan_sha256"])
    check("CLAUDE.md seul: AGENTS.md créé, CLAUDE.md intact, adaptation = ligne @AGENTS.md",
          rc == 0 and rc2 == 0 and prep["racine_creee"] == ["AGENTS.md"] and (pc / "CLAUDE.md").read_text(encoding="utf-8") == "perso claude" + NL
          and len(prep["adaptations"]) == 1 and prep["adaptations"][0].endswith("@AGENTS.md"), prep)

    # déjà conforme : aucune adaptation
    done = "".join("@" + r + NL for r in IMPORTS) + "- Si `brain/docs/` contient encore « À compléter » : proposer `/brain -a` si le projet a des fichiers et aucun rapport dans `brain/sources/audits/`, sinon `/brain -o`, une seule fois par session." + NL
    pd = mk(new_dir("root-done"), {"CLAUDE.md": "@AGENTS.md" + NL, "AGENTS.md": done})
    rc, prep = prepare(pd)
    check("racine déjà conforme: aucune adaptation, rien créé", rc == 0 and prep["adaptations"] == [] and prep["racine_creee"] == [], prep)

    # conflit apparu après validation sur CLAUDE.md
    pk = mk(new_dir("root-conflict"), {"README.md": "x"})
    rc, prep = prepare(pk)
    (pk / "CLAUDE.md").write_text("apparu" + NL, encoding="utf-8")
    b = tree(pk)
    rc, data = apply(pk, prep["plan_sha256"])
    check("conflit apparu sur CLAUDE.md: code 4, rien écrit", rc == 4 and tree(pk) == b and "CLAUDE.md" in data.get("conflits", []), data)


def installed_project(name, answers):
    proj = mk(new_dir(name), {"README.md": "# P" + NL})
    rc, prep = prepare(proj, answers)
    rc, data = apply(proj, prep["plan_sha256"])
    assert rc == 0, data
    return proj


def test_generated_brain():
    forbidden = re.compile(r"2brain|brain-main|bootstrap|capture|cap-", re.I)
    for label, answers in (("réponses complètes", ANSWERS_FULL), ("réponses nulles", ANSWERS_NULL)):
        proj = installed_project("gen-" + label.split()[1], answers)
        files = sorted(p.relative_to(proj / "brain").as_posix() for p in (proj / "brain").rglob("*") if p.is_file())
        check("brain/ généré (%s): seulement les 6 documents, registry/entries.jsonl et settings.json" % label,
              files == sorted(["docs/" + n for n in DOCS5] + ["registry/entries.jsonl", "settings.json"]), files)
        dirs = sorted(p.relative_to(proj / "brain").as_posix() for p in (proj / "brain").rglob("*") if p.is_dir())
        check("brain/ généré (%s): seulement docs/ et registry/" % label, dirs == ["docs", "registry"], dirs)
        hits = []
        for p in (proj / "brain").rglob("*"):
            rel = p.relative_to(proj / "brain").as_posix()
            if forbidden.search(rel):
                hits.append((rel, "chemin"))
            if p.is_file():
                for m in forbidden.finditer(p.read_text(encoding="utf-8")):
                    hits.append((rel, m.group(0)))
        check("brain/ généré (%s): aucune occurrence (insensible à la casse) de 2brain, brain-main, bootstrap, capture, CAP-" % label, not hits, hits[:5])
        texts = [p.read_text(encoding="utf-8") for p in (proj / "brain").rglob("*.md")]
        check("brain/ généré (%s): ni « créé à l'installation » ni renvoi au standard du dépôt" % label,
              not any("installation" in t.lower() or "dépôt" in t for t in texts))
    projn = installed_project("gen-fm", ANSWERS_NULL)
    check("brain/ généré: front matter METADATA, plan en 6 sections, réponses vides en « À compléter. »",
          all(doc_text(projn, n).startswith("---" + NL + "metadata_version: 1" + NL) and doc_text(projn, n).count(NL + "## ") == 6 for n in DOCS5)
          and doc_text(projn, "VISION.md").count("À compléter.") >= 4)
    check("brain/ généré: PROJECT.md note les modules comme intention seulement",
          "Modules envisagés (intention seulement ; rien n'est activé)" in doc_text(installed_project("gen-mod", ANSWERS_FULL), "PROJECT.md"))


def test_status_todo():
    home = new_dir("home-todo")
    proj = installed_project("todo-proj", ANSWERS_NULL)
    rc, st = status(proj, home)
    check("status: installation neuve -> 6 documents « À compléter », -o suggéré",
          st["brain"]["etat"] == "complet" and st["brain"]["documents_a_completer"] == 6
          and any("/brain -o" in n for n in st["notes"]), st)
    rc, out, _ = run(ACCOUNT / "status.py", "--project", proj, "--home", home, "--offline")
    check("status: sortie texte avec le nombre et la suggestion de -o", "encore « À compléter » : 6" in out and "/brain -o" in out, out)
    for n in ("VISION.md", "PROJECT.md"):
        path = proj / "brain" / "docs" / n
        path.write_text(path.read_text(encoding="utf-8").replace("À compléter", "Renseigné"), encoding="utf-8")
    rc, st = status(proj, home)
    check("status: 2 documents remplis -> 4 restants, noms listés",
          st["brain"]["documents_a_completer"] == 4
          and st["brain"]["documents_a_completer_noms"] == ["ARCHITECTURE.md", "CADRAGE.md", "REQUIREMENTS.md", "WORKFLOW.md"], st["brain"])
    for n in ("REQUIREMENTS.md", "ARCHITECTURE.md", "WORKFLOW.md", "CADRAGE.md"):
        path = proj / "brain" / "docs" / n
        path.write_text(path.read_text(encoding="utf-8").replace("À compléter", "Renseigné"), encoding="utf-8")
    before = tree(proj)
    rc, st = status(proj, home)
    check("status: 0 document « À compléter » -> aucune suggestion de -o, brain complet",
          st["brain"]["documents_a_completer"] == 0 and st["brain"]["etat"] == "complet" and not any("/brain -o" in n for n in st["notes"]), st)
    check("status: lecture seule", tree(proj) == before)
    rc, st = status(mk(new_dir("todo-abs"), {"a.txt": "x"}), home)
    check("status: brain absent -> 0 et pas de suggestion",
          st["brain"]["documents_a_completer"] == 0 and not any("/brain -o" in n for n in st["notes"]), st)


def test_onboarding_files():
    skill = REPO / "brain-main/skills/brain-onboarding/SKILL.md"
    text = skill.read_text(encoding="utf-8")
    head = text.split("---")[1]
    check("onboarding: SKILL.md existe, front matter sur le modèle de brain-capture", skill.is_file()
          and "name: brain-onboarding" in head and "disable-model-invocation: true" in head and "description:" in head
          and "version:" in head, head)
    check("onboarding: au plus 80 lignes", len(text.splitlines()) <= 80, len(text.splitlines()))
    forbidden = ("UPPER_SNAKE_CASE", "kebab-case", "metadata_version", "SemVer", "snake_case", "camelCase", "TAXONOMY.md")
    check("onboarding: aucune convention reprise", not any(w in text for w in forbidden))
    check("onboarding: marqueurs confirmé/hypothèse/à compléter, validation explicite, état par status.py, renvoi à /brain -i",
          all(w in text for w in ("`confirmé`", "`hypothèse`", "`à compléter`", "validation explicite", "status.py", "/brain -i")), text[:200])
    check("onboarding: ne modifie ni le registre ni brain-main/, ne décide rien, ne code rien",
          "ni le registre, ni `brain-main/`" in text and "Ne rien décider, ne rien coder" in text)
    check("onboarding: /compact absent", "/compact" not in text)
    proj = installed_project("onb-proj", ANSWERS_FULL)
    check("onboarding: présent dans le projet installé", sha(proj / "brain-main/skills/brain-onboarding/SKILL.md") == sha(skill))
    guide = (REPO / "brain-main/README.md").read_text(encoding="utf-8")
    check("guide: au plus 80 lignes", len(guide.splitlines()) <= 80, len(guide.splitlines()))
    check("guide: chaque commande présente", all(("`%s`" % c) in guide for c in
          ("/brain", "/brain -i", "/brain -u", "/brain -o", "/brain -c", "/brain -c <fichier>", "/brain -c 1", "/brain -s <sujet>", "/brain -r")))
    paths = re.findall(r"`(brain-main/[\w./-]+\.md)`", guide)
    check("guide: chemins de fichiers cités existent dans le projet installé (et skill d'onboarding et de capture cités)",
          bool(paths) and all((proj / p).is_file() for p in paths)
          and "brain-main/skills/brain-onboarding/SKILL.md" in paths and "brain-main/skills/brain-capture/SKILL.md" in paths,
          [p for p in paths if not (proj / p).is_file()])
    check("guide: règles (append only, proposition, locked, commit, convention avant création) et /compact absent",
          all(w in guide for w in ("append only", "Une proposition n'est pas une décision", "locked: true", "commit", "lire la convention concernée"))
          and "/compact" not in guide)
    check("guide: aucune copie de convention", not any(w in guide for w in ("UPPER_SNAKE_CASE", "kebab-case", "SemVer")))
    check("guide: aucune mention de documents inexistants", "COMMANDS.md" not in guide and "BOOTSTRAP.md" not in guide)


def test_onboarding_rules():
    text = (REPO / "brain-main/skills/brain-onboarding/SKILL.md").read_text(encoding="utf-8")
    flat = " ".join(text.split())
    check("onboarding: confirmé seulement si la source le dit explicitement, le déduit est une hypothèse",
          "seulement si la source le dit explicitement" in flat and "tout ce qui est déduit" in flat
          and "l'absence d'un fichier" in flat and "script" in flat)
    check("onboarding: les questions restent posées, une réponse brève ne dispense pas, seul l'humain arrête, reste « À compléter. »",
          "Les questions restent posées" in flat and "ne dispense pas des suivantes" in flat
          and "seul l'humain peut demander explicitement d'arrêter" in flat and "« À compléter. »" in flat)
    check("onboarding: validation document par document, une réponse globale n'est pas une validation",
          "Valider, document par document" in flat and "Nommer chaque document" in flat
          and "Une réponse globale" in flat and "n'est pas une validation" in flat)
    check("onboarding: au plus 80 lignes (règles ajoutées)", len(text.splitlines()) <= 80, len(text.splitlines()))


def test_gitignore():
    tpl = BOOT / "templates/gitignore.txt"
    rules = [l for l in tpl.read_text(encoding="utf-8").splitlines() if l and not l.startswith("#")]
    check("gitignore: modèle sous templates/gitignore.txt, trois règles exactes, commentaire d'explication",
          rules == ["brain/registry/_capture.lock", "brain/registry/_capture-staging/", "brain/registry/_capture-history/"]
          and tpl.read_text(encoding="utf-8").startswith("#"), rules)
    stray = [p.relative_to(BOOT).as_posix() for p in (BOOT / "templates").rglob(".gitignore")]
    check("gitignore: aucun .gitignore parasite sous templates/", not stray, stray)
    # absent : créé
    p0 = mk(new_dir("gi-absent"), {"README.md": "x" + NL})
    rc, prep = prepare(p0)
    emp_gi = p0 / "EMPLACEMENT" / ".gitignore"
    check("gitignore: absent -> prévisualisation le prévoit, contenu du modèle, aucune adaptation",
          rc == 0 and prep["gitignore_cree"] is True and emp_gi.is_file() and emp_gi.read_bytes() == tpl.read_bytes()
          and ".gitignore" in json.loads((p0 / "EMPLACEMENT/plan.json").read_text(encoding="utf-8"))["fichiers"]
          and not any(".gitignore" in a for a in prep["adaptations"]), prep)
    rc, data = apply(p0, prep["plan_sha256"])
    check("gitignore: absent -> créé à l'installation et au bilan",
          rc == 0 and (p0 / ".gitignore").read_bytes() == tpl.read_bytes() and data["bilan"]["gitignore"] is True, data)
    inst = sorted(p.name for p in (p0 / "brain").rglob("*") if p.is_file())
    check("gitignore: brain/ limité aux 6 documents, au registre et à settings.json, sans trace du système",
          inst == ["ARCHITECTURE.md", "CADRAGE.md", "PROJECT.md", "REQUIREMENTS.md", "VISION.md", "WORKFLOW.md", "entries.jsonl", "settings.json"], inst)
    # existant sans les lignes : inchangé à l'octet, lignes manquantes signalées
    raw = b"node_modules/\r\n.env\r\n"
    p1 = new_dir("gi-exist")
    (p1 / ".gitignore").write_bytes(raw)
    rc, prep = prepare(p1)
    ad = [a for a in prep["adaptations"] if a.startswith(".gitignore existe")]
    check("gitignore: existant -> non prévu, adaptation avec les trois lignes exactes",
          rc == 0 and prep["gitignore_cree"] is False and not (p1 / "EMPLACEMENT/.gitignore").exists()
          and len(ad) == 1 and ad[0].endswith(":" + NL + NL.join(rules)), prep["adaptations"])
    rc, data = apply(p1, prep["plan_sha256"])
    check("gitignore: existant -> inchangé à l'octet après installation, bilan sans création",
          rc == 0 and (p1 / ".gitignore").read_bytes() == raw and data["bilan"]["gitignore"] is False
          and data["bilan"]["adaptations"] == prep["adaptations"], data)
    # une seule ligne présente : seule la manquante est signalée
    p2 = new_dir("gi-one")
    (p2 / ".gitignore").write_text("  brain/registry/_capture.lock  " + NL, encoding="utf-8")
    rc, prep = prepare(p2)
    ad = [a for a in prep["adaptations"] if a.startswith(".gitignore existe")]
    check("gitignore: une ligne déjà présente -> seules les autres sont signalées",
          rc == 0 and len(ad) == 1 and ad[0].endswith(":" + NL + NL.join(rules[1:])), ad)
    # les lignes y sont : aucune adaptation
    p3 = new_dir("gi-full")
    (p3 / ".gitignore").write_text("a" + NL + NL.join(rules) + NL, encoding="utf-8")
    rc, prep = prepare(p3)
    check("gitignore: lignes déjà présentes -> aucune adaptation, rien créé",
          rc == 0 and not any(".gitignore" in a for a in prep["adaptations"]) and prep["gitignore_cree"] is False, prep["adaptations"])
    # mise à jour : modèle traité comme les autres, rien hors de brain-main/
    proj = installed_project("gi-upd", ANSWERS_FULL)
    src = new_dir("gi-upd-src")
    shutil.copytree(Env.source, src / "brain-main")
    (src / "brain-main/bootstrap/templates/gitignore.txt").write_text(tpl.read_text(encoding="utf-8") + "# plus tard" + NL, encoding="utf-8")
    before = tree(proj, skip=("brain-main",))
    rc, plan = runj(SCRIPTS / "update-brain-main.py", "--source", src / "brain-main", "--project", proj)
    check("gitignore: -u signale le changement du modèle comme adaptation, sans écrire",
          rc == 0 and any(a["path"] == "bootstrap/templates/gitignore.txt" for a in plan["adaptations_brain"])
          and tree(proj, skip=("brain-main",)) == before, plan)
    rc, done = runj(SCRIPTS / "update-brain-main.py", "--source", src / "brain-main", "--project", proj,
                    "--apply", "--plan-sha256", plan["plan_sha256"])
    check("gitignore: -u --apply n'écrit rien hors de brain-main/ (projet inchangé)",
          rc == 0 and tree(proj, skip=("brain-main",)) == before, done)


NON_ASCII = re.compile(r"[^\x00-\x7f]")


def run_cp1252(script, *args, inp=None):
    """Lance `python -I` dans un environnement forcé en cp1252 et PYTHONUTF8=0 (-I les ignore : le script doit s'en passer)."""
    env = dict(os.environ, PYTHONIOENCODING="cp1252", PYTHONUTF8="0")
    p = subprocess.run([sys.executable, "-I", "-B", str(script), *[str(a) for a in args]],
                       capture_output=True, input=inp, env=env, timeout=180)
    return p.returncode, p.stdout, p.stderr


def decode_utf8(data):
    try:
        return data.decode("utf-8")
    except UnicodeDecodeError:
        return None


def test_utf8_output():
    cap = REPO / "brain-main/skills/brain-capture/scripts"
    search = REPO / "brain-main/skills/brain-search/scripts/search-registry.py"
    review = REPO / "brain-main/skills/brain-review/scripts/review-registry.py"
    proj = mk(new_dir("u8-proj"), {"README.md": "x"})
    home = new_dir("u8-home")
    empty = new_dir("u8-empty")
    reg = mk(new_dir("u8-reg"), {"entries.jsonl": ""}) / "entries.jsonl"
    # (nom, script, arguments) : un cas qui imprime un caractère non ASCII, sinon --help
    cases = [
        ("status.py", ACCOUNT / "status.py", ["--project", proj, "--home", home, "--offline"]),
        ("fetch-source.py", ACCOUNT / "fetch-source.py", ["--help"]),
        ("install-account.py", SCRIPTS / "install-account.py", ["--home", home, "--dry-run"]),
        ("prepare-install.py", SCRIPTS / "prepare-install.py", ["--source", Env.source, "--project", proj, "--answers", "pas du json"]),
        ("apply-install.py", SCRIPTS / "apply-install.py", ["--project", proj, "--plan-sha256", "abc"]),
        ("cancel-install.py", SCRIPTS / "cancel-install.py", ["--project", proj]),
        ("set-router.py", SCRIPTS / "set-router.py", ["--project", proj, "--state", "on"]),
        ("update-brain-main.py", SCRIPTS / "update-brain-main.py", ["--source", empty, "--project", proj]),
        ("read-session.py", cap / "read-session.py", ["--session-id", "zzz", "--project", proj, "--config-dir", home]),
        ("append-captures.py", cap / "append-captures.py", ["--help"]),
        ("rollback-captures.py", cap / "rollback-captures.py", ["--n", "2"]),
        ("validate-registry.py", cap / "validate-registry.py", [reg]),
        ("search-registry.py", search, ["--project", proj]),
        ("review-registry.py", review, ["--limit", "-1", "--project", proj]),
    ]
    for name, script, args in cases:
        rc, out, err = run_cp1252(script, *args)
        text = decode_utf8(out + err)
        check("utf-8 sous cp1252: %s sans UnicodeEncodeError, sortie décodable en UTF-8, caractère non ASCII imprimé" % name,
              b"UnicodeEncodeError" not in err and b"Traceback" not in err and text is not None and NON_ASCII.search(text), (rc, err[-300:], out[:200]))
    # tous les --help (descriptions accentuées), sans erreur d'encodage
    helped = [(n, s) for n, s, _ in cases] + [("rollback-captures.py", cap / "rollback-captures.py")]
    for name, script in dict(helped).items():
        rc, out, err = run_cp1252(script, "--help")
        check("utf-8 sous cp1252: %s --help sans erreur d'encodage" % name,
              rc == 0 and b"UnicodeEncodeError" not in err and decode_utf8(out + err) is not None, (rc, err[-300:]))
    # le hook reste en JSON ASCII
    d = new_dir("u8-hook")
    rc, out, err = run_cp1252(ACCOUNT / "session-start.py", inp=json.dumps({"cwd": str(d), "source": "startup"}).encode("utf-8"))
    try:
        payload = json.loads(out.decode("ascii"))
    except Exception:
        payload = None
    check("utf-8 sous cp1252: le hook sort un JSON ASCII, rien sur stderr, code 0",
          rc == 0 and err == b"" and payload and "installer 2brain" in payload["hookSpecificOutput"]["additionalContext"], (rc, out[:200], err))
    # hook : cwd non ASCII lu sur stdin (octets UTF-8 bruts), cp1252 et PYTHONUTF8=0
    for label, content in (("vide", None), ("avec fichier", "x")):
        d = new_dir("u8-hook-é-" + label.replace(" ", "-"))
        if content is not None:
            (d / "note.txt").write_text(content, encoding="utf-8")
        inp = json.dumps({"source": "startup", "cwd": str(d)}, ensure_ascii=False).encode("utf-8")
        rc, out, err = run_cp1252(ACCOUNT / "session-start.py", inp=inp)
        try:
            payload = json.loads(out.decode("ascii"))
        except Exception:
            payload = None
        phrase = "Avant de commencer, souhaites-tu installer 2brain dans ce projet ?"
        if content is None:
            check("utf-8 sous cp1252: hook, dossier vide au nom accentué (%s) : JSON avec la question, code 0" % d.name,
                  "é" in d.name and rc == 0 and err == b"" and payload is not None
                  and phrase in payload["hookSpecificOutput"]["additionalContext"], (rc, out[:200], err))
        else:
            check("utf-8 sous cp1252: hook, dossier non vide au nom accentué (%s) : rien sur stdout ni stderr, code 0" % d.name,
                  "é" in d.name and rc == 0 and out == b"" and err == b"", (rc, out[:200], err))
        rmtree(d)


def pick(manifest_files, prefix_list):
    for p in prefix_list:
        if p in manifest_files:
            return p
    raise RuntimeError("fichier introuvable : %r" % prefix_list)


def test_update():
    upd = SCRIPTS / "update-brain-main.py"
    proj = project_with_files("upd")
    rc, prep = prepare(proj)
    rc, data = apply(proj, prep["plan_sha256"])
    bm = proj / "brain-main"
    man = json.loads((bm / "manifest.json").read_text(encoding="utf-8"))
    files = man["files"]
    names = [f for f in sorted(files) if f.endswith(".md") and not f.startswith(("bootstrap/", "adapters/"))]
    A, B, C, D, E, F, G = [pick(files, [n]) for n in names[:7]]
    # source 2 : copie modifiée
    s2root = new_dir("upd-src2")
    shutil.copytree(Env.source, s2root / "brain-main")
    s2 = s2root / "brain-main"
    (s2 / A).write_text((s2 / A).read_text(encoding="utf-8") + "\nmodif source A\n", encoding="utf-8")        # id / modifié
    (s2 / B).unlink()                                                                                          # id / supprimé
    (s2 / D).write_text("D version source\n", encoding="utf-8")                                                # mod / mod différemment
    (s2 / E).unlink()                                                                                          # mod / supprimé
    (s2 / G).write_text("G version source\n", encoding="utf-8")                                                # supprimé loc / modifié
    (s2 / "nouveau.md").write_text("nouveau\n", encoding="utf-8")                                              # absent / nouveau
    (s2 / "extra.md").write_text("extra source\n", encoding="utf-8")                                           # hors manifeste / autre contenu
    tpl = pick(files, ["bootstrap/templates/brain/docs/VISION.md"])
    (s2 / tpl).write_text((s2 / tpl).read_text(encoding="utf-8") + "\nmodèle changé\n", encoding="utf-8")
    (s2 / "bootstrap" / "VERSION").write_text("0.2.0\n", encoding="utf-8")
    # état local
    (bm / C).write_text("C modifié localement\n", encoding="utf-8")                                            # mod / identique
    (bm / D).write_text("D modifié localement\n", encoding="utf-8")
    (bm / E).write_text("E modifié localement\n", encoding="utf-8")
    (bm / F).unlink()                                                                                          # supprimé loc / identique
    (bm / G).unlink()
    (bm / "extra.md").write_text("extra local\n", encoding="utf-8")
    (bm / "perso.md").write_text("fichier local seulement\n", encoding="utf-8")

    brain_before = tree(proj / "brain")
    proj_before = tree(proj)
    rc, plan = runj(upd, "--source", s2, "--project", proj)
    paths = lambda k: sorted(e["path"] if isinstance(e, dict) else e for e in plan.get(k, []))
    check("mise à jour: plan sans écriture", rc == 0 and plan["ok"] and plan["ecrit"] is False and tree(proj) == proj_before, plan)
    check("mise à jour: identique/modifié -> mise à jour", A in paths("update") and tpl in paths("update") and "bootstrap/VERSION" in paths("update"), plan)
    check("mise à jour: identique/supprimé -> suppression proposée", paths("delete") == [B], plan)
    check("mise à jour: modifié/identique -> conservé", C in plan["kept"], plan)
    check("mise à jour: modifié/(modifié autrement|supprimé) -> conflits",
          {D, E} <= set(paths("conflicts")), plan["conflicts"])
    check("mise à jour: absent/nouveau -> ajout", "nouveau.md" in paths("add"), plan)
    check("mise à jour: hors manifeste avec autre contenu -> conflit", "extra.md" in paths("conflicts"), plan["conflicts"])
    check("mise à jour: supprimé localement/identique -> conservé supprimé, signalé", F in plan["kept_deleted"], plan)
    check("mise à jour: supprimé localement/modifié -> conflit", G in paths("conflicts"), plan["conflicts"])
    check("mise à jour: fichier local hors manifeste sans équivalent source -> signalé", "perso.md" in plan["local_only"], plan)
    check("mise à jour: adaptations_brain signalées (modèle changé)", any(a["path"] == tpl for a in plan["adaptations_brain"]), plan["adaptations_brain"])
    check("mise à jour: versions et plan_sha256", plan["versions"]["source"] == "0.2.0" and len(plan["plan_sha256"]) == 64, plan["versions"])

    rc, bad = runj(upd, "--source", s2, "--project", proj, "--apply", "--plan-sha256", "0" * 64)
    check("mise à jour: --apply avec empreinte erronée refusé (3), rien écrit", rc == 3 and tree(proj) == proj_before, bad)
    rc, bad = runj(upd, "--source", s2, "--project", proj, "--apply")
    check("mise à jour: --apply sans empreinte refusé", rc == 2 and tree(proj) == proj_before, bad)

    manifest_before = json.loads((bm / "manifest.json").read_text(encoding="utf-8"))
    rc, done = runj(upd, "--source", s2, "--project", proj, "--apply", "--plan-sha256", plan["plan_sha256"])
    check("mise à jour: --apply ok", rc == 0 and done.get("ok") and done["ecrit"], done)
    check("mise à jour: fichier mis à jour, ajout, suppression",
          (bm / A).read_bytes() == (s2 / A).read_bytes() and (bm / "nouveau.md").read_text(encoding="utf-8") == "nouveau\n" and not (bm / B).exists())
    check("mise à jour: modifications locales jamais écrasées (conservé, conflits, hors manifeste)",
          (bm / C).read_text(encoding="utf-8") == "C modifié localement\n" and (bm / D).read_text(encoding="utf-8") == "D modifié localement\n"
          and (bm / E).read_text(encoding="utf-8") == "E modifié localement\n" and (bm / "extra.md").read_text(encoding="utf-8") == "extra local\n"
          and (bm / "perso.md").is_file())
    check("mise à jour: supprimés localement restent supprimés (identique et conflit)", not (bm / F).exists() and not (bm / G).exists())
    man2 = json.loads((bm / "manifest.json").read_text(encoding="utf-8"))
    mf = man2["files"]
    check("mise à jour: manifeste mis à jour pour les seuls fichiers appliqués",
          mf[A] == sha(s2 / A) and mf["nouveau.md"] == sha(s2 / "nouveau.md") and B not in mf and man2["version"] == "0.2.0", man2["version"])
    check("mise à jour: fichiers en conflit gardent leur empreinte d'origine au manifeste",
          mf[D] == manifest_before["files"][D] and mf[E] == manifest_before["files"][E] and mf[G] == manifest_before["files"][G]
          and mf[C] == manifest_before["files"][C] and "extra.md" not in mf)
    check("mise à jour: brain/ strictement inchangé (empreintes)", tree(proj / "brain") == brain_before)
    check("mise à jour: rien écrit hors brain-main/", tree(proj, skip=("brain-main",)) == {k: v for k, v in proj_before.items() if not k.startswith("brain-main")})
    rc, plan2 = runj(upd, "--source", s2, "--project", proj)
    check("mise à jour: second plan sans mise à jour, mêmes conflits",
          rc == 0 and not plan2["update"] and not plan2["add"] and not plan2["delete"] and {D, E, G, "extra.md"} <= set(e["path"] for e in plan2["conflicts"]), plan2)

    # source modifiée entre le plan et l'application
    proj_b = project_with_files("upd-race")
    rc, prep = prepare(proj_b)
    apply(proj_b, prep["plan_sha256"])
    s3root = new_dir("upd-src3")
    shutil.copytree(Env.source, s3root / "brain-main")
    s3 = s3root / "brain-main"
    (s3 / "nouveau.md").write_text("v1\n", encoding="utf-8")
    rc, plan3 = runj(upd, "--source", s3, "--project", proj_b)
    (s3 / "nouveau.md").write_text("v2\n", encoding="utf-8")
    b = tree(proj_b)
    rc, out = runj(upd, "--source", s3, "--project", proj_b, "--apply", "--plan-sha256", plan3["plan_sha256"])
    check("mise à jour: plan recalculé différent -> refus (3), rien écrit", rc == 3 and tree(proj_b) == b, out)

    # refus
    srcrepo = fake_source_repo("upd-srcrepo")
    mk(srcrepo, {"brain-main/manifest.json": json.dumps({"version": "0", "files": {}})})
    b = tree(srcrepo, skip=(".git",))
    rc, out = runj(upd, "--source", s2, "--project", srcrepo)
    check("mise à jour: refus dans un faux dépôt source (même avec manifeste)", rc == 2 and tree(srcrepo, skip=(".git",)) == b, out)
    noman = mk(new_dir("upd-noman"), {"brain-main/README.md": "x"})
    b = tree(noman)
    rc, out = runj(upd, "--source", s2, "--project", noman)
    check("mise à jour: refus sans manifeste", rc == 2 and tree(noman) == b, out)
    nobm = mk(new_dir("upd-nobm"), {"a.txt": "x"})
    rc, out = runj(upd, "--source", s2, "--project", nobm)
    check("mise à jour: refus sans brain-main/", rc == 2, out)
    evil = project_with_files("upd-evil")
    rc, prep = prepare(evil)
    apply(evil, prep["plan_sha256"])
    mpath = evil / "brain-main" / "manifest.json"
    m = json.loads(mpath.read_text(encoding="utf-8"))
    m["files"]["../brain/docs/VISION.md"] = "0" * 64
    mpath.write_text(json.dumps(m), encoding="utf-8")
    b = tree(evil)
    rc, out = runj(upd, "--source", s3, "--project", evil)
    check("mise à jour: chemin sortant de brain-main/ dans le manifeste -> refus", rc == 2 and tree(evil) == b, out)


def settings_fixture(home, extra=None):
    data = {
        "permissions": {"allow": ["mcp__x"]},
        "env": {"A": "1"},
        "hooks": {
            "SessionStart": [{"hooks": [{"type": "command", "command": "echo", "args": ["autre"]}]}],
            "PostToolUse": [{"matcher": "Edit", "hooks": [{"type": "command", "command": "true"}]}],
        },
    }
    if extra:
        data.update(extra)
    p = home / ".claude" / "settings.json"
    p.parent.mkdir(parents=True, exist_ok=True)
    p.write_text(json.dumps(data, indent=4), encoding="utf-8")
    return data


def our_groups(settings):
    return [g for g in settings["hooks"]["SessionStart"]
            if any(str(a).replace("\\", "/").endswith("skills/brain/session-start.py") for h in g["hooks"] for a in h.get("args", []))]


def test_account():
    inst = SCRIPTS / "install-account.py"
    home = new_dir("home-acc")
    orig = settings_fixture(home)
    sp = home / ".claude" / "settings.json"
    orig_hash = sha(sp)
    rc, out = runj(inst, "--home", home, "--dry-run")
    check("compte: --dry-run n'écrit rien", rc == 0 and out["dry_run"] and out["actions"] and sha(sp) == orig_hash
          and not (home / ".claude/skills").exists() and not (home / ".claude/2brain-bootstrap").exists(), out)
    rc, out = runj(inst, "--home", home)
    s = json.loads(sp.read_text(encoding="utf-8"))
    check("compte: installation ok", rc == 0 and out["ok"], out)
    backups = list((home / ".claude/2brain-bootstrap/backups").glob("settings.*.json"))
    check("compte: sauvegarde de settings.json présente et identique à l'original",
          len(backups) == 1 and sha(backups[0]) == orig_hash, backups)
    check("compte: autres clés et autres hooks conservés",
          s["permissions"] == orig["permissions"] and s["env"] == orig["env"] and s["hooks"]["PostToolUse"] == orig["hooks"]["PostToolUse"]
          and s["hooks"]["SessionStart"][0] == orig["hooks"]["SessionStart"][0])
    groups = our_groups(s)
    check("compte: un groupe SessionStart, matcher startup, forme sans shell (command=python, args=[script])",
          len(groups) == 1 and groups[0]["matcher"] == "startup" and groups[0]["hooks"][0]["type"] == "command"
          and groups[0]["hooks"][0]["command"] == sys.executable and len(groups[0]["hooks"][0]["args"]) == 1
          and "shell" not in groups[0]["hooks"][0], groups)
    sd = home / ".claude/skills/brain"
    skill = (sd / "SKILL.md").read_text(encoding="utf-8")
    check("compte: __PYTHON__ remplacé par le chemin absolu de Python",
          "__PYTHON__" not in skill and Path(sys.executable).as_posix() in skill)
    check("compte: fichiers posés (SKILL.md, hook, status, fetch, VERSION)",
          all((sd / n).is_file() for n in ("SKILL.md", "session-start.py", "status.py", "fetch-source.py", "VERSION"))
          and (sd / "VERSION").read_text(encoding="utf-8").strip() == (BOOT / "VERSION").read_text(encoding="utf-8").strip())
    check("compte: le script de hook installé existe au chemin enregistré", Path(groups[0]["hooks"][0]["args"][0]).is_file())
    empty = new_dir("acc-empty")
    rc, o, e = run(Path(groups[0]["hooks"][0]["args"][0]), inp=json.dumps({"cwd": str(empty), "source": "startup"}))
    check("compte: le hook installé répond dans un dossier vide", rc == 0 and "installer 2brain" in o, o)
    # réinstallation
    rc, out = runj(inst, "--home", home)
    s2 = json.loads(sp.read_text(encoding="utf-8"))
    check("compte: réinstallation sans doublon", rc == 0 and len(our_groups(s2)) == 1 and len(s2["hooks"]["SessionStart"]) == 2, s2["hooks"])
    check("compte: deuxième sauvegarde distincte", len(list((home / ".claude/2brain-bootstrap/backups").glob("settings.*.json"))) == 2)

    # désinstallation
    rc, out = runj(inst, "--home", home, "--uninstall")
    s3 = json.loads(sp.read_text(encoding="utf-8"))
    check("compte: --uninstall retire son groupe et son dossier, garde le reste",
          rc == 0 and not sd.exists() and not our_groups(s3) and s3["hooks"]["SessionStart"] == orig["hooks"]["SessionStart"]
          and s3["hooks"]["PostToolUse"] == orig["hooks"]["PostToolUse"] and s3["permissions"] == orig["permissions"], out)
    check("compte: --uninstall garde les sauvegardes et affiche ce qui est retiré",
          len(list((home / ".claude/2brain-bootstrap/backups").glob("settings.*.json"))) >= 3 and any("suppression" in a for a in out["actions"]), out)
    rc, out = runj(inst, "--home", home, "--uninstall")
    check("compte: --uninstall répété sans effet", rc == 0 and "rien à retirer" in out["actions"], out)

    # skill étranger
    home2 = new_dir("home-foreign")
    settings_fixture(home2)
    mk(home2 / ".claude/skills/brain", {"SKILL.md": "étranger\n"})
    b = tree(home2)
    rc, out = runj(inst, "--home", home2)
    check("compte: refus d'écraser un skills/brain étranger, rien écrit", rc == 2 and tree(home2) == b, out)
    rc, out = runj(inst, "--home", home2, "--uninstall")
    check("compte: --uninstall ne supprime pas un skills/brain étranger", rc == 0 and (home2 / ".claude/skills/brain/SKILL.md").is_file(), out)

    # remplacement de la sonde
    home3 = new_dir("home-probe")
    probe_settings = {
        "model": "x",
        "hooks": {"SessionStart": [
            {"hooks": [{"type": "command", "command": sys.executable,
                        "args": [str(home3 / ".claude/2brain-sonde/session_start.py")]}]},
            {"hooks": [{"type": "command", "command": "echo", "args": ["garde"]}]},
        ]},
    }
    mk(home3 / ".claude", {"settings.json": json.dumps(probe_settings), "2brain-sonde/session_start.py": "pass\n",
                           "2brain-sonde/hook.log": "log\n", "2brain-sonde/settings.json.avant-sonde": "{\n    \"avant\": \"é\"\n}", "skills/brain/SKILL.md": "sonde\n", "skills/autre/SKILL.md": "autre\n"})
    rc, out = runj(inst, "--home", home3)
    check("sonde: sans --replace-probe, refus", rc == 2 and (home3 / ".claude/2brain-sonde").exists(), out)
    rc, out = runj(inst, "--home", home3, "--replace-probe", "--dry-run")
    check("sonde: --dry-run annonce la copie de la sauvegarde d'avant-sonde, sans écrire",
          rc == 0 and any("settings.avant-sonde.json" in a for a in out["actions"]) and not (home3 / ".claude/2brain-bootstrap").exists(), out)
    check("sonde: --replace-probe --dry-run n'écrit rien", rc == 0 and (home3 / ".claude/2brain-sonde").exists()
          and (home3 / ".claude/skills/brain/SKILL.md").read_text(encoding="utf-8") == "sonde\n", out)
    probe_bytes = (home3 / ".claude/2brain-sonde/settings.json.avant-sonde").read_bytes()
    rc, out = runj(inst, "--home", home3, "--replace-probe")
    s4 = json.loads((home3 / ".claude/settings.json").read_text(encoding="utf-8"))
    flat = json.dumps(s4)
    check("sonde: --replace-probe retire groupe de la sonde, skills/brain et 2brain-sonde, installe /brain",
          rc == 0 and "2brain-sonde" not in flat and not (home3 / ".claude/2brain-sonde").exists()
          and (home3 / ".claude/skills/brain/VERSION").is_file() and "sonde" not in (home3 / ".claude/skills/brain/SKILL.md").read_text(encoding="utf-8").lower().split("---")[1]
          and len(our_groups(s4)) == 1 and any("garde" in json.dumps(g) for g in s4["hooks"]["SessionStart"])
          and s4["model"] == "x" and (home3 / ".claude/skills/autre/SKILL.md").is_file(), out)

    kept = home3 / ".claude/2brain-bootstrap/backups/settings.avant-sonde.json"
    check("sonde: sauvegarde d'avant-sonde conservée à l'identique après --replace-probe",
          kept.is_file() and kept.read_bytes() == probe_bytes and "é".encode("utf-8") in probe_bytes
          and any("settings.avant-sonde.json" in a for a in out["actions"]), out)
    home3b = new_dir("home-probe-over")
    mk(home3b / ".claude", {"settings.json": "{}", "2brain-sonde/settings.json.avant-sonde": "nouveau", "skills/brain/SKILL.md": "sonde\n",
                            "2brain-bootstrap/backups/settings.avant-sonde.json": "ancien"})
    rc, out = runj(inst, "--home", home3b, "--replace-probe")
    bdir = home3b / ".claude/2brain-bootstrap/backups"
    check("sonde: copie d'avant-sonde déjà présente jamais écrasée (suffixe)",
          rc == 0 and (bdir / "settings.avant-sonde.json").read_text(encoding="utf-8") == "ancien"
          and (bdir / "settings.avant-sonde-1.json").read_text(encoding="utf-8") == "nouveau", out)

    # format de settings.json conservé
    home7 = new_dir("home-format")
    fixture = {"model": "é", "permissions": {"allow": ["a"]}, "hooks": {"PostToolUse": [{"hooks": [{"type": "command", "command": "true"}]}]}}
    raw7 = json.dumps(fixture, indent=4, ensure_ascii=False)
    (home7 / ".claude").mkdir()
    (home7 / ".claude/settings.json").write_bytes(raw7.encode("utf-8"))
    rc, out = runj(inst, "--home", home7)
    new7 = (home7 / ".claude/settings.json").read_bytes().decode("utf-8")
    d7 = json.loads(new7)
    ours7 = d7.pop("hooks")
    orig7 = dict(fixture)
    orig7.pop("hooks")
    check("format: 4 espaces, LF, sans retour final conservés (é non échappé)",
          rc == 0 and new7.split("\n")[1].startswith('    "') and "\r" not in new7
          and not new7.endswith("\n") and "é" in new7 and "\\u00e9" not in new7, new7[:120])
    check("format: seul le bloc hooks diffère, ordre des clés conservé",
          d7 == orig7 and list(json.loads(new7)) == ["model", "permissions", "hooks"]
          and ours7["PostToolUse"] == fixture["hooks"]["PostToolUse"] and len(ours7["SessionStart"]) == 1, out)
    home8 = new_dir("home-format-crlf")
    (home8 / ".claude").mkdir()
    (home8 / ".claude/settings.json").write_bytes((json.dumps({"a": 1}, indent="\t").replace("\n", "\r\n") + "\r\n").encode("utf-8"))
    rc, out = runj(inst, "--home", home8)
    new8 = (home8 / ".claude/settings.json").read_bytes().decode("utf-8")
    check("format: tabulations, CRLF et retour final conservés",
          rc == 0 and new8.endswith("\r\n") and '\r\n\t"a"' in new8 and "\n" not in new8.replace("\r\n", ""), repr(new8[:60]))

    # JSON invalide
    home4 = new_dir("home-badjson")
    mk(home4 / ".claude", {"settings.json": "{ pas du json"})
    b = tree(home4)
    rc, out = runj(inst, "--home", home4)
    check("compte: settings.json invalide -> refus, rien écrit", rc == 2 and tree(home4) == b, out)
    rc, out = runj(inst, "--home", home4, "--uninstall")
    check("compte: settings.json invalide -> --uninstall refuse aussi", rc == 2 and tree(home4) == b, out)
    home5 = new_dir("home-badshape")
    mk(home5 / ".claude", {"settings.json": json.dumps({"hooks": {"SessionStart": "oops"}})})
    b = tree(home5)
    rc, out = runj(inst, "--home", home5)
    check("compte: hooks.SessionStart mal formé -> refus, rien écrit", rc == 2 and tree(home5) == b, out)

    # sans settings.json
    home6 = new_dir("home-nosettings")
    rc, out = runj(inst, "--home", home6)
    s6 = json.loads((home6 / ".claude/settings.json").read_text(encoding="utf-8"))
    check("compte: sans settings.json, créé avec le seul hook", rc == 0 and len(our_groups(s6)) == 1 and set(s6) == {"hooks"}, s6)


ADAPTER = REPO / "brain-main" / "adapters" / "claude-code"
AGENTS3 = ("orchestrator", "worker-sonnet", "worker-haiku")
CLAUDE5 = sorted([".claude/agents/%s.md" % n for n in AGENTS3] + [".claude/settings.json", ".claude/README.md"])
ORCH_TOOLS = "Read, Grep, Glob, Agent(worker-sonnet, worker-haiku), SendMessage, TaskStop, AskUserQuestion"
ACTIF = "actif à la prochaine conversation"
# Écarts documentés entre .claude/agents/ du dépôt 2brain et l'adaptateur (texte propre au dépôt 2brain retiré) :
# seul l'orchestrateur diffère (description, date, première phrase) ; les deux workers sont identiques.
ADAPTER_GAPS = {
    "orchestrator": [
        ("description: \"Chef d'orchestre unique de 2brain : dialogue", "description: \"Chef d'orchestre unique du projet : dialogue"),
        ("updated: \"2026-11-05\"", "updated: \"2026-11-06\""),
        ("Tu es l'orchestrateur de 2brain, chef d'orchestre unique.", "Tu es l'orchestrateur de ce projet, chef d'orchestre unique."),
    ],
    "worker-sonnet": [],
    "worker-haiku": [],
}


def lf(path):
    return Path(path).read_bytes().decode("utf-8").replace("\r\n", "\n")


def front(path):
    """Champs du front matter (clé: valeur sur une ligne)."""
    head = lf(path).split("\n---\n")[0].split("\n")[1:]
    return {l.split(":", 1)[0]: l.split(":", 1)[1].strip() for l in head if ":" in l and not l.startswith(" ")}


def claude_files(proj):
    root = Path(proj) / ".claude"
    return sorted(p.relative_to(proj).as_posix() for p in root.rglob("*") if p.is_file()) if root.is_dir() else []


def test_adapter():
    check("adaptateur: source présente (3 agents, settings.json, README.md)",
          all((ADAPTER / "agents" / (n + ".md")).is_file() for n in AGENTS3) and (ADAPTER / "settings.json").is_file() and (ADAPTER / "README.md").is_file())
    check("adaptateur: settings.json = agent orchestrator seulement",
          json.loads(lf(ADAPTER / "settings.json")) == {"agent": "orchestrator"})
    for n in AGENTS3:
        expected = lf(REPO / ".claude" / "agents" / (n + ".md"))
        ok = True
        for old, new in ADAPTER_GAPS[n]:
            ok = ok and expected.count(old) == 1
            expected = expected.replace(old, new)
        check("adaptateur: %s identique à .claude/agents/ du dépôt, hors écarts documentés (%d)" % (n, len(ADAPTER_GAPS[n])),
              ok and lf(ADAPTER / "agents" / (n + ".md")) == expected, n)
    check("adaptateur: aucun « 2brain » restant dans l'orchestrateur", "2brain" not in lf(ADAPTER / "agents" / "orchestrator.md"))
    readme = lf(ADAPTER / "README.md")
    check("adaptateur: README couvre correspondance, contrat, limites non garanties et retrait",
          all(w in readme for w in ("`opus`", "`sonnet`", "`haiku`", "§9", "Bash", "ne peut pas interroger", "aller-retour", "Desktop", "Retrait", "agent"))
          and "non vérifié" in readme.lower() and front(ADAPTER / "README.md").get("title"))

    # installation dans un projet vide
    proj = new_dir("adp-empty")
    rc, prep = prepare(proj)
    plan = json.loads((proj / "EMPLACEMENT/plan.json").read_text(encoding="utf-8")) if (proj / "EMPLACEMENT/plan.json").is_file() else {"fichiers": {}}
    check("adaptateur: prévisualisation (projet vide) liste les 5 fichiers .claude/ et le routeur actif",
          rc == 0 and prep["claude_fichiers"] == CLAUDE5 and prep["routeur"]["etat"] == ACTIF
          and prep["conflits"] == [] and prep["adaptations"] == [] and all(f in plan["fichiers"] for f in CLAUDE5), prep)
    check("adaptateur: la prévisualisation n'écrit pas dans .claude/ du projet", not (proj / ".claude").exists())
    rc, data = apply(proj, prep["plan_sha256"])
    b = data.get("bilan", {})
    check("adaptateur: installation (projet vide): 5 fichiers posés, bilan routeur actif et nouvelle conversation",
          rc == 0 and claude_files(proj) == CLAUDE5 and b.get("claude") == CLAUDE5 and b["routeur"]["etat"] == ACTIF
          and "nouvelle conversation" in b["message"], data)
    check("adaptateur: settings.json installé contient agent: orchestrator",
          json.loads((proj / ".claude/settings.json").read_text(encoding="utf-8")) == {"agent": "orchestrator"})
    fm = {n: front(proj / ".claude/agents" / (n + ".md")) for n in AGENTS3}
    check("adaptateur: agents installés avec les bons model, tools, omitClaudeMd",
          fm["orchestrator"]["model"] == "opus" and fm["orchestrator"]["tools"] == ORCH_TOOLS and "omitClaudeMd" not in fm["orchestrator"]
          and fm["worker-sonnet"]["model"] == "sonnet" and fm["worker-sonnet"]["tools"] == "Read, Grep, Glob, Edit, Write, Bash"
          and fm["worker-haiku"]["model"] == "haiku" and fm["worker-haiku"]["tools"] == "Read, Grep, Glob, Edit, Bash"
          and fm["worker-sonnet"]["omitClaudeMd"] == "true" and fm["worker-haiku"]["omitClaudeMd"] == "true"
          and all(fm[n]["name"] == n for n in AGENTS3), fm)
    check("adaptateur: l'orchestrateur n'a ni Bash, ni Write, ni Edit",
          not any(w in fm["orchestrator"]["tools"] for w in ("Bash", "Write", "Edit")))
    files = sorted(p.relative_to(proj / "brain").as_posix() for p in (proj / "brain").rglob("*") if p.is_file())
    trace = re.compile(r"claude|router|routeur|orchestrat|worker|adapter|adaptateur|2brain|brain-main|bootstrap", re.I)
    hits = []
    for p in (proj / "brain").rglob("*"):
        rel = p.relative_to(proj / "brain").as_posix()
        if trace.search(rel):
            hits.append((rel, "chemin"))
        if p.is_file():
            m = trace.search(p.read_text(encoding="utf-8"))
            if m:
                hits.append((rel, m.group(0)))
    check("adaptateur: brain/ limité aux 6 documents, au registre et à settings.json, sans trace du système ni du routeur",
          files == sorted(["docs/" + n for n in DOCS5] + ["registry/entries.jsonl", "settings.json"]) and not hits, (files, hits[:3]))
    check("adaptateur: aucun __pycache__ ni EMPLACEMENT restant", not list(proj.rglob("__pycache__")) and not (proj / "EMPLACEMENT").exists())
    check("adaptateur: la source est aussi copiée dans brain-main/ (manifeste compris)",
          (proj / "brain-main/adapters/claude-code/settings.json").is_file()
          and "adapters/claude-code/agents/orchestrator.md" in json.loads((proj / "brain-main/manifest.json").read_text(encoding="utf-8"))["files"])

    # status.py : actif, absent, partiel
    home = new_dir("home-adp")
    rc, st = status(proj, home)
    check("adaptateur: status.py -> routeur configuré (et non actif), avec la raison", st["routeur"] == "configuré"
          and "s'applique aux conversations ouvertes après l'installation" in st["routeur_raison"] and "session courante" in st["routeur_raison"], st)
    rc, out, _ = run(ACCOUNT / "status.py", "--project", proj, "--home", home, "--offline")
    check("adaptateur: status.py (texte) mentionne le routeur", "Routeur" in out and "configuré" in out and "actif" not in out, out)
    variants = []
    p1 = new_dir("adp-part1")
    shutil.copytree(proj / ".claude", p1 / ".claude")
    (p1 / ".claude/settings.json").write_text("{}", encoding="utf-8")
    p2 = new_dir("adp-part2")
    shutil.copytree(proj / ".claude", p2 / ".claude")
    (p2 / ".claude/agents/worker-haiku.md").unlink()
    p3 = new_dir("adp-part3")
    shutil.copytree(proj / ".claude", p3 / ".claude")
    (p3 / ".claude/settings.json").write_text(json.dumps({"agent": "autre"}), encoding="utf-8")
    p4 = new_dir("adp-part4")
    shutil.copytree(proj / ".claude", p4 / ".claude")
    (p4 / ".claude/settings.json").write_text("{ illisible", encoding="utf-8")
    p5 = new_dir("adp-solo")
    (p5 / ".claude").mkdir()
    (p5 / ".claude/settings.json").write_text('{"agent": "orchestrator"}', encoding="utf-8")
    for q in (p1, p2, p3, p4, p5):
        variants.append(status(q, home)[1]["routeur"])
    check("adaptateur: status.py -> désactivé (agents présents, clé absente) puis partiel (agent manquant, autre valeur, JSON illisible, clé sans agents)",
          variants == ["désactivé"] + ["partiel"] * 4, variants)
    rc, st = status(p1, home)
    check("adaptateur: status.py désactivé -> raison qui suggère /brain -d on", "`/brain -d on`" in st["routeur_raison"], st)
    rc, st = status(p3, home)
    check("adaptateur: status.py partiel (autre valeur de agent) -> raison qui la signale, et note", "autre valeur" in st["routeur_raison"] and "\"autre\"" in st["routeur_raison"]
          and any("Routeur partiel" in n for n in st["notes"]), st)
    pa = mk(new_dir("adp-abs"), {"README.md": "x"})
    check("adaptateur: status.py -> routeur absent", status(pa, home)[1]["routeur"] == "absent")

    # .claude/settings.json existant : jamais modifié, clé signalée
    for label, content, etat, ad_word in (
        ("sans clé agent", '{\r\n\t"model": "x",\r\n\t"permissions": {"allow": ["a"]}\r\n}\r\n', "non activé", '"agent": "orchestrator"'),
        ("agent autre valeur", '{"agent": "autre"}', "non activé", "Conflit"),
        ("agent déjà orchestrator", '{"agent": "orchestrator", "x": 1}', ACTIF, None),
        ("JSON illisible", "{ pas du json", "non activé", '"agent": "orchestrator"'),
    ):
        ps = mk(new_dir("adp-settings"), {"README.md": "x"})
        (ps / ".claude").mkdir()
        raw = content.encode("utf-8")
        (ps / ".claude/settings.json").write_bytes(raw)
        rc, prep = prepare(ps)
        pre_ok = rc == 0 and prep["routeur"]["etat"] == etat and ".claude/settings.json" not in prep["claude_fichiers"]
        joined = "".join(prep["adaptations"])
        if ad_word:
            pre_ok = pre_ok and ad_word in joined
        else:
            pre_ok = pre_ok and prep["adaptations"] == []
        if label == "agent autre valeur":
            pre_ok = pre_ok and ".claude/settings.json" in prep["conflits"]
        else:
            pre_ok = pre_ok and prep["conflits"] == []
        if label == "sans clé agent":
            pre_ok = pre_ok and "pas actif tant" in joined and "n'est pas modifié" in joined
        check("settings.json existant (%s): prévisualisation, routeur %s, clé signalée" % (label, etat), pre_ok, prep)
        rc, data = apply(ps, prep["plan_sha256"])
        check("settings.json existant (%s): installation, octets inchangés, autres fichiers .claude/ posés, bilan routeur %s" % (label, etat),
              rc == 0 and (ps / ".claude/settings.json").read_bytes() == raw and data["bilan"]["routeur"]["etat"] == etat
              and claude_files(ps) == CLAUDE5 and data["bilan"]["claude"] == [f for f in CLAUDE5 if f != ".claude/settings.json"], data)
        if etat == "non activé":
            check("settings.json existant (%s): le bilan donne la raison" % label,
                  ".claude/settings.json" in data["bilan"]["routeur"]["raison"])

    # agent existant différent : conflit, sans écrasement
    pc = mk(new_dir("adp-conflict"), {"README.md": "x"})
    (pc / ".claude/agents").mkdir(parents=True)
    mine = "---\nname: orchestrator\n---\nà moi\n".encode("utf-8")
    (pc / ".claude/agents/orchestrator.md").write_bytes(mine)
    rc, prep = prepare(pc)
    check("agent existant différent: conflit signalé, routeur non activé",
          rc == 0 and ".claude/agents/orchestrator.md" in prep["conflits"] and prep["routeur"]["etat"] == "non activé", prep)
    before = tree(pc, skip=("EMPLACEMENT",))
    rc, data = apply(pc, prep["plan_sha256"])
    check("agent existant différent: application refusée (4), rien écrasé ni posé",
          rc == 4 and (pc / ".claude/agents/orchestrator.md").read_bytes() == mine and tree(pc, skip=("EMPLACEMENT",)) == before
          and not (pc / "brain-main").exists() and ".claude/agents/orchestrator.md" in data["conflits"], data)
    pr = mk(new_dir("adp-readme"), {".claude/README.md": "perso\n"})
    rc, prep = prepare(pr)
    check("README.md de .claude/ existant différent: conflit", rc == 0 and ".claude/README.md" in prep["conflits"], prep)
    # agent identique : pas de conflit
    pi = new_dir("adp-same")
    (pi / ".claude/agents").mkdir(parents=True)
    shutil.copyfile(Env.source / "adapters/claude-code/agents/worker-haiku.md", pi / ".claude/agents/worker-haiku.md")
    raw_h = (pi / ".claude/agents/worker-haiku.md").read_bytes()
    rc, prep = prepare(pi)
    check("agent existant identique: pas de conflit, signalé déjà présent, non recopié",
          rc == 0 and prep["conflits"] == [] and prep["claude_deja_presents"] == [".claude/agents/worker-haiku.md"]
          and ".claude/agents/worker-haiku.md" not in prep["claude_fichiers"] and len(prep["claude_fichiers"]) == 4, prep)
    rc, data = apply(pi, prep["plan_sha256"])
    check("agent existant identique: installation ok, fichier inchangé, routeur actif",
          rc == 0 and (pi / ".claude/agents/worker-haiku.md").read_bytes() == raw_h and claude_files(pi) == CLAUDE5
          and data["bilan"]["routeur"]["etat"] == ACTIF, data)
    pf = mk(new_dir("adp-file"), {".claude": "fichier"})
    b0 = tree(pf)
    rc, prep = prepare(pf)
    check("`.claude` fichier: prévisualisation refusée, rien touché", rc == 2 and tree(pf) == b0, prep)
    pe = mk(new_dir("adp-evil"), {"README.md": "x"})
    rc, prep = prepare(pe)
    (pe / "EMPLACEMENT/.claude/hooks").mkdir(parents=True)
    (pe / "EMPLACEMENT/.claude/hooks/x.sh").write_text("x", encoding="utf-8")
    plan_path = pe / "EMPLACEMENT/plan.json"
    plan = json.loads(plan_path.read_text(encoding="utf-8"))
    plan["fichiers"][".claude/hooks/x.sh"] = sha(pe / "EMPLACEMENT/.claude/hooks/x.sh")
    plan_path.write_text(json.dumps(plan), encoding="utf-8")
    rc, data = apply(pe, sha(plan_path))
    check("plan avec un chemin .claude/ hors adaptateur: refus (3), rien posé", rc == 3 and not (pe / "brain-main").exists() and not (pe / ".claude").exists(), data)

    # -u : n'écrit pas dans .claude/, signale le changement d'adaptateur
    upd = SCRIPTS / "update-brain-main.py"
    pu = project_with_files("adp-upd")
    rc, prep = prepare(pu)
    apply(pu, prep["plan_sha256"])
    s2root = new_dir("adp-src2")
    shutil.copytree(Env.source, s2root / "brain-main")
    s2 = s2root / "brain-main"
    (s2 / "adapters/claude-code/agents/worker-haiku.md").write_text("changé\n", encoding="utf-8")
    (s2 / "bootstrap/VERSION").write_text("0.9.0\n", encoding="utf-8")
    claude_before, all_before = tree(pu / ".claude"), tree(pu)
    rc, plan = runj(upd, "--source", s2, "--project", pu)
    ac = plan.get("adaptations_claude", [])
    check("-u: le plan signale le changement d'adaptateur (.claude/ n'est pas modifié), sans écrire",
          rc == 0 and [a["path"] for a in ac] == ["adapters/claude-code/agents/worker-haiku.md"] and ".claude/ n'est pas modifié" in ac[0]["note"]
          and tree(pu) == all_before, plan)
    rc, done = runj(upd, "--source", s2, "--project", pu, "--apply", "--plan-sha256", plan["plan_sha256"])
    check("-u: --apply n'écrit que dans brain-main/, .claude/ strictement inchangé, adaptation rappelée",
          rc == 0 and tree(pu / ".claude") == claude_before
          and tree(pu, skip=("brain-main",)) == {k: v for k, v in all_before.items() if not k.startswith("brain-main")}
          and (pu / "brain-main/adapters/claude-code/agents/worker-haiku.md").read_text(encoding="utf-8") == "changé\n"
          and [a["path"] for a in done["adaptations_claude"]] == ["adapters/claude-code/agents/worker-haiku.md"], done)
    rc, plan0 = runj(upd, "--source", Env.source, "--project", pu)
    check("-u: source revenue à l'identique de l'installation -> adaptateur remis, seulement l'adaptateur signalé",
          all(a["path"].startswith("adapters/claude-code/") for a in plan0["adaptations_claude"]), plan0["adaptations_claude"])
    rc, plan1 = runj(upd, "--source", Env.source, "--project", proj)
    check("-u: aucun changement d'adaptateur -> aucune adaptation signalée", rc == 0 and plan1["adaptations_claude"] == [], plan1)

    # documents modifiés par cette décision
    skill = lf(ACCOUNT / "SKILL.md")
    check("SKILL.md: sans Bash général, brief complet (session substituée, contenu mot pour mot, append only), -c suit le skill de capture, au plus 70 lignes",
          "Sans Bash" in skill and "orchestrateur du dépôt 2brain" not in skill and "${CLAUDE_SESSION_ID}` déjà substitué" in skill
          and "mot pour mot" in skill and "append only" in skill and "validation" in skill
          and "demande de suivre `brain-main/skills/brain-capture/SKILL.md`" in skill and len(skill.splitlines()) <= 70, len(skill.splitlines()))
    check("SKILL.md: le brief autorise expressément les écritures de -c, -c <fichier>, -c 1 et -i, rappelle l'append only, interdit commit et push",
          "autorise expressément" in skill and "`-c` et `-c <fichier>` : `brain/registry/` par `append-captures.py`" in skill
          and "le staging prévu par le skill de capture" in skill and "`brain/sources/sessions/`" in skill
          and "`-c 1` : le rollback par `rollback-captures.py`" in skill and "`-i` : l'écriture de `.claude/` par `apply-install.py`" in skill
          and "append only" in skill and "tout commit et tout push" in skill and len(skill.splitlines()) <= 70, len(skill.splitlines()))
    check("SKILL.md: bilan d'installation avec routeur et nouvelle conversation, -u signale l'adaptateur",
          "`routeur`" in skill and "nouvelle conversation" in skill and "adaptations_claude" in skill)
    onb = lf(REPO / "brain-main/skills/brain-onboarding/SKILL.md")
    check("onboarding: formulation générale (plus « orchestrateur du dépôt 2brain »)", "orchestrateur du dépôt 2brain" not in onb and "session orchestrateur" in onb)
    boot = lf(BOOT / "README.md")
    check("bootstrap/README: plus d'orchestrateur propre au dépôt 2brain, section adaptateur, provenance et limites écrites",
          "Orchestrateur du dépôt 2brain" not in boot and "### 9.1. Adaptateur Claude Code" in boot
          and "Décision de l'humain du 2026-11-06 : « Je veux que le router soit automatiquement appliqué dès l'installation »" in boot
          and "aucune économie de coût n'a été mesurée" in boot.lower() and "empêche l'activation automatique" in boot and "à confirmer en session" in boot)
    guide = lf(REPO / "brain-main/README.md")
    check("guide: routeur actif par défaut, renvois à ROUTER.md et .claude/README.md, retrait, au plus 80 lignes",
          "Routeur actif par défaut" in guide and "brain-main/ROUTER.md" in guide and ".claude/README.md" in guide and "clé `agent`" in guide
          and len(guide.splitlines()) <= 80, len(guide.splitlines()))
    tpl = lf(BOOT / "templates/AGENTS.md")
    check("modèle AGENTS.md: ligne orchestrateur/ROUTER, outil sans adaptateur, pas d'import de ROUTER.md",
          "orchestrateur et applique `brain-main/ROUTER.md`" in tpl and "sans adaptateur" in tpl and "ROUTER §9" in tpl
          and not any("ROUTER" in x for x in import_lines(tpl)), import_lines(tpl))
    check("version: 0.7.0 dans VERSION", (BOOT / "VERSION").read_text(encoding="utf-8").strip() == "0.7.0")
    hacct = new_dir("home-adp-version")
    rc, out = runj(SCRIPTS / "install-account.py", "--home", hacct)
    check("compte: install-account --home temporaire copie la version 0.7.0",
          rc == 0 and (hacct / ".claude/skills/brain/VERSION").read_bytes() == b"0.7.0\n", out)


def test_skill():
    text = (ACCOUNT / "SKILL.md").read_text(encoding="utf-8")
    head = text.split("---")[1]
    check("SKILL.md: front matter (name, argument-hint, disable-model-invocation: true)",
          "name: brain" in head and "argument-hint: [-i | -u | -a | -o | -c | -c fichier | -c 1 | -s sujet | -r | -d on|off | -m [nombre]]" in head and "disable-model-invocation: true" in head, head)
    check("SKILL.md: pas de syntaxe !", "!`" not in text and "```!" not in text)
    check("SKILL.md: aiguille -c -s -r vers les scripts et vers brain-capture (plus de renvoi au lot 3)", "lot 3" not in text and "read-session.py" in text and "brain-main/skills/brain-capture/SKILL.md" in text)
    check("SKILL.md: règles de validation, délégation worker-sonnet, jamais de simulation",
          "worker-sonnet" in text and "Jamais de simulation" in text and "validation" in text)
    forbidden = ("UPPER_SNAKE_CASE", "kebab-case", "metadata_version", "SemVer", "snake_case", "TAXONOMY")
    check("SKILL.md: aucune copie de convention", not any(w in text for w in forbidden))
    check("SKILL.md: léger (au plus 70 lignes)", len(text.splitlines()) <= 70, len(text.splitlines()))
    check("SKILL.md: YAHOU ! après -u seulement si --apply code 0, sans conflit_non_ecrits, avec updated, added ou deleted non vide ; au plus 70 lignes",
          "YAHOU !" in text and "code 0" in text and "conflits_non_ecrits" in text and "`updated`, `added` ou `deleted`" in text
          and "jamais s'il n'y a rien" in text and len(text.splitlines()) <= 70, len(text.splitlines()))
    check("SKILL.md: -o aiguille vers le skill d'onboarding, fin de -i propose -o et une nouvelle conversation",
          "brain-main/skills/brain-onboarding/SKILL.md" in text and "/brain -o" in text and "nouvelle conversation" in text)
    check("SKILL.md: /compact seulement pour l'interdire", all(re.search(r"jamais|Jamais|interdit", ln) for ln in text.splitlines() if "/compact" in ln))
    check("SKILL.md: __PYTHON__ présent avant installation", "__PYTHON__" in text)
    check("VERSION: 0.7.0", (BOOT / "VERSION").read_text(encoding="utf-8").strip() == "0.7.0")
    check("SKILL.md: Python ponctuel de lecture avec -X utf8, routeur `configuré` (pas actif) à l'état",
          "`-X utf8`" in text and "`configuré`" in text)
    # chaque script est autonome : aucun import local
    local_names = {p.stem.replace("-", "_") for p in list(SCRIPTS.glob("*.py")) + list(ACCOUNT.glob("*.py"))}
    bad = []
    for p in list(SCRIPTS.glob("*.py")) + list(ACCOUNT.glob("*.py")):
        for line in p.read_text(encoding="utf-8").splitlines():
            if line.startswith(("import ", "from ")):
                mod = line.split()[1].split(".")[0]
                if mod in local_names:
                    bad.append((p.name, line))
    check("scripts: aucun import local (autonomes)", not bad, bad)


def test_router_guard():
    text = (ACCOUNT / "SKILL.md").read_text(encoding="utf-8")
    step8 = text.split("8. Fin :")[1].split("## Parcours `-u`")[0]
    check("fin de -i: routeur installé mais pas appliqué à cette conversation, nouvelle conversation, puis /brain -o",
          "ne s'applique pas à cette conversation" in step8 and "nouvelle conversation" in step8 and "`/brain -o`" in step8 and "`actif à la prochaine conversation`" in step8, step8)
    check("fin de -i: question explicite (ouvre la nouvelle conversation ou continue ici sans routeur), aucune suite d'office",
          "question explicite" in step8 and "continuer ici sans routeur" in step8 and "Ne lancer aucune suite de soi-même" in step8, step8)
    check("fin de -i: `non activé` donne la raison et les lignes à ajouter, une nouvelle conversation seule ne suffit pas",
          "`non activé`" in step8 and "la raison et les lignes à ajouter" in step8 and "ne suffit pas" in step8, step8)
    guard = text.split("## Garde-fou : session non routée")[1].split("## Parcours `-i`")[0]
    check("garde-fou: Bash, Write ou Edit + `routeur: configuré` => non routée, présenté comme déduction, orchestrateur inchangé",
          "Bash, Write ou Edit" in guard and "`routeur: configuré`" in guard and "déduction" in guard and "inchangée" in guard and "dépôt source" in guard, guard)
    check("garde-fou: /brain signale que la session n'est pas routée et qu'il faut une nouvelle conversation",
          "**`/brain`**" in guard and "n'est probablement pas routée" in guard and "nouvelle conversation" in guard, guard)
    check("garde-fou: -o, -c, -c <fichier>, -c 1, -u demandent une confirmation explicite avant d'écrire, sinon arrêt",
          all(("`%s`" % c) in guard.split("**`-s`")[0] for c in ("-o", "-c", "-c <fichier>", "-c 1", "-u"))
          and "confirmation explicite" in guard and "s'arrêter" in guard, guard)
    check("garde-fou: -s et -r signalent en une ligne et continuent",
          "**`-s`, `-r`**" in guard and "en une ligne et continuer" in guard, guard)
    lines = len(text.splitlines())
    check("SKILL.md du compte: au plus 70 lignes (garde-fou inclus)", lines <= 70, lines)
    tpl = (BOOT / "templates/AGENTS.md").read_text(encoding="utf-8")
    needs = ("Bash, Write ou Edit", '"agent": "orchestrator"', "probablement pas routé", "déduction", "une fois par session", "nouvelle conversation", "Aucune autre action")
    check("modèle AGENTS.md: consigne de session non routée", all(w in tpl for w in needs), tpl)
    proj = installed_project("guard-proj", ANSWERS_FULL)
    rendered = (proj / "AGENTS.md").read_text(encoding="utf-8")
    check("AGENTS.md rendu dans un projet installé: consigne présente, aucun placeholder", all(w in rendered for w in needs) and "{{" not in rendered, rendered)
    files = sorted(p.relative_to(proj / "brain").as_posix() for p in (proj / "brain").rglob("*") if p.is_file())
    check("brain/ généré: aucun nouveau fichier (6 documents, registre et settings.json)", files == sorted(["docs/" + n for n in DOCS5] + ["registry/entries.jsonl", "settings.json"]), files)
    guide = (REPO / "brain-main/README.md").read_text(encoding="utf-8")
    check("guide: au plus 80 lignes, routeur jamais appliqué à la conversation d'installation",
          len(guide.splitlines()) <= 80 and "jamais à la conversation d'installation" in guide, len(guide.splitlines()))
    adr = (ADAPTER / "README.md").read_text(encoding="utf-8")
    check("adaptateur README: nouvelle conversation seulement, session non routée = déduction sans garantie",
          "nouvelle conversation seulement" in adr and "est une déduction d'après les outils, sans garantie" in adr)


def test_set_router():
    sr = SCRIPTS / "set-router.py"

    def proj_with(name, settings_bytes, with_agents=True):
        d = new_dir(name)
        (d / ".claude/agents").mkdir(parents=True)
        if with_agents:
            for n in AGENTS3:
                (d / ".claude/agents" / (n + ".md")).write_text("x\n", encoding="utf-8")
        if settings_bytes is not None:
            (d / ".claude/settings.json").write_bytes(settings_bytes)
        return d

    def raw(d):
        f = d / ".claude/settings.json"
        return f.read_bytes() if f.is_file() else None

    def go(d, state, *extra):
        return runj(sr, "--project", d, "--state", state, *extra)

    # on : crée si absent
    d = proj_with("sr-create", None)
    rc, out = go(d, "on")
    check("set-router on: crée settings.json (2 espaces, LF, retour final) avec la seule clé agent",
          rc == 0 and out.get("ecrit") is True and raw(d) == b'{\n  "agent": "orchestrator"\n}\n', (rc, out, raw(d)))
    check("set-router: le JSON dit que l'effet est pour la conversation suivante, jamais la conversation en cours",
          "conversation suivante" in out.get("conversation_suivante", "") and "jamais à la conversation en cours" in out.get("conversation_suivante", ""), out)
    # on : ajoute, conserve autres clés et format (tabulations, CRLF, sans retour final)
    orig = b'{\r\n\t"model": "x",\r\n\t"permissions": {"allow": ["a"]}\r\n}'
    d = proj_with("sr-add", orig)
    rc, out = go(d, "on")
    check("set-router on: ajoute la clé, garde autres clés, ordre, tabulations, CRLF, absence de retour final",
          rc == 0 and raw(d) == b'{\r\n\t"model": "x",\r\n\t"permissions": {\r\n\t\t"allow": [\r\n\t\t\t"a"\r\n\t\t]\r\n\t},\r\n\t"agent": "orchestrator"\r\n}', (rc, raw(d)))
    before = raw(d)
    rc, out = go(d, "on")
    check("set-router on: déjà actif, rien à écrire, dit", rc == 0 and out.get("ecrit") is False and out.get("action") == "aucune" and "déjà activé" in out.get("message", "") and raw(d) == before, out)
    rc, out = go(d, "off")
    check("set-router off: retire seulement la clé agent, format conservé (tabulations, CRLF, sans retour final)",
          rc == 0 and out.get("ecrit") is True and json.loads(raw(d)) == {"model": "x", "permissions": {"allow": ["a"]}}
          and b"\r\n\t" in raw(d) and not raw(d).endswith(b"\n"), (rc, raw(d)))
    before = raw(d)
    rc, out = go(d, "off")
    check("set-router off: déjà inactif, rien à écrire, dit", rc == 0 and out.get("ecrit") is False and "déjà désactivé" in out.get("message", "") and raw(d) == before, out)
    # on / off / on
    d = proj_with("sr-cycle", b'{\n    "a": 1,\n    "b": [2]\n}\n')
    base = raw(d)
    r1 = go(d, "on")[0]
    s1 = raw(d)
    r2 = go(d, "off")[0]
    s2 = raw(d)
    r3 = go(d, "on")[0]
    check("set-router: on puis off puis on (indentation 4 conservée, off rend la structure d'origine, on rétablit l'état de on)",
          (r1, r2, r3) == (0, 0, 0) and json.loads(s2) == json.loads(base) and raw(d) == s1 and b'    "agent": "orchestrator"' in s1, (s1, s2))
    d = proj_with("sr-empty", b'{"agent": "orchestrator"}')
    rc, out = go(d, "off")
    check("set-router off: objet vide mais fichier conservé", rc == 0 and (d / ".claude/settings.json").is_file() and json.loads(raw(d)) == {}, raw(d))
    # refus : octets inchangés
    d = proj_with("sr-noagents", b'{"x": 1}', with_agents=False)
    before = raw(d)
    rc, out = go(d, "on")
    check("set-router on: agents absents, refus sans écrire, message d'adaptateur et d'installation",
          rc == 2 and out.get("ok") is False and raw(d) == before and "brain-main/adapters/claude-code/" in out.get("erreur", "") and "installation" in out.get("erreur", ""), out)
    d = proj_with("sr-noagents2", None, with_agents=False)
    rc, out = go(d, "on")
    check("set-router on: agents absents et settings absent, refus, aucun fichier créé", rc == 2 and raw(d) is None, out)
    for label, content in (("autre valeur", b'{"agent": "autre", "x": 1}'), ("JSON invalide", b"{ pas du json"), ("non-objet", b"[1, 2]")):
        for state in ("on", "off"):
            d = proj_with("sr-refus", content)
            rc, out = go(d, state)
            check("set-router %s: %s, refus sans changement à l'octet" % (state, label), rc == 2 and out.get("ok") is False and raw(d) == content, (rc, out))
    d = proj_with("sr-off-nofile", None)
    rc, out = go(d, "off")
    check("set-router off: settings absent, rien à écrire, fichier non créé", rc == 0 and out.get("ecrit") is False and raw(d) is None, out)
    # dry-run
    d = proj_with("sr-dry", b'{"x": 1}')
    before = raw(d)
    rc, out = go(d, "on", "--dry-run")
    d2 = proj_with("sr-dry2", None)
    rc2, out2 = go(d2, "on", "--dry-run")
    check("set-router --dry-run: n'écrit rien (fichier existant intact, fichier absent non créé)",
          rc == 0 and rc2 == 0 and raw(d) == before and raw(d2) is None and out.get("ecrit") is False and out.get("dry_run") is True, (out, out2))
    # dépôt source simulé
    d = fake_source_repo("sr-source")
    (d / ".claude/agents").mkdir(parents=True)
    for n in AGENTS3:
        (d / ".claude/agents" / (n + ".md")).write_text("x\n", encoding="utf-8")
    rc, out = go(d, "on")
    check("set-router: dépôt source simulé, refus sans écrire", rc == 2 and "dépôt source" in out.get("erreur", "") and raw(d) is None, out)
    # seul .claude/settings.json change
    d = proj_with("sr-only", b'{"x": 1}')
    (d / "README.md").write_text("r\n", encoding="utf-8")
    t0 = tree(d)
    go(d, "on")
    t1 = tree(d)
    changed = sorted(k for k in set(t0) | set(t1) if t0.get(k) != t1.get(k))
    check("set-router: seul .claude/settings.json change", changed == [".claude/settings.json"], changed)
    # status.py : désactivé après off, configuré après on
    home = new_dir("sr-home")
    d = installed_project("sr-installed", ANSWERS_FULL)
    st0 = status(d, home)[1]["routeur"]
    rc, off = go(d, "off")
    cfg_off = (d / ".claude/settings.json").read_text(encoding="utf-8")
    st1 = status(d, home)[1]
    rc, on = go(d, "on")
    cfg_on = (d / ".claude/settings.json").read_text(encoding="utf-8")
    st2 = status(d, home)[1]["routeur"]
    check("status.py: configuré à l'installation, désactivé après off (suggère /brain -d on), configuré après on",
          st0 == "configuré" and st1["routeur"] == "désactivé" and "/brain -d on" in st1["routeur_raison"] and st2 == "configuré", (st0, st1["routeur"], st2))
    if os.environ.get("SHOW_SET_ROUTER"):
        print("-- INFO set-router on (projet installé) --")
        print(json.dumps(on, ensure_ascii=False, indent=2))
        print("-- INFO set-router off (projet installé) --")
        print(json.dumps(off, ensure_ascii=False, indent=2))
        print("-- INFO settings.json après off --" + NL + cfg_off + "-- INFO settings.json après on --" + NL + cfg_on)
    skill = (ACCOUNT / "SKILL.md").read_text(encoding="utf-8")
    check("SKILL.md: -d on|off (aide, aiguillage), autorisation expresse de settings.json par set-router.py, conversation suivante, pas de garde-fou, refus avec aide",
          "-d on|off" in skill and "set-router.py" in skill and "`-d` : l'écriture de `.claude/settings.json` par `set-router.py` seul" in skill
          and "conversation suivante" in skill and "jamais concerné" in skill and "refuser et afficher l'aide" in skill
          and "sans seconde validation" in skill and len(skill.splitlines()) <= 70, len(skill.splitlines()))
    guide = (REPO / "brain-main/README.md").read_text(encoding="utf-8")
    check("guide: /brain -d on|off présent, au plus 80 lignes", "/brain -d on" in guide and "/brain -d off" in guide and len(guide.splitlines()) <= 80, len(guide.splitlines()))
    check("projet installé: set-router.py présent (copie du moteur)", (d / "brain-main/bootstrap/scripts/set-router.py").is_file())
    ad = "".join(prepare(mk(new_dir("sr-adapt"), {"README.md": "x", ".claude/settings.json": '{"model": "x"}'}))[1]["adaptations"])
    check("adaptation à l'installation (settings.json existant): mentionne /brain -d on", "/brain -d on" in ad and '"agent": "orchestrator"' in ad, ad)


def main():
    global TMP
    sys.stdout.reconfigure(encoding="utf-8")
    TMP = tempfile.mkdtemp(prefix="2brain-bootstrap-tests-")
    try:
        for name, fn in (("hook", test_hook), ("status", test_status), ("fetch", test_fetch)):
            try:
                fn()
            except Exception:
                check("exception dans le groupe " + name, False, traceback.format_exc()[-600:])
        try:
            setup_fetched()
        except Exception:
            check("exception dans la préparation de la source", False, traceback.format_exc()[-600:])
        for name, fn in (("prepare/cancel", test_prepare_cancel), ("installation", test_install),
                         ("racine", test_root_files), ("brain généré", test_generated_brain),
                         ("status -o", test_status_todo), ("onboarding", test_onboarding_files),
                         ("mise à jour", test_update), ("compte", test_account), ("skill", test_skill),
                         ("adaptateur", test_adapter), ("garde-fou routeur", test_router_guard), ("set-router", test_set_router),
                         ("règles d'onboarding", test_onboarding_rules), ("gitignore", test_gitignore),
                         ("sortie UTF-8", test_utf8_output)):
            try:
                fn()
            except Exception:
                check("exception dans le groupe " + name, False, traceback.format_exc()[-600:])
    finally:
        rmtree(TMP)
    ok = sum(1 for _, r in RESULTS if r)
    print("\nRÉSULTAT : %d PASS, %d FAIL sur %d" % (ok, len(RESULTS) - ok, len(RESULTS)))
    return 0 if ok == len(RESULTS) else 1


if __name__ == "__main__":
    sys.exit(main())
