---
titre: Rapport de rangement
description: Comment on a rangé ModLab (ancien système 2brain retiré, docs rangés, rien perdu), écrit pour en faire un module ou skill bFrame.
maj: 2026-11-13
---

# Rapport : ranger un projet existant dans bFrame

Première fois faite à la main sur ModLab, le 2026-11-13. But : en tirer un skill (ex. `rangement`) qui s'enchaîne après `onboarding`.

## 1. Ce que le skill doit produire

- Un projet sans traces d'un ancien système d'organisation (ici 2brain : `brain/`, `brain-main/`, `AGENTS.md`, `.claude/README.md`, clé `agent` dans `.claude/settings.json`, en-têtes YAML, lignes `.gitignore`).
- Deux étages clairs : `.bframe/` = pilotage court (cap, état, décisions, idées, tâches, avec liens) ; `docs/` = détail (architecture, ADR, design, faisabilité).
- Les docs dépassés dans `docs/archive/` avec un `README.md` « dépassé par… / extrait vers… ». Jamais de suppression.
- Les docs « direction pas encore cadrée » dans leur propre dossier (ici `docs/communaute/`), pas dans l'archive.
- Une seule liste d'idées (`.bframe/IDEES.md`) et une seule liste de tâches (`.bframe/TACHES.md`).
- `CLAUDE.md` réduit aux règles stables ; `AGENTS.md` = 3 lignes de renvoi (d'autres IA le lisent).

## 2. Les étapes, dans l'ordre

| # | Étape | Qui | Contrôle |
| --- | --- | --- | --- |
| 1 | Inventaire en lecture seule : chaque fichier → nature, date, statut (fait foi / dépassé / brouillon / inconnu), infos uniques, contradictions avec le doc qui fait foi. Inclure les fichiers supprimés non commités (`git show HEAD:…`). | worker-sonnet | chaque fichier a une ligne |
| 2 | Carte de destination proposée à l'humain + principes + points où on le contredit | orchestrateur | — |
| 3 | Validation par lot (4 questions max) | humain | — |
| 4a | Retirer les traces de l'ancien système | worker-haiku | recherche du nom de l'ancien système = vide hors archive |
| 4b | Ranger `docs/` : déplacer, extraire mot pour mot les sections encore vivantes des docs archivés, sommaire, README d'archive (en parallèle de 4a, fichiers disjoints) | worker-sonnet | table ancien → nouveau |
| 5 | Mettre à jour `CLAUDE.md`, le doc qui fait foi (encadré « écarts depuis », sans réécrire le verrouillé), les liens dans agents et code | worker-sonnet | grep des anciens chemins = vide |
| 6 | Remonter dans `.bframe/` : une ligne par décision (avec lien), une ligne par idée (avec marqueur de source), les écarts en tâches | orchestrateur | marqueurs comptés |
| 7 | Contre-vérification par un second worker : fichiers déplacés identiques à HEAD (`cmp`), copies intégrales, idées toutes présentes, liens non cassés | worker-haiku | 7 verdicts |

## 3. Les questions à poser à l'humain (le reste se décide seul)

1. Principes : deux étages, archive au lieu de suppression, une seule liste d'idées.
2. Chaque doc d'exploration qui contredit le cap (ici : communauté / économie contre « personnel d'abord ») : piste, vraie direction ou archive ?
3. Outil de suivi externe (ici ClickUp) : l'arrêter au profit de `TACHES.md` ?
4. Règles utiles cachées dans un doc archivé (ici les règles de vérification de Cline) : les reprendre dans `CLAUDE.md` ?
5. Écart entre une règle écrite et le réel (ici « pas de remote » alors qu'`origin` existe) : voulu ou à corriger ?
6. Où mettre ce rapport.

## 4. Pièges rencontrés (à coder dans le skill)

- **Clé de démarrage cassée** : `.claude/settings.json` pointait vers un agent supprimé. C'est la trace la plus urgente : à traiter en premier.
- **Suppressions non commitées** : l'info « perdue » est encore dans `HEAD`. Toujours inventorier avec `git status` + `git show HEAD:`, et ressortir ce qui est unique (ici l'audit daté du 6 nov.) avant de nettoyer.
- **Doc à la fois « ne jamais suivre » et « source d'une section vivante »** (ici `00-cadrage` §6 et §8) : extraire ces sections mot pour mot dans un doc vivant, sinon l'archive n'est jamais vraiment morte.
- **Un doc référencé par le code ne se déplace pas sans corriger le code** (ici `deep-research-report.md`, cité dans 7 fichiers de `ui/`, dont un texte affiché) : le worker doit chercher les références avant de déplacer, et s'abstenir si elles sont dans le code.
- **Un doc « exploration » peut cacher du code déjà écrit** (ici des écrans Communauté, Place de marché, Profil existent dans `ui/`) : le signaler à l'humain, ça change la discussion sur le cap.
- **Deux dossiers d'images aux mêmes noms mais contenus différents** : ne pas dédoublonner au nom, comparer les octets.
- **Agents de projet construits sur l'ancien outil de suivi** (ici `pm.md` tout entier sur ClickUp) : renvoi d'une phrase, réécriture en sujet à part.
- **Moteur générique vs contenu projet** : vérifier par grep du nom du projet que le dossier du moteur (ici `brain-main/`) ne contient rien de propre au projet avant de le laisser partir.
- Rien n'est commité : l'humain décide du commit à la fin.

## 5. Les briefs qui ont marché

- Inventaire : lecture seule stricte, une ligne par fichier avec « infos UNIQUES » et « contradictions », rapport sans sortie brute.
- Rangement : « déplacement simple, pas git mv », « copie MOT POUR MOT », preuve par nombre de lignes source vs copie, « liste les références hors de ton périmètre au lieu de les corriger ».
- Mise à jour du doc verrouillé : « ne pas réécrire, corriger les chemins + un encadré d'écarts ».
- Contre-vérification : `cmp` contre `HEAD`, comptage de marqueurs, vérification des chemins entre backticks.

## 6. Ce qui reste à faire sur ModLab après ce rangement

Voir `TACHES.md` : déplacer le rapport communauté (avec correction du code), fusionner les deux dossiers d'images, vérifier deux écarts de l'audit, réécrire `pm.md` (sujet « Les 8 agents spécialisés »), cadrer la direction communauté.

## 7. Résultat de la contre-vérification

Faite par un second worker (lecture seule), 7 verdicts OK, aucune perte détectée :
- 11 fichiers déplacés identiques à `HEAD` (2 ne diffèrent que par les fins de ligne CRLF) ; audit identique.
- 3 extractions intégrales (seul un en-tête ajouté, plus une note signalée dans `faisabilite.md`).
- 35/35 idées présentes dans `IDEES.md` (B1–B20, P1–P15), sens vérifié sur 15.
- 26 entrées du registre 2brain couvertes par l'audit archivé.
- Aucun chemin cassé.
Limites : liens markdown `[…](…)` non vérifiés ; un commentaire de code touché (`installer.rs`, `cargo check` OK). À coder dans le skill : vérifier en ignorant les fins de ligne, et ne pas écrire pendant que le contrôle tourne.
