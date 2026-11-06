---
metadata_version: 1
title: Adaptateur Claude Code de l'Agent Router
description: Décrit l'adaptateur Claude Code installé dans .claude/ : correspondance profil, agent et modèle, outils, contrat du routeur et limites non garanties.
version: "0.1.2"
status: draft
created: "2026-11-06"
updated: "2026-11-06"
type: reference
scope:
  level: domain
  targets: [claude-code]
depends_on:
  - "brain-main/ROUTER.md"
---

# Adaptateur Claude Code

Cet adaptateur applique `brain-main/ROUTER.md` (§9). Les règles (rôles, sélection, brief, rapport, contrôle, trace) n'y sont pas recopiées : voir ce document. `/brain -i` le copie dans `.claude/` du projet ; sa source est `brain-main/adapters/claude-code/`. Une nouvelle conversation démarre alors en orchestrateur, qui délègue aux workers. Décision de l'humain du 2026-11-06.

## Correspondance

| Profil | Agent | Modèle | `tools` |
| --- | --- | --- | --- |
| `strategic` | `orchestrator` | `opus` | `Read, Grep, Glob, Agent(worker-sonnet, worker-haiku), SendMessage, AskUserQuestion` |
| `standard` | `worker-sonnet` | `sonnet` | `Read, Grep, Glob, Edit, Write, Bash` |
| `light` | `worker-haiku` | `haiku` | `Read, Grep, Glob, Edit, Bash` |

- Les workers ont `omitClaudeMd: true`, `disallowedTools: mcp__*`, ni `Agent` ni `SendMessage`.
- `worker-haiku` n'a pas `Write` : créer un fichier relève de `standard`.
- Pas de mode plan (il exige d'écrire un fichier) : les plans sont présentés dans la conversation et validés par `AskUserQuestion`.
- `settings.json` : `{"agent": "orchestrator"}` seulement. Aucun hook, plugin, MCP ni `permissions`.
- Les noms `worker-sonnet` et `worker-haiku` portent le modèle (exception à NAMING §2.1, limitée à `.claude/agents/`).

## Contrat du routeur (§9)

| Exigence | Où |
| --- | --- |
| Écriture et exécution interdites à l'orchestrateur | `tools` de `orchestrator` : ni `Bash`, ni `Write`, ni `Edit` |
| Cibles de délégation limitées | `Agent(worker-sonnet, worker-haiku)` |
| Un modèle par profil | champ `model` de chaque agent |
| Contexte des workers isolé | `omitClaudeMd: true` et brief autonome |
| Format de rapport | corps des deux workers (§4) |
| Ce qui n'est pas garanti | section suivante |

## Ce qui n'est pas garanti

1. `Bash` chez les workers contourne toute interdiction : seules la consigne du prompt et le mode de permission s'opposent à une écriture dans `.claude/` ou à un `git commit`.
2. Les modèles ne sont pas verrouillés. Le paramètre `model` d'un appel passe avant le champ de l'agent : l'orchestrateur pourrait surcharger un worker. La consigne est de ne jamais le passer.
3. Un worker ne peut pas interroger l'orchestrateur : il s'arrête en `blocked`, et la reprise a un coût.
4. Chaque modification passe par un aller-retour vers un worker : plus lent, et la consommation n'est pas mesurée.
5. Le prompt de l'orchestrateur remplace le prompt par défaut : des consignes natives peuvent être perdues. Les outils propres à Desktop (liste de tâches, etc.) peuvent manquer à l'orchestrateur.
6. Claude Code conserve les transcriptions des sous-agents dans `~/.claude/`, hors du projet.
7. Non vérifié en session réelle : démarrage en orchestrateur, délégation depuis `/brain`, permissions des workers en arrière-plan.
8. Application dans une nouvelle conversation seulement : la clé `agent` n'a aucun effet dans la conversation d'installation (test réel : pas d'orchestrateur, `worker-sonnet` introuvable). Le constat « session non routée » (Bash, Write ou Edit disponibles alors que le routeur est configuré) est une déduction d'après les outils, sans garantie.
9. Version cible : Claude Code 2.1.317 ou plus. Comportement inconnu avec une version plus ancienne (le CLI 2.1.22 n'a pas été essayé).

## Retrait

`/brain -d off` retire la clé `agent` de `.claude/settings.json` (`/brain -d on` la remet), puis ouvrir une nouvelle conversation ; retirer la clé à la main reste valable. Les agents restent disponibles sans effet. Un `.claude/settings.json` déjà présent n'est jamais modifié à l'installation : le routeur reste inactif jusqu'à `/brain -d on` ou à l'ajout manuel de `"agent": "orchestrator"`.
