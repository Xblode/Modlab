---
metadata_version: 1
title: "Interface React"
type: reference
description: "Interface de ModLab, en React, TypeScript et Vite."
version: "0.1.0"
status: draft
created: "2026-11-06"
updated: "2026-11-06"
updated_from:
  - "brain/sources/audits/20261106-1108.md"
---

# Interface React

Ce dossier contient l'interface de ModLab, en React, TypeScript et Vite.

## Organisation

- `src/app/` : les 10 écrans.
- `src/components/` : les composants, en atomic design.
- `src/hooks/` : les hooks.
- `src/api.ts` : le lien vers l'hôte Tauri.
- `src/shell.css` : les styles du shell.

## Règles

- `docs/design/shell-maquette.html` est la source de vérité visuelle : toute évolution passe d'abord par la maquette.
- Le rendu par jeu suit les capacités déclarées par les adaptateurs (ADR 0005).
- Les tests d'interface viendront plus tard.
