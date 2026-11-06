---
metadata_version: 1

title: "Convention de nommage"
type: convention
description: "Définit les principes communs de nommage des fichiers et dossiers des projets utilisant le socle de standardisation."
summary: "Convention universelle de nommage des fichiers et dossiers : concept et responsabilité avant description, noms courts, contexte fourni par l'arborescence, casse par type de fichier, centralisation des variantes, portabilité, exceptions et règles applicables aux agents IA. Elle ne couvre pas les identifiants internes au code."

version: "1.0.1"
status: review

created: "2026-11-02"
updated: "2026-11-06"

scope:
  level: universal
  targets:
    - all-projects

depends_on:
  - "brain-main/conventions/METADATA.md"

updated_from:
  - "brain-main/conventions/METADATA.md"
  - "brain-main/conventions/TAXONOMY.md"
  - "brain-main/conventions/README.md"

tags:
  - naming
  - files
  - standardization
---

# NAMING — Convention de nommage

## 1. Objectifs et périmètre

### 1.1. Objectif

Principes communs de nommage des fichiers et dossiers : organisation simple et compréhensible, cohérente entre projets, facile à parcourir pour humains et agents IA, adaptable aux technologies, évolutive sans multiplication inutile des règles. Le nommage doit permettre d'identifier rapidement la responsabilité d'un élément, en tenant compte de son emplacement dans l'arborescence.

Conventions associées (catalogue : [README.md](README.md)) : [METADATA.md](METADATA.md) pour le front matter des fichiers Markdown, [TAXONOMY.md](TAXONOMY.md) pour les valeurs des champs de registre. Les fichiers de métadonnées associés à une ressource suivent cette convention de nommage (METADATA §6.8).

### 1.2. Périmètre

Couvre : documents et documentation, dossiers et sous-dossiers, code source, modules/services/composants, configuration, données et registres, ressources et multimédias, fichiers générés et temporaires.

Définit uniquement le nommage des fichiers et dossiers. Variables, fonctions, classes, propriétés, composants au niveau de leur déclaration et autres identifiants internes relèvent des conventions propres aux langages et technologies.

### 1.3. Vocabulaire normatif

| Terme | Signification |
| --- | --- |
| DOIT | Règle obligatoire, sauf contrainte technique incompatible. |
| NE DOIT PAS | Pratique à proscrire, sauf nécessité technique. |
| DEVRAIT | Pratique recommandée par défaut. |
| PEUT | Possibilité autorisée selon le contexte. |

Une préférence ne devient pas une obligation simplement parce qu'elle apparaît dans cette convention.

## 2. Philosophie générale

### 2.1. Nommer selon le concept et la responsabilité

Un nom doit représenter le concept principal ou la responsabilité de l'élément. Le nommage doit privilégier la fonction réelle du fichier plutôt que sa description détaillée. Un nom court est préférable lorsque le contexte permet de comprendre sa signification.

| À privilégier | À éviter par défaut |
| --- | --- |
| `card.tsx` | `main-card-component.tsx` |
| `payment.ts` | `payment-processing-service-module.ts` |
| `VISION.md` | `PROJECT_GLOBAL_VISION.md` |
| `ARCHITECTURE.md` | `PROJECT_TECHNICAL_ARCHITECTURE.md` |
| `DECISIONS.md` | `ARCHITECTURAL_DECISION_REGISTER.md` |

### 2.2. Privilégier les noms courts

Les noms d'un seul mot sont privilégiés lorsqu'ils sont suffisamment explicites (`components/`, `services/`, `card.tsx`, `ARCHITECTURE.md`). Les noms composés restent autorisés lorsqu'ils apportent une distinction utile (`user-profile.ts`, `payment-gateway.ts`, `PROJECT_BRIEF.md`). La simplicité ne doit jamais rendre le nom ambigu.

### 2.3. Utiliser le contexte fourni par l'arborescence

Le chemin fait partie de l'identification : le nom ne doit pas répéter inutilement des informations déjà présentes dans les dossiers parents. Recommandé : `components/card.tsx`, `components/button.tsx` ; à éviter : `components/component-card.tsx`. Même principe pour la documentation, les services, les ressources et les autres catégories.

### 2.4. Éviter les noms artificiellement descriptifs

Ne pas accumuler des qualificatifs sans distinction réelle. Éviter, lorsqu'ils sont redondants : `main`, `primary`, `global`, `default`, `base`, `manager`, `handler`, `component`, `module`, `file`.

Ces termes ne sont pas interdits : ils doivent correspondre à une distinction fonctionnelle réelle. `user-manager.ts` peut être pertinent si le concept de manager existe réellement dans l'architecture ; `main-card.tsx` n'est pas justifié simplement parce que le composant est le plus utilisé.

### 2.5. Favoriser la stabilité

Les noms doivent rester valables pendant la durée de vie de leur responsabilité. Éviter les noms temporaires ou liés à un état de développement (`new-card.tsx`, `card-v2.tsx`, `final-architecture.md`, `test-final.md`). Une évolution doit normalement modifier le contenu ou la version documentaire, pas entraîner une succession de renommages.

## 3. Conventions générales de casse

### 3.1. Convention par défaut

Le `kebab-case` (minuscules, mots séparés par des tirets : `user-profile.ts`, `design-system/`) est la convention technique générale, recommandée pour les fichiers techniques, les ressources et les dossiers lorsqu'aucune contrainte spécifique ne s'applique.

### 3.2. Documents de référence

Les documents structurants et de référence utilisent de préférence le `UPPER_SNAKE_CASE` (mots en majuscules séparés par des underscores : `VISION.md`, `ROADMAP.md`, `PROJECT_BRIEF.md`, `TECHNICAL_OVERVIEW.md`). Les noms d'un seul mot restent privilégiés lorsqu'ils suffisent. Cela distingue visuellement les documents de référence des fichiers techniques et des notes ordinaires.

Les noms imposés par des outils (`README.md`, `AGENTS.md`, `CLAUDE.md`, `SKILL.md`) sont conservés. Les autres fichiers Markdown peuvent utiliser le `kebab-case` (ex. `docs/guides/installation.md`, `docs/notes/2026-11-02-review.md`).

### 3.3. Conventions complémentaires

Les autres styles peuvent être utilisés lorsqu'ils sont imposés ou recommandés par une technologie.

| Convention | Exemple | Usage possible |
| --- | --- | --- |
| `kebab-case` | `user-profile.ts` | Convention technique générale |
| `UPPER_SNAKE_CASE` | `PROJECT_BRIEF.md` | Documents de référence |
| `snake_case` | `user_profile.py` | Modules Python |
| `camelCase` | `userProfile.js` | Certains projets JavaScript |
| `PascalCase` | `UserProfile.java` | Classes et fichiers associés dans certains langages |

Il ne faut pas forcer une technologie à adopter une casse incompatible avec ses pratiques établies.

### 3.4. Extensions

Les extensions doivent être conservées et utiliser de préférence les minuscules (`card.tsx`, `config.json`). Les extensions multiples sont autorisées lorsqu'elles correspondent à une convention technique (`card.test.tsx`, `card.module.css`). Il ne faut pas créer artificiellement des extensions personnalisées pour remplacer une organisation documentaire cohérente.

## 4. Principes de nommage des fichiers

### 4.1. Identifier la responsabilité principale

Un fichier doit avoir une responsabilité identifiable (`card.tsx`, `payment.ts`, `authentication.ts`). Il peut contenir plusieurs éléments étroitement liés, à condition qu'ils participent au même concept. Le nom ne doit pas nécessairement énumérer tous les éléments contenus.

### 4.2. Utiliser des noms composés lorsque nécessaire

Un nom composé est pertinent lorsqu'il distingue plusieurs éléments de même catégorie (`services/payment.ts`, `payment-gateway.ts`, `payment-provider.ts`). Il peut préciser un domaine, une fonctionnalité, une plateforme, un format, un rôle technique ou une distinction fonctionnelle. Le nombre de mots n'est pas limité arbitrairement, mais chaque mot doit apporter une information utile.

### 4.3. Éviter les préfixes redondants

Les préfixes correspondant au type de fichier sont généralement inutiles lorsque l'extension ou le dossier suffit (`components/card.tsx` plutôt que `components/component-card.tsx`). Cette règle ne concerne pas les préfixes imposés par une technologie ou utiles à la découverte automatique.

### 4.4. Abréviations

Les abréviations reconnues et largement utilisées sont autorisées (`api`, `ui`, `db`, `css`, `svg`, `auth`). Les abréviations propres à un projet ou difficiles à comprendre sont à éviter. Le nom doit rester interprétable sans documentation supplémentaire pour chaque fichier.

## 5. Principes de nommage des dossiers

### 5.1. Convention générale

Les dossiers utilisent par défaut le `kebab-case` en minuscules (`components/`, `design-system/`, `user-settings/`). Les noms courts sont privilégiés.

### 5.2. Noms génériques

Autorisés lorsque leur périmètre est suffisamment clair : `docs/`, `src/`, `assets/`, `components/`, `hooks/`, `services/`, `styles/`, `types/`, `tests/`, `config/`. Leur utilisation doit être cohérente dans le projet. Un dossier générique ne doit pas devenir un espace de stockage pour des éléments sans relation.

### 5.3. Organisation par domaine

Lorsque le projet possède plusieurs domaines fonctionnels, l'organisation par domaine (ex. `features/auth/{components,services,types}`, `features/products/…`) peut être préférable à une multiplication des dossiers techniques globaux ; elle regroupe les éléments d'une même fonctionnalité. Elle ne doit pas être imposée à tous les projets.

### 5.4. Profondeur de l'arborescence

L'arborescence doit rester aussi simple que possible. Un niveau de dossier supplémentaire est justifié lorsqu'il regroupe plusieurs éléments cohérents, définit un domaine identifiable, sépare des responsabilités distinctes, correspond à une exigence technique ou améliore réellement la navigation. Éviter de créer des dossiers contenant uniquement un fichier, lorsque cette séparation n'apporte aucun bénéfice.

> **Note d'auteur (non normative, à arbitrer), reprise telle qu'écrite :** si on commence un projets par quelque choses de petit il faut pas non plus qu'il prévoit trop simple alors je pense qu'il faut toujours prévoir un projets moyen qui va facilement s'upgrade vers en gros, imagine que de base tu mes card.tsx dans un dossier général a la création du site car c'est une nouvelle création, bas on va devoir le déplacer plus tard

### 5.5. Singularité et pluralité

La pluralité peut désigner une collection de ressources (`components/`, `services/`, `hooks/`, `templates/`) ; la singularité convient aux dossiers représentant un domaine ou une fonctionnalité (`auth/`, `payment/`, `profile/`). Ce n'est pas une obligation grammaticale absolue : la cohérence de l'arborescence prime.

## 6. Conventions selon les formats et technologies

Recommandations générales ; les conventions spécialisées restent responsables des détails propres à leurs technologies.

### 6.1. Documents Markdown

Documents structurants en `UPPER_SNAKE_CASE` de préférence (`VISION.md`, `ARCHITECTURE.md`, `ROADMAP.md`, `DECISIONS.md`, `NAMING.md`) ; documents secondaires, notes et guides en `kebab-case` possible (`installation.md`, `deployment-guide.md`, `2026-11-02-review.md`) : voir §3.2. Les conventions propres aux projets peuvent préciser la nomenclature documentaire.

### 6.2. Templates

Les templates doivent être nommés selon le document ou l'élément qu'ils permettent de générer (ex. `templates/project/ARCHITECTURE.md`, `templates/documents/meeting.md`). Éviter `template-1.md`, `new-template.md`, `template-final.md`.

### 6.3. Skills, règles et agents

Les noms doivent identifier leur fonction (ex. `skills/project-review/SKILL.md`). Les noms de fichiers imposés par une plateforme ou un système d'agents doivent être conservés. La convention spécialisée détermine les formats exacts attendus par chaque environnement.

### 6.4. Code source

Les fichiers de code utilisent par défaut le `kebab-case`, sauf convention propre au langage ou au framework (`card.tsx`, `user-service.ts`, `use-cart.ts`). Ils doivent représenter un concept ou une responsabilité ; leur nommage ne doit pas être confondu avec celui des fonctions, classes ou identifiants qu'ils contiennent.

### 6.5. Composants React

Pour les projets React adoptant cette convention, les fichiers de composants utilisent par défaut le `kebab-case` (`card.tsx`, `button.tsx`, `modal.tsx`, `navigation.tsx`), sans noms redondants tels que `card-component.tsx`. Le nom du composant déclaré respecte les conventions React, notamment le `PascalCase` (`export function Card()`). Nom de fichier et nom de composant répondent à deux responsabilités différentes.

React exige que les noms des composants commencent par une majuscule, mais ne prescrit pas une casse universelle pour les noms de fichiers : la convention du projet tranche ce point.

### 6.6. Hooks

Les hooks peuvent utiliser le préfixe `use-` dans le nom du fichier (`hooks/use-auth.ts`, `use-cart.ts`, `use-theme.ts`) pour faciliter leur identification. Les règles de déclaration et de nommage des fonctions restent celles de la convention React/JavaScript spécialisée.

### 6.7. Services et modules

Les services utilisent un nom correspondant au domaine ou à leur responsabilité (`services/payment.ts`, `authentication.ts`, `notification.ts`). Il n'est pas nécessaire d'ajouter systématiquement le suffixe `service`. Un suffixe reste acceptable lorsque plusieurs types de fichiers portant le même nom coexistent dans un même périmètre ou lorsqu'un outil l'impose.

### 6.8. Fichiers de types

Les fichiers de types utilisent généralement le nom du domaine concerné (`types/product.ts`, `user.ts`, `payment.ts`). Un fichier `types.ts` est autorisé lorsque son emplacement définit clairement son périmètre. Éviter les fichiers globaux qui regroupent des types sans relation.

### 6.9. Tests

Les fichiers de tests utilisent les conventions du framework de test choisi (`card.test.tsx`, `card.spec.tsx`). Le projet doit privilégier une convention cohérente. Le suffixe `test` ou `spec` est pertinent : il permet la découverte automatique et différencie le code source des tests.

### 6.10. Styles

Les fichiers de styles utilisent le nom de l'élément concerné (`card.css`, `card.module.css`, `global.css`, `tokens.css`). Il n'est pas nécessaire de créer un fichier de styles distinct par composant lorsque le projet utilise une autre méthode cohérente de gestion du style.

### 6.11. Fichiers de configuration

Les configurations utilisent de préférence les noms attendus par les outils (`package.json`, `tsconfig.json`, `eslint.config.js`, `vite.config.ts`, `.editorconfig`, `.gitignore`, `.env.example`). Les noms imposés sont conservés sans tentative de normalisation.

### 6.12. Python, Java et autres langages

Les conventions générales ne remplacent pas les conventions idiomatiques des langages : Python `user_profile.py` ; Java `UserProfile.java`, `PaymentService.java` ; JavaScript / TypeScript `user-profile.ts`. Chaque convention spécialisée précise les adaptations nécessaires.

### 6.13. Données et registres

Les fichiers de données utilisent un nom décrivant leur contenu ou leur responsabilité (`settings.json`, `products.json`, `events.jsonl`, `workflow.yaml`, `metadata.toml`). Les extensions doivent correspondre réellement au format du contenu.

Nommage des fichiers et clés internes peuvent différer (un `user-profile.json` peut contenir des clés en `camelCase` si le schéma du projet le prévoit). La structure interne relève du format de données ou de la convention spécialisée.

### 6.14. Bases de données

Les noms des bases, tables, colonnes et migrations relèvent des conventions techniques propres au système utilisé. Les scripts de migration doivent conserver le format attendu par l'outil (ex. `migrations/202611020900_create_users.sql`). Les préfixes temporels ou numériques sont pertinents lorsqu'ils garantissent un ordre d'exécution ou une identification unique.

### 6.15. Ressources multimédias

Les ressources utilisent des noms simples correspondant à leur contenu ou à leur fonction (`logo.svg`, `icon-search.svg`, `avatar-placeholder.webp`, `hero.webp`). Une précision supplémentaire est autorisée lorsqu'elle identifie une véritable variante (`logo-light.svg`, `logo-dark.svg`, `hero-desktop.webp`, `hero-mobile.webp`). Il n'est pas nécessaire d'intégrer systématiquement la taille, la résolution, la couleur ou d'autres caractéristiques dans le nom : elles peuvent être gérées par les métadonnées, le format ou la structure du dossier.

### 6.16. Fichiers audio et vidéo 

Utiliser un nom identifiant le contenu (`intro.mp4`, `ambient.wav`, `presentation.mp3`).

> **Note d'auteur (non normative, à arbitrer), reprise telle qu'écrite :** Ici on pourrais faire un super truc, je vais souvent avoir un fichier pas nomé correctement, pour les fichier img, vidéo, audio avec des nom qui ne définise rien exemple "4df6s51f.png" quand le model voit ca dans un dossier il doit demander le contexte du contenu pour trouver un bon nom en suivant ce qui est dit en dessous

Pour les ressources versionnées ou chronologiques, une date ou un numéro de séquence peut être ajouté (`2026-11-02-interview.mp4`).

## 7. Gestion des noms courts et des collisions

### 7.1. Principe de résolution

Lorsqu'un nom générique devient ambigu, privilégier cet ordre :

1. Utiliser le contexte du dossier.
2. Séparer les domaines si cela correspond à l'organisation réelle.
3. Ajouter un qualificatif utile au nom.
4. Utiliser une nomenclature spécifique si une contrainte technique l'impose.

Plusieurs fichiers de même domaine : `services/payment.ts`, `payment-gateway.ts`, `payment-validation.ts`. Le qualificatif doit représenter une différence réelle.

### 7.2. Gestion des noms génériques

Vigilance particulière :

| Nom | Utilisation appropriée | Risque |
| --- | --- | --- |
| `index` | Point d'entrée ou fichier attendu par un outil | Multiplication des fichiers peu explicites |
| `config` | Configuration d'un domaine identifié | Configuration globale indéterminée |
| `utils` | Fonctions utilitaires cohérentes et limitées | Accumulation de fonctions disparates |
| `types` | Types regroupés dans un périmètre clair | Fichier global difficile à maintenir |
| `common` | Éléments réellement partagés | Fourre-tout transversal |
| `shared` | Ressources partagées entre plusieurs domaines | Absence de responsabilité identifiable |
| `helpers` | Fonctions auxiliaires cohérentes | Duplication ou accumulation |

Ces noms ne sont pas interdits ; ils doivent être utilisés uniquement lorsque leur périmètre reste identifiable.

### 7.3. Cas des fichiers `index`

Pertinents lorsqu'une technologie ou une architecture les utilise comme point d'entrée. Ils ne doivent pas être créés systématiquement pour chaque dossier. Un `index.ts` peut servir de point d'export public d'un module, mais ne doit pas remplacer des noms de fichiers explicites lorsqu'il n'apporte aucune valeur.

### 7.4. Éviter les doublons de casse

Les fichiers ne doivent pas se distinguer uniquement par la casse (à éviter : `Card.tsx` et `card.tsx`) : certains systèmes de fichiers ne distinguent pas correctement ces deux noms. Règle particulièrement importante pour les projets partagés entre Windows, macOS et Linux.

## 8. Centralisation des variantes et séparation des responsabilités

### 8.1. Principe de centralisation

Privilégier l'organisation par concept plutôt que la création d'un fichier distinct par variante. Les variantes cohérentes d'un même élément devraient être gérées dans le même fichier (ex. `components/card.tsx` avec les variantes Default, Product, Horizontal, Featured, Compact), contrôlées par des propriétés, des paramètres ou une autre méthode adaptée à la technologie.

### 8.2. Exemple React

Un seul `Card` avec une prop `variant` (`"default" | "product" | "horizontal" | "featured" | "compact"`) ; il n'est pas nécessaire de créer `main-card.tsx`, `product-card.tsx`, `horizontal-card.tsx`, `featured-card.tsx`, `compact-card.tsx` lorsque ces composants représentent en réalité des variantes du même élément.

### 8.3. Quand séparer un fichier ?

La création d'un fichier distinct est pertinente lorsque : la responsabilité devient indépendante ; le composant devient suffisamment complexe ; il possède une logique métier spécifique ; il doit être réutilisé indépendamment ; sa séparation améliore réellement la lisibilité ; il nécessite un cycle de vie ou des dépendances distinctes ; une contrainte technique ou architecturale le justifie.

Exemple : `components/card.tsx` et `components/product/product-card.tsx`, pertinent si `ProductCard` devient un composant métier autonome plutôt qu'une simple variante visuelle de `Card`.

### 8.4. Éviter les deux extrêmes

- **Sur-segmentation :** créer un fichier pour chaque petite variante, chaque état visuel ou chaque fragment de logique.
- **Centralisation excessive :** regrouper des composants sans relation dans un fichier unique sous prétexte de limiter le nombre de fichiers.

La règle est la cohésion fonctionnelle : un fichier doit regrouper ce qui appartient naturellement au même concept, sans devenir un espace de stockage arbitraire.

### 8.5. La simplicité avant le nombre de fichiers

Aucun nombre maximal de responsabilités par fichier n'est fixé. La convention encourage une architecture où chaque séparation possède une justification identifiable. Le nombre de fichiers n'est pas un indicateur direct de qualité architecturale.

## 9. Fichiers sources, générés et temporaires

### 9.1. Fichiers sources

Éléments maintenus directement par les développeurs ou contributeurs ; ils doivent respecter les conventions de nommage applicables au projet.

### 9.2. Fichiers générés

Doivent être placés dans les emplacements prévus par les outils ou par l'architecture (`dist/`, `build/`, `.next/`, `coverage/`). Il n'est pas nécessaire d'ajouter artificiellement `generated` aux noms lorsque l'emplacement les identifie déjà. Ils ne doivent pas être renommés ou modifiés manuellement lorsque l'outil responsable attend un nom précis. Leur source de vérité reste le fichier ou le mécanisme qui permet de les générer.

### 9.3. Fichiers temporaires

Doivent utiliser les emplacements prévus (`tmp/`, `cache/`) et ne pas se multiplier dans les dossiers de code ou de documentation. Éviter notamment `card-new.tsx`, `card-old.tsx`, `card-backup.tsx`, `card-copy.tsx`. Les sauvegardes et historiques doivent être gérés par les mécanismes adaptés, notamment Git lorsque celui-ci est utilisé.

## 10. Contraintes de portabilité

### 10.1. Caractères recommandés

Pour les noms techniques partagés entre plusieurs systèmes, privilégier : lettres ASCII minuscules, chiffres, tirets, underscores lorsque la convention les prévoit, points nécessaires aux extensions. Éviter les espaces, caractères accentués, caractères spéciaux et symboles non indispensables (recommandé `user-profile.ts`, `configuration.json`, `project-guide.md` ; à éviter `user profile.ts`, `configuration!.json`, `guide du projet.md`). Cela favorise la compatibilité entre systèmes de fichiers, outils de développement, URL et scripts.

### 10.2. Caractères et noms réservés

Les noms ne doivent pas utiliser les caractères incompatibles avec les systèmes ciblés. En particulier, éviter dans les noms de fichiers et dossiers :

```
< > : " / \ | ? *
```

Éviter également les espaces ou points en fin de nom. Certains noms sont réservés par Windows, notamment `CON`, `PRN`, `AUX`, `NUL`, `COM1`, `LPT1` ; tenir compte aussi des autres variantes réservées par la plateforme.

### 10.3. Portabilité entre systèmes

Éviter les noms qui diffèrent uniquement par les majuscules et minuscules. Éviter également les conventions qui dépendent d'un comportement spécifique à un système de fichiers sans nécessité. La convention doit favoriser le fonctionnement sur Windows, macOS et Linux lorsqu'un projet est destiné à être partagé entre ces plateformes.

## 11. Gestion des exceptions

### 11.1. Principe

Une exception est autorisée lorsqu'une règle générale ou spécialisée entre en conflit avec une contrainte réelle du projet. Les exceptions doivent rester limitées à leur périmètre et ne modifient pas implicitement la convention générale.

### 11.2. Motifs acceptables

Un framework ; une bibliothèque ; un outil de génération ; une API externe ; une contrainte de compatibilité ; une nomenclature métier ; une convention déjà établie et cohérente dans le projet ; une architecture particulière.

### 11.3. Documentation des exceptions

Une exception durable ou susceptible d'être modifiée par un agent IA doit être documentée dans la convention spécialisée ou dans la documentation propre au projet. Elle doit préciser :

1. La règle concernée.
2. Le périmètre de l'exception.
3. Sa justification.
4. Le comportement attendu.

Les exceptions évidentes et imposées par un framework n'ont pas besoin d'être documentées individuellement si leur fonctionnement est déjà clairement établi.

### 11.4. Éviter les exceptions inutiles

Il ne faut pas créer une règle spécifique pour chaque fichier ou chaque cas particulier. Lorsqu'une exception devient fréquente, il faut réexaminer la convention spécialisée concernée ; une adaptation cohérente peut devenir une règle de domaine plutôt qu'une succession d'exceptions locales.

## 12. Articulation avec les conventions spécialisées

### 12.1. Héritage

Niveaux (universel, domaine, projet) et règles d'héritage : [METADATA.md](METADATA.md) §7. Dans ce document, « convention générale » désigne le niveau universel : socle commun dont les conventions spécialisées (Web/React/Next.js, Mobile, Logiciel/Python/Java, Création/Design/Audiovisuel…) héritent les principes et qu'elles précisent selon les besoins du domaine. Ces exemples illustrent une organisation logique : les conventions peuvent être regroupées ou subdivisées selon les besoins réels.

### 12.2. Responsabilité de chaque niveau

- **Convention générale** : principes universels (simplicité, clarté, cohérence, non-redondance, centralisation, gestion des exceptions, portabilité).
- **Convention spécialisée** : adaptations du domaine (casse des fichiers, préfixes et suffixes, nommage des modules, conventions de tests, noms imposés par les frameworks, structures techniques).
- **Convention du projet** : uniquement les adaptations locales nécessaires.

### 12.3. Ordre de priorité

En cas de conflit, appliquer l'ordre suivant :

1. Contraintes obligatoires de la plateforme, du langage ou de l'outil.
2. Exceptions explicitement définies pour le projet.
3. Convention spécialisée applicable.
4. Convention générale.

Une exception de projet ne s'applique que dans son périmètre ; elle ne modifie pas les règles des autres projets ni celles du socle commun.

### 12.4. Limitation de la duplication

Les conventions spécialisées ne doivent pas recopier intégralement la convention générale : elles doivent la référencer et préciser uniquement les règles supplémentaires ou les adaptations nécessaires, pour que la documentation reste modulaire et maintenable.

## 13. Exemples d'organisation

### 13.1. Architecture Web avec React et Next.js

Organisation illustrative : `src/app/` (`layout.tsx`, `page.tsx`, `products/page.tsx`, `products/[slug]/page.tsx`), `src/components/` (`card.tsx`, `button.tsx`, `modal.tsx`), `src/hooks/use-cart.ts`, `src/services/` (`payment.ts`, `product.ts`), `src/styles/` (`global.css`, `tokens.css`), `src/types/` (`product.ts`, `user.ts`), `src/assets/` (`logo.svg`, `hero.webp`), `docs/` (`VISION.md`, `ARCHITECTURE.md`, `ROADMAP.md`, `DECISIONS.md`), plus `README.md`, `package.json`, `tsconfig.json` à la racine.

Les noms `page.tsx` et `layout.tsx` sont conservés parce qu'ils correspondent aux conventions de Next.js ; les autres fichiers suivent la convention générale.

### 13.2. Organisation d'un composant

Simple : `components/card.tsx`. Plus complexe mais justifiée : `components/card/card.tsx`, `card-media.tsx`, `card-header.tsx`, pertinente lorsque les éléments possèdent suffisamment de logique et de responsabilités pour justifier leur séparation ; elle ne doit pas être créée simplement pour séparer chaque fragment visuel.

## 14. Règles applicables aux agents IA

Les agents IA doivent appliquer les règles suivantes lorsqu'ils créent, modifient ou organisent des fichiers et dossiers.

### 14.1. Avant de créer un fichier

L'agent DOIT :

1. Examiner l'emplacement et son contexte.
2. Identifier la responsabilité du fichier envisagé.
3. Consulter les conventions applicables.
4. Vérifier si un fichier correspondant existe déjà.
5. Vérifier si la responsabilité peut être intégrée à un fichier existant.
6. Respecter les conventions imposées par les outils et frameworks.

L'agent NE DOIT PAS créer un nouveau fichier simplement parce qu'une variante ou une fonctionnalité supplémentaire apparaît.

### 14.2. Lors du choix du nom

L'agent DEVRAIT :

- Privilégier un nom court.
- Utiliser le contexte du chemin.
- Éviter les répétitions.
- Utiliser un seul mot lorsque cela suffit.
- Ajouter un qualificatif uniquement lorsqu'il apporte une distinction utile.
- Respecter la casse applicable au domaine.
- Éviter les abréviations obscures.
- Préserver les noms techniques imposés.

### 14.3. Lors d'une modification de l'arborescence

L'agent NE DOIT PAS :

- Renommer massivement les fichiers sans justification.
- Modifier arbitrairement les noms imposés par un framework.
- Créer des dossiers supplémentaires sans raison.
- Introduire plusieurs conventions concurrentes.
- Remplacer un nom existant cohérent uniquement pour satisfaire une préférence stylistique.

Un renommage doit être justifié par une amélioration réelle de la clarté, de la cohérence ou de la maintenabilité.

### 14.4. Lorsqu'une ambiguïté existe

Si plusieurs noms sont possibles, l'agent doit privilégier celui qui :

1. Correspond le mieux à la responsabilité.
2. Utilise le moins de mots nécessaire.
3. S'intègre à la nomenclature existante.
4. Respecte les conventions applicables.

Si le choix implique une décision architecturale importante, l'agent doit signaler les options plutôt que créer une nouvelle règle implicitement.

### 14.5. Préserver la cohérence existante

Lorsqu'un projet possède déjà une convention cohérente, l'agent doit la prendre en compte. La convention générale ne justifie pas une normalisation systématique de tous les fichiers existants. Les modifications doivent rester proportionnées au travail demandé.

## 15. Vérification de conformité

### 15.1. Contrôle d'un nom

Un nom doit être vérifié selon les critères suivants : compréhensible ? représente-t-il correctement le concept ou la responsabilité ? suffisamment court ? le contexte du dossier le rend-il explicite ? contient-il des informations redondantes ? respecte-t-il la casse applicable ? compatible avec les contraintes techniques ? cohérent avec les fichiers voisins ?

### 15.2. Contrôle de l'arborescence

L'arborescence doit être vérifiée selon les critères suivants : les dossiers représentent-ils des catégories ou domaines identifiables ? les fichiers sont-ils placés dans le bon périmètre ? existe-t-il des dossiers inutiles ? une profondeur excessive ? des responsabilités distinctes regroupées sans raison ? des variantes équivalentes multipliées en fichiers ? des fichiers génériques servant de fourre-tout ? les noms sont-ils cohérents entre les différents niveaux ?

### 15.3. Contrôle technique

Vérifier également : les noms réservés par les plateformes ; les extensions et formats ; les fichiers attendus par les frameworks ; les conventions de tests et de génération ; les collisions de casse ; les chemins et imports après renommage ; la cohérence entre les noms déclarés et les noms attendus par les outils.

### 15.4. Checklist de validation

Avant de considérer une nouvelle organisation comme conforme :

- Les noms correspondent aux responsabilités réelles.
- Les noms courts sont privilégiés lorsque le contexte suffit.
- Les noms composés apportent une distinction utile.
- Les informations déjà présentes dans le chemin ne sont pas répétées.
- Les variantes cohérentes sont centralisées.
- Les séparations de fichiers sont justifiées.
- Les conventions techniques sont respectées.
- Les fichiers générés et temporaires sont correctement organisés.
- Les exceptions nécessaires sont explicites.
- L'organisation reste compréhensible pour un nouvel agent IA.

### 15.5. Niveau de conformité

Deux dimensions :

- **Conformité nominale :** respect des règles de syntaxe, de casse, de portabilité et des conventions applicables.
- **Conformité organisationnelle :** pertinence des noms, cohérence de l'arborescence et adéquation entre les fichiers et leurs responsabilités.

Un nom peut être techniquement conforme tout en étant mal choisi sur le plan organisationnel : la vérification doit prendre en compte les deux dimensions.

## 16. Références techniques

Références pour les adaptations propres aux technologies ; elles ne remplacent pas les conventions générales de ce document.

- [MDN — Dealing with files](https://developer.mozilla.org/en-US/docs/Learn_web_development/Getting_started/Environment_setup/Dealing_with_files) : nommage des fichiers et dossiers Web.
- [PEP 8 — Python Style Guide](https://peps.python.org/pep-0008/) : modules, packages et classes Python.
- [Effective Go](https://go.dev/doc/effective_go) : nommage Go.
- [Google Java Style Guide](https://google.github.io/styleguide/javaguide.html) : Java, association fichiers et classes.
- [React — Your First Component](https://react.dev/learn/your-first-component) : noms de composants React.
- [Next.js — Project Structure](https://nextjs.org/docs/app/getting-started/project-structure) : fichiers et dossiers Next.js.
- [Next.js — Layouts and Pages](https://nextjs.org/docs/app/getting-started/layouts-and-pages) : fichiers de routes et layouts.
- [Microsoft — Naming Files, Paths, and Namespaces](https://learn.microsoft.com/en-us/windows/win32/fileio/naming-a-file) : contraintes de nommage Windows.
- [Git — core.ignoreCase](https://git-scm.com/docs/git-config#Documentation/git-config.txt-coreignoreCase) : systèmes de fichiers sensibles ou non à la casse.

## 17. Principe directeur

La convention de nommage doit faciliter l'organisation et la compréhension des projets, et non leur imposer une architecture artificielle. Le nom doit être aussi court que possible, mais aussi explicite que nécessaire. Le contexte doit compléter le nom, les variantes doivent rester regroupées lorsqu'elles appartiennent au même concept et les séparations doivent répondre à de véritables responsabilités. Les conventions spécialisées et les contraintes techniques peuvent préciser ou adapter ces principes sans remettre en cause le socle commun.

**La meilleure convention est celle qui permet de retrouver, comprendre et maintenir un élément sans avoir besoin de multiplier les mots, les fichiers ou les règles.**
