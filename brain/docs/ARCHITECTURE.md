---
metadata_version: 1
title: "Architecture"
type: specification
description: "Comment le projet est organisé et conçu."
version: "0.2.0"
status: draft
created: "2026-11-06"
updated: "2026-11-06"
updated_from:
  - "docs/01-architecture-decidee.md"
  - "brain/sources/audits/20261106-1108.md"
---

# Architecture

## 1. Objet

Comment le projet est organisé et conçu.

## 2. État actuel

Seules les informations confirmées figurent en section 3 ; rien n'est inventé.

## 3. Informations confirmées

Outils :

Cargo, Vite, Tauri, ClickUp, agents Claude Code dans .claude/agents/

Référence :

`docs/01-architecture-decidee.md`, verrouillé le 15 septembre 2026, est la référence unique. En cas de désaccord, ce document prévaut sur ce résumé.

Principe central :

Un seul moteur, plusieurs clients. L'UI, la CLI et le serveur MCP parlent à la même API du Core Rust, par IPC Tauri ou JSON-RPC local. Aucun client n'invente son propre état. (Source : §3.)

Stack (source : §8) :

- Core : Rust.
- Interface : React, TypeScript et Vite, dans un shell Tauri (WebView2 sous Windows).
- Base de données : SQLite en mode WAL, avec le Core comme seul écrivain.
- Déploiement Skyrim : USVFS via FFI Rust, derrière l'adaptateur de jeu.
- Tri de l'ordre de chargement : libloot.
- Plateformes de mods : un Provider par plateforme, avec les API officielles.

Extension (source : §3) :

- Un Game Adapter porte les règles d'un jeu : identification, structure, modding, installation, diagnostic, lancement, outils.
- Un Provider porte une plateforme de mods : recherche, projets, versions, fichiers, dépendances, téléchargement, métadonnées.

## 4. Contenu

Données (source : §5) :

- Racine : `F:\ModLab`, avec le stockage par contenu, les téléchargements, les jeux, la base, la corbeille et les journaux.
- Trois idées structurantes : stockage par contenu (CAS), révisions immuables avec journal d'événements, loadout sérialisé en fichiers versionnés dans un dépôt git interne.

Organisation du dépôt (source : `brain/sources/audits/20261106-1108.md` §2) :

- `crates/modlab-core` : archives, conflits, téléchargement, contrat d'adaptateur, loadouts, stockage, transactions, base SQLite et journal d'événements.
- `crates/modlab-game-skyrim` : moteur, FOMOD, réglages graphiques, installateur, import MO2, USVFS.
- `crates/modlab-game-minecraft` : adaptateur Minecraft.
- `crates/modlab-ai` : couche IA.
- `crates/modlab-cli` : ligne de commande.
- `crates/modlab-ui` : hôte Tauri de l'interface.
- `ui` : interface React.
- `fixtures` : faux Skyrim et faux Minecraft pour les tests.
- `spikes` : explorations jetables.

## 5. Questions ouvertes

- Risque connu : aucun équivalent Rust mature à Mutagen pour lire et écrire les records Bethesda. Plan : orchestrer xEdit, puis évaluer un parseur Rust en Phase 4 ou plus, avec un sidecar .NET en repli. (Source : `docs/01-architecture-decidee.md` §4.)
- À suivre : l'orchestration xEdit reste bloquée faute de binaire. (Source : `spikes/README.md`, d'après l'audit.)
- Hypothèse à valider : aucun Provider ne serait présent dans le Core à ce jour (déduit de la liste des modules, à vérifier).

## 6. Références

À compléter.
