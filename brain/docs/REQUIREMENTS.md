---
metadata_version: 1
title: "Exigences"
type: specification
description: "Ce que le projet doit accomplir : besoins et exigences."
version: "0.2.0"
status: draft
created: "2026-11-06"
updated: "2026-11-06"
updated_from:
  - "docs/01-architecture-decidee.md"
  - "CLAUDE.md"
---

# Exigences

## 1. Objet

Ce que le projet doit accomplir : besoins et exigences.

## 2. État actuel

Seules les informations confirmées figurent en section 3 ; rien n'est inventé.

## 3. Informations confirmées

Sécurité des fichiers :

- Toute mutation suit le même cycle, quel que soit le client (UI, CLI, MCP) : intention, plan lisible, approbation, snapshot, application, vérification, puis commit ou retour arrière automatique. (Source : `docs/01-architecture-decidee.md` §5.)
- Rien n'est jamais écrit directement dans les dossiers du jeu ni supprimé directement : tout reste réversible, avec une corbeille interne. (Source : `CLAUDE.md`.)
- Les tests de sécurité des fichiers du Core sont non négociables. (Source : `CLAUDE.md`.)

Transparence :

L'automatisation ne doit jamais devenir une boîte noire : chaque action explique quoi, pourquoi, quels fichiers, et comment revenir en arrière. (Source : `docs/01-architecture-decidee.md` §1.)

Accès de l'IA :

- L'IA n'a jamais d'accès direct aux fichiers ni à un shell arbitraire. Elle passe toujours par MCP, les permissions ModLab, puis le Core. (Source : `docs/01-architecture-decidee.md` §7.2.)
- Trois niveaux de permission : lecture automatique, mutation réversible configurable, action dangereuse toujours soumise à une confirmation explicite. (Source : `docs/01-architecture-decidee.md` §7.2.)

Multi-jeux :

Le Core ne connaît aucun jeu ni aucune plateforme. Ajouter un jeu ne doit rien changer au noyau. (Source : `docs/01-architecture-decidee.md` §3 et §13.)

Interface :

`docs/design/shell-maquette.html` est la source de vérité visuelle. Toute évolution de l'interface passe d'abord par la maquette. (Source : `docs/01-architecture-decidee.md` §2.)

## 4. Contenu

Jalons de sortie de la feuille de route (source : `docs/01-architecture-decidee.md` §13) :

- Phase 0 : la CLI détecte Skyrim, importe une archive, crée un loadout vide et journalise une transaction.
- Phase 1 : « Je joue à Skyrim lancé par ModLab avec ma vraie modlist ».
- Phase 2 : « Je n'ouvre plus MO2 », et Minecraft prouve la généralité de l'adaptateur.
- Phase 3 : « Claude installe un mod et m'explique un conflit ».
- Phase 4 : « ModLab voit plus de choses que MO2 + LOOT + xEdit réunis ».
- Phase 5 : « Je résous un crash sans ouvrir un log à la main ».
- Phase 6 : « Ajouter un jeu n'a rien changé au noyau ».

## 5. Questions ouvertes

- À compléter : les jalons déjà atteints ne sont pas écrits.
- À compléter : les exigences de performance ne sont pas chiffrées dans les sources lues.

## 6. Références

À compléter.
