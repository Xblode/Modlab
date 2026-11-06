---
metadata_version: 1
title: Adaptateur Claude Code de l'Agent Router
description: Décrit l'adaptateur Claude Code installé dans .claude/ : correspondance profil, agent et modèle, outils, contrat du routeur et limites non garanties.
version: "0.2.0"
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
| `strategic` | `orchestrator` | `opus` | `Read, Grep, Glob, Agent(worker-sonnet, worker-haiku), SendMessage, TaskStop, AskUserQuestion` |
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
| Signaux de suivi, arrêt d'un worker (§7.1) | section « Signaux de suivi » |
| Ce qui n'est pas garanti | section suivante |

## Signaux de suivi

Pour la gestion adaptative (ROUTER §7.1), l'orchestrateur dispose de :

- le rapport de chaque worker (statut, résultat, fichiers, tests, incertitudes) ;
- la notification de fin d'un worker : `tool_uses`, durée et `subagent_tokens`, qui est la taille du contexte final, pas la consommation ;
- le statut `partial` d'un worker qui constate un dépassement du brief.

Ce qu'il ne peut pas surveiller : la progression d'un worker en cours, sa consommation réelle de jetons, l'état de son cache (5 minutes par défaut ; une reprise après ce délai réécrit tout le contexte). Le tarif de `haiku` est multiplié par 5 au-delà de 100 000 jetons de prompt : repère de coût, pas limite. L'orchestrateur ne lit jamais le fichier `output_file`, qui alourdit son contexte.

**Arrêt d'un worker en cours : possible avec `TaskStop`.** La documentation (https://code.claude.com/docs/en/tools-reference, tableau des outils : « Stops a running background task by ID » ; https://code.claude.com/docs/en/sub-agents) nomme `TaskStop` ; il est dans les `tools` de `orchestrator` (`Read, Grep, Glob, Agent(...), SendMessage, TaskStop, AskUserQuestion`). L'orchestrateur peut arrêter un worker dont le travail devient disproportionné ou inutile ; ce qu'il a déjà écrit reste sur le disque, à contrôler (`git status`, par un worker). Recadrer par `SendMessage` ne s'applique qu'au tour suivant du worker. Constaté le 2026-11-06 d'après la documentation, non essayé en session réelle.

## Scénarios à observer en session réelle

| Scénario | À observer |
| --- | --- |
| Grosse mission découpée | L'orchestrateur découpe (§2.2), vérifie à chaque point de contrôle, lance la suite en séquence |
| Reprise utile | Blocage ou petite suite : reprise par `SendMessage` dans les 5 minutes, sans réécriture complète du contexte |
| Handoff | Après délai ou changement de sujet : nouveau worker avec fait, décisions, fichiers, reste à faire |
| Rapport court puis demande de détail | Rapport sans sortie brute ; l'orchestrateur demande la commande ou le fichier cité en cas de doute |
| Dépassement avec `partial` | Le worker s'arrête, rend `partial` et explique l'écart ; l'orchestrateur révise le plan |

## Ce qui n'est pas garanti

1. `Bash` chez les workers contourne toute interdiction : seules la consigne du prompt et le mode de permission s'opposent à une écriture dans `.claude/` ou à un `git commit`.
2. Les modèles ne sont pas verrouillés. Le paramètre `model` d'un appel passe avant le champ de l'agent : l'orchestrateur pourrait surcharger un worker. La consigne est de ne jamais le passer.
3. Un worker ne peut pas interroger l'orchestrateur : il s'arrête en `blocked`, et la reprise a un coût.
4. Chaque modification passe par un aller-retour vers un worker : plus lent, et la consommation n'est pas mesurée.
5. Le prompt de l'orchestrateur remplace le prompt par défaut : des consignes natives peuvent être perdues. Les outils propres à Desktop (liste de tâches, etc.) peuvent manquer à l'orchestrateur.
6. Claude Code conserve les transcriptions des sous-agents dans `~/.claude/`, hors du projet.
7. Non vérifié en session réelle : démarrage en orchestrateur, délégation depuis `/brain`, permissions des workers en arrière-plan.
8. Application dans une nouvelle conversation seulement : la clé `agent` n'a aucun effet dans la conversation d'installation (test réel : pas d'orchestrateur, `worker-sonnet` introuvable). Le constat « session non routée » (Bash, Write ou Edit disponibles alors que le routeur est configuré) est une déduction d'après les outils, sans garantie.
9. Arrêt d'un worker par `TaskStop` non essayé en session réelle ; aucune mesure de consommation (section « Signaux de suivi »).
10. Version cible : Claude Code 2.1.317 ou plus. Comportement inconnu avec une version plus ancienne (le CLI 2.1.22 n'a pas été essayé).

## Retrait

`/brain -d off` retire la clé `agent` de `.claude/settings.json` (`/brain -d on` la remet), puis ouvrir une nouvelle conversation ; retirer la clé à la main reste valable. Les agents restent disponibles sans effet. Un `.claude/settings.json` déjà présent n'est jamais modifié à l'installation : le routeur reste inactif jusqu'à `/brain -d on` ou à l'ajout manuel de `"agent": "orchestrator"`.
