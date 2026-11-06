---
metadata_version: 1
title: "Projet"
type: reference
description: "Identité, contexte, périmètre, utilisateurs et contraintes du projet."
version: "0.2.0"
status: draft
created: "2026-11-06"
updated: "2026-11-06"
updated_from:
  - "CLAUDE.md"
  - "docs/01-architecture-decidee.md"
  - "Cargo.toml"
---

# Projet

## 1. Objet

Identité, contexte, périmètre, utilisateurs et contraintes du projet.

## 2. État actuel

Seules les informations confirmées figurent en section 3 ; rien n'est inventé.

## 3. Informations confirmées

Nature du projet :

Gestionnaire de mods personnel, Skyrim d'abord et multi-jeux, positionné comme « Cursor du modding ». Application desktop Rust + Tauri + React.

Identité :

Benjamin Planchon, projet perso de long terme, docs en français, code en anglais

Modules envisagés (intention seulement ; rien n'est activé) :

Core, adaptateurs de jeu (Skyrim, Minecraft), UI, CLI, MCP, tests sur fixtures

Périmètre :

Ce dépôt contient uniquement le code source et la documentation. Les données réelles (archives, mods déployés, base SQLite) sont dans `F:\ModLab`. Les deux ne se mélangent jamais. (Source : `CLAUDE.md`.)

Licence :

GPL-3. (Sources : `docs/01-architecture-decidee.md` ; `Cargo.toml`.)

Contraintes :

- Aucun push vers GitHub sans demande explicite. (Source : `CLAUDE.md`.)
- Aucune installation ni mise à jour d'outils globaux, et rien en dehors de ce dépôt ou de `F:\ModLab`, sans demande préalable. (Source : `CLAUDE.md`.)
- Ne jamais contourner les protections de Nexus (téléchargements automatisés sans Premium, scraping). (Source : `CLAUDE.md`.)
- Code et commentaires en anglais ; documentation, ClickUp et textes d'interface en français. (Source : `CLAUDE.md`.)

## 4. Contenu

Utilisateurs :

Trois niveaux d'usage cohabitent dans la même application : Débutant (installer, mettre à jour, lancer, réparer), Avancé (conflits, dépendances, ordre de chargement, journaux), Expert (fichiers, graphe de dépendances et de conflits, console, API, MCP, commandes avancées). (Source : `docs/01-architecture-decidee.md`.)

Modules présents dans le code :

`modlab-core`, `modlab-cli`, `modlab-ai`, `modlab-game-skyrim`, `modlab-game-minecraft`, `modlab-ui`. (Source : `Cargo.toml`.)

## 5. Questions ouvertes

- Hypothèse à valider : le module MCP n'est pas commencé, car aucun crate MCP n'existe dans le dépôt (déduit du contenu de `Cargo.toml`).
- À compléter : le public visé au-delà de l'usage personnel.

## 6. Références

À compléter.
