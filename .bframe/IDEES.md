---
titre: Idées
description: Les idées de l'humain, rangées par sujet.
maj: 2026-11-28
---

# Idées

Une ligne datée par idée, sous son sujet. Une idée n'est pas une décision. Les grandes directions (superpouvoirs, communauté, IA) sont dans `VISION.md`. Sources d'origine, en archive : `idees.md` (B1–B20, A1–A3), `00-cadrage-technique.md` §14 (P1–P15), `faisabilite.md`.

## Outils du jeu (LOOT, indicateurs)

- 2026-09-14 : Indicateurs SKSE, ENB, ReShade, Community Shaders, upscalers et génération d'images (DLSS, FSR, XeSS, OptiScaler), par signatures de fichiers, DLL et clés INI : un catalogue versionné `capacité → fichiers`. Rien n'est affiché si rien n'est détecté ; une DLL inconnue est signalée plutôt que devinée ; cohérence avec la carte graphique (ex. DLSS FG demande une RTX 40). Limite : les mods PureDark (Patreon) n'ont que leurs fichiers comme indice. (A1)
- 2026-09-14 : LOOT intégré en 3 niveaux : 1) piloté (lancer `LOOT.exe --auto-sort` dans l'environnement de la branche, relire `plugins.txt`) ; 2) dans le processus via `libloot` (tri et messages dans l'onglet Plugins, sur une branche jetable) ; 3) métadonnées seules (masterlist YAML : prérequis, incompatibilités, sales éditions). Recommandé : 1 et 3 d'abord, 2 quand le moteur est stable. Même bouton « Trier avec LOOT » quel que soit le niveau. (A2)

- 2026-09-15 : Outils à orchestrer : Skyrim — Nemesis/Pandora, DynDOLOD/TexGen/xLODGen, BodySlide, Wrye Bash, Crash Logger SSE/Trainwreck ; Witcher 3 — Script Merger, REDmod.

## Rangement des docs et migration vers bFrame

- 2026-11-27 : Pour bFrame (hors ModLab) : un `worker-architect`, garde-fou de cohérence court et indépendant du langage, qui vérifie qu'un changement respecte l'architecture d'ensemble. Proposé par ChatGPT, pas tranché.
- 2026-11-28 : Si l'extension de bFrame faite pour ModLab marche (VISION, PRODUIT, ARCHITECTURE, DESIGN, ROADMAP en plus des 5 boîtes), l'ajouter comme fonctionnalité de bFrame.
- 2026-11-13 : faire de ce rangement (nettoyer un ancien système, ranger les docs, ne rien perdre) un module ou skill bFrame réutilisable, à partir du rapport de cette première fois.

## Pendant le jeu

- 2026-09-15 : Mode Théâtre : overlay réduit pendant le jeu (FPS, révision, chrono, marquer un bug). (B1)
- 2026-09-15 : Marque-page en jeu : une touche capture image, position et révision ; carte « à investiguer » dans Santé. (B2)
- 2026-09-15 : Journal de session lisible : durée, zones, avertissements Papyrus, crash ; « demander à Claude » pré-rempli. (B15)
- 2026-09-14 : Modes de lancement : stable (verrouillé), labo (branche), bench (enregistrement + PresentMon). (P4)
- 2026-09-14 : Raccourci « Jouer » global (touche dédiée, Stream Deck) qui lance le loadout actif. (P15)

## Performance et matériel

- 2026-09-15 : Profil matériel : détection GPU/CPU/RAM/VRAM, presets cohérents, alerte « dépasse ta VRAM ». (B3)
- 2026-09-15 : Coût FPS par mod : bench A/B qui attribue un coût aux mods lourds. (B5)
- 2026-09-14 : ENB / ReShade gérés comme « mods racine », avec profils de presets. (P13)

## Branches, révisions et sauvegardes

- 2026-09-15 : Sandbox en un clic : « Essayer ce mod » sur une branche jetable, puis Garder ou Jeter. (B4)
- 2026-09-15 : Sauvegardes liées aux révisions : alerte si une save est chargée avec une autre branche. (B8)
- 2026-09-14 : Comparateur de loadouts côte à côte (plugins, INI). (P5)
- 2026-09-14 : Timeline visuelle avec marqueurs de crashs, sessions et mises à jour. (P10)

## Installation et fichiers

- 2026-09-14 : Installation intelligente (« je donne l'archive, ça va au bon endroit ») : analyser l'archive et montrer un plan de placement avant d'écrire, y compris les fichiers de la racine du jeu (SKSE, ENB, `d3d11.dll`) que MO2 ne gère pas sans Root Builder. Détection : fichiers racine connus, `Data/` et ses sous-dossiers, FOMOD, BAIN, archive « nue ». Tout ce qui va à la racine est enregistré (provenance, hash) pour désinstaller proprement et repérer les écrasements. (A3)
- 2026-09-15 : Résolveur visuel de textures : comparer gagnant et perdant, règle gardée par branche. (B7)
- 2026-09-15 : Surveillance du dossier Téléchargements : archive reconnue par hash, plan de placement proposé. (B20)
- 2026-09-15 : Migration guidée depuis MO2 / Vortex / Prism, jamais destructive, ancien outil utilisable en parallèle. (B17)
- 2026-09-14 : « Pourquoi ce fichier ? » : clic droit sur un fichier du jeu → mod, archive, révision, qui l'écrase. (P2)
- 2026-09-14 : Détecter les fichiers « fantômes » du dossier du jeu et proposer de les adopter. (P12)
- 2026-09-14 : Page mod façon README GitHub : image Nexus, description, changelog, mes notes, mes choix FOMOD. (P1)

## Métadonnées

- 2026-11-25 : Métadonnées via l'API officielle Nexus, en option dans Réglages avec une clé personnelle de l'utilisateur, désactivée sans clé ; interrogée seulement pour les mods déjà téléchargés, sans téléchargement automatique. Remplit nom, version, catégorie, auteur. Reportée par l'utilisateur : local d'abord. Même principe possible plus tard pour CurseForge (clé) et Modrinth (API publique).

## Réglages

- 2026-11-13 : Éditeur clé/valeur des fichiers de config d'un mod, avec différences et surcharge par branche ; « preset » = branche + canal + outils + réglages de lancement.
- 2026-09-15 : Réglages transversaux : un curseur ModLab qui écrit dans plusieurs INI/JSON, tracé et réversible. (B18)
- 2026-09-14 : INI as tokens : réglages INI en panneau nommé, avec diff par loadout. (P8)

## Mises à jour et maintenance

- 2026-09-15 : Épingles de version avec raison : ne pas mettre à jour tant que X n'est pas compatible. (B6)
- 2026-09-15 : Mise à jour sûre : lit le changelog Nexus, repère « nouvelle partie requise », propose une branche de test. (B9)
- 2026-09-15 : Nettoyage d'espace avec preuves (archives, sorties, blobs orphelins), via la corbeille interne. (B10)
- 2026-09-15 : Rappels de maintenance : « 12 mods depuis le dernier DynDOLOD », « 30 jours sans LOOT ». (B14)
- 2026-09-14 : Score de risque par mod (DLL SKSE, version du jeu, âge, conflits) avant installation. (P3)
- 2026-09-14 : Digest nocturne : nouvelles versions Nexus, changelogs résumés, « sans risque » ou « à tester ». (P7)
- 2026-09-14 : Mode hors ligne complet, avec cache des pages Nexus déjà vues. (P14)

## Diagnostic et IA

- 2026-09-15 : IA locale cible : modèle par défaut Qwen2.5-1.5B (≈ 1 Go) ; au choix Llama-3.2-3B, Phi-3.5-mini ; Qwen3 8B suggéré si la carte a 8 Go de VRAM (plafond 8-9B, au-delà ça déborde en RAM). Chargé à la demande, déchargé après inactivité, jamais pendant une partie : une demande faite en jeu attend la fin de la partie. Grammaire contrainte (GBNF) pour que le modèle n'invente aucun fait.
- 2026-09-15 : Outils MCP cibles, par niveau : lecture ; modification réversible ; action dangereuse ; et transactionnels (`modlab_plan`, `modlab_apply`, `modlab_history`).

- 2026-09-15 : Mode « debugger de problèmes » : parcourir les problèmes comme des points d'arrêt (Appliquer / Ignorer / Noter). (B13)
- 2026-09-15 : Assistant de premier lancement : lit le crash log et les temps de chargement, propose l'étape suivante. (B19)
- 2026-09-14 : Claude accueille un mod : lit README et FOMOD, propose les choix, explique les prérequis manquants. (P6)
- 2026-09-14 : Palette Ctrl+K intelligente : « désactive tous les mods de météo » → Claude construit le plan. (P9)

## Partage (voir aussi Direction communauté)

- 2026-09-15 : Collection « as code » partageable : export texte d'une branche, sans assets (Wabbajack-lite). (B12)
- 2026-09-14 : Export « carte de visite » d'un loadout (image ou markdown). (P11)
- 2026-09-15 : Tableau de bord « ma vie de moddeur » : heures par branche, crashs / 10 h, mods par mois. (B11)

## Direction communauté

- 2026-11-06 : Profils, modpacks, patches, place de marché, économie : la direction est dans `VISION.md`, l'étude complète en archive (`deep-research-report.md`, dossier `communaute/`). Direction validée, pas encore cadrée.

## Minecraft

- 2026-09-15 : Une branche par serveur : mods du modpack, version de Java, RAM et JVM par branche. (B16)

## Table des mods

- 2026-09-15 : repenser un jour la table des mods façon Asana / ClickUp ; garder la table actuelle de la maquette en attendant.

## Les 8 agents spécialisés

- 2026-11-24 : les workers bFrame vivent comme sous-agents Cursor du compte, mêmes règles quel que soit le modèle. L'orchestrateur reste le chat, pas un sous-agent.

## Sans sujet

- (vide)
