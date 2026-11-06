---
metadata_version: 1
title: "Guide de brain-main"
type: index
description: "Guide court de 2brain pour une IA : rôle de brain-main/ et de brain/, commandes /brain, niveaux de lecture et règles."
version: "1.4.1"
status: draft
created: "2026-11-06"
updated: "2026-11-06"
---

# brain-main

Moteur 2brain : conventions, outils et règles réutilisables d'un projet à l'autre. Les conventions chargées précèdent ce guide : ne pas les recopier, s'y référer.

## Séparation stricte

- `brain-main/` : le système (ce dossier). Jamais de contenu du projet ici.
- `brain/` : le projet. `docs/` (VISION, CADRAGE, PROJECT, REQUIREMENTS, ARCHITECTURE, WORKFLOW), `registry/entries.jsonl` (registre de capture), `sources/` (créé à la première capture ou au premier audit : `sessions/`, `audits/`). Aucune trace du système dans `brain/`.

## Commandes `/brain`

| Commande | Fait quoi | Quand | Écrit |
| --- | --- | --- | --- |
| `/brain` | État du projet et aide | Pour savoir où en est le projet | non |
| `/brain -i` | Installe 2brain, puis propose `-o` | Projet sans `brain-main/` | oui, après validation |
| `/brain -u` | Met à jour `brain-main/` | Version plus récente de 2brain | oui, après validation |
| `/brain -a` | Audit en profondeur d'un projet existant : rapport, puis écritures validées une à une | Projet existant sans rapport d'audit | oui, après validation de chaque écriture |
| `/brain -o` | Remplit `brain/docs/` avec le projet | `brain/docs/` contient « À compléter » | oui, après validation |
| `/brain -c` | Capture la session courante dans le registre | Fin d'un échange à garder | oui (extrait et registre) |
| `/brain -c <fichier>` | Capture un fichier existant | Une source externe à garder | oui (registre) |
| `/brain -c 1` | Annule la dernière capture | Capture erronée | oui, après confirmation |
| `/brain -s <sujet>` | Cherche dans le registre | Retrouver une information | non |
| `/brain -r` | Review : rapport sur le registre | Sur demande seulement | non |
| `/brain -d on\|off` | Active ou désactive la délégation (routeur) | Changer le routeur | oui, `.claude/settings.json` |
| `/brain -m`, `/brain -m <nombre>` | README des dossiers maîtres : inventaire et propositions dans la limite `readme_max` ; avec un nombre, règle d'abord ce maximum (`brain/settings.json`, 8 par défaut) | Voir, compléter ou élargir les README | oui, chaque README validé un par un |

Les commandes s'exécutent avec le skill `/brain` du compte. Si une commande échoue, relayer le message, ne rien simuler.

## Trois niveaux de lecture

1. **Chargé à chaque session** : le README des conventions et `TAXONOMY.md`, puis ce guide.
2. **Lu quand l'outil sert** :
   - `brain-main/skills/brain-onboarding/SKILL.md` : `/brain -o` ; `brain-main/skills/brain-audit/SKILL.md` : `/brain -a` ; `brain-main/skills/brain-readme/SKILL.md` : `/brain -m` ;
   - `brain-main/skills/brain-capture/SKILL.md` : `/brain -c`, `-c <fichier>`, `-c 1` ;
   - `brain-main/registry/SCHEMA.md` : contrat des lignes du registre ;
   - `brain-main/bootstrap/README.md` : installation, mise à jour, détail de chaque commande ;
   - `brain-main/ROUTER.md` : délégation entre agents ;
   - `brain-main/conventions/NAMING.md` (nom de fichier ou de dossier) et `brain-main/conventions/METADATA.md` (en-tête, version, date) : à la tâche.
3. **Jamais lu, seulement exécuté** : les scripts (`brain-main/skills/*/scripts/`), les tests (`brain-main/bootstrap/tests/`) et les modèles (`brain-main/bootstrap/templates/`). `-s` et `-r` n'ont pas de `SKILL.md` : lancer le script avec `--help` si besoin.

## Règles

- Registre `brain/registry/entries.jsonl` en append only : ne jamais réécrire ni renuméroter ; ajouter par `append-captures.py` seulement.
- Une proposition n'est pas une décision : seule une validation explicite de l'humain décide ; avec `/brain -a`, `-o`, `-u`, `-i`, `-m`, rien n'est écrit avant la validation de ce qui a été présenté.
- Ne lancer ni Review ni synchronisation externe sans demande claire ; pas de commit ni de push sans demande. **Search sans demander** : avant une implémentation ou si le sujet a pu être traité, chercher dans le registre par `worker-haiku` (lecture seule), ou soi-même sans routeur ; un essai de script se fait sur une copie.
- **Avant un commit**, relire les README des dossiers modifiés et rapporter ce qui a été relu (le hook du compte refuse une fois le `git commit` à relire ; il arrête et donne la consigne, sans garantir la lecture).
- Ne pas modifier un document avec `locked: true` sans demande explicite.
- **Obligatoire avant de créer ou modifier un document** : lire la convention concernée, au moins les sections utiles : `NAMING.md` pour un nom de fichier ou de dossier, `METADATA.md` pour un en-tête, une version ou une date. **Avant de travailler dans un dossier**, lire son README le plus proche (dossier courant, sinon un parent ; modèle : `METADATA.md` §6.9) et le tenir à jour.

## Routeur actif par défaut

Dans Claude Code, `/brain -i` installe l'adaptateur dans `.claude/` : les conversations ouvertes ensuite démarrent en orchestrateur (le routeur s'applique à partir de la conversation suivant l'installation, jamais à la conversation d'installation), qui dialogue, décide et délègue à `worker-sonnet` et `worker-haiku`, sans Bash, Write ni Edit. Règles : `brain-main/ROUTER.md`. Correspondance et limites : `.claude/README.md`. Retrait ou remise : `/brain -d off` ou `/brain -d on` (effet à la conversation suivante) ; enlever la clé `agent` à la main reste valable. Un `.claude/settings.json` existant n'est jamais modifié à l'installation : `/brain -d on` y ajoute `"agent": "orchestrator"`.

## Sources de capture et registre

- `brain/sources/sessions/` reçoit les extraits de session produits par `/brain -c`, un fichier par capture (`AAAAMMJJ-HHMM-<8 premiers caractères de l'id>.txt`). Un extrait n'est jamais réécrit : les numéros de ligne cités par le registre doivent rester valables.
- Un extrait peut contenir des passages de conversation ; le masquage des secrets n'est pas une garantie. **Relire chaque extrait avant tout commit.**
- Le contrat du registre est `brain-main/registry/SCHEMA.md`, complété par `brain-main/conventions/TAXONOMY.md`. Chaque entrée garde l'auteur, la source et la validation : une capture non validée reste une proposition.

## Catalogue de brain-main

| Chemin | Rôle |
| --- | --- |
| `conventions/` | Conventions communes (métadonnées, nommage, taxonomies) |
| `skills/brain-capture/`, `brain-search/`, `brain-review/`, `brain-onboarding/`, `brain-audit/`, `brain-readme/`, `skills/README.md` | Outils de `/brain -c`, `-s`, `-r`, `-o`, `-a`, `-m` ; index des skills |
| `registry/SCHEMA.md` | Contrat du registre |
| `bootstrap/` | Spécification de `/brain`, scripts d'installation, tests, modèles |
| `ROUTER.md` | Règles de délégation entre agents |
| `adapters/claude-code/` | Source de l'adaptateur copié dans `.claude/` |
