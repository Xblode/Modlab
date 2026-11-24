---
titre: Tâches
description: Ce qu'il y a à faire, du plus proche au plus lointain.
maj: 2026-11-24
---

# Tâches

Une case par tâche, avec son sujet entre parenthèses.

## Maintenant

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
- [ ] Benjamin regarde l'appli et décide : garder (commit) ou annuler
- [ ] Brancher le choix de thème Clair/Sombre/Auto et « Taille de police » des Réglages (boutons sans effet aujourd'hui)
- [ ] Raccourci « ⌃Q » affiché pour Quitter sans gestionnaire ; bouton « Console » du composeur sans action ; « Skyrim en cours » écrit en dur dans la barre de statut (`SidebarFooter.tsx`)
- [ ] Raccourcir les encadrés « Ce que le Core ne suit pas encore » (tableau de bord) et l'explication des Mondes
- [ ] Vérifier les écrans non vus : thème clair, Minecraft, Santé, Sauvegardes, dialogues
- [x] 2026-11-24 : CLAUDE.md : l'appli `ui/` est la référence visuelle, la maquette devient un historique
- [ ] Aligner sur cette règle : `docs/01-architecture-decidee.md` (l.15, 53, 246, 325, 327), `docs/README.md`, `docs/design/design-system.md`, agents `frontend-dev` et `ui-designer`, `.bframe/PROJET.md`
- [ ] Icône soleil pour le thème « Clair » (aujourd'hui une étincelle) ; police dédiée (Geist ou IBM Plex Sans) seulement si Benjamin le veut

Défauts relevés par la contre-vérification, pour le lot 2 (Bilan de la Phase 2) :
- [ ] Fermer ModLab pendant une partie : demander confirmation, et retrouver la session au redémarrage (`lib.rs` `launch_skse`)
- [ ] `_ResourcePack.esl` : confirmer s'il est implicite comme les officiels (`plugins.rs`)
- [ ] Chemin `Plugins.txt` selon l'édition (Steam, GOG, Epic)
- [ ] Manifeste SKSE : garder la plus ancienne sauvegarde ; ne retirer l'enregistrement qu'après restauration ; verrou commun case à cocher / lancement
- [ ] `import_mods` dans une instance non vide : positions en double

## Ensuite

- [ ] Lot 2 — socle multi-jeux : contrat de jeu étendu, code Skyrim sorti de l'app, corbeille réelle, toutes les actions dans le cycle sûr (Bilan de la Phase 2)
- [ ] Lot 3 — Minecraft : vraie case activer, instances avec version et loader, mods synchronisés, lancement par le launcher officiel, navigateur Modrinth (Minecraft utilisable)
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

- [x] 2026-11-13 : README « Lancer et tester », build UI et mise en forme Rust réparés, 4 commits poussés sur GitHub (`ea8fc76`) (Rangement des docs et passage à bFrame)
- [x] 2026-11-13 : nettoyer 2brain, ranger docs/, remonter idées et décisions, contre-vérifier, écrire le rapport (Rangement des docs et passage à bFrame)
- [x] 2026-11-13 : poser bFrame et cadrer le projet (Cadrage)
