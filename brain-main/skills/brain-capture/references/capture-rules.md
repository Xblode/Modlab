# Règles de capture `/brain -c` (0.1.5)

Taxonomies canoniques : `brain-main/conventions/TAXONOMY.md`.

## Fidélité et couverture

- Collecter large la matière **projet** utile ; ne pas sélectionner seulement les « meilleures » idées.
- Exclure du métier : commentaires de test/génération, simulations méta, instructions temporaires de test.
- Ne pas transformer hypothèse → fait, recommandation assistant → choix user, citation → approbation.
- Préserver conditions, exceptions, refus, alternatives, incertitudes, **URLs et références exactes**.

## Atomicité

Une capture = **une information identifiable**.

| Règle | Détail |
| --- | --- |
| Un élément | Une idée, contrainte, décision, tâche, observation distincte → une CAP |
| Lier plutôt que fusionner | Infos liées mais distinctes → deux CAP + relation |
| Idée + explications | Ensemble OK si **même** info enrichie (autonomie) |
| Anti-fusion | Ne pas fusionner : convention + emplacement ; multi-exigences ; décision + proposition + question ; idées user et assistant indépendantes |
| Anti-sur-segmentation | Ne pas éclater chaque phrase d’explication d’**une** même info |

### Exemples

**Bonne** — deux CAP :

1. Convention : « le fichier de convention ClickUp s’appelle `clickup.md` »
2. Suggestion d’emplacement : « proposer sous `brain/docs/` ou `brain/conventions/` »  
   Relation : `2 clarifies 1` ou `related`.

**Mauvaise** — une CAP fourre-tout : « clickup.md sous docs/ + validé + à sync ».

**Bonne** — idée + raisons dans la même CAP si une seule orientation : « Local = SoT parce que ClickUp est un visualiseur et que le gratuit est limité ».

**Mauvaise** — fusion user « je veux des agents » + assistant « orchestrateur central recommandé » sous un seul `author`.

**Bonne** — décision user séparée de la question encore ouverte, même dans le même message.

Régression (ne pas réécrire l’historique) : CAP-333 (composite à scinder à l’avenir), CAP-329, CAP-332. **Mieux vaut plusieurs captures précises et reliées** qu’une fusion. Scinder si les éléments peuvent évoluer, se décider, s’exécuter ou se retrouver **indépendamment**. Une idée + ses explications nécessaires restent ensemble. Pas une phrase = une capture.

## Intention → type (déterministe)

`intention` = référence sémantique. Appliquer le mapping de `TAXONOMY.md` **automatiquement** pour fixer `type`.

Cas obligatoires :

| intention | type | Interdit |
| --- | --- | --- |
| `orientation` | `proposition` | `decision` |
| `suggestion` | `proposition` | `decision` |
| `question` | `open_question` | `decision` |
| `idea` | `idea` | `decision` |

Proposition assistant ≠ décision utilisateur.  
`selected` / `confirmed` / `favorable` / `endorsed` ≠ `explicit_user_approval` automatique.

Détail des valeurs : `TAXONOMY.md`.

## Position, validation, status

Trois dimensions **indépendantes** (+ `space` opérationnel de rangement).

- Position = posture ; validation = preuve d’approbation formelle ; status Capture = toujours `extracted`.
- Accords partiels → tag `approval_global_partial`, `validation: none`.
- `selected` / `confirmed` / `favorable` / `endorsed` ≠ approbation automatique.
- Citation, reformulation, répétition, analyse assistant ≠ approbation utilisateur.
- Phrase assistant « je valide » ≠ `explicit_user_approval`.
- Orientation générale + ajustements demandés → ne pas marquer tous les détails comme approuvés.

## Space

| space | Usage Capture |
| --- | --- |
| `atelier` | Idées, propositions, explorations, orientations et questions **non établies** |
| `referentiel` | Connaissances / conventions / décisions **suffisamment établies** — y compris un fait stable **sans** approbation formelle |
| `journal` | Événements / historique |
| `pilotage` | Tâches, jalons, suivi |

Classer ≠ valider. `space` n’est pas une validation. Incertitude → `atelier`.  
Proposition / orientation assistant non établie → `atelier`. Ne pas exiger `explicit_user_approval` pour tout `referentiel`.  
`position: proposed` ne force pas `referentiel`. Régression : CAP-003, CAP-005, CAP-006, CAP-173, CAP-211 (ne pas réécrire).

## Préservation absolue des URLs et références

Exigence **critique**.

1. Toute URL explicitement présente dans la source et pertinente pour la CAP doit être conservée **complète** (paramètres, ancres, IDs).
2. Interdit : remplacer par « lien fourni », « URL ClickUp », troncature.
3. Stocker dans **`refs[]`** (tableau de strings exactes) **et** reprendre dans `content` / `evidence`.
4. Plusieurs URLs → toutes dans `refs[]`.
5. Chemins, noms de fichiers, IDs concrets : même discipline dans `content` + `refs` si ancre de recherche.
6. Ne jamais inventer / reconstruire une URL absente.
7. `origin.url` = URL de la **source document** éventuelle — **pas** le seul stock des liens métier du contenu.

### Exemple

Source : `https://app.clickup.com/90151633729/v/f/1201110000005979/90156746497`

```json
"content": "… folder ClickUp : https://app.clickup.com/90151633729/v/f/1201110000005979/90156746497 …",
"refs": ["https://app.clickup.com/90151633729/v/f/1201110000005979/90156746497"]
```

## Contenu autonome et retrouvabilité

- Titre **spécifique** (noms concrets : CardCarousel, Minecraft, `vision.md`, …).
- Contenu compréhensible hors source ; options A/B/C définies.
- Tags utiles : `ambiguous`, `approval_global_partial`, `potential`, sujets courts (`clickup`, `inbox`, …) — sans taxonomie de tags figée.
- Éviter titres génériques (« Point important », « Suite »).

## Relations

Priorités : `answers`/`answered_by` ; `clarifies`/`clarified_by` ; `evolves_*` ; `contradicts` (≠ simple différence) ; proposition ↔ validation/rejet.

Une précision n’est pas une contradiction. Une évolution n’est pas forcément un remplacement. Une nouvelle idée **n’efface pas** une ancienne : relier (`clarifies` / `evolves_*` / `contradicts` / `answers`). Ne pas inventer de liens. Schéma de relations **local** au fichier.

- Réciproques **recommandées** quand les deux CAP du run sont connues (`answers`↔`answered_by`, `clarifies`↔`clarified_by`, `contradicts`↔`contradicted_by`) — pas obligatoires pour `related`.
- Pas de lien pour seule proximité thématique.
- Pas de double `evolves_*` + `supersedes_candidate` sur la même paire.
- Cibles locales `CAP-NNN` uniquement (pas d’inter-runs en 0.1.5).

## Provenance — `origin.locators[]`

Inchangé vs 0.1.3 : `lines` / `offset` / `window` ; multi-passages → plusieurs locators.

## Identité

| Champ | Rôle |
| --- | --- |
| `id` | Local au fichier (`CAP-NNN`) — en production séquentielle : continuer la numérotation, ne jamais réutiliser ni réécrire |
| `run_id` | `production` ou id du dossier test-run |
| `capture_ref` | `"{run_id}:{id}"` |
| `refs` | Références littérales (URLs, …) |

## Actionable

Identifier : tâches potentielles, objectifs, jalons, sujets de cadrage, exigences, contraintes, questions ouvertes, dépendances.

`intention: task|milestone` même sans validation ni hiérarchie. Sujets de cadrage = CAP individuelles. Pas de `tasks.jsonl` ni d’objets ClickUp. L’organisation progressive relève de Brain Review.

## Écriture registre (0.1.5)

Staging séparé → `append-captures.py` seulement. Anciennes lignes **intouchables**. IDs réservés après rollback. Voir `rollback.md`.

## Trois couches

1. Validation technique (mécanique)  
2. Audit de capture (`AUDIT.md`)  
3. Évaluation humaine  

Le validateur ne fait que (1).
