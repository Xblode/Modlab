---
titre: État
description: Où en est le projet, sujet par sujet.
maj: 2026-11-28
projet: ModLab
etat: actif
sujet_en_cours: Revue d'architecture
prochaine_action: Passe de cadrage en cours (après la conversation ChatGPT du 27 nov., 14 prompts numérotés 0 à 13, le 0 est fait) : 4 contradictions tranchées et périmètre reporté dans `PROJET.md` ; tests de caractérisation avant le prompt 3 et audit dans `ARCHITECTURE.md` §7 validés ; passe de cadrage finie, `conv.txt` en archive. Benjamin veut attaquer directement les 3 problèmes (logique éparpillée, pas de responsable unique du cycle de vie, chaque jeu à sa façon), en commençant par la logique éparpillée, lot par lot sans carte complète (décision du 28 nov.). En cours : commit + push de sauvegarde (demandés par Benjamin) et inventaire rapide de la couche Tauri ; ensuite découpage en lots
decisions_attendues: 1
---

# État

## Où on en est
Phases 0 et 1 finies le 15 sept. : Skyrim se lance par ModLab avec la vraie modlist. Phase 2 (interface, IA locale, annuler/revenir en arrière) avancée jusqu'au 20 sept., puis pause de 7 semaines. bFrame posé le 13 nov. Le 25 nov., le livrable 1 de Mon jeu est terminé : chemins, scan vanilla en lecture, résumé et bouton Nettoyer (plan des extras, sans effacement), boutons SKSE/LOOT, détection SKSE comme un mod. Sujet en cours : le refactor validé le même jour. Lots 1 à 5 faits (Rust 1, 3 et 5 le 25 nov., `cargo test --workspace` passé selon le worker) ; lot 6 partiel : install et désinstall en transaction avec corbeille, lancement SKSE encore hors cycle ; lot 7 reporté. Décision instances et branches notée le même jour (livrée) : l'instance est le parent, une branche n'existe qu'à l'intérieur, chaque loadout déjà en base reste sa propre instance. **26 nov.** : case activer Minecraft livrée — rename `*.jar` ↔ `*.jar.disabled` (convention déjà dans le scanner), UI branchée. **Supprimer** dans le tableau Mods retire aussi le jar de `mods/` (trash interne). **Deps Minecraft Infos** livré : parse kind + filtre `[[dependencies.ownId]]`, jar-in-jar dans `own_ids`, reverse « Utilisé par », badge required-only, tableau Infos.

## Sujets

| Sujet | État | Prochaine action |
| --- | --- | --- |
| Revue d'architecture | en cours | 28 nov. : principes et méthode notés (`PROJET.md`, `DECISIONS.md`). Attend les 13 prompts de Benjamin. Les autres sujets « en cours » sont en pause pendant la revue. Prompt 1/13 (refonte de la doc) fait le 28 nov. : 10 fichiers dans `.bframe/`, `docs/` et `img/` supprimés du dépôt (archive dans `.bframe/archive/`), contre-vérifié. Puis `CLAUDE.md`, `AGENTS.md` et README en français, un README par dossier racine (`crates/`, `ui/`, `fixtures/`, `spikes/`), `cargo check` et `npm run build` OK selon le worker. Travail Cursor de la semaine passée et refonte : rien n'est commité |
| Refactor | en cours | Lots 1 à 5 faits le 25 nov. (`cargo test --workspace` passé selon le worker). Lot 6 partiel : install/désinstall en transaction et corbeille ; lancement SKSE (copie, USVFS, plugins.txt) et `sync_engine_mod_direct_install` encore hors cycle. Lot 7 (ts-rs) reporté. Plan validé le 25 nov. après deux audits (UI + shell, dépôt + dépendances) |
| Mon jeu | assez fait | Livrable 1 terminé le 25 nov. : chemins, scan vanilla en lecture seule, résumé et bouton Nettoyer (plan des extras Data, sans effacement), boutons SKSE/LOOT, détection SKSE comme un mod. Ensuite : essai manuel dans Tauri, polish éventuel. Pas de versioning de bibliothèque |
| Bilan de la Phase 2 | en cours | Lot 1 codé et contre-vérifié le 24 nov. (plugins.txt, SKSE, archives `Data/`, ordre des mods), commité en local le 24 nov. (`dd951f5`, sans push), pas encore essayé en jeu ; état des lieux du 24 nov. en archive (dépassé) |
| Look de l'interface | en cours | 26 nov. : Console → 3ᵉ mode **PowerShell** (REPL one-shot via `run_powershell`, UI only) ; aussi slot action principale `ShellToolbar`, barre favoris Web, sidebar Réglages, Chat Cursor, Checkbox mod, PopoverMenu context. Règles : `DESIGN.md`. Non vus : tableau de bord, thème clair, dialogues, DnD Compact ; multi-WebView2 reporté |
| Refonte de la barre de gauche | assez fait | Codé le 24 nov., build OK, pas encore vu à l'écran ni commité. Carte : nom, Jouer, Partager. Arbre façon Cursor. Révision, compteurs et tuiles moteur sur le tableau de bord de l'instance. Grille de 9 pixels dans le pied pendant une partie |
| Minecraft utilisable | en cours | 26 nov. : **Tester le lancement** (barre Mods, à côté d’Ajouter) — même Cap B JVM que Jouer, fenêtre 1×1, stream Console, succès = marqueurs menu (`Sound engine started` / Fabric completed…), kill auto, timeout 120 s. deps Infos Xaero-like OK. Suite : essai manuel smoke + Infos + Jouer Iris/Sodium |
| Switch d'instance | assez fait | 26 nov. : snapshot léger au switch ; hydratation auto en arrière-plan retirée (elle re-gelait). Hydratation complète au premier chargement et après mutations |
| Barre d'instance (shell) | assez fait | 26 nov. : slot `.tbar-primary-slot` — select branche ↔ CTA « Installer NeoForge … » (même `--ctl-sm`) ; Installer retiré des popovers loader (`useMcLoaderBranch.primaryAction`). Compaction + DS + `DESIGN.md` « Slot action principale ». Build UI OK |
| Découpe App par espace | assez fait | 26 nov. : `App.tsx` ~536 lignes (composeur) ; câblages `shell/` + `spaces/` (mods, instance, utility, dock) ; `npm run build` OK |
| Faits UI uniques + docs IA | assez fait | 26 nov. : un propriétaire par fait (canal=sidebar, révision/compteurs=ContextToolbar, SKSE/MC=BranchHeader) ; hero Mods = `instanceName` (pas `name` branche) ; interdiction lire l'archive ; README portes core/minecraft/shell ; ETAT obligatoire après avance |
| Shell header + Packs tableau | assez fait | 26 nov. : slots Titlebar → header → toolbar → sheet ; Packs/shaders en `InstanceFolderTable` |
| Install mods + loaders liste | assez fait | 26 nov. : refresh post-install allégé (mesures seules) ; ligne virtuelle loader MC dans Mods après install ; sync au snapshot |
| Panneau Chat (ex-Claude) | assez fait | 26 nov. : tags `@mod` = `Chip` tone=mod ; wrap 1↔2 miroir ; modes plan/debug/multi |
| Tableau Mods responsive | assez fait | 26 nov. : Cat. badge ghost ; Jeux = version seule (~76 px) ; colonne Load. (F/N/Fb/Q) si `gameId===minecraft` via `data-loader-col` — absente pour Skyrim ; T2 garde Load. |
| Sidebar catégories | assez fait | 26 nov. : Catégories > Favoris ; toggle vides (mini) + … → Réglages Catégorie ; Jeux ouvert ; Mods vide = Importer / sites / dossier |
| Design system (Réglages) | assez fait | 26 nov. : Couleurs dark-first (hex `:root`, soft 16%/68% documenté) ; Badge/Chip alignés ; DESIGN.md Surfaces ; Toolbar compaction ; PopoverMenu context |
| Rangement des docs et passage à bFrame | assez fait | Rapport prêt pour en faire un skill : `rapport-rangement.md` (archive ; son modèle « `docs/` = détail » est remplacé le 28 nov.) |
| Direction communauté | à creuser | Cadrer avec une interview : ça change le cap « personnel d'abord » et peut-être le MVP |
| Les 8 agents spécialisés | assez fait | Retirés le 24 nov. Le travail passe par les 3 workers bFrame |
| Travail non enregistré | à creuser | Décider quoi faire des 2 README non suivis et des branches `feature/phase-2-shell`, `feature/unified-game-header` |
| Premier lancement SKSE raté | à creuser | Noter la prochaine occurrence ; cause inconnue (sept.) |

## Décisions qui attendent l'humain
- Où est la vraie modlist aujourd'hui, et le disque F: (MO2 et sa `usvfs_x64.dll`) est-il branché ? Nécessaire pour l'essai en jeu.