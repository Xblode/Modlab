---
metadata_version: 1
title: Audit initial du projet ModLab
type: report
description: Première lecture du projet existant, avec vision, fonctionnement par dossier maître, écarts candidats et propositions.
summary: Audit en lecture seule, sans exécution du build ni des tests. Le projet suit une architecture verrouillée autour d'un moteur Rust unique. Les écarts relevés sont des candidats et les pistes des propositions, à valider par l'humain.
status: draft
created: "2026-11-06"
updated: "2026-11-06"
scope:
  level: project
  targets: [modlab]
updated_from:
  - "CLAUDE.md"
  - "AGENTS.md"
  - "docs/01-architecture-decidee.md"
tags: [audit, modlab]
---

# Audit initial du projet ModLab

Marqueurs : `confirmé` (source citée), `hypothèse` (déduction), `à compléter` (information absente).

## 1. Vision du projet

- Gestionnaire de mods personnel, Skyrim d'abord et multi-jeux, positionné « Cursor pour le game modding » (`confirmé` : `CLAUDE.md`, `docs/01-architecture-decidee.md` §1).
- Un seul moteur, plusieurs clients : le Core Rust détient tout l'état, l'UI, la CLI et le MCP en sont clients (`confirmé` : `docs/01-architecture-decidee.md` §3).
- Le Core ne connaît ni jeu (`GameAdapter`) ni plateforme (`Provider`) (`confirmé` : même document, §3).
- Toute mutation suit le cycle plan, approbation, snapshot, application, vérification, puis commit ou retour arrière automatique, avec une corbeille interne (`confirmé` : même document, §5).
- L'IA passe toujours par MCP, puis les permissions à 3 niveaux, puis le Core (`confirmé` : même document, §7.2).
- Stack verrouillée : Rust, SQLite à écrivain unique, React, TypeScript, Vite et Tauri ; données dans `F:\ModLab`, hors dépôt (`confirmé` : même document, §2 et §8).
- Risque nommé : pas d'équivalent Rust à Mutagen, plan en trois paliers à partir de xEdit (`confirmé` : même document, §4).
- Le dépôt n'a aucun dépôt distant configuré (`confirmé` : `.git/config`).
- L'avancement correspond à la Phase 2 de la feuille de route (`hypothèse` : fichiers présents et ADR 0005). L'état d'avancement n'est pas écrit (`à compléter`).

## 2. Fonctionnement par dossier maître

- **`crates/modlab-ui`** (315 fichiers, dont 298 sous `.modlab-dev/`) : hôte Tauri 2 de l'interface ; navigateur intégré en second WebView2, car Nexus envoie `X-Frame-Options` (`confirmé` : `Cargo.toml`). `src/lib.rs` fait 2 040 lignes et porte probablement les commandes exposées à l'interface (`hypothèse`). `.modlab-dev/` est ignoré par git (`confirmé` : `.gitignore`).
- **`ui`** (90 fichiers) : React 19, Vite 7, TypeScript 5.9 (`confirmé` : `ui/package.json`) ; 10 écrans dans `src/app/`, composants en atomic design, 6 hooks. Le rendu par jeu suit les capacités déclarées par les adaptateurs (`confirmé` : ADR 0005). Aucun test d'interface, prévu plus tard (`confirmé` : `CLAUDE.md`).
- **`docs`** (24 fichiers) : référence verrouillée, idées, 5 ADR, maquette HTML, plan du shell. Deux documents dépassés sont conservés comme contexte (`confirmé` : `CLAUDE.md`).
- **`img`** (14 fichiers) : images de jeux et captures (`hypothèse` : noms de fichiers), avec des doublons apparents dans `docs/design/`. Usage non vérifié (`à compléter`).
- **`crates/modlab-core`** (10 fichiers) : archives, conflits, téléchargement, contrat d'adaptateur, loadouts, stockage, transactions, base SQLite et journal d'événements (`confirmé` : `lib.rs`). 66 tests comptés (`confirmé` : comptage, tests non lancés).
- **`fixtures`** (10 fichiers, README existant) : faux Skyrim et faux Minecraft, avec des fichiers de substitution en texte (`confirmé` : `fixtures/README.md`).
- **`crates/modlab-game-skyrim`** (8 fichiers) : moteur, FOMOD, réglages graphiques, installateur, import MO2, USVFS ; environ 55 tests (`confirmé` : modules et comptage).
- **`spikes`** (7 fichiers, README existant) : explorations jetables ; xEdit reste bloqué, faute de binaire (`confirmé` : `spikes/README.md`).
- **Hors plafond de 8 dossiers** : `crates/modlab-game-minecraft` (15 tests) et `crates/modlab-ai` (6 tests) (`confirmé` : scan et comptage). `crates/modlab-cli` n'est pas retenu par le scan et n'a aucun test (`confirmé` : comptage).

## 3. Écarts candidats

- **E1. Front matter absent** sur 14 fichiers Markdown, dont `CLAUDE.md`, `docs/01-architecture-decidee.md` et les ADR (METADATA §4.2 et §6.1). Les documents dépassés n'ont donc pas de statut lisible par une machine (`confirmé` : scan).
- **E2. Nom hors convention** : `docs/Modlab — Document de cadrage & architecture.md` (NAMING §3.1, §3.2, §3.4). Ce nom est cité dans d'autres documents (`confirmé`).
- **E3. Casse du code et des dossiers d'interface** : PascalCase et camelCase, alors que NAMING §6.4 à §6.6 recommande le kebab-case par défaut. La convention du projet peut trancher (§6.5), mais elle n'est pas écrite (`à compléter`).
- **E4. `AGENTS.md` et `CLAUDE.md` sans renvoi l'un vers l'autre** : un agent qui ne lit que `AGENTS.md` ne voit pas les règles de ModLab (`hypothèse`).
- **E5. Équipe d'agents** : `CLAUDE.md` annonce 8 agents ; `.claude/agents/` contient aussi l'orchestrateur et les workers. Leur articulation n'est pas écrite (`à compléter`).
- **E6. Commentaire périmé** : l'en-tête de `crates/modlab-core/src/lib.rs` dit que `GameAdapter`, les transactions et le stockage ne sont pas encore là, alors que ces modules existent (`confirmé`). Aucun `Provider` repéré dans le Core (`hypothèse`).
- **E7. Référence périmée** : l'ADR 0005 cite `ui/src/Dashboard.tsx`, qui n'existe plus à cet endroit (`confirmé` : arborescence).
- **E8. Résidus possibles** : `tauri-dev.log` n'est pas ignoré par git (`confirmé` : `.gitignore`) ; `shared/` et `dist/` le sont (`confirmé` : `.gitignore`). Leur utilité reste à établir (`à compléter`).
- **E9. Chemins propres au poste** : le script `tauri:dev` contient `C:/Users/planc/...` (`confirmé` : `ui/package.json`).
- **E10. Deux dossiers d'images** : `img/` et `docs/design/`, sans règle écrite (`à compléter`).

## 4. Propositions priorisées

Haute :
- P1. Remplir les 5 documents de `brain/docs/` encore « À compléter » avec `/brain -o`, à partir de la référence verrouillée.
- P2. Relier `AGENTS.md` et `CLAUDE.md`.
- P3. Écrire l'état réel de la feuille de route.

Moyenne :
- P4. Ajouter les README manquants des dossiers maîtres.
- P5. Écrire la convention de casse de l'interface.
- P6. Ajouter un front matter aux références, document par document.
- P7. Corriger le commentaire du Core et la référence de l'ADR 0005.
- P8. Ajouter un test d'intégration de la CLI sur les fixtures.
- P9. Découper `crates/modlab-ui/src/lib.rs` et `crates/modlab-core/src/lib.rs` par domaine.

Basse :
- P10. Retirer le chemin absolu de `ui/package.json`.
- P11. Clarifier `tauri-dev.log`, `shared/`, les images en double et le `.docx`.
- P12. Décider du sort des documents dépassés.
- P13. Mettre à jour la section « Team » de `CLAUDE.md`.
- P14. Décider de la suite du blocage xEdit et du MCP : aucun crate MCP dans le dépôt (`hypothèse` : Phase 3 non commencée).

## 5. Manques et limites

- Build et tests non lancés : leur état réel est inconnu (`à compléter`). Les comptages viennent d'une recherche textuelle.
- Exclus par le scan : `.git`, `brain`, `brain-main`, `.claude`, `target`, `node_modules`, `dist`.
- Non ouverts : 26 fichiers de plus de 1 Mo et 48 fichiers binaires, ainsi que les données de `F:\ModLab`.
- Secrets : aucun fichier repéré d'après les noms ; un secret dans un fichier au nom ordinaire ne serait pas détecté.
- Lus en partie : `NAMING.md`, `METADATA.md`, ADR 0005, `plan-shell-v2.md`, `crates/modlab-core/src/lib.rs`.
- Non lus : `docs/00-cadrage-technique.md`, `docs/Modlab — Document de cadrage & architecture.md`, `docs/deep-research-report.md`, `docs/.clinerules.txt`, `docs/ModLab_Cahier_des_Charges.docx`, ADR 0001 à 0004, `docs/design/shell-maquette.html`, le corps des modules Rust, les composants React, `brain/docs/`.
- L'arbre de travail contient des modifications non commitées : cet audit lit l'état du moment (`hypothèse`).
