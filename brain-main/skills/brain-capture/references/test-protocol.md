# Protocole de test — `/brain -c` 0.1.5

## Sources officielles

| Échantillon | Fichier | Rôle |
| --- | --- | --- |
| 1 | `lab/sources/conversation-reference-1.txt` | Corpus conception initiale |
| 2 | `lab/sources/conversation-reference-2.txt` | Corpus indépendant |
| 3 | `lab/sources/conversation-reference-3.txt` | Smoke régression (≠ généralisation) |
| 4 | `lab/sources/conversation-reference-4.txt` | Capture **production** 0.1.5 |

Ne pas comparer les CAP entre sources. `erreur-1.txt` n’est pas une source officielle.

## Modes

| Mode | Usage |
| --- | --- |
| `production` | Living `brain/registry/entries.jsonl` via `append-captures.py` seulement |
| `test-run` | `lab/_test-runs/MMDD-HHMM/` (suffixe `-2`, `-3`… si collision ; jamais écraser) |

Un test-run **ne touche pas** la production.

## Types de tests

| Test | Objectif |
| --- | --- |
| Smoke protection + rollback | `scripts/run_corrective_015_tests.py` |
| Smoke validateur | fichier vide KO ; production sans flag OK |
| Capture source 4 | Production réelle 0.1.5 |
| Test A / B | Généralisation / lien — hors freeze 0.1.5 |

## Smoke technique

```bash
python brain-main/skills/brain-capture/scripts/run_corrective_015_tests.py
python brain-main/bootstrap/tests/run_commands_tests.py
python brain-main/skills/brain-capture/scripts/validate-registry.py brain/registry/entries.jsonl
```

Le validateur reste **mécanique**. Il ne certifie pas la couverture sémantique.

## Artefacts 0.1.5

| Artefact | Couche |
| --- | --- |
| `entries.jsonl` | Registre (append seulement) |
| `_capture-history/<op>/` | Traçabilité / rollback |
| `lab/validate-015.json` | 1 — technique |
| `lab/AUDIT-ref4.md` | 2 — audit capture source 4 |
| `lab/CORRECTIVE_015_REPORT.md` | Bilan correctif |

Évaluation humaine = couche 3.
