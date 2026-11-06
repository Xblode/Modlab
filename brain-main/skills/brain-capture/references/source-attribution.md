# Attribution des sources — `/brain -c` 0.1.5

## Hiérarchie d’attribution

Du plus fiable au moins :

1. **Métadonnées de rôle** explicites (labels `User` / `Assistant` / système).
2. **Structure conversationnelle** (alternance claire des tours, en-têtes de message).
3. **Signatures ou marqueurs réels** de plateforme **observés** (jamais inventés).
4. **Indices rédactionnels** (style) — **appoint uniquement**, jamais seuls pour `author_confidence: high`.
5. **Incertitude** — si non établissable → `uncertain` ou `low`.

Ne **pas** inventer de signatures ChatGPT / Claude / Cursor. Marqueur UI non vérifié en live → « à confirmer en live » dans `evidence` / `note`.

## Niveaux `author_confidence`

| Niveau | Quand |
| --- | --- |
| `high` | Structure source permet une attribution **fiable** (labels ou tours clairement séparés) |
| `medium` | Attribution **probable** mais indirecte (export partiellement structuré, collage avec indices) |
| `low` | Indices faibles ; doute sérieux |
| `uncertain` | Auteur non établissable correctement |

**Règle** : le style rédactionnel **seul** ne suffit jamais pour `high`.

## Formats réellement rencontrés

| Format source | Traitement typique |
| --- | --- |
| Export avec labels `User` / `Assistant` | `high` si cohérent |
| Collage brut sans labels (`conversation-reference-*.txt`) | Déduire par alternance / structure → souvent `medium` ; passages ambigus → `low` / `uncertain` |
| User colle un long bloc assistant puis commente | CAP commentaire = `author: user` ; CAP contenu cité si utile = `author: assistant` (ou uncertain) ; liées |
| Citation inline | Auteur du propos cité ≠ auteur du message actuel |

## Auteur principal vs contributions

| Concept | Lieu | Règle |
| --- | --- | --- |
| Auteur principal | `author` | Qui **porte** l’idée capturée |
| Confiance | `author_confidence` | Voir seuils ci-dessus |
| Contribution | `origin.locators[].role` + `note` | Rôle du passage, pas second auteur global |
| Enrichissement | `evidence` | Mentionner reformulation assistant |
| Idées indépendantes | deux CAP | `related` / `clarifies` — ne pas fusionner |

Pas de tableau `contributors`.

## Citations ≠ approbations

- Coller / citer ≠ approuver.
- Une citation de l’assistant **ne devient pas** une idée utilisateur.
- Une proposition assistant **approuvée** conserve `author: assistant` ; la validation est une dimension séparée.
- « Je valide » **dit par l’assistant** n’est jamais une approbation utilisateur.
- Vérifier la cohérence `author` ↔ `origin.locators[].role`. Multi-rôles + idées indépendantes → **deux CAP**.
- Réduire `author_confidence` plutôt que deviner.

### Régressions (ne pas réécrire)

| Cas | Leçon |
| --- | --- |
| CAP-270 | « Je valide son orientation générale, mais je ferais 5 ajustements… » — auteur = **qui a dit** la phrase ; locators `assistant` ⇏ `author: user`. |
| CAP-278 | « Feu vert après ajustements » — même discipline auteur / preuve d’approbation. |
| CAP-219 | Localisateurs de **plusieurs rôles** → scinder si infos indépendantes. |

## Lien avec grounding

- Attribution claire + formulation présente → souvent `source_explicit`.
- Contenu reformulé par l’agent de capture → `source_interpreted` (indépendant de `author_confidence`).
- Multi-passages → `source_contextual`.
