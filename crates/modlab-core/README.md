# Porte d'entrée — Core (`modlab-core`)

## À quoi sert ce crate

Moteur Rust unique de ModLab : **SQLite + journal d'événements = vérité**. UI, CLI et MCP n'écrivent l'état que via ce Core. Aucune logique Skyrim, Minecraft, Nexus ou CurseForge ici — seulement des primitives et des contrats (`GameAdapter`, transactions, loadouts).

## Fichiers / modules clés

- `store`, `transaction` : cycle plan → snapshot → apply → rollback.
- `loadout`, `instance_root`, `install`, `archive`, `download` : profils, racines d'instance, installation, archives, téléchargements.
- `game_adapter` : trait d'adaptateur de jeu (implémenté dans les crates `modlab-game-*`).
- `conflicts`, `mod_meta`, `mod_config`, `files`, `data_root` : conflits, métadonnées, chemins, racine des données.

## Ne pas

- Ajouter de règles propres à un jeu ou à une plateforme.
- Contourner le cycle transaction pour écrire sur disque ou en base.
- Tester contre une installation réelle (utiliser `fixtures/`).

## Ensuite

- Adaptateurs jeu : `crates/modlab-game-skyrim`, `crates/modlab-game-minecraft`.
- Shell Tauri / commandes : `crates/modlab-tauri` (si présent) et `ui/`.
- Architecture verrouillée : `docs/01-architecture-decidee.md` §3 et §5.

Tests : sécurité fichiers non négociable ; intégration sur fixtures uniquement.