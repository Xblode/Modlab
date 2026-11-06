# Schéma registre — vue Skill (0.1.5)

Source de vérité : `brain-main/registry/SCHEMA.md` + `brain-main/conventions/TAXONOMY.md`.

Compatibilité historique 0.1.x conservée. Pas de 0.1.4.1.

## Champs (delta ≥ 0.1.4 ; 0.1.5 = contrat d’écriture)

| Champ | Notes |
| --- | --- |
| `intention` | **Primaire** — choisir d’abord |
| `type` | **Dérivé** via mapping taxonomy (automatique) |
| `refs` | Tableau optionnel de strings littérales (URLs, chemins, IDs) |
| Autres | Inchangés vs 0.1.3 (`run_id`, `capture_ref`, `locators`, …) |

## Mapping intention → type

Voir taxonomy / SCHEMA. Points non négociables :

- `orientation` → `proposition` (jamais `decision`)
- `suggestion` → `proposition` (jamais `decision`)
- `question` → `open_question` (jamais `decision`)

## Space

`atelier` (exploration / non établi) \| `referentiel` (connaissance **établie**, y compris sans `explicit`) \| `journal` \| `pilotage`  
Classer ≠ valider. Proposition non établie → `atelier`.

## refs vs origin.url

- `refs[]` : liens / ancres **du contenu métier** (multiples OK).
- `origin.url` : URL éventuelle du **document source** — pas le stock multi-liens métier.

## Relations / identité

Inchangé : cibles `CAP-NNN` locales au fichier ; pas d’inter-runs.

## Validateur (lecture seule)

```bash
python brain-main/skills/brain-capture/scripts/validate-registry.py path/entries.jsonl --require-intention --json-out lab/validate-015.json
```

Fichier vide + `--require-intention` = ERROR. `--json-out` **hors** du registre. Mécanique uniquement.

## Écriture / rollback

```bash
python brain-main/skills/brain-capture/scripts/append-captures.py --registry brain/registry/entries.jsonl --staging .../entries.jsonl --source ... --run-validator --validate-json-out lab/validate-015.json
python brain-main/skills/brain-capture/scripts/rollback-captures.py --n 1
```

Voir [rollback.md](rollback.md). Relations `to` = `CAP-NNN` **local** (pas d’inter-runs).
