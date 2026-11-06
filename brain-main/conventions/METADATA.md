---
metadata_version: 1

title: "Convention universelle des métadonnées"
type: convention
description: "Définit les règles communes de déclaration, de structuration, de traçabilité et de validation des métadonnées dans les projets."
summary: "Convention universelle définissant le socle de métadonnées des documents, les adaptations propres aux différents formats et le modèle des README de dossiers maîtres. Elle établit les propriétés communes, leur classification, les règles d'héritage, la traçabilité des modifications et les modalités de validation automatisée."

version: "1.1.0"
status: approved

created: "2026-11-02"
updated: "2026-11-06"

scope:
  level: universal
  targets:
    - all-projects

depends_on:
  - "brain-main/conventions/NAMING.md"
  - "brain-main/conventions/TAXONOMY.md"

updated_from:
  - "brain-main/skills/brain-audit/SKILL.md"
  - "brain-main/bootstrap/README.md"

tags:
  - metadata
  - documentation
  - standardization
---

# Convention universelle des métadonnées

## 1. Objectifs et périmètre

### 1.1 Objectifs

Standard commun pour identifier, décrire, contextualiser et tracer les informations des fichiers d'un projet : compréhension par les agents IA, propriétés uniformes entre projets, type/statut/périmètre, indexation et recherche ciblée, provenance des modifications, validation automatique, compatibilité entre formats, héritage des conventions générales par les conventions spécialisées.

### 1.2 Périmètre

Tous les projets utilisant le socle commun : documents Markdown, conventions et standards, règles et instructions pour agents IA, Skills et templates, spécifications, décisions et comptes rendus, registres et données structurées, configuration, code source (dans la mesure où une documentation structurée est pertinente), ressources graphiques, fichiers de conception et documents binaires. Modèle conceptuel commun : tous les formats ne sont pas contraints à une représentation identique.

### 1.3 Hors périmètre

Non définis ici (conventions complémentaires possibles) : taxonomie complète des informations, nommage des fichiers, organisation des répertoires, architecture des registres, règles métier d'un projet, contenu documentaire, paramètres techniques des applications.

### 1.4 Conventions associées

- [NAMING.md](NAMING.md) : nommage des fichiers, y compris des fichiers associés (§6.8).
- [TAXONOMY.md](TAXONOMY.md) : valeurs des champs de registre. Ses champs `type`, `status` et `tags` ne sont **pas** les propriétés de front matter définies ici : voir [README.md](README.md).
- Cette convention applique son propre format (front matter ci-dessus) et celui de NAMING (nom du fichier).

## 2. Principes généraux

Toute implémentation doit respecter : **lisibilité** (métadonnées compréhensibles sans outil spécialisé) ; **simplicité** (ne déclarer que les propriétés ayant une utilité identifiable) ; **cohérence** (mêmes noms et types pour les mêmes concepts) ; **progressivité** (ajout de propriétés selon les besoins) ; **extensibilité** (les conventions spécialisées complètent le socle) ; **compatibilité** (respecter les possibilités natives de chaque format) ; **traçabilité** (identifier dates et sources des dernières modifications) ; **maintenance** (éviter les mises à jour manuelles inutiles) ; **non-duplication** (ne pas recopier des informations déjà disponibles dans une source fiable) ; **prévisibilité** (mêmes conventions entre projets).

### 2.1 Séparation des responsabilités

Les métadonnées ne remplacent jamais le contenu : elles permettent de comprendre et de localiser l'information, pas de reproduire l'intégralité de son contenu. Distinguer métadonnées (décrivent le document), contenu, configuration (paramètres d'un système), données métier (manipulées par le projet) et historique (événements et modifications successives). Une propriété ne doit pas être placée dans les métadonnées simplement parce qu'elle pourrait être utile à un agent IA.

## 3. Structure standard

### 3.1 Modèle commun

Les documents compatibles avec une déclaration de métadonnées utilisent le modèle suivant (vocabulaire commun, propriétés non toutes obligatoires : §4.1, §4.2) :

```
metadata_version: 1

title: "Titre du document"
type: convention
description: "Description courte du document."
summary: "Résumé du document."

version: "1.0.0"
status: approved

created: "2026-11-02"
updated: "2026-11-02"

scope:
  level: universal
  targets:
    - all-projects

extends:
  - "brain-main/conventions/PARENT.md"

depends_on:
  - "brain-main/conventions/DEPENDENCY.md"

updated_from:
  - "sources/reference.md"

tags:
  - example
```

### 3.2 Convention de nommage des propriétés

Noms de propriétés exclusivement en `snake_case`, en minuscules (`metadata_version`, `updated_from`…). Les conventions spécialisées doivent conserver cette règle. Les valeurs textuelles peuvent utiliser la langue appropriée au projet.

### 3.3 Types de données

String (`"Document de référence"`), Integer (`1`), Boolean (`true`), Date (ISO 8601, `"2026-11-02"`), Array (`["web", "mobile"]`), Object (`scope: { ... }`).

- Dates documentaires au format `YYYY-MM-DD`.
- Horodatages complets possibles lorsqu'une précision horaire est nécessaire, avec un fuseau horaire explicite.
- Les valeurs textuelles représentant des dates doivent être entre guillemets.

## 4. Définition des propriétés

### 4.1 Classification

- **Obligatoire** : doit être renseigné pour les documents concernés.
- **Recommandé** : doit être renseigné lorsque son utilité est avérée.
- **Facultatif** : peut être utilisé selon les besoins.
- **Conditionnel** : requis uniquement lorsque la condition définie par la convention s'applique. Une propriété conditionnelle peut devenir obligatoire lorsqu'une convention spécialisée le prévoit.

### 4.2 Propriétés du socle commun

| Propriété | Classification | Type | Description |
| --- | --- | --- | --- |
| `metadata_version` | Obligatoire | Integer | Version du schéma de métadonnées utilisé. |
| `title` | Obligatoire | String | Titre explicite du document. |
| `type` | Obligatoire | String | Nature du document. |
| `description` | Obligatoire | String | Description courte de sa responsabilité. |
| `status` | Obligatoire | String | État actuel du document. |
| `created` | Obligatoire | Date | Date de création initiale. |
| `updated` | Obligatoire | Date | Date de dernière modification significative. |
| `version` | Recommandé | String | Version propre au document. |
| `summary` | Recommandé | String | Synthèse permettant d'évaluer rapidement son contenu. |
| `scope` | Conditionnel | Object | Périmètre d'application du document. |
| `extends` | Conditionnel | Array | Conventions dont le document hérite. |
| `depends_on` | Facultatif | Array | Documents nécessaires à sa compréhension ou son application. |
| `updated_from` | Recommandé | Array | Sources utilisées lors de la dernière mise à jour. |
| `tags` | Facultatif | Array | Mots-clés facilitant l'indexation. |

### 4.3 `metadata_version`

Version du schéma de métadonnées (ex. `metadata_version: 1`), indépendante de la version du document ; permet aux outils d'interpréter les propriétés. Une évolution incompatible du schéma doit entraîner une nouvelle version majeure du schéma ; une propriété ajoutée de manière compatible ne nécessite pas automatiquement une nouvelle version majeure.

### 4.4 `title`

Titre humain. Doit être explicite, compréhensible sans connaître le nom du fichier, ne pas contenir inutilement le nom du projet, rester cohérent avec le contenu réel.

### 4.5 `type`

Nature du document. Valeurs du socle commun :

| Valeur | Utilisation |
| --- | --- |
| `convention` | Définit des règles communes. |
| `standard` | Définit une norme ou un modèle à respecter. |
| `guide` | Explique une méthode ou une pratique. |
| `specification` | Décrit un comportement ou une solution attendue. |
| `decision` | Formalise une décision. |
| `plan` | Décrit une organisation ou une trajectoire prévue. |
| `report` | Présente une analyse ou un compte rendu. |
| `reference` | Regroupe des informations de référence. |
| `skill` | Définit une compétence ou procédure destinée à un agent. |
| `agent` | Décrit un agent spécialisé. |
| `template` | Définit un modèle réutilisable. |
| `index` | Permet de retrouver et organiser des ressources. |
| `data` | Contient des données structurées. |

Une convention spécialisée peut ajouter de nouveaux types ; ils doivent être documentés et réutilisés de manière cohérente.

### 4.6 `description`

Responsabilité principale du document, qui doit permettre de comprendre immédiatement pourquoi le fichier existe. Recommandations : une phrase ; formulation directe ; éviter les formulations génériques ; décrire la fonction du document, pas seulement son sujet.

### 4.7 `summary`

Synthèse autonome qui doit permettre à un agent de déterminer si la lecture complète est pertinente. Recommandations : environ 2 à 5 phrases ; idéalement moins de 500 caractères ; mentionner les principaux sujets couverts ; signaler les responsabilités ou limites importantes ; éviter de répéter simplement la description. Un résumé n'est pas un remplacement du document ; il doit être mis à jour lorsque le contenu évolue suffisamment pour que l'ancien résumé devienne trompeur.

### 4.8 `version`

Version du document. Pour les conventions, standards, spécifications et ressources réutilisables, utiliser SemVer `MAJOR.MINOR.PATCH` (ex. `"1.2.0"`) :

| Niveau | Signification | Exemple |
| --- | --- | --- |
| MAJOR | Modification incompatible ou changement important du contrat. | 1.0.0 → 2.0.0 |
| MINOR | Ajout compatible de règles ou de capacités. | 1.0.0 → 1.1.0 |
| PATCH | Correction sans modification substantielle du contrat. | 1.0.0 → 1.0.1 |

S'applique aux documents possédant une véritable gestion de versions ; les notes, comptes rendus et captures ponctuelles n'ont pas besoin d'une version artificielle.

**Important :** `version` désigne toujours la version du document, jamais celle du logiciel décrit. La version d'un produit ou système décrit appartient au contenu ou à une extension spécialisée explicitement définie.

### 4.9 `status`

| Valeur | Signification |
| --- | --- |
| `draft` | Document en cours de rédaction. |
| `review` | Document soumis à vérification. |
| `approved` | Document validé pour son utilisation prévue. |
| `deprecated` | Document conservé mais dont l'utilisation est déconseillée. |
| `archived` | Document conservé à titre historique. |

- Un document `deprecated` ou `archived` ne doit pas être supprimé automatiquement.
- Lorsqu'un document est remplacé, il est recommandé d'indiquer son successeur dans son contenu ou dans une extension de relation adaptée.
- `approved` : validé selon les règles applicables à son périmètre, pas universellement applicable.

### 4.10 `created`

Date de création initiale ; ne change jamais lors des modifications ultérieures.

### 4.11 `updated`

Date de dernière modification significative (pas de dernière consultation). Doit être actualisée lorsque le contenu ou les métadonnées ayant une incidence sur son interprétation changent ; ne doit pas être modifiée pour une simple consultation, ouverture ou vérification sans changement.

### 4.12 `scope`

Périmètre d'application ; particulièrement important pour les conventions, standards, règles et instructions. Structure : `scope: {level: <valeur>, targets: [<éléments concernés>]}` en YAML (ex. `level: domain`, `targets: [web-development]` ; `level: project`, `targets: [project-alpha]`).

| `level` | Périmètre |
| --- | --- |
| `universal` | Ensemble des projets utilisant le socle commun. |
| `domain` | Domaine spécialisé. |
| `project` | Projet particulier. |

- `level` est obligatoire lorsque `scope` est renseigné.
- `targets` doit contenir au moins une valeur.
- Les valeurs doivent être identifiables et cohérentes avec la taxonomie applicable.
- Un document ne doit pas revendiquer un périmètre plus large que celui réellement couvert.
- Pour un document purement informatif sans périmètre d'application particulier, `scope` peut être omis.

### 4.13 `extends`

Conventions dont le document hérite ; réservée aux documents qui complètent ou spécialisent une convention existante (liste YAML, ex. `"brain-main/conventions/METADATA.md"`). Règles :

- Les références doivent pointer vers des documents existants.
- Les chemins sont relatifs à la racine du dépôt.
- Les règles héritées ne doivent pas être recopiées inutilement.
- Une règle locale ne peut pas modifier silencieusement une règle héritée.
- Les incompatibilités doivent être explicites et justifiées.

### 4.14 `depends_on`

Documents dont dépend la compréhension ou l'application du document courant. `extends` = héritage normatif ; `depends_on` = dépendance documentaire ou fonctionnelle (une convention peut dépendre d'un document sans en hériter). Ne pas déclarer systématiquement tous les documents mentionnés dans le contenu : réservée aux dépendances réellement utiles.

### 4.15 `updated_from`

Sources de la dernière mise à jour significative : chemin de fichier relatif à la racine du dépôt, URL, identifiant stable provenant d'un registre, ou référence à une source externe identifiable. Règles :

- Ne renseigner que les sources réellement utilisées.
- Ne pas recopier les conversations ou documents entiers.
- Préférer une référence stable plutôt qu'une description libre.
- Ne pas considérer cette propriété comme un historique complet.
- Remplacer les sources lors d'une nouvelle mise à jour significative, plutôt que d'accumuler indéfiniment les anciennes.
- Si aucune source externe ou identifiable n'a été utilisée, la propriété peut être omise.

### 4.16 `tags`

Mots-clés d'indexation. Les tags doivent : être courts ; être écrits en minuscules ; utiliser le `snake_case` si plusieurs mots sont nécessaires ; réutiliser les termes existants de la taxonomie lorsqu'ils existent ; éviter de répéter les informations déjà présentes dans `type` ou `scope`. Les tags restent facultatifs.

## 5. Propriétés conditionnelles et extensions

### 5.1 Principe

Le socle commun ne doit pas devenir une collection de propriétés spécialisées : une propriété ne concernant qu'une catégorie de documents doit être définie dans la convention spécialisée correspondante. Exemples de propriétés propres à une catégorie : `scope` (conventions, standards et règles) ; `extends` (conventions et standards héritant d'autres conventions) ; `depends_on` (documents présentant des dépendances significatives) ; `updated_from` (documents nécessitant une traçabilité des sources) ; `summary` (documents suffisamment longs ou complexes).

### 5.2 Extensions spécialisées

Les conventions spécialisées peuvent ajouter des propriétés, p. ex. un bloc `web:` (`framework: nextjs`, `rendering: server_components`) à côté des propriétés du socle dans le front matter d'une « Convention Web » (`scope.level: domain`, `extends` METADATA). Règles :

- Les propriétés du socle commun conservent leur signification.
- Une extension ne peut pas redéfinir le type ou la signification d'une propriété existante.
- Les extensions doivent être documentées.
- Les noms doivent rester explicites.
- Les propriétés spécialisées doivent être regroupées sous un espace de noms propre au domaine.
- Les propriétés inutilisées ne doivent pas être déclarées.

Un projet peut également définir une extension locale si ses besoins le justifient.

### 5.3 Compatibilité ascendante

Une convention spécialisée doit rester compatible avec le socle commun. Si une évolution nécessite de modifier une propriété universelle, la modification doit être apportée au niveau universel et faire l'objet d'une évolution explicite de la convention concernée.

## 6. Conventions par format de fichier

### 6.1 Markdown

Format de référence pour les documents. Métadonnées en YAML Front Matter placé au tout début du fichier. Règles :

1. Le front matter doit être le premier élément du fichier.
2. Il commence et se termine par une ligne contenant `---`.
3. Il respecte la syntaxe YAML.
4. Les propriétés communes utilisent les noms définis dans cette convention.
5. Le contenu Markdown commence après la fermeture du front matter.
6. Les propriétés non pertinentes sont omises.
7. Un seul bloc de métadonnées est utilisé.

Le YAML doit être compatible avec un parseur YAML 1.2. Il est recommandé d'utiliser des guillemets pour les dates, les versions et les chaînes pouvant être interprétées comme des valeurs particulières.

### 6.2 TOML

Destiné en priorité aux fichiers de configuration ; ne doit pas être utilisé comme remplacement automatique du YAML Front Matter. Lorsqu'un fichier TOML doit décrire ses propres métadonnées, elles peuvent être regroupées dans une section `[metadata]` (`metadata_version = 1`, `title = "…"`, `created = 2026-11-02`…). Règles :

- La section `[metadata]` est distincte des paramètres fonctionnels.
- Les propriétés documentaires ne doivent pas être mélangées aux paramètres métier.
- Les propriétés facultatives sont omises lorsqu'elles ne sont pas pertinentes.
- Le format TOML doit respecter sa version de spécification adoptée par le projet.

Un fichier de configuration ne doit pas être enrichi de métadonnées simplement pour respecter une uniformité visuelle.

### 6.3 JSON

Objet `_metadata` lorsque le fichier représente une ressource structurée nécessitant ses propres métadonnées (ex. `{"_metadata": {"metadata_version": 1, "title": "…", "type": "data", …}, "projects": []}`). Règles :

- `_metadata` est un espace réservé aux métadonnées documentaires.
- Les données métier restent au niveau fonctionnel prévu.
- Les propriétés non pertinentes sont omises.
- Les objets doivent respecter la syntaxe JSON standard.
- Les clés et types doivent rester cohérents avec le modèle commun.

Si le format d'un outil impose un schéma strict ne permettant pas d'ajouter `_metadata`, utiliser un index externe plutôt que de casser sa compatibilité.

### 6.4 JSONL

Données structurées en enregistrements indépendants ; la convention ne modifie pas son fonctionnement et ne crée pas de nouveau modèle de registre. Chaque ligne doit rester un objet JSON autonome et valide.

Les métadonnées doivent être intégrées à chaque enregistrement uniquement lorsque celui-ci représente un objet documentaire possédant réellement ses propres propriétés (ex. `{"id":"DOC-001","_metadata":{…},"content":"…"}`). Les propriétés propres au registre (ex. `{"id":"CAP-001","type":"capture","created":"2026-11-02"}`) ne nécessitent pas automatiquement un bloc `_metadata`. Règles :

- Ne pas ajouter de ligne d'en-tête non JSON.
- Ne pas introduire de commentaires.
- Ne pas transformer le registre en document Markdown.
- Ne pas imposer de métadonnées documentaires à chaque événement.
- Ne pas dupliquer les métadonnées déjà présentes dans le schéma fonctionnel du registre.

JSONL conserve sa fonction première : faciliter l'ajout, la lecture et le traitement automatisé des enregistrements.

### 6.5 Code source

Les fichiers de code source ne doivent pas recevoir de YAML Front Matter artificiel ; la documentation doit utiliser les mécanismes adaptés au langage (docblock TypeScript, docstring Python…). Règles :

- Utiliser les commentaires documentaires natifs.
- Documenter la responsabilité du module ou de la fonction.
- Éviter de répéter le nom du fichier, le chemin ou la version du projet.
- Ne pas imposer de propriétés documentaires à chaque fichier source.
- Préférer les mécanismes de documentation déjà adoptés par le langage ou le framework.

Les métadonnées du projet ou du logiciel doivent être déclarées dans leur fichier de référence approprié, et non répétées dans chaque fichier source.

### 6.6 Images et ressources graphiques

Métadonnées natives possibles ; selon le format, privilégier EXIF, IPTC, XMP, ou les métadonnées natives des logiciels de conception. Propriétés documentaires possibles : titre, description, date de création, auteur (lorsque pertinent), droits d'utilisation, version ou état (lorsque nécessaire). Ne pas modifier inutilement les fichiers originaux pour y intégrer des métadonnées. Attention aux informations personnelles ou techniques présentes dans les métadonnées natives, notamment les coordonnées GPS.

### 6.7 Documents bureautiques

DOCX, XLSX, PPTX et formats similaires : privilégier les propriétés natives lorsqu'elles sont disponibles. Les métadonnées externes peuvent les compléter lorsque les besoins d'indexation dépassent les capacités natives. Éviter de maintenir deux versions contradictoires des mêmes informations.

### 6.8 Fichiers binaires et formats incompatibles

Lorsque le fichier ne permet pas d'intégrer directement des métadonnées exploitables, utiliser un index externe ou un fichier associé (ex. `assets/metadata/logo.svg.json` : objet `_metadata` + `"asset": "assets/logo.svg"`). Règles :

- Le fichier associé doit identifier précisément la ressource.
- Le nommage des fichiers associés doit suivre la convention de nommage générale.
- Les chemins doivent être résolus sans ambiguïté.
- L'index externe ne doit pas devenir une copie complète du contenu original.
- La ressource originale reste la source de vérité pour son contenu.

### 6.9 README de dossier maître

Un dossier maître est un carrefour : il regroupe des fonctionnalités et l'on y décide où aller. Il porte un README, panneau de circulation de l'agent. Pas de README par fonctionnalité ni par sous-dossier ; le nombre de README d'un projet est plafonné par `readme_max` (`brain/settings.json`, 8 par défaut). Un README existant n'est jamais remplacé : une adaptation est proposée à part.

- **Lecture.** Avant de travailler dans un dossier, lire le README le plus proche (dossier courant, sinon un parent). Sans README, continuer.
- **En-tête.** Front matter du §3.1, réduit à `metadata_version`, `title`, `type: index`, `description`, `status`, `created`, `updated`. `description` dit en une phrase à quoi sert le dossier, pour qu'une IA décide d'en lire la suite ; `summary` seulement si le README est long.
- **Corps, dans cet ordre.** (1) À quoi sert ce dossier. (2) Fonctionnalités : ce qu'il fait, par ensemble cohérent. (3) Où aller ensuite : dossier, document ou commande selon l'intention. (4) Ce qu'il ne faut pas y faire. Une section sans contenu réel est omise : le modèle n'est jamais recopié vide.
- **Pas de liste de fichiers** : elle se périme. Renvoyer vers le dossier, le document ou la commande qui font foi.
- **À jour.** Un travail qui change le rôle, une fonctionnalité ou la destination d'un dossier met à jour son README dans le même travail (§8.2, §8.5).
- **Index.** Un README dont la fonction est de cataloguer des documents (par exemple `conventions/README.md`, `skills/README.md`) est un index : sa liste est son contenu, et les quatre questions ci-dessus ne lui sont pas imposées.

## 7. Hiérarchie et héritage des conventions

### 7.1 Trois niveaux d'application

- **Niveau 1 — Universel** : règles communes à tous les projets utilisant le socle (métadonnées, nommage, taxonomie, standardisation documentaire).
- **Niveau 2 — Domaine** : complète les règles universelles pour un domaine (développement Web, Mobile, direction artistique, développement logiciel).
- **Niveau 3 — Projet** : adaptations nécessaires à un projet particulier.

### 7.2 Déclaration du périmètre

Chaque convention doit déclarer son périmètre à travers `scope` (§4.12). Les chemins sont illustratifs et doivent correspondre à l'organisation réelle du dépôt.

### 7.3 Règles d'héritage

Une convention spécialisée hérite des règles de ses conventions parentes.

Elle peut : ajouter des propriétés ; ajouter des règles spécifiques ; préciser les modalités d'application ; définir des exceptions locales justifiées.

Elle ne peut pas : supprimer silencieusement une propriété obligatoire ; modifier la signification d'une propriété universelle ; contredire une convention parente sans déclaration explicite ; reproduire intégralement les règles héritées sans justification.

### 7.4 Gestion des conflits

En cas de contradiction :

1. Identifier les documents concernés.
2. Vérifier leur statut et leur périmètre.
3. Identifier la règle héritée.
4. Identifier la règle locale contradictoire.
5. Déterminer si l'exception est explicitement autorisée.
6. Signaler le conflit lorsqu'il ne peut pas être résolu par la hiérarchie.

Une convention locale ne devient pas prioritaire simplement parce qu'elle est plus récente. Toute modification d'une règle universelle doit être réalisée au niveau universel.

## 8. Gestion des versions et des mises à jour

### 8.1 Version documentaire

`version` représente l'évolution du document ; ne pas la confondre avec la version du projet, d'un logiciel, d'une API, du schéma de données, ni de la convention des métadonnées elle-même. Chaque concept doit disposer de son propre emplacement lorsqu'il est nécessaire.

### 8.2 Mise à jour de `updated`

`updated` doit être modifié lorsqu'une modification significative intervient, par exemple : modification d'une règle ; ajout ou suppression d'une section ; correction d'une information importante ; modification d'une dépendance ; modification du périmètre ; modification des métadonnées ayant une incidence sur l'interprétation. Une modification purement cosmétique et sans conséquence documentaire ne nécessite pas nécessairement une nouvelle version.

### 8.3 Gestion de `updated_from`

Lorsqu'une mise à jour repose sur des sources identifiables, les références doivent être actualisées. Exemple : avant `["brain/decisions/DEC-001.md"]` ; après une nouvelle mise à jour `["brain/decisions/DEC-001.md", "brain/decisions/DEC-005.md"]`. La liste représente les sources de la dernière mise à jour, pas l'ensemble des sources historiques du document, retrouvables dans Git ou dans les mécanismes historiques déjà utilisés par le projet.

### 8.4 Historique

L'historique détaillé des modifications ne doit pas être intégré systématiquement au front matter. Utiliser les mécanismes existants : historique Git, journal d'activité, registre des décisions, registre de provenance, historique propre à l'outil concerné. La convention ne crée pas un nouveau système d'historisation.

### 8.5 Prévention des métadonnées obsolètes

Une mise à jour documentaire doit vérifier : que `description` représente toujours le contenu ; que `summary` reste fidèle au document ; que `status` est correct ; que `updated` est actualisé ; que `version` est cohérente avec l'évolution ; que `scope` n'est pas devenu incorrect ; que les références `extends` et `depends_on` existent ; que `updated_from` reflète les sources de la dernière mise à jour. Une modification du contenu ne doit pas entraîner automatiquement une modification de toutes les propriétés.

## 9. Structure documentaire et recherche ciblée

### 9.1 Organisation des documents

Les métadonnées ne remplacent pas une structure de contenu cohérente. Les documents longs doivent utiliser des titres Markdown hiérarchisés.

### 9.2 Sections identifiables

Pour faciliter la recherche ciblée : titres explicites ; hiérarchie logique ; éviter les titres trop génériques ; numérotation lorsque la stabilité des références est importante ; éviter les sections excessivement longues.

### 9.3 Sommaire

Pas obligatoire ; recommandé lorsque le document contient suffisamment de sections pour que la navigation devienne utile. Il doit rester cohérent avec les titres réellement présents. Un sommaire généré automatiquement est préférable à un sommaire manuel lorsque l'outillage le permet.

### 9.4 Lecture sélective par les agents

Un agent doit pouvoir procéder progressivement :

1. Lire les métadonnées.
2. Évaluer la pertinence du document.
3. Lire le résumé.
4. Consulter le sommaire si nécessaire.
5. Lire uniquement les sections utiles.
6. Consulter le document complet si la tâche l'exige.

Un agent ne doit pas systématiquement charger tous les documents d'un projet.

## 10. Gestion des fichiers sans métadonnées natives

### 10.1 Principe

Tous les fichiers n'ont pas besoin d'intégrer directement leurs métadonnées. Lorsque le format ne le permet pas ou que la modification serait inappropriée, un mécanisme d'indexation externe peut être utilisé.

### 10.2 Priorité des sources

Pour déterminer les informations relatives à un fichier, respecter l'ordre suivant :

1. Métadonnées natives fiables.
2. Métadonnées intégrées au fichier.
3. Fichier associé explicitement référencé.
4. Index externe du projet.
5. Informations déduites du contexte, identifiées comme telles.

Une information déduite ne doit jamais être présentée comme une métadonnée vérifiée.

### 10.3 Cohérence

Si plusieurs sources déclarent la même propriété : identifier la source de vérité ; éviter les mises à jour concurrentes ; signaler les divergences ; ne pas écraser automatiquement une information fiable par une valeur déduite.

## 11. Bonnes pratiques pour les agents IA

### 11.1 Découverte d'un document

Lorsqu'un agent découvre un document :

1. Lire son front matter ou son mécanisme de métadonnées.
2. Vérifier son type.
3. Vérifier son statut.
4. Vérifier son périmètre.
5. Examiner ses dépendances.
6. Évaluer son résumé.
7. Lire les sections pertinentes.

L'agent doit prendre en compte les métadonnées avant d'utiliser le contenu comme référence.

### 11.2 Documents obsolètes ou remplacés

Un agent doit tenir compte de `status` :

- `draft` : ne pas considérer comme une règle officiellement validée.
- `review` : considérer comme une proposition en cours de vérification.
- `approved` : peut être utilisé dans le cadre de son périmètre.
- `deprecated` : rechercher une éventuelle alternative.
- `archived` : utiliser principalement à des fins historiques.

Le statut ne doit jamais être utilisé isolément pour déterminer la fiabilité du contenu.

### 11.3 Gestion des dépendances

Avant d'appliquer une convention spécialisée : identifier ses conventions parentes ; lire les règles héritées pertinentes ; vérifier les éventuelles exceptions ; détecter les contradictions ; ne pas supposer qu'une dépendance est facultative simplement parce qu'elle n'a pas été chargée.

### 11.4 Mise à jour des métadonnées

Un agent qui modifie un document doit : préserver les propriétés existantes valides ; modifier uniquement les propriétés concernées ; ne jamais inventer une source ; ne pas modifier artificiellement `created` ; actualiser `updated` lorsqu'une modification significative intervient ; actualiser `summary` si nécessaire ; adapter `version` selon les règles applicables ; conserver les extensions reconnues ; signaler les informations manquantes plutôt que les inventer.

### 11.5 Métadonnées et confiance

Les métadonnées décrivent un document mais ne prouvent pas à elles seules la véracité de son contenu. Un agent doit distinguer information déclarée, vérifiée, déduite, proposée, validée ; ces distinctions appartiennent au contenu, au système de provenance ou aux conventions métier appropriées, et ne doivent pas être artificiellement ajoutées au socle universel des métadonnées.

## 12. Automatisation et validation

### 12.1 Objectifs

La validation automatique doit permettre de détecter : propriétés obligatoires manquantes ; propriétés inconnues non documentées ; types incorrects ; valeurs de statut invalides ; dates mal formatées ; versions invalides ; références inexistantes ; problèmes de hiérarchie ; métadonnées incohérentes avec le contenu.

### 12.2 Niveaux de validation

Trois niveaux sont recommandés :

1. **Syntaxe** : le bloc de métadonnées peut être correctement parsé.
2. **Structure** : présence des propriétés obligatoires ; type de chaque valeur ; valeurs autorisées ; conformité des structures imbriquées.
3. **Cohérence** : cohérence entre `title`, `description` et le contenu ; validité des chemins ; cohérence des dépendances ; compatibilité des extensions ; cohérence entre le statut et l'usage du document.

### 12.3 Règles de validation

| Contrôle | Niveau | Résultat attendu |
| --- | --- | --- |
| Présence de `metadata_version` | Syntaxe / structure | Obligatoire |
| Présence de `title`, `type`, `description`, `status`, `created`, `updated` | Structure | Obligatoire |
| Format des dates | Structure | ISO 8601 |
| Valeur de `status` | Structure | Valeur autorisée |
| Format de `version` | Structure | SemVer si versionnement applicable |
| Structure de `scope` | Structure | Conforme au modèle |
| Existence de `extends` / de `depends_on` | Cohérence | Référence valide |
| Validité de `updated_from` | Cohérence | Référence exploitable |
| Fidélité de `summary` | Cohérence | Vérification sémantique |

Les contrôles conditionnels doivent tenir compte du type de document. Un fichier sans `scope` n'est pas nécessairement non conforme.

### 12.4 Tolérance aux extensions

Un validateur universel doit accepter les propriétés supplémentaires lorsqu'elles appartiennent à une extension reconnue, et ne pas rejeter systématiquement un document contenant une propriété spécialisée. Une propriété inconnue et non documentée doit pouvoir être signalée.

### 12.5 Sévérité des erreurs

- **Error** : métadonnées invalides ou incompatibles avec le contrat.
- **Warning** : métadonnées incomplètes ou incohérentes sans blocage immédiat.
- **Info** : amélioration facultative ou recommandation.

Une erreur de validation ne doit pas entraîner automatiquement la suppression ou la correction destructive d'un document.

### 12.6 Validation automatisée du contenu

Certaines propriétés ne peuvent pas être validées par un simple schéma (résumé fidèle au contenu, description suffisamment précise, dépendance réellement nécessaire, périmètre correctement défini). Ces vérifications peuvent être assistées par un agent IA, mais doivent être distinguées des validations structurelles déterministes.

## 13. Intégration avec Capture et Review

### 13.1 Capture

Responsable de la collecte initiale. Peut : identifier les documents concernés ; lire leurs métadonnées ; extraire les informations pertinentes ; conserver les références des sources ; proposer des métadonnées initiales lorsqu'un nouveau document est créé. Ne doit pas modifier arbitrairement les documents de référence existants. Les informations incertaines doivent rester identifiées comme telles.

### 13.2 Review

Responsable de la consolidation. Peut : détecter les métadonnées manquantes ; identifier les documents dont les descriptions sont obsolètes ; détecter les incohérences entre résumé et contenu ; vérifier les références et dépendances ; proposer des modifications ; mettre à jour les propriétés lorsque les règles d'autorisation le permettent.

### 13.3 Préservation de l'historique

Capture et Review doivent respecter les règles suivantes :

1. Ne jamais modifier `created` après sa création.
2. Ne jamais inventer une source de mise à jour.
3. Ne pas effacer les références historiques conservées dans les mécanismes de provenance.
4. Ne pas remplacer silencieusement un document validé.
5. Ne pas transformer une proposition en décision validée sans autorisation appropriée.
6. Ne pas confondre mise à jour des métadonnées et modification du contenu.
7. Préserver les propriétés spécialisées.
8. Utiliser les mécanismes historiques existants pour tracer les changements.

Une correction de métadonnées peut être réalisée sans modifier le contenu documentaire si celui-ci reste valide.

## 14. Exemples de conformité

### 14.1 Convention universelle

`metadata_version`, `title`, `type: convention`, `description`, `version`, `status`, `created`, `updated`, `scope` (`level: universal`, `targets: [all-projects]`) : propriétés communes présentes, périmètre explicite.

### 14.2 Convention spécialisée

Comme 14.1 avec `scope.level: domain`, `targets: [web-development]` et `extends` (METADATA, SOFTWARE) : périmètre déclaré, conventions parentes référencées sans recopier leurs règles.

### 14.3 Document informatif simple

Seulement `metadata_version`, `title`, `type: report`, `description`, `status`, `created`, `updated` : versionnement et dépendances omis car non nécessaires.

### 14.4 Document avec sources

`type: report`, `summary`, `version`, `status: review`, `updated_from` (`brain/research/RESEARCH-001.md`, `https://example.com/documentation`) : sources de la dernière mise à jour identifiables.

## 15. Exemples de non-conformité

### 15.1 Propriétés obligatoires manquantes

Front matter avec seulement `title` et `description` : `metadata_version`, `type`, `status`, `created`, `updated` absents.

### 15.2 Noms incohérents

`documentTitle`, `document_type`, `lastUpdate` : noms différents de ceux du socle commun.

### 15.3 Version ambiguë

`version: "V2"` : non conforme pour un document versionné selon SemVer (format non standard).

### 15.4 Périmètre mal structuré

`scope: "Tous les projets"` : non conforme pour une convention nécessitant un périmètre (structure attendue non respectée).

### 15.5 Dépendance non résolue

`extends: ["brain-main/conventions/UNKNOWN.md"]` : non conforme, la convention parente référencée n'existe pas.

### 15.6 Métadonnées redondantes

`project_name`, `software_version`, `repository` dans le front matter : non conforme aux principes de conception si ces informations sont déjà disponibles dans une source fiable et n'ont aucune nécessité documentaire. Les métadonnées ne doivent pas devenir une copie du contexte complet du projet.

## 16. Recommandations d'implémentation

### 16.1 Priorités

L'implémentation doit être progressive :

1. **Standard documentaire** : YAML Front Matter pour Markdown ; propriétés communes ; noms et types uniformes ; statuts et types documentés.
2. **Validation** : parseur YAML ; propriétés obligatoires, valeurs autorisées, références documentaires.
3. **Automatisation** : lecture des métadonnées par les agents ; mises à jour facilitées ; vérification des résumés et dépendances ; contrôles intégrés dans Capture et Review.
4. **Formats complémentaires** : règles d'intégration JSON et TOML ; compatibilité JSONL préservée ; métadonnées natives exploitées ; index externe uniquement lorsque nécessaire.

### 16.2 Principe de validation

La validation doit être centralisée autour d'un même modèle conceptuel. Il ne faut pas développer une logique documentaire différente pour chaque type de fichier : chaque format dispose d'un mécanisme d'extraction adapté, puis transmet les propriétés extraites au même moteur de validation.

### 16.3 Évolution de la convention

Toute évolution doit : préserver la compatibilité lorsque cela est possible ; documenter les nouvelles propriétés ; identifier les propriétés devenues obsolètes ; prévoir une migration lorsqu'un changement incompatible intervient ; éviter les migrations massives sans bénéfice réel ; mettre à jour les exemples et les règles de validation.

## 17. Règles finales

Contrat minimal de cette convention :

1. Les documents Markdown utilisent un YAML Front Matter lorsqu'ils nécessitent des métadonnées documentaires.
2. Les propriétés communes utilisent les noms définis dans cette convention.
3. Les propriétés obligatoires doivent être présentes.
4. Les propriétés non pertinentes doivent être omises.
5. Les propriétés spécialisées doivent être documentées dans leur convention respective.
6. Les conventions déclarent leur périmètre avec `scope`.
7. L'héritage normatif est déclaré avec `extends`.
8. Les dépendances documentaires sont déclarées avec `depends_on`.
9. La version documentaire est indépendante de la version du logiciel décrit.
10. Les dates respectent ISO 8601.
11. Les sources de la dernière mise à jour sont déclarées avec `updated_from` lorsqu'elles sont identifiables.
12. L'historique détaillé reste dans les mécanismes existants.
13. Les formats incompatibles ne doivent pas recevoir artificiellement des métadonnées.
14. Les extensions ne doivent pas modifier silencieusement le sens du socle commun.
15. Les agents IA doivent préserver les métadonnées existantes et ne jamais inventer les informations manquantes.

## 18. Glossaire

Définis ailleurs : métadonnées (§2.1), front matter (§6.1), extension (§5.2), périmètre (§4.12), héritage (§4.13), dépendance (§4.14), version documentaire (§4.8). Socle commun : ensemble des propriétés universelles. Provenance : origine d'une information ou d'une modification. Validation : vérification de la conformité structurelle et sémantique. Index externe : structure décrivant des fichiers ne supportant pas directement les métadonnées.
