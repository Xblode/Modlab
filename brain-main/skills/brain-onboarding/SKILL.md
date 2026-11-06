---
name: brain-onboarding
version: 0.4.0
description: >-
  Remplit brain/docs/ (VISION, CADRAGE, PROJECT, REQUIREMENTS, ARCHITECTURE, WORKFLOW)
  avec le projet, par propositions marquées confirmé, hypothèse ou à compléter,
  écrites seulement après validation explicite. Utiliser pour /brain -o.
disable-model-invocation: true
---

# /brain -o

Onboarding : proposer le contenu de `brain/docs/` d'après le projet, sans rien décider. Ce skill est appelé par `/brain -o` (voir `brain-main/bootstrap/README.md`). Pour lire un projet existant en profondeur d'abord : `/brain -a` (`brain-main/skills/brain-audit/SKILL.md`). Il ne reprend aucune convention : `brain-main/conventions/METADATA.md` se lit avant toute écriture (voir l'étape 7).

## Garde-fous

- Une proposition reste une proposition : rien n'est écrit avant la validation explicite de ce qui vient d'être présenté, donnée dans le message qui suit.
- Ne rien décider, ne rien coder. Ne modifier ni le registre, ni `brain-main/`, ni les fichiers du projet hors `brain/docs/`.
- Ne jamais remplacer une information confirmée sans le dire et sans accord explicite.
- Les questions restent posées. Une réponse brève (« simple essai ») ne dispense pas des suivantes : seul l'humain peut demander explicitement d'arrêter. Une question sans réponse ne bloque pas : l'élément reste « À compléter. ».
- Sans Bash (session orchestrateur, cas normal d'un projet installé) : déléguer l'écriture à l'agent `worker-sonnet` s'il existe. Il ne voit pas la conversation : le brief donne les chemins, le contenu validé mot pour mot et les sections à mettre à jour. La validation reste chez l'orchestrateur.

## Étapes

1. **État.** Lire `status.py --json` (lancé par le skill du compte). Si `brain` est `absent` : dire de faire `/brain -i` et s'arrêter. Noter les documents listés dans `documents_a_completer_noms`. Si `documents_a_creer` contient `cadrage.md` (projet installé avant ce document) : proposer de le créer, à l'étape 5, et ne rien créer sans validation ; le modèle est `brain-main/bootstrap/templates/brain/docs/CADRAGE.md` (`{{date}}` = date du jour), s'il manque, dire de faire `/brain -u`. Si le compteur est 0 et que rien n'est à reprendre : le dire et s'arrêter.
2. **Lire les documents.** Lire les documents de `brain/docs/` (six, `cadrage.md` s'il existe) : réponses déjà notées en section 3 (« Informations confirmées »), manques restants. Ce qui est déjà confirmé n'est pas refait.
3. **Projet avec des fichiers.** S'il existe un rapport dans `brain/sources/audits/` (`audit_dernier` de `status.py`), lire le dernier et repartir de ce qui y est `confirmé`, sans le redemander ; ce qui y est `hypothèse` ou `à compléter` reste à valider. Sinon, ou pour le reste : lecture ciblée, sans parcours exhaustif : README, manifestes (`package.json`, `pyproject.toml`, `Cargo.toml`, etc.), structure de premier niveau, documentation existante (`docs/`, `AGENTS.md`, `CLAUDE.md`). Chaque fait relevé garde sa source (fichier).
4. **Projet vide.** Poser des questions adaptées, par document, en partant des réponses déjà notées. Les poser par petits groupes. Ne pas inventer de réponse.
5. **Proposer, document par document.** Pour chacun des six documents, présenter le contenu proposé par section. Chaque élément porte l'un de ces marqueurs :
   - `confirmé` : seulement si la source le dit explicitement, soit une citation d'un fichier lu, soit une réponse de l'humain ;
   - `hypothèse` : tout ce qui est déduit, y compris d'un script ou de l'absence d'un fichier, à valider ;
   - `à compléter` : information absente.
   Présenter les documents dans l'ordre VISION, CADRAGE, PROJECT, REQUIREMENTS, ARCHITECTURE, WORKFLOW.
6. **Valider, document par document.** Nommer chaque document (VISION, CADRAGE, PROJECT, REQUIREMENTS, ARCHITECTURE, WORKFLOW) et demander pour chacun ce qui doit être écrit. Une réponse globale (« le confirmé seulement », « tout ») n'est pas une validation : redemander, document par document. Rien n'est écrit sur une approbation générale ou passée.
7. **Écrire** seulement ce qui est validé, dans les sections 3 à 5 du document concerné, en gardant le plan existant. Écrire un `confirmé` comme information confirmée, un `hypothèse` comme hypothèse déclarée, un `à compléter` comme « À compléter. ». Avant d'écrire, lire `brain-main/conventions/METADATA.md` §4.8, §4.11 et §8, puis mettre à jour `updated` et `version` selon ces règles.
8. **Bilan.** Relancer `status.py --json`, indiquer les documents qui contiennent encore « À compléter », et rappeler que `/brain -o` peut être relancé dans une autre session.

## Plusieurs sessions

`/brain -o` se relance autant que nécessaire. À chaque lancement : lire l'état, indiquer les manques restants, ne proposer que ce qui manque ou ce que l'humain demande de revoir. Ce qui est confirmé n'est ni redemandé ni réécrit.

## Hors périmètre

Décider d'une architecture, d'une exigence ou d'un module ; activer un outil ; capturer une session (c'est `/brain -c`) ; modifier une convention.
