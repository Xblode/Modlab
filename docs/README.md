# Documentation du projet

## Ce qui fait foi

- `01-architecture-decidee.md` : architecture verrouillée, référence unique. En cas de désaccord, ce document l'emporte.
- `ui/` : référence visuelle de l'interface. L'ancienne maquette HTML a été retirée, remplacée par `ui/`.

## Décisions

- `adr/` : les décisions d'architecture (ADR 0001 à 0005).

## Design

- `design/` : `plan-shell-v2.md`, `design-system.md` (tokens, composants), maquettes `.h2d` (dashboard, mods, réglages) et `img/`. L'ancienne maquette HTML a été retirée, remplacée par `ui/`.
- `design/regles-interface.md` : règles d'interface ModLab (décisions du 2026-11-24, consensus des références, études). Référence pour faire évoluer `ui/`.

## Détail vivant

- `faisabilite.md` : notes de faisabilité (indicateurs SKSE/ENB/FG, LOOT, installation manuelle).
- `superpouvoirs.md` : superpouvoirs privés (IA locale, chargement à la demande).

## Pas encore cadré

- `communaute/` : direction communauté / profils / économie, à ne pas implémenter avant cadrage.

## Archive

- `archive/` : documents dépassés, conservés pour l'historique humain. **Agents : hors périmètre par défaut** — ne pas ouvrir ni lire `archive/**` sauf demande explicite de l'humain. Ne jamais les mettre en œuvre tels quels (voir `archive/README.md`).

Pilotage du projet (état, décisions, idées, tâches) : `.bframe/`
