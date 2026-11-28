---
titre: Projet
description: Le cap concret du projet en une page, son périmètre et la carte de la doc. S'écrit au cadrage et quand le cap change.
maj: 2026-11-28
---

# Projet

## Pourquoi
Un seul endroit pour installer, régler et lancer ses jeux moddés, sans jamais rien faire en cachette. Le rêve et les principes qui le guident sont dans `VISION.md`.

## Pour qui
Benjamin et les utilisateurs avancés d'abord. L'interface reste simple pour qu'un débutant s'en serve aussi.

## Le problème
- Côté usage : MO2, LOOT, xEdit, CurseForge et Prism sont des outils séparés, avec beaucoup de travail à la main et des journaux à lire soi-même. (hypothèse)
- Côté code : l'app a grandi brique par brique. Beaucoup de choses marchent, mais la logique est éparpillée (la couche Tauri est devenue un second moteur), aucune ressource n'a un seul responsable de son cycle de vie (l'interface dit « supprimé », le disque non) et chaque jeu fait à sa façon. La revue d'architecture en cours règle ça avant d'ajouter quoi que ce soit.

## Le MVP
Skyrim SE et Minecraft fiables, de bout en bout (fin de la phase 2 de `ROADMAP.md`) :
1. Installer, activer, ordonner et retirer des mods, base et disque toujours d'accord.
2. Voir les conflits et les dépendances.
3. Lancer le jeu, tout étant annulable.

## Réussi quand
Benjamin joue un mois à Skyrim et à Minecraft sans ouvrir MO2, CurseForge ni Prism.

## Périmètre du gestionnaire (validé le 2026-11-28)
Ce que le gestionnaire doit savoir faire, en cible. Ce qu'il fait aujourd'hui : `PRODUIT.md`. L'ordre : `ROADMAP.md`.

- **Organisation** : jeu → instances → branches → révisions (façon commit). Une instance se crée vierge, se clone, s'exporte, se partage, se sauvegarde et se restaure. Import depuis d'autres outils : plus tard (l'import MO2 existe déjà).
- **Mods** : une bibliothèque centrale garde les archives et l'identité des mods (téléchargés une fois, sans doublon ; nom, version, version du jeu, URL). Un mod installé vit dans une instance ; la branche dit lesquels sont actifs ; le dossier du jeu reflète toujours la branche active. Un même mod est reconnu d'une version à l'autre. Supprimer un fichier garde une trace.
- **Mods en jeu** : activation, ordre, priorités, conflits ; dépendances manquantes, versions compatibles, proposition de correction, compatibilité jeu et loader.
- **Loaders et extensions** (SKSE, Forge, NeoForge, Fabric…) : cycle complet (installer, mettre à jour, désinstaller, archiver).
- **Fichiers du jeu** : détecter les modifications faites hors de ModLab, sauvegarder avant de remplacer, restaurer, réparer. Sauvegardes et configs gérées, synchronisation en option.
- **Lancement** : vérifier, préparer, régler, suivre la partie, nettoyer après.
- **Manifeste reproductible** : exporter une branche (versions exactes, URL, ordre, config) pour la recréer ailleurs.
- **Fiabilité** : prévisualiser avant d'agir, annuler, reprendre après un plantage, journal de tout, détection et réparation.
- **Tâches longues** : un seul moteur de tâches (file, progression, annulation, reprise).
- **Automatisation** : installations et mises à jour groupées, corrections proposées, modèles d'instance, vérifications périodiques.
- **Sources** : le navigateur intégré sert Nexus, CurseForge, Modrinth, GitHub, les sites officiels ou une URL donnée à la main, sans les concurrencer.
- **Dépendances système** (Java, .NET…) : seulement quand un jeu en a besoin.
- **Hors ligne d'abord**, données locales. Le cloud viendra peut-être plus tard.
- **Hors périmètre** : installer ou mettre à jour les jeux eux-mêmes (Steam, launcher Minecraft) ; on peut seulement changer la version de Minecraft d'une instance.
- **Priorités techniques** : fiabilité, performance et extensibilité, sans sacrifier l'une des trois. La compatibilité avec d'autres outils (MO2) est un plus, jamais au prix des performances. Un nouveau jeu s'ajoute par un adaptateur, à terme.

## Pas maintenant
Tout ce qui vient après la phase 2 dans `ROADMAP.md`. La ligne de commande, MCP, les plugins et l'IA attendent que le gestionnaire de mods soit stable. On ne touche pas au design de l'interface pendant le cadrage.

## Principes de travail (depuis le 2026-11-28)
1. Regard neuf : aucune architecture, décision ou fonctionnalité n'est intouchable. Toute proposition qui casse l'existant est signalée comme telle.
2. Code propre : responsabilités séparées, logique regroupée, pas de duplication inutile.
3. Fonctionnalités complètes : cycle de vie, erreurs, interactions et limites pensés avant d'ajouter un système.
4. Standards professionnels adaptés à un gestionnaire de mods.
5. Stabilité avant extension.
6. Progression maîtrisée : par domaines cohérents, sans régression, en gardant ce qui marche.

À éviter : fonctionnalités dispersées dans plusieurs couches sans raison ; exceptions propres à un jeu dans le Core ; systèmes parallèles qui font la même chose ; choix faits seulement pour que ça marche vite ; docs qui ne collent plus au produit ; corrections locales qui aggravent l'ensemble.

Méthode : monolithe modulaire, découpé par domaines (DDD) et en ports/adaptateurs (hexagonal), sans dogme. Chaque ressource a un seul responsable dans le Core, qui garantit tout son cycle de vie. Avant chaque changement : « où ça doit vivre, et est-ce que ça existe déjà ? ». **Le code fait foi** : si la doc dit autre chose, on corrige la doc.

## Où est quoi
Toute la doc vit dans `.bframe/`, en français. Une information n'est écrite qu'à un seul endroit.

| Fichier | Contenu |
| --- | --- |
| `PROJET.md` | Le cap concret, le périmètre, les principes, cette carte |
| `VISION.md` | Le rêve à long terme et les grandes directions |
| `PRODUIT.md` | Ce que fait l'application aujourd'hui, ses concepts et ses écrans |
| `ARCHITECTURE.md` | L'architecture réelle (carte C4 en 3 zooms), ses contraintes et ses écarts avec la cible |
| `DESIGN.md` | Les règles d'interface et le design system |
| `ROADMAP.md` | Les grandes étapes, sans les tâches |
| `DECISIONS.md` | Le journal des décisions (il sert aussi d'ADR) |
| `ETAT.md` | Où en est chaque sujet, la prochaine action |
| `IDEES.md` | Les idées, rangées par sujet |
| `TACHES.md` | Les tâches : maintenant, ensuite, plus tard |
| `archive/` | Les anciens docs, gardés tels quels, lus seulement sur demande |

Hors de `.bframe/` : seulement `CLAUDE.md`, `AGENTS.md` et un README par dossier de la racine (plus celui de la racine), tous en français.
