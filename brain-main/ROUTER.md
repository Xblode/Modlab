---
metadata_version: 1
title: Agent Router
type: specification
description: Spécifie le routage de la délégation entre un orchestrateur unique et des workers d'exécution.
summary: Un orchestrateur unique dialogue avec l'humain, décide, rédige des briefs, délègue à des workers et contrôle leurs rapports. Ce document fixe les rôles, les profils, les critères de sélection, le découpage des missions volumineuses, le brief minimal, le rapport décisionnel court, le contrôle, le choix entre reprise et nouveau worker, la gestion adaptative, la trace et le contrat qu'un adaptateur doit respecter. Il ne contient aucun nom d'outil ni de modèle.
version: "0.2.0"
status: draft
created: "2026-11-05"
updated: "2026-11-06"
scope:
  level: universal
  targets: [brain-instances]
depends_on:
  - "brain-main/conventions/METADATA.md"
tags:
  - routing
  - agents
  - delegation
---

# Agent Router

Spécification universelle de routage. Un adaptateur propre à un outil l'applique ; il ne la copie pas. Les sections sont numérotées pour que les autres fichiers y renvoient.

## 1. Rôles et profils

**Orchestrateur** : seul interlocuteur de l'humain. Il dialogue, analyse, arbitre, lit de façon ciblée, rédige les briefs, contrôle les rapports et consolide. Il ne modifie aucun fichier et n'exécute aucune commande. Il répond de la cohérence du résultat final.

**Worker** : exécute une mission décrite par un brief. Il ne délègue pas, ne décide pas de l'architecture, n'invente pas d'hypothèse. Il s'arrête quand il manque une information.

| Profil | Pour |
| --- | --- |
| `strategic` | Coordination, arbitrage, contrôle qualité |
| `standard` | Développement, refactoring, audit, analyse approfondie |
| `light` | Recherche ciblée, extraction, vérification, correction mineure |

`strategic` est réservé à l'orchestrateur. Les workers relèvent de `standard` ou `light`.

## 2. Sélection

S'arrêter au premier « oui » :

1. Répondre avec ce qui est connu ou une lecture ciblée, sans rien modifier ? L'orchestrateur répond, sans délégation.
2. Micro-tâche (un fichier, résultat évident) ? Une délégation à `light`, sans contre-vérification.
3. Résultat vérifiable mécaniquement, sans jugement ? `light`.
4. Sinon, `standard`.

- Doute entre `light` et `standard` : `light` seulement si l'échec coûte peu.
- Échec de `light` : une reprise par `standard`, puis retour à l'humain.
- Parallèle : deux workers au plus, sans dépendance de résultat ni fichier commun.

### 2.1 Critères

| Critère | Lecture |
| --- | --- |
| Complexité | Nombre de fichiers et de décisions à prendre |
| Type de travail | Lecture, modification, vérification |
| Compétences | Jugement ou application d'une règle |
| Coût | Prix du modèle × volume attendu, y compris la reprise en cas d'échec |
| Contexte nécessaire | Peut-il tenir dans un brief ? |
| Parallélisation | Indépendance réelle |

### 2.2 Découpage

Signes d'une mission volumineuse : beaucoup de fichiers ou de longues sources à traiter, plusieurs livrables distincts, un résultat dont dépend la suite. Dans ce cas, l'orchestrateur la divise en sous-tâches ciblées, chacune avec un périmètre et un livrable précis. Il les place en séquence quand l'une dépend de l'autre, et vérifie le résultat à chaque point de contrôle avant de lancer la suite. Pas de découpage inutile : une petite mission reste entière.

## 3. Brief

Le strict nécessaire à cette mission, parmi :

- objectif ;
- périmètre ;
- fichiers concernés ;
- documents à lire, de préférence une section plutôt que le document entier ;
- contraintes citées avec leur référence ;
- actions autorisées ;
- résultat attendu et critères de réussite, dont les vérifications à exécuter ;
- forme du rapport, seulement si elle diffère du format court (§4) : par exemple une sortie brute à restituer telle quelle.

Pas d'historique de la conversation : le brief se suffit. Il ne répète pas ce que le prompt du worker porte déjà, sauf le rappel qu'exige la règle d'un adaptateur.

## 4. Rapport

Format décisionnel court par défaut. Le worker rend :

1. statut : `done`, `partial` ou `blocked` ;
2. résultat ;
3. fichiers modifiés, avec les lignes concernées ;
4. tests : commande, code de sortie, comptes ;
5. incertitudes et décisions à prendre.

La taille du rapport est proportionnelle à la mission, sans plafond chiffré. Les preuves complètes restent accessibles à la demande : commande à rejouer ou fichier temporaire cité. La sortie brute n'est jointe que si elle est décisive, en cas d'échec (l'extrait utile), ou si le brief la demande. Pas de journal d'exploration.

`partial` couvre aussi le cas où le worker constate que la mission dépasse nettement le brief (volume, fichiers, difficulté) : il s'arrête, rend ce qui est fait et explique l'écart, plutôt que de continuer.

Si la mission prépare ou fait un commit, le rapport dit quels README des dossiers modifiés ont été relus et lesquels corrigés.

## 5. Connaissances

L'orchestrateur répond de la distribution des connaissances, sans tout lire lui-même. Il lit les métadonnées puis les sections utiles (METADATA §9.4), ou confie l'extraction d'un long document à `light`. Il transmet au worker la règle citée et sa référence. Un worker peut lire un document précis si le rapport coût/qualité le justifie. Avant de travailler dans un dossier, tout agent, worker compris, lit le README le plus proche (dossier courant, sinon un parent), même si le brief ne le cite pas (METADATA §6.9) ; sans README, il continue.

## 6. Question en cours de mission

Le worker s'arrête et rend `blocked` avec ses questions. L'orchestrateur répond, en reprenant ce même worker si la comparaison du §6.1 le justifie. S'il n'a pas la réponse, ou si une décision humaine est nécessaire, il sollicite l'humain.

### 6.1 Reprise ou nouveau worker

Comparer le coût estimé. Reprendre un worker coûte son contexte hérité, relu à chaque tour restant. Un nouveau worker coûte son démarrage, la relecture nécessaire et le brief. Reprendre en cas de blocage, de question ou de petite suite sur le même sujet, quand l'historique sert vraiment. Sinon, lancer un nouveau worker avec un handoff synthétique : ce qui est fait, les décisions prises, les fichiers concernés, le reste à faire. Aucune interdiction de reprendre : c'est un choix de coût.

## 7. Contrôle

- Sans modification : cohérence du rapport.
- Avec modification : vérifications exécutées dans la mission et rapportées (commande, code, compte), puis lecture par l'orchestrateur des passages modifiés. Le contrôle repose sur des preuves vérifiables, pas sur l'affirmation du worker ; en cas de doute, l'orchestrateur demande le détail (commande à rejouer, fichier cité, sortie brute).
- Exception (risque élevé ou résultat douteux) : contre-vérification par un second worker.

### 7.1 Gestion adaptative

À chaque rapport et à chaque signal disponible, l'orchestrateur compare la progression, le contexte consommé et le coût au plan. Il peut alors réviser le plan, changer de profil ou de stratégie, recadrer le worker ou arrêter. Le worker rend `partial` s'il constate que la mission dépasse nettement le brief (§4). L'orchestrateur ne surveille que les signaux que l'adaptateur expose (§9).

## 8. Trace

Tâche, agent, statut, résultat, décisions, références. En V1, la trace figure dans la réponse finale et dans Git, sans nouveau registre. Aucun raisonnement interne dans le Second Brain.

## 9. Contrat d'adaptateur

Un adaptateur doit :

- interdire l'écriture et l'exécution à l'orchestrateur ;
- limiter ses cibles de délégation ;
- associer chaque profil à un modèle ;
- isoler le contexte des workers ;
- porter le format de rapport (§4) ;
- dire quels signaux de suivi existent (progression, contexte, coût, durée) et ce qu'on ne peut pas surveiller, ainsi que la capacité d'arrêter un worker en cours (§7.1) ;
- écrire ce qui n'est pas garanti.

Un outil sans adaptateur travaille normalement selon `AGENTS.md`, sans routage, avec les mêmes règles de registre.
