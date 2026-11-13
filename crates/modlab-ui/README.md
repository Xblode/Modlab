# Hôte Tauri de l'interface

Ce dossier fait tourner l'interface React (`ui/`) dans une application de bureau Tauri 2.

## Ce qu'il fait

- Il relie l'interface au moteur et aux adaptateurs de jeu.
- Il fournit un navigateur intégré, sous la forme d'un second WebView2, car Nexus envoie `X-Frame-Options`.

## Fichiers principaux

- `src/lib.rs` : point central de l'hôte.
- `src/browser.rs` : navigateur intégré.
- `src/downloads.rs` : téléchargements.
- `src/main.rs` : point d'entrée.
- `tauri.conf.json` : configuration de Tauri.

`.modlab-dev/` est ignoré par git.
