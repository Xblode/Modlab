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
