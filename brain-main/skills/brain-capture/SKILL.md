---
name: brain-capture
version: 0.1.5
description: >-
  Capture fidèle d'une conversation dans le registre Brain (append sécurisé,
  empreintes SHA-256, rollback /brain -c 1). Fenêtres, multi-passes,
  intention→type, locators, refs, audit final. Ne jamais réécrire les anciennes
  CAP. Utiliser pour /brain -c, rollback 1, ou extraction registre.
disable-model-invocation: true
---

# /brain -c

Capture conversationnelle → registre Brain (production ou test-run isolé).

Ce skill est appelé par `/brain -c` (voir `brain-main/bootstrap/README.md`).

**Version** : 0.1.5

La source peut être un extrait de session produit par `scripts/read-session.py` (seuls les blocs `[USER]` sont des messages humains ; voir `brain-main/bootstrap/README.md` §8).

**Protection** : aucune exécution normale ne peut détruire, réécrire, renuméroter ou supprimer silencieusement des informations déjà présentes. Append contrôlé uniquement. Rollback explicite : `/brain -c 1`.

**Philosophie** : collecter la matière utile avant consolidation. Une information non validée reste utile. Une proposition assistant non approuvée se conserve. La capture n'est pas une sélection des « meilleures » idées.

## Avant de commencer

1. [references/capture-rules.md](references/capture-rules.md)
2. [references/registry-schema.md](references/registry-schema.md)
3. [references/source-attribution.md](references/source-attribution.md)
4. `brain-main/conventions/TAXONOMY.md` (taxonomies + mapping intention→type + spaces)
5. `brain-main/registry/SCHEMA.md`
6. [references/rollback.md](references/rollback.md) si `/brain -c 1`
7. Le registre **cible**
8. Contexte léger, si présents : `brain/docs/VISION.md`, `PROJECT.md`, `ARCHITECTURE.md`, `WORKFLOW.md`, `INTEGRATIONS.md`

## Mode registre

Le mode est **choisi explicitement** selon la cible demandée (par l’utilisateur ou la consigne de session). **Aucune** bascule automatique selon le nom de fichier source.

| Mode | Cible | `run_id` |
| --- | --- | --- |
| `production` | `brain/registry/entries.jsonl` | `production` |
| `test-run` | `lab/_test-runs/MMDD-HHMM/` (dossier **vide** + copie du contrat `brain-main/registry/SCHEMA.md` si besoin) | = nom du dossier (ex. `1031-1512`) |

### Création d’un test-run

1. Calculer `MMDD-HHMM` (heure locale).
2. Si le dossier existe déjà → **ne jamais écraser**. Essayer `MMDD-HHMM-2`, puis `-3`, etc., jusqu’à un nom libre.
3. Créer le dossier vide ; y placer `entries.jsonl` (nouveau) ; `run_id` = nom final du dossier.
4. Optionnel : `COMPARISON.md` en fin de run.

Jamais de génération comparative artificielle en production. Skill-direct uniquement (pas de `_gen.py` pour fabriquer les CAP).

### Production séquentielle

- **Append only technique** : uniquement via `scripts/append-captures.py` (verrou, SHA-256, staging, historique).
- **Interdit** : `write_text` / Write / reconstruction du JSONL historique / tri / reformatage / suppression de lignes / remplacement du fichier.
- Le validateur est **lecture seule** sur le registre ; `--json-out` vers un fichier **séparé**.
- Prochain `id` = max(IDs présents, IDs **réservés** dans `_capture-history/` y compris `rolled_back`) + 1, zero-pad ≥ 3. Les trous sont acceptés.
- `capture_ref` = `production:CAP-NNN`.
- Test-run : **jamais** le registre de production ; cible `lab/_test-runs/<id>/entries.jsonl` uniquement.

### `/brain -c 1` (rollback)

Si l’utilisateur demande `/brain -c 1` (ou « annuler la dernière capture ») :

1. **Ne pas** capturer.
2. Exécuter `python brain-main/skills/brain-capture/scripts/rollback-captures.py --n 1`.
3. Suivre [references/rollback.md](references/rollback.md). En cas de refus : ne rien supprimer.

Seul `1` est supporté.

## Workflow

Ordre strict (ne pas inverser validation et corrections) :

1. **Brouillon des captures**
   1. Identifier la source exacte.
   2. **Choisir le mode** (explicite) ; créer le dossier test-run si besoin ; fixer `run_id`.
   3. Lire schéma + taxonomie + registre cible (pour IDs existants en production).
   4. Comprendre le contexte projet.
   5. **Découper en fenêtres** (~5 échanges user/assistant). Chevauchement contextuel. **Même agent / même conversation / mémoire de travail globale**. Éviter les doublons inter-fenêtres.
   6. Pour chaque fenêtre, enchaîner **4 passes** (même agent) :
      - Passe 1 — informations
      - Passe 2 — actionable
      - Passe 3 — relations
      - Passe 4 — contexte d’autonomie + **références/URLs**
   7. **Atomicité** (capture-rules) : lier plutôt que fusionner.
   8. Choisir **`intention`** (primaire) → appliquer le **mapping → `type`** (taxonomy) ; `position` (`null` si non identifiable) ; `author` + `author_confidence`.
   9. Choisir **`space`** selon taxonomy (défaut `atelier` si incertitude ; pas de `referentiel` auto pour proposition non établie).
   10. Renseigner **`grounding`**, **`origin.locators[]`**, et **`refs[]`** pour toute URL / référence concrète de la source liée à la CAP.
   11. Fixer `capture_ref` = `"{run_id}:{id}"`.
   12. `validation` / `approved_by` / `approval_scope` **uniquement** sur preuves — anti-faux-explicit.
   13. Contenu autonome + titre spécifique (retrouvabilité) ; ne pas inventer.
   14. Relations utiles (priorités capture-rules) ; cibles `CAP-NNN` locales au **fichier** (en production = tout le living).
2. **Audit final source↔registre** — voir section Audit (inclure checklist **URLs**).
3. **Appliquer TOUTES les corrections** sur le **brouillon / staging** (pas le registre actif).
4. Écrire uniquement les **nouvelles** lignes dans `brain/registry/_capture-staging/<operation-id>/entries.jsonl`.
5. **Empreinte initiale** puis append sécurisé (ne pas toucher le registre autrement) :
   `python brain-main/skills/brain-capture/scripts/append-captures.py --registry <cible>/entries.jsonl --staging <staging>/entries.jsonl --source <source> --run-validator --validate-json-out <rapport-hors-registre>`
6. Si l’append refuse ou restaure : **arrêt** ; ne pas « réparer » les anciennes CAP.
7. Garantir que le rapport validateur correspond au registre **après** append.
8. Produire `AUDIT.md` ; `COMPARISON.md` si test-run ; **bilan template**.

**Règle** : aucun changement de registre **après** validation sans re-exécuter le validateur **et** régénérer `validate.json`.

## Audit final (obligatoire)

1. Couverture thèmes / messages → CAP ou exclusion motivée.
2. Atomicité : fusions abusives ? splits manquants ?
3. Provenance : `locators[]` non vides et exacts ?
4. **URLs / refs** : toute URL source pertinente est-elle dans `refs[]` **et** dans `content` (pas « lien fourni ») ?
5. Grounding / anti-faux-explicit ; mapping intention→type respecté ?
6. Space cohérent (pas de `referentiel` abusif) ?
7. Relations prioritaires posées ; pas de validation implicite via liens.
8. Corrections appliquées **avant** le validateur.

### Template `AUDIT.md`

```markdown
# AUDIT — capture <run_id>

| | |
| --- | --- |
| Source | <chemin> |
| Cible | <chemin entries.jsonl> |
| Couche | Audit de capture (≠ validation technique ≠ évaluation humaine) |

## Couverture source↔registre
| Zone / thème | Statut | CAP(s) / remarque |
| --- | --- | --- |

## Atomicité et richesse
- Splits effectués :
- CAP courtes liées / enrichies :
- Incomplet signalé :

## Provenance et refs
- Locators : oui/non
- URLs/refs conservées : oui/non/exemples
- Multi-passages : oui/non

## Grounding, mapping, space, approbation
- intention→type OK :
- Anti-faux-explicit :
- Space (atelier par défaut) :

## Contradictions / tensions
- Relations posées :

## Corrections avant clôture
- Liste courte
```

## Trois couches de résultat

| Couche | Artefact | Rôle |
| --- | --- | --- |
| 1. Validation technique | `validate.json` + validateur | Schéma, enums, mapping, ids, relations, locators, refs structure |
| 2. Audit de capture | `AUDIT.md` | Couverture, atomicité, URLs, contradictions |
| 3. Évaluation humaine | Revue utilisateur | Fidélité réelle |

Le validateur Python **ne fait que (1)**. « OK » ≠ qualité sémantique.

## Checklist couverture

- Messages pertinents examinés ?
- Propositions assistant conservées ?
- Atomicité + titres spécifiques ?
- URLs / chemins / IDs dans `refs[]` + content ?
- Mapping intention→type appliqué ?
- Space non abusif ?
- Tâches / jalons / cadrage individuels ?
- Relations utiles (réponses, précisions, contradictions) ?
- `locators[]`, `run_id`, `capture_ref`, `grounding`, `author_confidence` ?
- Bruit de test exclu ?
- Audit final produit ?

## Interdictions

- Valider une décision ; écrire docs / ClickUp ; fusionner / renuméroter / réécrire contenu existant.
- Écrire directement dans `entries.jsonl` (Write, `write_text`, reconstruction JSON, normalisation des anciennes lignes).
- Corriger automatiquement une ancienne entrée invalide.
- Réutiliser un ID déjà présent **ou réservé** (y compris après rollback).
- Forcer test-run uniquement parce que la source s’appelle `conversation-reference-*.txt`.
- Génération comparative artificielle / `_gen.py` pour fabriquer les CAP.
- Transformer citation / analyse / « je valide » **assistant** en approbation utilisateur.
- Convertir `favorable` / `endorsed` / `selected` / `confirmed` en `explicit_user_approval` sans preuve.
- Remplacer une URL par une périphrase.
- Inventer des signatures plateforme.
- Lancer Review / Sync / Search (hors scope).
- `git reset` / `checkout` / `clean` pour « annuler » une capture.

## Bilan attendu (obligatoire)

```markdown
### Bilan `/brain -c`

| | |
| --- | --- |
| Source | <chemin ou « conversation courante »> |
| Mode | `production` \| `test-run` |
| Cible | <chemin entries.jsonl> |
| run_id | <run_id> |
| IDs ajoutés | CAP-XXX → CAP-YYY (N) |
| Validateur | OK \| KO (`--require-intention` + `--json-out`) |
| validate.json | oui/non |
| AUDIT.md | oui/non |
| Production touchée | non \| oui |

**Couches** — technique : OK/KO · audit capture : fait/partiel · éval. humaine : en attente/—

**Comptes** — total N · explicit E · none F · `set` S · partial P · `position` null Z

**Intentions** — <intention: count ; …>

**Grounding** — explicit Ge · contextual Gc · interpreted Gi · uncertain Gu

**Refs** — URLs/ancres conservées : U · formulaires « lien fourni » évités : oui/non

**Actionnable** — milestones M · tasks T · cadrage individuels C

**Relations** — R liens utiles · clarifies/contradicts : n · doubles évités : oui/non/partiel

**Ambigus** — <CAP-… + motif court ; ou « aucun »>

**Bruit exclu** — <… ; ou « aucun »>

**Fenêtres** — W traitées · mémoire globale : oui

**Couverture** — messages pertinents : OK/partiel · propositions conservées : OK/partiel · nuances : OK/partiel · locators : OK/partiel · refs : OK/partiel

<!-- test-run seulement : -->
**Test-run** — COMPARISON.md : oui/non · type : smoke-régression \| corpus \| Test-A · vs réf. <run ou —>
```

En **production**, omettre la ligne `Test-run`.

## Ressources

- [references/capture-rules.md](references/capture-rules.md)
- [references/registry-schema.md](references/registry-schema.md)
- [references/source-attribution.md](references/source-attribution.md)
- [references/test-protocol.md](references/test-protocol.md)
- [references/rollback.md](references/rollback.md)
- [references/manual-production-sequence.md](references/manual-production-sequence.md)
- [scripts/validate-registry.py](scripts/validate-registry.py)
- [scripts/append-captures.py](scripts/append-captures.py)
- [scripts/rollback-captures.py](scripts/rollback-captures.py)
- `brain-main/conventions/TAXONOMY.md`
