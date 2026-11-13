# Moteur ModLab

Ce dossier contient le Core Rust, seul détenteur de l'état de ModLab. L'interface, la CLI et le MCP en sont des clients. Il ne connaît ni un jeu précis ni une plateforme de mods précise.

## Modules

- `archive` : archives.
- `conflicts` : conflits.
- `download` : téléchargements.
- `game_adapter` : contrat d'adaptateur de jeu.
- `loadout` : loadouts.
- `store` : stockage.
- `transaction` : transactions.
- `lib.rs` : base SQLite et journal d'événements.

## Tests

Les tests de sécurité des fichiers sont non négociables. Les tests d'intégration utilisent `fixtures/`, jamais le vrai jeu.

Référence : `docs/01-architecture-decidee.md` §3 et §5.
