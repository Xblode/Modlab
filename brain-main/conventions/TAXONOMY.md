---
metadata_version: 1

title: "Taxonomies Brain (communes)"
type: convention
description: "Centralise les taxonomies partagées des champs de capture et de registre Brain."
summary: "Définit les valeurs et la sémantique des champs de registre communs (intention, type, position, validation, status, space, relations, grounding, confidences, refs) et les règles de compatibilité historique. Le contrat JSON mécanique reste brain/registry/SCHEMA.md."

version: "1.0.2"
status: review

created: "2026-10-31"
updated: "2026-11-06"

scope:
  level: universal
  targets:
    - brain-registries

depends_on:
  - "brain-main/conventions/METADATA.md"

updated_from:
  - "brain-main/conventions/METADATA.md"
  - "brain-main/conventions/NAMING.md"
  - "brain-main/conventions/README.md"

tags:
  - taxonomy
  - registry
  - capture
---

# Taxonomies Brain (communes)

| | |
| --- | --- |
| Compatible registre | 0.1.4 (rétrocompat 0.1.x) |
| Évolution | Ajouter une valeur = versioning + note de compatibilité ; **ne jamais** retirer silencieusement une valeur encore présente dans des registres historiques |

Les Skills appliquent ces règles ; `brain/registry/SCHEMA.md` reste le contrat JSON mécanique. Les champs ci-dessous sont ceux des **entrées de registre**, pas les propriétés de front matter de [METADATA.md](METADATA.md) (voir [README.md](README.md)). « Compatible registre » est la version du registre, pas du document. Les fichiers de registre suivent [NAMING.md](NAMING.md).

## 1. `intention` (référence sémantique principale — générations ≥ 0.1.4)

Dit *ce que l’information veut être* dans le discours. Une capture a **une** intention principale.

| Valeur | Signification | Usage |
| --- | --- | --- |
| `idea` | Possibilité explorée | Piste ouverte, non choisie |
| `suggestion` | Proposition formulée | Souvent assistant ; non décidée |
| `preference` | Préférence exprimée | Goût / priorisation personnelle |
| `orientation` | Direction provisoire | Choix de travail non figé |
| `question` | Question ouverte | Attente de réponse |
| `decision` | Choix explicitement effectué | Preuve de tranchage dans la source |
| `requirement` | Besoin / contrainte | Ce qui doit être respecté |
| `convention` | Règle de fonctionnement | Comment on travaille |
| `task` | Action à réaliser | Même potentielle |
| `milestone` | Jalon structurant | Étape de roadmap |
| `observation` | Constat factuel | Sans posture de choix |
| `risk` | Risque identifié | Menace / limite |
| `feedback` | Retour / critique / correction | Commentaire sur une proposition |

**Règles**

- Ne pas inventer de nouvelles valeurs sans mise à jour de ce document + SCHEMA.
- `orientation` / `suggestion` / `question` **ne sont pas** des `decision`.
- Une proposition assistant reste `suggestion` (ou `idea`) même si le user dit « bonne base » (`approval_global_partial`).

**Incompatibilités** (éviter)

- `intention: decision` pour « B ou C » non tranché → `orientation` + tag `ambiguous`.
- `intention: decision` pour une suggestion assistant (pas de choix user).
- Confondre `feedback` et `decision` (un avis n’est pas un choix).

## 2. `type` (compatibilité historique 0.1.1)

Nature structurelle héritée. **Ne plus l’interpréter indépendamment** de `intention` pour les nouvelles captures : `intention` prime, `type` est dérivé.

### Mapping déterministe `intention` → `type` (obligatoire en génération ≥ 0.1.4)

| intention | type |
| --- | --- |
| `idea` | `idea` |
| `suggestion` | `proposition` |
| `preference` | `observation` |
| `orientation` | `proposition` |
| `question` | `open_question` |
| `decision` | `decision` |
| `requirement` | `observation` |
| `convention` | `convention` |
| `task` | `task` |
| `milestone` | `task` |
| `observation` | `observation` |
| `risk` | `observation` |
| `feedback` | `observation` |

Le Skill **applique ce mapping automatiquement** après choix de `intention`. Ne jamais forcer `type: decision` si `intention` ∈ {`orientation`,`suggestion`,`question`,`idea`,`preference`,`feedback`,`risk`}.

**Historique** : les anciennes entrées peuvent violer ce mapping ; ne pas les réécrire. Le validateur n’impose le mapping qu’avec `--require-intention` (générations neuves).

**Valeur `event`** : réservée aux constats d’événement calendaire / fait daté ; intention typique `observation`. Ne pas inventer d’intention `event`.

## 3. `position`

Posture de l’auteur face à l’info. Indépendante de `validation`. Nullable.

| Valeur | Signification |
| --- | --- |
| `proposed` | Proposition formulée |
| `exploring` | Piste explorée |
| `favorable` | Avis favorable |
| `endorsed` | Bonne base / matière à conserver |
| `opposed` | Opposition |
| `selected` | Option explicitement choisie (peut rester non formellement approuvée) |
| `confirmed` | Info / règle confirmée dans le discours |
| `uncertain` | Hésitation |
| `null` | Aucune posture claire |

**Règles critiques**

- `selected` / `confirmed` / `favorable` / `endorsed` **ne déclenchent pas** `validation: explicit_user_approval`.
- Ne pas déduire abusivement une position.

## 4. `validation` / `approved_by` / `approval_scope`

Preuve d’**approbation formelle**, distincte de la posture.

| validation | Sens |
| --- | --- |
| `none` | Pas d’approbation formelle |
| `explicit_user_approval` | Accord explicite identifiable |
| `implicit` | Historique uniquement — **ne plus attribuer** en génération neuve |

| approval_scope | Sens |
| --- | --- |
| `item` | Cette capture |
| `set` | Ensemble identifiable de captures |
| `null` | Pas d’approbation |

Cohérence mécanique : `explicit` ⇒ `approved_by: user` + scope `item|set` ; `none|implicit` ⇒ `approved_by: null` + scope `null`.

Tag `approval_global_partial` : accord « en gros » → **toujours** `validation: none`.

## 5. `status` (opérationnel)

État de traitement dans le pipeline Capture. Indépendant de la vérité projet.

| Valeur | Sens |
| --- | --- |
| `extracted` | Sortie de Capture (seule valeur autorisée en Capture) |

D’autres statuts pourront exister plus tard (Review) — **hors scope Capture**.

## 6. `space`

Boîte logique de rangement. **Le classement n’est jamais une validation.**

| Valeur | Responsabilité | Exemples |
| --- | --- | --- |
| `atelier` | Réflexion, exploration, propositions, questions | Suggestions, orientations, idées, questions ouvertes |
| `referentiel` | Connaissances **établies / suffisamment stables** pour servir de référence | Conventions confirmées, décisions formellement approuvées, faits stables |
| `journal` | Événements et historique | « Implémentation 1.1 lancée », faits datés |
| `pilotage` | Tâches, objectifs, jalons, suivi | `task`, `milestone`, arbo ClickUp de pilotage |

**Règles**

1. Proposition / suggestion / orientation / question / idea **non** formellement validées → **`atelier`** (défaut).
2. Ne **pas** mettre automatiquement en `referentiel` faute de mieux.
3. En cas d’incertitude → **`atelier`**.
4. `space: referentiel` + `validation: none` est un **signal d’alerte** (REVIEW) : réservé aux cas où la source présente déjà une règle/connaissance comme établie **sans** pour autant constituer une approbation formelle d’item — documenter dans `evidence` ; sinon rester en `atelier`.
5. `task` / `milestone` → `pilotage` (ou `atelier` si pure exploration de future tâche).

### Cas limites

| Cas | space |
| --- | --- |
| Assistant propose 4 espaces | `atelier` |
| User valide `vision.md` à 100 % (`explicit`) | `referentiel` |
| User dit « bonne base » sur une architecture | `atelier` + tag `approval_global_partial` |
| Observation « ClickUp a deux espaces » | `journal` ou `atelier` selon contexte (fait vs exploration) |
| Jalon roadmap proposé non verrouillé | `pilotage` + `validation: none` |

## 7. Relations (`relations[].rel`)

Liens utiles à la navigation future — pas un score de densité.

| Rel | Sens | Réciproque recommandée |
| --- | --- | --- |
| `related` | Lien thématique utile | optionnelle |
| `clarifies` / `clarified_by` | Précision sans remplacement | oui si les deux côtés connus |
| `answers` / `answered_by` | Réponse à une question | oui |
| `evolves_from` / `evolves_to` | Évolution historique | oui |
| `conflicts_with` | Tension active non close | souvent symétrique |
| `contradicts` / `contradicted_by` | Incompatibilité affirmée étayée | oui |
| `resolves` | Clôt une tension | selon cas |
| `supersedes_candidate` | Candidat à remplacement | pas de double avec evolves |
| `parent_of` / `child_of` | Hiérarchie (jalons, cadrage) | oui |

**Priorités** : question↔réponse ; précision↔idée ; évolutions ; contradictions ≠ simples différences ; proposition↔validation/rejet.

Cibles = `CAP-NNN` **local au même fichier / run**. Pas de relations inter-runs en 0.1.4.

## 8. `grounding`

Ancrage du **contenu** dans la source (≠ `confidence` extraction, ≠ `author_confidence`).

| Valeur | Sens |
| --- | --- |
| `source_explicit` | Formulation explicitement présente |
| `source_contextual` | Compris via plusieurs passages |
| `source_interpreted` | Reformulation agent raisonnable |
| `uncertain` | Ancrage insuffisant |

## 9. `confidence` et `author_confidence`

| Champ | Responsabilité | Valeurs |
| --- | --- | --- |
| `confidence` | Qualité de l’extraction du contenu | `high` \| `medium` \| `low` |
| `author_confidence` | Fiabilité de l’attribution d’auteur | `high` \| `medium` \| `low` \| `uncertain` |

Seuils : Skill `source-attribution.md` (`high` seulement si structure source fiable ; style seul ≠ `high`).

## 10. Références concrètes (`refs`)

Conserver **littéralement** URLs, chemins, identifiants et ancres présents dans la source et liés à la capture.

- Type : tableau de strings non vides (optionnel si aucune référence).
- Chaque URL complète exacte (paramètres et `#ancres` inclus).
- Ne pas utiliser `origin.url` comme seul emplacement multi-liens métier.
- Le `content` doit aussi citer ces références (pas « lien fourni »).

## 11. Compatibilité historique

| Génération | Exigence |
| --- | --- |
| Pré-0.1.3 | `intention` / `position` / `grounding` / `refs` / mapping peuvent manquer — OK hors `--require-intention` |
| 0.1.3 | Champs 0.1.3 ; mapping intention→type parfois incohérent — **ne pas réécrire** |
| ≥ 0.1.4 | Mapping intention→type obligatoire sous `--require-intention` ; `refs` si URLs dans la source |

Ne jamais supprimer ni fusionner d’anciennes entrées pour « aligner » une taxonomie.
