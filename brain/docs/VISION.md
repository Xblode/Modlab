---
metadata_version: 1
title: "Vision"
type: reference
description: "Pourquoi le projet existe, ses objectifs et sa vision à long terme."
version: "0.2.0"
status: draft
created: "2026-11-06"
updated: "2026-11-06"
updated_from:
  - "brain/sources/audits/20261106-1108.md"
  - "CLAUDE.md"
---

# Vision

## 1. Objet

Pourquoi le projet existe, ses objectifs et sa vision à long terme.

## 2. État actuel

Seules les informations confirmées figurent en section 3 ; rien n'est inventé.

## 3. Informations confirmées

Objectif :

Un seul moteur (le Core) dont l'UI, la CLI et le MCP sont des clients. Toute modification est réversible via le cycle plan → approbation → snapshot → application → vérification → commit. L'IA passe toujours par le MCP et les niveaux de permission.

Positionnement :

Gestionnaire de mods personnel, Skyrim d'abord et multi-jeux, positionné « Cursor pour le game modding », et non comme un MO2 plus joli. Projet de long terme, construit avec Claude Code comme coéquipier natif via MCP. (Source : `CLAUDE.md`.)

Principes :

- Le Core ne connaît ni un jeu précis (`GameAdapter`) ni une plateforme de mods précise (`Provider`). (Source : `docs/01-architecture-decidee.md` §3.)
- Rien n'est supprimé directement : une corbeille interne sert de filet. (Source : `docs/01-architecture-decidee.md` §5.)

## 4. Contenu

À compléter.

## 5. Questions ouvertes

- Hypothèse à valider : le projet en serait à la Phase 2 de la feuille de route (déduit des fichiers présents et de l'ADR 0005).
- À compléter : l'état d'avancement réel de la feuille de route n'est pas écrit.

## 6. Références

À compléter.
