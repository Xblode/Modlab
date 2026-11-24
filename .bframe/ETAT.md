---
titre: État
description: Où en est le projet, sujet par sujet.
maj: 2026-11-24
projet: ModLab
etat: actif
sujet_en_cours: Bilan de la Phase 2
prochaine_action: Essai en jeu du lot 1 par Benjamin (trouver d'abord usvfs_x64.dll / disque F:), puis lot 2
decisions_attendues: 1
---

# État

## Où on en est
Phases 0 et 1 finies le 15 sept. : Skyrim se lance par ModLab avec la vraie modlist. Phase 2 (interface, IA locale, annuler/revenir en arrière) avancée jusqu'au 20 sept., puis pause de 7 semaines. bFrame posé le 13 nov.

## Sujets

| Sujet | État | Prochaine action |
| --- | --- | --- |
| Bilan de la Phase 2 | en cours | Lot 1 codé et contre-vérifié le 24 nov. (plugins.txt, SKSE, archives `Data/`, ordre des mods), commité en local le 24 nov. (`dd951f5`, sans push), pas encore essayé en jeu ; état des lieux : `.bframe/etat-des-lieux-2026-11-24.md` |
| Look de l'interface | en cours | 24 nov. : passe unslop, puis « respirer » (densité confortable + réglage Compact, texte 14/12, finitions shadcn) et hiérarchie (un bouton principal, infobulles, états vides, barre de statut). Tout non commité, build OK. Règles : `docs/design/regles-interface.md`. L'appli est la référence visuelle, plus la maquette. Non vus à l'écran : tableau de bord, thème clair, Minecraft, dialogues, glisser-déposer en Compact |
| Minecraft utilisable | prêt à planifier | Lot 3 du plan ; 4 choix recommandés (instance, lancement, navigateur, données) encore à confirmer |
| Rangement des docs et passage à bFrame | assez fait | Rapport prêt pour en faire un skill : `.bframe/rapport-rangement.md` |
| Direction communauté | à creuser | Cadrer avec une interview : ça change le cap « personnel d'abord » et peut-être le MVP |
| Les 8 agents spécialisés | à creuser | Décider s'ils restent à côté des workers bFrame ou deviennent des skills |
| Travail non enregistré | à creuser | Décider quoi faire des 2 README non suivis et des branches `feature/phase-2-shell`, `feature/unified-game-header` |
| Premier lancement SKSE raté | à creuser | Noter la prochaine occurrence ; cause inconnue (sept.) |

## Décisions qui attendent l'humain

- Où est la vraie modlist aujourd'hui, et le disque F: (MO2 et sa `usvfs_x64.dll`) est-il branché ? Nécessaire pour l'essai en jeu.
- Commiter les changements d'interface du 24 nov. (unslop + respirer + hiérarchie), après les avoir regardés dans la fenêtre.
