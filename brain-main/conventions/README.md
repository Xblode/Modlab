---
metadata_version: 1

title: "Conventions brain-main"
type: index
description: "Catalogue des conventions communes à toute instance Brain et règles d'articulation entre elles."
summary: "Liste les quatre documents du dossier, la responsabilité unique de chacun, les termes à ne pas confondre entre métadonnées de document et champs de registre, et la règle de lecture (relire les quatre pour modifier une convention, sinon seulement les pertinentes)."

version: "1.0.2"
status: review

created: "2026-10-31"
updated: "2026-11-06"

scope:
  level: universal
  targets:
    - brain-instances

updated_from:
  - "brain-main/conventions/METADATA.md"
  - "brain-main/conventions/NAMING.md"
  - "brain-main/conventions/TAXONOMY.md"

tags:
  - conventions
  - catalogue
---

# Conventions brain-main

Conventions **communes** à toute instance Brain : langage partagé des registres, documents et Skills, pas le contenu d'un projet.

## Catalogue

Chaque règle a un seul document responsable. Les autres le référencent sans la recopier. Ce fichier est l'index : il ne définit aucune règle propre en dehors de celles ci-dessous.

| Fichier | Responsabilité |
| --- | --- |
| [METADATA.md](METADATA.md) | Métadonnées des documents : front matter, propriétés, versions, dates, sources de mise à jour, héritage entre conventions ; modèle des README de dossiers maîtres (§6.9) |
| [NAMING.md](NAMING.md) | Nommage des fichiers et dossiers, casse, vocabulaire normatif (DOIT / DEVRAIT / PEUT), règles pour les agents |
| [TAXONOMY.md](TAXONOMY.md) | Taxonomies des champs de capture / registres (`intention`, `type`, `position`, `status`, `space`, relations, `grounding`, confidences, `refs`) |

## Termes à ne pas confondre

| Terme | Document (METADATA) | Entrée de registre (TAXONOMY) |
| --- | --- | --- |
| `type` | Nature du document (`convention`, `guide`, `index`…) | Nature structurelle héritée de l'entrée (`idea`, `decision`…) |
| `status` | État du document (`draft` … `archived`) | État de traitement dans le pipeline (`extracted`) |
| `tags` | Mots-clés d'indexation du document | Étiquettes de capture (`ambiguous`…) |
| `scope` / `space` | `scope` : périmètre d'application du document | `space` : boîte logique de rangement d'une capture |

## Lecture

- Modifier une convention : relire intégralement les quatre fichiers du dossier (ce README et les trois conventions), puis mettre à jour le front matter de chacun des documents touchés ([METADATA.md](METADATA.md) §8).
- Travailler sur une partie précise : lire uniquement les conventions pertinentes (nommage → NAMING ; front matter ou README de dossier → METADATA ; champs de registre → TAXONOMY).

## Règles

- Ne pas dupliquer ici le catalogue documentaire (`brain/docs/README.md`).
- Ne pas y placer les conventions d'instance (ClickUp, naming projet, etc.) → `brain/conventions/`.
- La compatibilité historique des valeurs d'enum est documentée dans [TAXONOMY.md](TAXONOMY.md).
