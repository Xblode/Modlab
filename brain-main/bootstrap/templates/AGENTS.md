---
metadata_version: 1
title: "Instructions agents"
type: index
description: "Point d'entrée des agents IA : contexte 2brain chargé à chaque session et consignes de base."
version: "0.7.0"
status: draft
created: "{{date}}"
updated: "{{date}}"
---

# Instructions agents

Ce projet utilise 2brain. Le moteur est dans `brain-main/` (conventions, outils, guide) ; le contenu du projet est dans `brain/` (documents, registre). Le contenu du projet va dans `brain/`, jamais dans `brain-main/`.

## Routeur

Dans Claude Code, la session démarre en orchestrateur et applique `brain-main/ROUTER.md` (adaptateur dans `.claude/`, décrit par `.claude/README.md`). Un outil sans adaptateur travaille normalement selon ce fichier, sans routage, avec les mêmes règles de registre (ROUTER §9).

Si tu as Bash, Write ou Edit alors que `.claude/settings.json` contient `"agent": "orchestrator"`, tu n'es probablement pas routé (déduction d'après tes outils) : le dire une fois par session et proposer d'ouvrir une nouvelle conversation. Aucune autre action n'est imposée.

## Contexte chargé

Ces fichiers sont importés dans cet ordre :

@brain-main/conventions/README.md
@brain-main/conventions/TAXONOMY.md
@brain-main/README.md

Pour un outil qui n'applique pas les imports `@` : lire ces mêmes fichiers dans le même ordre (README des conventions, `brain-main/conventions/TAXONOMY.md`, puis le guide `brain-main/README.md`), puis `brain-main/conventions/NAMING.md` et `brain-main/conventions/METADATA.md` au moment voulu. Les instructions détaillées d'un outil se lisent quand l'outil sert, au chemin donné par le guide.

## Consignes

- **Obligatoire avant de créer ou modifier un document** : lire `brain-main/conventions/NAMING.md` pour un nom de fichier ou de dossier, et `brain-main/conventions/METADATA.md` pour un en-tête, une version ou une date (au moins les sections utiles).
- **Avant de travailler dans un dossier** : lire son README le plus proche (dossier courant, sinon un parent), puis le tenir à jour si ton travail change son rôle ou ses fonctionnalités (modèle : `brain-main/conventions/METADATA.md` §6.9).
- Si `brain/docs/` contient encore « À compléter » : proposer `/brain -a` si le projet a des fichiers et aucun rapport dans `brain/sources/audits/`, sinon `/brain -o`, une seule fois par session.
- **Recherche sans demander** : avant de proposer une implémentation, ou quand le sujet a pu être déjà traité, chercher dans le registre (`brain/registry/entries.jsonl`) sans demander, en lançant `worker-haiku` (lecture seule : `brain-main/skills/brain-search/scripts/search-registry.py` ou Grep). Sans cet agent (session non routée), lancer soi-même la recherche. Elle n'écrit rien. Review et Sync restent sur demande claire ; un essai de script se fait sur une copie.
- Ne rien modifier sans validation explicite de l'humain. Une proposition n'est pas une décision.
