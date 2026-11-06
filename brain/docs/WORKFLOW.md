---
metadata_version: 1
title: "Méthode de travail"
type: guide
description: "Comment le projet fonctionne, évolue et est travaillé."
version: "0.2.0"
status: draft
created: "2026-11-06"
updated: "2026-11-06"
updated_from:
  - "CLAUDE.md"
  - "docs/01-architecture-decidee.md"
  - "brain/sources/audits/20261106-1108.md"
---

# Méthode de travail

## 1. Objet

Comment le projet fonctionne, évolue et est travaillé.

## 2. État actuel

Seules les informations confirmées figurent en section 3 ; rien n'est inventé.

## 3. Informations confirmées

Documents de référence (source : `CLAUDE.md`) :

- `docs/01-architecture-decidee.md` : architecture verrouillée, à lire avant tout travail structurant.
- `docs/design/shell-maquette.html` : source de vérité visuelle, à suivre au pixel près dans l'interface React.
- `docs/idees.md` : notes de faisabilité et idées non planifiées.
- `docs/00-cadrage-technique.md` et `docs/Modlab — Document de cadrage & architecture.md` : contexte dépassé, à ne jamais mettre en œuvre tels quels.

Versionnement :

Git en local, sans push vers GitHub sauf demande explicite. (Source : `CLAUDE.md`.) Aucun dépôt distant n'est configuré. (Source : `.git/config`, d'après l'audit.)

Tests (source : `CLAUDE.md`) :

Le moteur d'abord : tests d'intégration Rust sur des fixtures (un « faux Skyrim »), jamais sur le vrai jeu. Les tests d'interface viennent plus tard.

Suivi :

ClickUp, plan gratuit. Les agents se distinguent par des tags, jamais par des champs personnalisés. (Source : `CLAUDE.md`.)

## 4. Contenu

Équipe d'agents Claude Code (source : `docs/01-architecture-decidee.md` §11) :

- `pm` : ClickUp, jalons, décisions à trancher, point hebdomadaire.
- `architect` : contrats Rust, ADR, revue.
- `core-dev` : moteur Rust.
- `skyrim-dev` : adaptateur Skyrim et orchestration des outils.
- `mcp-dev` : serveur MCP, permissions, recettes.
- `ui-designer` : maquette HTML, tokens, revue visuelle.
- `frontend-dev` : React et TypeScript, composants, IPC Tauri, tests.
- `qa` : fixtures et tests du moteur Rust.

Évolution de l'interface :

Toute évolution passe d'abord par la maquette. (Source : `docs/01-architecture-decidee.md` §2.)

## 5. Questions ouvertes

- À compléter : l'articulation entre ces 8 agents et l'orchestrateur et les workers présents dans `.claude/agents/` n'est pas écrite.
- À compléter : la convention de casse des fichiers de l'interface n'est pas écrite.
- À compléter : la fréquence réelle des commits et de la revue n'est pas écrite.

## 6. Références

À compléter.
