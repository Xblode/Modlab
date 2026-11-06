# Rollback `/brain -c 1` — Brain Capture 0.1.5

Le chiffre `1` désigne **une exécution complète** de Capture, jamais une entrée CAP. Seul `n=1` est supporté.

Cette commande **n’est pas** une nouvelle capture.

## Commande

```bash
python brain-main/skills/brain-capture/scripts/rollback-captures.py --n 1
```

Équivalent Skill : `/brain -c 1`.

## Historique

`brain/registry/_capture-history/<operation-id>/`

| Fichier | Rôle |
| --- | --- |
| `manifest.json` | id, date, version Skill, source, IDs, SHA/taille avant/après, statut |
| `before-registry.jsonl` | Copie **byte-for-byte** du registre avant ajout |
| `appended-entries.jsonl` | Lot ajouté exact |
| `reverted-entries.jsonl` | Copie du bloc retiré après rollback réussi |

Statuts : `started` \| `committed` \| `failed` \| `rolled_back`.

Un dossier d’opération n’est **jamais** réécrit silencieusement par une opération suivante.

## Préconditions (sinon : refus, aucune suppression)

1. Une opération `committed` non déjà `rolled_back` existe.
2. SHA-256 et taille du registre actif = `after` du manifeste.
3. Snapshot `before-registry.jsonl` intact (SHA = `before`).
4. Registre actif = `before-registry.jsonl` + `appended-entries.jsonl` **exactement**.
5. Le bloc ajouté est encore le **suffixe** du fichier.

Échecs : aucune opération ; registre changé depuis ; autre capture ajoutée ; snapshot inverifiable ; déjà annulé ; état corrompu ou ambigu.

## Exécution si conforme

1. Archiver le suffixe dans `reverted-entries.jsonl`.
2. Restaurer le snapshot par écriture atomique (`os.replace`).
3. Recalculer hash/taille ; valider la restauration.
4. Marquer `rolled_back`.
5. Conserver tous les fichiers de récupération.

Interdit : `git reset` / `checkout` / `clean` ; deviner quelles lignes retirer ; réutiliser les IDs de l’opération annulée.

## IDs réservés

Les IDs d’une opération `rolled_back` restent réservés. Le prochain ID = max(IDs présents, IDs des manifestes) + 1. Les trous de numérotation sont acceptés.

## Écriture normale

Seul [scripts/append-captures.py](../scripts/append-captures.py) peut ajouter des CAP. Le rollback est le **seul** cas où des lignes quittent le registre actif.
