---
titre: Tâches
description: Ce qu'il y a à faire, du plus proche au plus lointain.
maj: 2026-11-13
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
- [ ] Contre-vérification du lot 1 par un second développeur, et vérifier la version de SKSE avant de le copier
- [ ] Essai en jeu par Benjamin : lancer depuis ModLab, lire `skse64.log`

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
