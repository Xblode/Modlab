---
titre: Décisions
description: Les choix validés par l'humain, du plus récent au plus ancien.
maj: 2026-11-13
---

# Décisions

Une ligne par décision validée. Les plus récentes en haut. Les décisions techniques d'avant bFrame sont dans `docs/01-architecture-decidee.md` et `docs/adr/`.

| Date | Sujet | Le choix | Pourquoi | Écarté |
| --- | --- | --- | --- | --- |
| 2026-11-13 | Enregistrement | Corriger 2 lignes mortes de l'UI et la mise en forme Rust, puis 4 commits séparés et push sur `origin/master` | Respecter « build et fmt doivent passer » ; pouvoir annuler un commit seul | Commit sans corrections ; commit sans push |
| 2026-11-13 | Direction communauté | Le rapport communauté / profils / place de marché / économie est une vraie direction, à cadrer | Choix de Benjamin | Simple piste ; archive |
| 2026-11-13 | Rangement des docs | Arrêter ClickUp : `.bframe/TACHES.md` est la seule liste de tâches | Double saisie qui a dérapé en septembre | Garder les deux |
| 2026-11-13 | Rangement des docs | Reprendre dans CLAUDE.md les règles de vérification de Cline (fmt, clippy, tests, build, commits normalisés), puis archiver le fichier Cline | Déjà suivies dans les commits | Archiver sans rien reprendre |
| 2026-11-13 | Rangement des docs | `.bframe/` reste court (pilotage, liens) ; `docs/` garde le détail ; les docs dépassés vont dans `docs/archive/`, jamais supprimés ; une seule liste d'idées dans `.bframe/IDEES.md` | Reprise de session légère, rien perdu | Tout dans `.bframe/` |
| 2026-11-13 | Nettoyage de l'ancien 2brain | Nettoyer ses traces après l'onboarding, liste validée avant | Éviter que des sessions suivent de vieilles consignes | Laisser tel quel |
| 2026-11-13 | Cadrage | La copie en ligne (« origin ») est voulue ; toujours aucun push sans demande | Choix de Benjamin | Pas de dépôt distant |
| 2026-11-13 | Cadrage | Réussi quand : un mois de Skyrim sans ouvrir MO2 | Simple à constater, colle au MVP | — |
| 2026-11-13 | Cadrage | MVP = « je n'ouvre plus MO2 » : installer/activer, voir les conflits, lancer, tout annulable | C'est le jalon de Phase 2 | MVP plus petit, MVP centré IA ou Minecraft |
| 2026-09-18 | Interface | Le shell est piloté par les « capacités » de chaque jeu (onglets, outils) fournies par l'adaptateur | Le Core et l'UI ne connaissent pas un jeu précis | Onglets codés en dur dans l'UI — détail : `docs/adr/0005-*` |
| 2026-09-18 | IA locale | Binding `llama-cpp-2` | Compile et se lie, voir spike | `candle` — détail : `docs/adr/0004-*` |
| 2026-09-15 | Import MO2 | Import en lecture seule, par référence, sans passer par le stockage interne (CAS) | Ne jamais toucher l'instance MO2 | Copie dans le CAS — détail : `docs/adr/0003-*` |
| 2026-09-15 | Conflits de fichiers | Le dernier mod de la séquence gagne | Confirmé par le vrai comportement d'USVFS | — ; sens de l'ordre MO2 encore ouvert — détail : `docs/adr/0002-*` |
| 2026-09-15 | Déploiement | USVFS par défaut, appelé directement depuis Rust | La DLL expose une interface C propre | Liens physiques en plan B — détail : `docs/adr/0001-*` |
| 2026-09-15 | Architecture | Licence GPL-3 | Cohérent avec MO2, Wabbajack, Mutagen | — |
| 2026-09-15 | Architecture | Disque compatible MO2 + stockage interne par contenu (CAS) derrière | Exportable vers MO2, pas de doublons | — |
| 2026-09-15 | Architecture | L'interface suit le style de la maquette HTML, sans refonte ; toute évolution passe d'abord par la maquette | La maquette est la source visuelle | Refonte visuelle |
| 2026-09-15 | Architecture | Analyse fine des plugins : piloter le vrai xEdit en v1 | Pas d'équivalent Rust à Mutagen | Parser Rust maison, Mutagen en C# (à revoir en phase 4) |
| 2026-09-15 | Architecture | Rust + Tauri + React, données dans `F:\ModLab` | Voir `docs/01-architecture-decidee.md` §2 | C# / .NET / Mutagen |
