---
titre: Tâches
description: Ce qu'il y a à faire, du plus proche au plus lointain.
maj: 2026-11-25
---

# Tâches

Une case par tâche, avec son sujet entre parenthèses.

## Maintenant

Refactor 2026-11-25 :
- [x] 1. Ménage : UI (code mort, classes CSS mortes) et Rust (thiserror retiré de modlab-ui, serde_json en dev-dependency, [workspace.dependencies], en-tête core, .gitignore sans src-tauri, test archive perso retiré) (fait 2026-11-25)
- [x] 2. Factorisation UI : Dialog commun, table canaux unique, ligne de chemin unique Réglages/Mon jeu, Button/états vides (fait 2026-11-25)
- [x] 3. Un seul calcul de chemins + même racine CLI/UI : `fuse_path` unique ; `mods_location` / `config_location` sur GameAdapter ; `resolve_data_root` partagé CLI et UI (MODLAB_DATA_ROOT, sinon F:\ModLab si la base est là, sinon .modlab-dev) (fait 2026-11-25)
- [x] 4. Source unique de détection SKSE (fait 2026-11-25)
- [x] 5. Découpage mécanique de modlab-ui/src/lib.rs en commands/ (mods, paths, loadouts, browser, downloads, settings, launch, assistant, shell) (fait 2026-11-25)
- [ ] 6. Sécurité fichiers : install et désinstall en transaction (fait 2026-11-25) — dossier de mod renommé dans `<racine>/trash/`, install préparé à côté puis rename, rollback si échec. Lancement SKSE (copie, USVFS, plugins.txt) encore hors cycle ; `sync_engine_mod_direct_install` aussi
- [ ] 7. (reporté) Types TS générés depuis Rust

Mon jeu — livrable 1 :
- [x] 2026-11-25 : Mémoire et décisions (Mon jeu)
- [x] 2026-11-25 : Chemins Core (`game_paths` / getGamePaths) : jeu, mods installés, téléchargements, configs/.ini (Mon jeu)
- [x] 2026-11-25 : Détection SKSE Core : par le nom et le site, plus `open_external_url` ; install par instance, comme un mod ; pas de bibliothèque centralisée (Mon jeu)
- [x] 2026-11-25 : UI boutons outils : SKSE et LOOT — installé → actif ; absent → grisé, clic = page officielle ; après téléchargement, détection → Mods → install dans l'instance (Mon jeu)
- [x] 2026-11-25 : UI chemins branchés sur le Core (Mon jeu)
- [x] 2026-11-25 : Scan vanilla Core (`game_vanilla_scan`) : lecture seule, lister les excès et les configs à part, aucun nettoyage automatique (Mon jeu)
- [x] 2026-11-25 : Brancher l'UI vanilla sur `game_vanilla_scan` (Mon jeu)
- [x] 2026-11-25 : Contrôle vanilla : résumé et bouton Nettoyer, plan des extras Data, sans effacement (Mon jeu)

Hors de ce livrable : versioning de la bibliothèque, centralisation de SKSE.

Refonte de la barre de gauche :
- [x] 2026-11-24 : Étape 1 : composants communs (bouton icône en deux tailles, ligne de barre, titre de section sans chevron, pastille de canal)
- [x] 2026-11-24 : Étape 2 : haut de la barre (nom, Jouer, Partager)
- [x] 2026-11-24 : Étape 3 : sections et arbre des jeux
- [x] 2026-11-24 : Étape 4 : retirer « aperçu navigateur » du profil
- [x] 2026-11-24 : Étape 5 : tableau de bord par instance
- [x] 2026-11-24 : Grille de 9 pixels dans le pied de la barre pendant qu'une instance tourne
- [ ] Regarder la barre et le tableau de bord à l'écran, puis commit si ça va (Refonte de la barre de gauche)

Lot 1 — Skyrim vraiment jouable (Bilan de la Phase 2) :
- [x] 2026-11-24 : Vérifier sur le web l'ordre de `modlist.txt` de MO2, `plugins.txt`, SKSE. Résultat : dans MO2 la 1re ligne de `modlist.txt` gagne les conflits, donc l'import ModLab est inversé (à corriger avec l'ordre des mods)
- [x] 2026-11-24 : Corriger l'installation des archives qui contiennent un dossier `Data/`, avec un test (`place_content` dans `installer.rs`)
- [x] 2026-11-24 : Écrire `plugins.txt` (ordre + actifs) pour l'instance, lié par USVFS comme MO2 ; page Plugins montre l'ordre réel (`plugins.rs`, bibliothèque `esplugin` de LOOT). Liaison réelle non testée sans le jeu
- [x] 2026-11-24 : Réparer SKSE : détection par les fichiers et la version, plugins SKSE par USVFS, copie à la racine avec sauvegarde, refus clair, lancement non bloquant, bilan après partie (`skse.rs`, `engine.rs`). Non testé en jeu
- [x] 2026-11-24 : Réordonner les mods (cycle sûr, glisser-déposer, Alt+flèches) et corriger l'import MO2 inversé. Les instances importées avant gardent l'ordre inversé : ré-importer
- [x] 2026-11-24 : Contre-vérification du lot 1 (aucun défaut bloquant) ; version de SKSE vérifiée avant toute copie
- [ ] Essai en jeu par Benjamin : lancer depuis ModLab, lire le bilan SKSE et la page Plugins. Bloqué tant que `usvfs_x64.dll` (MO2) et le disque F: ne sont pas trouvés sur ce PC
- [ ] Vérifier le nom `VirtualLinkFile` sur la vraie `usvfs_x64.dll` et traduire l'erreur en français (Bilan de la Phase 2)
- [ ] Réinstaller les mods installés avant le 24 nov. (rangés en `Data/Data`) et ré-importer les instances MO2 (ordre inversé) (Bilan de la Phase 2)

Look de l'interface (unslop) :
- [x] 2026-11-24 : Commit local du lot 1 après vérifications (`dd951f5`, `7b33f40`) ; tri des versions SKSE dans le message d'erreur
- [x] 2026-11-24 : Installer unslop pour ce projet ; passe unslop sur `ui/src/` (espacements, contrastes, tailles de texte, icônes, rayons, animations), build OK
- [x] 2026-11-24 : Recherche (règles des logiciels + études) et document `docs/design/regles-interface.md`
- [x] 2026-11-24 : Densité confortable + réglage Compact (Réglages → Apparence), texte 14/12, finitions shadcn
- [x] 2026-11-24 : Un seul bouton principal, infobulles avec raccourcis réels (composant `Tooltip`), états vides courts, barre de statut jeu / instance
- [x] 2026-11-24 : Commit et push de l'interface, des docs et du lot 1 (`dd951f5` à `e7de272` sur `origin/master`)
- [ ] Brancher le choix de thème Clair/Sombre/Auto et « Taille de police » des Réglages (boutons sans effet aujourd'hui)
- [ ] Raccourci « ⌃Q » affiché pour Quitter sans gestionnaire ; bouton « Console » du composeur sans action ; « Skyrim en cours » écrit en dur dans la barre de statut (`SidebarFooter.tsx`)
- [ ] Raccourcir les encadrés « Ce que le Core ne suit pas encore » (tableau de bord) et l'explication des Mondes
- [ ] Vérifier les écrans non vus : thème clair, Minecraft, Santé, Sauvegardes, dialogues
- [x] 2026-11-24 : CLAUDE.md : l'appli `ui/` est la référence visuelle, la maquette devient un historique
- [ ] Aligner sur cette règle : `docs/01-architecture-decidee.md` (l.15, 53, 246, 325, 327), `docs/README.md`, `docs/design/design-system.md`, `.bframe/PROJET.md`
- [ ] Icône soleil pour le thème « Clair » (aujourd'hui une étincelle) ; police dédiée (Geist ou IBM Plex Sans) seulement si Benjamin le veut

Défauts relevés par la contre-vérification, pour le lot 2 (Bilan de la Phase 2) :
- [ ] Fermer ModLab pendant une partie : demander confirmation, et retrouver la session au redémarrage (`lib.rs` `launch_skse`)
- [ ] `_ResourcePack.esl` : confirmer s'il est implicite comme les officiels (`plugins.rs`)
- [ ] Chemin `Plugins.txt` selon l'édition (Steam, GOG, Epic)
- [ ] Manifeste SKSE : garder la plus ancienne sauvegarde ; ne retirer l'enregistrement qu'après restauration ; verrou commun case à cocher / lancement
- [ ] `import_mods` dans une instance non vide : positions en double

## Ensuite

- [ ] Lot 2 — socle multi-jeux : contrat de jeu étendu, code Skyrim sorti de l'app, corbeille réelle, toutes les actions dans le cycle sûr (Bilan de la Phase 2)
- [x] 2026-11-26 : Création d’instance Minecraft — modal nom + cascade MC/loader/version, unicité, ensure vanilla avant loader non-vanilla (Minecraft utilisable)
- [x] 2026-11-26 : Lot 3 partiel — case activer Minecraft : rename `*.jar` ↔ `*.jar.disabled` (`instance::set_mod_enabled`), UI `set_mod_enabled` branchée (Minecraft utilisable)
- [ ] Lot 3 — reste : mods synchronisés, navigateur Modrinth. Install drop-in jar déjà en place (décision 2026-11-25) (Minecraft utilisable)
- [ ] Gérer les fichiers posés à côté de `Data/` dans une archive (ENB, `d3d11.dll`) : aujourd'hui perdus à l'installation (Bilan de la Phase 2)
- [ ] Savoir où est la vraie modlist et créer `F:\ModLab` (Bilan de la Phase 2)

- [ ] Trancher le sort du travail non enregistré et des deux branches locales (Travail non enregistré)

- [ ] Déplacer `docs/deep-research-report.md` dans `docs/communaute/` en corrigeant ses 7 renvois dans `ui/` (dont un texte affiché dans les Réglages) (Direction communauté)
- [ ] Fusionner les deux dossiers d'images (`img/` hors Git et `docs/design/img/`) : les `Assets_IA_profil` diffèrent d'un dossier à l'autre (Rangement des docs)

## Plus tard

- [ ] Rendre le script `tauri:dev` de `ui/package.json` portable : il contient des chemins `C:/Users/planc/...` en dur (Bilan de la Phase 2)
- [ ] Vérifier deux écarts relevés par l'audit du 6 nov. : commentaire périmé dans `crates/modlab-core/src/lib.rs`, chemin `C:/Users/planc` dans `ui/package.json` (Rangement des docs)
- [ ] Ajouter un test d'intégration de la CLI, qui n'en a aucun (Bilan de la Phase 2)

## Fait

- [x] 2026-11-25 : installeur guidé FOMOD complet (5 types de groupe, typeDescriptor/dependencyType, flags, étapes invisibles, order, images module et options, conditionalFileInstalls, priority, récapitulatif ; choix recalculés côté Rust). Reste : `moduleDependencies`, recochage d'une option devenue Recommandée. Pas encore cliqué dans l'app.

- [x] 2026-11-25 : table Mods — Pri. triable (glisser autorisé en ordre croissant), filtre cyclique sur État, Version non triable, colonne Jeux (version du jeu, loader Minecraft, plage si plusieurs versions). Renommage étendu (Nexus avec jeton, manifestes jar Fabric/Quilt/Forge/NeoForge prioritaires, `loaders` persisté), rejoué au chargement de branche ; tout mod sans indice passe en « Autre ».

- [x] 2026-11-25 : catégories remplies à l'ouverture d'une branche (mots-clés nets, contenu d'archive, manifestes Minecraft, petite table de mods connus) ; une ancienne valeur sans source hors de la liste du jeu (« uu ») est recalculée. Clippy à zéro avertissement.

- [x] 2026-11-25 : installation routée par l'adaptateur du jeu (plus d'installeur Skyrim en dur). Minecraft : jar drop-in, zip → jars / resourcepacks / shaderpacks ; un dossier par instance `<data root>/instances/minecraft/.<slug>` (mods, config, resourcepacks, shaderpacks, saves), chemin mémorisé. Reste : lancer Minecraft sur ce dossier (profil du launcher, fichier hors dépôt → demander) ; anciens jars du `.minecraft` global non déplacés.

- [x] 2026-11-25 : colonnes Taille (`mod_.size_bytes`, persistée) et Date (date de téléchargement `downloaded_at`, posée au lancement ; mois + année, jour en infobulle ; Info montre Téléchargé le et Installé le) sur Mods. Tri d'affichage par clic d'en-tête (Mod, Version, Taille, Date, Catégorie), priorité inchangée, glisser coupé pendant un tri, mémorisé par jeu.

- [x] 2026-11-25 : installations et commandes longues hors du thread UI (`spawn_blocking`, verrou SQLite limité à la transaction, pastille « Installation… », double clic refusé). À essayer sur un gros mod.
- [x] 2026-11-25 : métadonnées par couches avec provenance par champ (`field_sources`), motif de nom Nexus (`SkyUI-12604-6-11-…` → SkyUI 6.11), fomod/info.xml et meta.ini. Titre de page non lu (reste ouvert).
- [x] 2026-11-25 : catégories fixes par jeu avec icône (Skyrim 15, Minecraft 13), choix manuel marqué `user`.
- [x] 2026-11-25 : pastille « Dépendance inactive / manquante » dans État (masters Skyrim, DLL SKSE, ids Minecraft) ; résolution par l'adaptateur.

- [x] 2026-11-25 : pourcentage de téléchargement en direct dans la colonne État et le panneau Téléchargements (écouteur WebView2 `download://progress`, Mo reçus si taille inconnue). Pas encore essayé sur un vrai téléchargement.
- [x] 2026-11-25 : croix « Fermer le panneau » en haut à droite du panneau droit à onglets.

- [x] 2026-11-13 : README « Lancer et tester », build UI et mise en forme Rust réparés, 4 commits poussés sur GitHub (`ea8fc76`) (Rangement des docs et passage à bFrame)
- [x] 2026-11-13 : nettoyer 2brain, ranger docs/, remonter idées et décisions, contre-vérifier, écrire le rapport (Rangement des docs et passage à bFrame)
- [x] 2026-11-13 : poser bFrame et cadrer le projet (Cadrage)
