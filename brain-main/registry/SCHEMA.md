# Schéma du registre Brain (`entries.jsonl`)

Chaque ligne de `entries.jsonl` est un objet JSON unique. Contrat **0.1.4** (extension compatible de 0.1.3).

`entries.jsonl` reste le registre de **capture**. D’autres registres spécialisés sont une orientation future — non créés automatiquement ici.

Taxonomies sémantiques : `brain-main/conventions/TAXONOMY.md`.

## Champs

| Champ | Obligatoire | Description |
| --- | --- | --- |
| `id` | oui | Identifiant local au fichier (`CAP-NNN`) |
| `run_id` | oui en génération **≥ 0.1.3** ; absent OK sur historiques | Ex. `production`, `1031-1512` |
| `capture_ref` | oui ≥ 0.1.3 | `"{run_id}:{id}"` |
| `uid` | non | UUID optionnel |
| `type` | oui | Enum 0.1.1 — **dérivé de `intention`** en ≥ 0.1.4 (voir mapping) |
| `intention` | oui ≥ 0.1.3 | Référence sémantique principale (voir taxonomy) |
| `position` | oui ≥ 0.1.3 (`null` OK) | Posture |
| `status` | oui | Capture : toujours `extracted` |
| `space` | oui | `atelier` \| `referentiel` \| `journal` \| `pilotage` |
| `validation` | oui | `none` \| `implicit` \| `explicit_user_approval` |
| `author` | oui | `user` \| `assistant` |
| `author_confidence` | oui ≥ 0.1.3 | `high` \| `medium` \| `low` \| `uncertain` |
| `approved_by` | oui | `user` \| `null` |
| `approval_scope` | oui | `item` \| `set` \| `null` |
| `title` / `content` / `evidence` | oui | Contenu autonome ; titres spécifiques |
| `confidence` | oui | Extraction : `high` \| `medium` \| `low` |
| `grounding` | oui ≥ 0.1.3 | `source_explicit` \| `source_contextual` \| `source_interpreted` \| `uncertain` |
| `origin` | oui | Provenance (ci-dessous) |
| `refs` | non (mais **requis en pratique** dès qu’une URL/réf. concrète est dans la source pour cette CAP) | Tableau de strings littérales (URLs complètes, chemins, IDs) |
| `project` / `created_at` / `storage` | oui | |
| `relations` / `clickup_ref` / `feeds_doc` / `tags` | non | |

### Mapping `intention` → `type` (générations ≥ 0.1.4)

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

Sous `--require-intention`, une entrée **nouvelle** doit respecter ce mapping (ERROR sinon). Les historiques hors mapping ne sont **pas** réécrits ; hors flag, pas d’ERROR mapping.

### `refs[]` (0.1.4)

```json
"refs": [
  "https://app.clickup.com/90151633729/v/f/1201110000005979/90156746497",
  "brain/docs/vision.md"
]
```

- Chaque élément : string non vide, **littérale** (pas de périphrase).
- URLs : conserver paramètres et ancres.
- `origin.url` reste optionnel (URL du document source) — **ne remplace pas** `refs` pour les liens métier du contenu.
- Absent ou `[]` OK s’il n’y a aucune référence concrète ; si le content évoque un « lien » sans URL → signal REVIEW côté validateur.

### `origin` (locators inchangés vs 0.1.3)

```json
{
  "source": "lab/sources/conversation-reference-1.txt",
  "url": null,
  "locators": [
    {"kind": "lines", "start": 120, "end": 125, "role": "user"}
  ]
}
```

Conventions `lines` / `offset` / `window` : identiques à 0.1.3.

### Identité et relations

- `capture_ref` = `run_id + ":" + id`.
- `relations[].to` = `CAP-NNN` **local au même fichier** (en production living = tout `entries.jsonl`).
- Pas de relations inter-fichiers / inter-runs en 0.1.4.
- Append only : ne pas réécrire les anciennes lignes.

### Approbation / space

- `explicit_user_approval` ⇒ `approved_by: user` et `approval_scope` ∈ {`item`,`set`}.
- `selected` / `confirmed` / `favorable` / `endorsed` ≠ validation automatique.
- `space` ≠ validation ; propositions non établies → `atelier`.

## Trois couches

1. Validation technique (mécanique) — 2. Audit de capture — 3. Évaluation humaine.  
« OK » validateur ≠ qualité sémantique.

## Validateur

```bash
python brain-main/skills/brain-capture/scripts/validate-registry.py brain/registry/entries.jsonl
python brain-main/skills/brain-capture/scripts/validate-registry.py path/entries.jsonl --require-intention
python brain-main/skills/brain-capture/scripts/validate-registry.py path/entries.jsonl --require-intention --json-out validate.json
```

`--require-intention` (générations ≥ 0.1.3/0.1.4) : vide = ERROR ; exige intention, position, author_confidence, grounding, run_id, capture_ref ; origin+locators ; **mapping intention→type** ; `refs` si présent doit être un tableau de strings non vides.
