---
titre: Feuille de route
description: Les grandes étapes de ModLab et la condition pour passer à la suivante ; on la met à jour quand une étape se termine ou change d'ordre.
maj: 2026-11-28
---

# Feuille de route

Étapes seulement, pas de tâches (voir `TACHES.md`). Où on en est : `ETAT.md`. Principe : stabilité avant extension (voir `PROJET.md`).

## Fait

- **Phase 0, fondations** (15 sept.) : Core Rust, CLI, essais techniques (USVFS, archives, IA locale, MCP).
- **Phase 1, Skyrim jouable** (15 sept.) : adaptateur Skyrim, import MO2, déploiement USVFS, lancement SKSE, conflits de fichiers.

## En cours

- **Phase 2, MVP « Skyrim et Minecraft fiables »** : interface, installation, conflits, lancement, tout annulable. En trois lots (où en est chacun : `TACHES.md`) :
  - Lot 1, Skyrim vraiment jouable.
  - Lot 2, socle multi-jeux : contrat de jeu étendu, code Skyrim sorti de l'hôte, toutes les actions dans le cycle sûr.
  - Lot 3, Minecraft utilisable.
- **Revue d'architecture** (depuis le 28 nov.) : mettre la doc en accord avec le code, puis traiter les écarts de `ARCHITECTURE.md` §7 un par un, avec validation de Benjamin avant tout changement qui casse l'existant. Les autres sujets sont en pause pendant la revue.

## Ensuite

Condition d'entrée commune : le gestionnaire de mods est stable (installer, activer, voir les conflits, lancer, annuler) et la revue d'architecture est rendue.

- **Gestionnaire stabilisé** : fin de la phase 2, un mois sur Skyrim et Minecraft sans autre outil (critère de `PROJET.md`).
- **Diagnostic et maintenance** : vérifier une instance, détecter les incohérences, analyser dépendances et conflits, repérer les erreurs, proposer des réparations guidées. Pensé dès le départ pour être exposé plus tard à la CLI, au MCP et à l'IA, sans les développer. Entrée : gestionnaire stabilisé.
- **CLI, MCP, plugins et chat IA** : la CLI et l'IA locale (présentes, gelées) reprennent sur le même Core que l'interface ; un serveur MCP avec permissions à trois niveaux. Entrée : diagnostic en place, plan et approbation visibles par l'humain.
- **Délégation à l'IA** : l'IA enchaîne elle-même des actions de maintenance, toujours sous les permissions de ModLab. Entrée : MCP en place.
- **Analyse fine des plugins** : conflits au niveau des records, tri LOOT, xEdit. Entrée : MCP en place, et un vrai besoin constaté à l'usage (voir le risque Mutagen, `ARCHITECTURE.md` §8).
- **Superpouvoirs** : bisect, recettes, base de connaissances. Entrée : analyse des plugins et journal fiable. Détail dans `VISION.md`.
- **Autres jeux** (Witcher 3 et suivants) : Entrée : ajouter un jeu ne change plus le Core (écarts E1 à E5 de `ARCHITECTURE.md` traités).
- **Communauté** (profil, boutique, atelier de patchs) : seulement des écrans d'essai aujourd'hui. Entrée : cadrage fait avec Benjamin (voir `ETAT.md`).
