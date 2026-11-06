# Instructions — test manuel production séquentiel (3 sources)

**Ne pas exécuter automatiquement.** L’humain (ou un agent dédié sur consigne) lance **trois** `/brain -c` successifs.

## Prérequis workspace isolé

1. Copier / ouvrir un workspace de test contenant le Skill 0.1.4 et les trois sources.
2. Registre **vide** : `brain/registry/entries.jsonl` = fichier vide (0 octet) ou absent puis créé vide au premier append.
3. **Pas** de dossier `_test-runs/` utilisé pour ce test.
4. **Pas** de `_capability-test/`.
5. **Pas** de `_gen.py`, reconstruction, Review, Sync.
6. Mode **`production`** explicite à chaque appel (`run_id: production`).

## Appel 1 — source 1

```
/brain -c
```

Consigne minimale à coller :

> Mode **production**. Source unique : `lab/sources/conversation-reference-1.txt`.  
> Append dans `brain/registry/entries.jsonl` (registre initialement vide).  
> Appliquer Skill 0.1.4 (atomicité, mapping intention→type, space, refs/URLs, relations).  
> Ne pas modifier le Skill. Produire AUDIT.md dans `lab/AUDIT-ref1.md` (hors living).  
> Valider avec `--require-intention --json-out lab/validate.json`.

### Vérifications après appel 1

| Check | Attendu |
| --- | --- |
| Nombre d’entrées N1 | > 0 |
| IDs | uniques, `CAP-001`… |
| `run_id` | `production` |
| Validateur | OK bloquant |
| URLs | si présentes dans la source (ex. ClickUp L1277) → dans `refs[]` + content |
| Anciennes | n/a |

## Appel 2 — source 2

Même skill, **sans** vider le registre.

> Mode **production**. Source : `lab/sources/conversation-reference-2.txt`.  
> **Append only** — ne modifier aucune ligne existante. Continuer les IDs après le max actuel.

### Vérifications après appel 2

| Check | Attendu |
| --- | --- |
| Total | N1 + N2 |
| Intégrité | hash/contenu des N1 premières lignes **identique** à après appel 1 |
| IDs | pas de collision |
| Validateur | OK sur fichier complet |
| Mapping / space | spot-check orientations ≠ type decision ; suggestions en atelier |

## Appel 3 — source 3

> Mode **production**. Source : `lab/sources/conversation-reference-3.txt`.  
> Append only ; IDs à la suite.

### Vérifications après appel 3

Même grille + total N1+N2+N3 ; intégrité des blocs 1 et 2 inchangés.

## Audit final cumulé (manuel)

Sans reconstruction de projet : confronter les **trois sources** au `entries.jsonl` living :

1. Couverture thématique par source.
2. URLs / refs critiques.
3. Atomicité (fusions abusives).
4. Mapping intention→type.
5. Space.
6. Relations intra-fichier (réponses, contradictions).
7. Absence de réécriture historique.

Documenter dans un `AUDIT-CUMUL.md` (hors automatisation obligatoire de ce correctif).
