---
titre: Design
description: Les principes d'interface, les valeurs (tokens) et les règles des composants communs de ModLab ; on le met à jour quand une valeur de `ui/src/shell.css` ou une règle d'interface change.
maj: 2026-11-28
---

# Design

L'application `ui/` est la référence visuelle. La source des valeurs est `ui/src/shell.css` (bloc `:root`) ; la page **Réglages > Design system** les montre en vrai. En cas d'écart entre ce fichier et le CSS, le CSS gagne et ce fichier se corrige.

## Principes

- **Sombre d'abord.** Le thème sombre est le thème du produit. Une surcharge claire existe dans le CSS (`data-theme="light"`) mais rien ne l'active : les boutons Clair / Sombre / Auto de Réglages n'ont pas d'effet.
- **Peu de traits, des nuances de fond.** Les zones se séparent par 4 niveaux de fond (plus clair = plus haut), pas par des bordures épaisses.
- **Un seul bouton principal par vue** : Jouer, en vert.
- **Une seule couleur d'accent** (bleu), réservée à la sélection, au focus et aux liens. Les couleurs de sens (vert, orange, rouge, conflits, généré, Chat) disent un état et ne servent à rien d'autre.
- **Honnêteté.** Ce que le moteur ne sait pas faire se dit en clair (« pas encore branché au Core ») ; on n'invente jamais une donnée.
- **Pas de décor.** Pas de dégradé de teinte décoratif, pas de glow, pas d'emoji, pas de majuscules espacées, pas d'italique. Un seul jeu d'icônes (`ui/src/Ico.tsx`). Méthode de nettoyage : « unslop ».
- **Mouvement discret.** Animer seulement couleur, fond, bordure, opacité et transform, avec une seule courbe. Respecter `prefers-reduced-motion`.
- **Le clavier compte.** Palette Ctrl+K, panneau du bas Ctrl+J, enregistrer Ctrl+S ; chaque bouton à icône seule a une infobulle courte avec son raccourci.

## Mise en page

- **Barre de titre** 40 px : onglets du jeu à gauche, actions (Web, Chat, Console, recherche) et fenêtre à droite.
- **Barre de gauche** pleine hauteur, redimensionnable (200 px au minimum).
- **Colonne centrale** : en-tête de l'instance, barre de branche, puis la feuille (le contenu).
- **Colonne utilitaire** à droite, 320 px par défaut, redimensionnable. Elle a la même hauteur que la colonne centrale et la rétrécit au lieu de la recouvrir. Un seul propriétaire : `LayoutShell` avec `SideDrawer`.
- **Panneau du bas** (Transactions, Téléchargements) sur Ctrl+J.
- **Règle du cadre** : un élément utilisé sur au moins 2 onglets ou écrans va dans un emplacement du shell ; une page ne dessine que son contenu.
- **Un fait = un propriétaire.** Un fait (canal, révision, compteurs, nom d'instance, SKSE, loader) ne s'affiche qu'à un seul endroit. Le tableau des propriétaires est dans `ui/README.md` (section « Un fait = un propriétaire »).

## Tokens

### Surfaces (sombre)

| Token | Valeur | Rôle |
| --- | --- | --- |
| `--c1` | `#1b1a19` | Fond de l'app et de la barre de gauche |
| `--c2` | `#232221` | Contenu |
| `--c3` | `#2a2928` | Surfaces relevées, cartes |
| `--c4` | `#323130` | Survol, boutons de base, utilitaire |
| `--c5` | `#3c3a38` | Pressé, survol des surcouches |
| `--bg-overlay` | `#2e2d2b` | Menus, dialogues, infobulles |

Alias : `--bg-app` (c1), `--bg-content` (c2), `--bg-raised` (c3), `--bg-utility` (c4), `--hover-ov` (c5).

### Texte et lignes

| Token | Valeur | Rôle |
| --- | --- | --- |
| `--t1` | `#e8e6e2` | Texte principal |
| `--t2` | `#aaa59e` | Texte secondaire |
| `--t3` | `#938e86` | Texte discret, valeurs vides |
| `--t4` | `#4a4844` | Très discret |
| `--line` | blanc 10 % | Séparateur et contour discret (1 px) |
| `--line2` | blanc 15 % | Contour de contrôle et de champ |
| `--sel` / `--sel-line` | accent 14 % / 55 % | Fond et liseré de sélection ; le focus utilise un contour de 2 px en `--accent` |

Épaisseur standard des bordures : 1 px.

### Couleurs de sens

| Token | Valeur | Rôle |
| --- | --- | --- |
| `--accent` | `#5a9cf5` | Sélection, focus, liens |
| `--on-accent` | `#0f1620` | Texte sur fond accent ou danger (le blanc y est trop faible) |
| `--green` / `--green-btn` | `#3fb950` / `#238636` | Santé OK ; bouton Jouer et action principale |
| `--warn` | `#d29922` | Avertissement, mise à jour |
| `--danger` | `#f85149` | Bloquant, destructif |
| `--info` | `#5a9cf5` | Information |
| `--win` / `--lose` | `#39d0d8` / `#f778ba` | Écrase / est écrasé (une priorité, ni bien ni mal) |
| `--gen` | `#a371f7` | Généré par un outil |
| `--claude` | `#d97757` | Ce qui vient du Chat |
| `--run` | `#6e3a27` | État « jeu en cours » |

### Rayons

| Token | Valeur | Usage |
| --- | --- | --- |
| `--r` | 12 px | Cartes, panneaux, dialogues, popovers et menus (`PopoverMenu`) |
| `--r-ctl` | 8 px | Boutons, champs, sélecteurs, lignes de liste |
| `--r-tag` | 6 px | Badges et pastilles |

### Densité

Deux densités, Confortable par défaut ; le réglage est dans Réglages > Apparence (`data-density` sur la racine).

| Token | Confortable | Compact | Pour |
| --- | --- | --- | --- |
| `--row` | 32 px | 28 px | Lignes de liste, de table et d'arbre |
| `--ctl` | 36 px | 32 px | Boutons et champs |
| `--ctl-sm` | 32 px | 28 px | Boutons d'icône, contrôles de barre |

### Texte

Les tailles de texte sont une préférence, pas une règle figée (voir `DECISIONS.md`). Aujourd'hui :

| Token | Valeur | Usage |
| --- | --- | --- |
| `--fs-xs`, `--fs-sm` | 11 px | Titres de section, petits libellés, infobulles |
| `--fs-md`, `--fs-lg` | 13 px | Corps : lignes, boutons, onglets |
| `--fs-xl` | 16 px | Sous-titres |
| `--fs-2xl` | 20 px | Titre de page |

Rien sous 11 px. Graisses 400 et 500, 600 pour les titres. Police de l'interface : Segoe UI Variable Text (puis Segoe UI, `system-ui`). Police des chemins et du code : Cascadia Code (puis JetBrains Mono, Consolas). Chiffres tabulaires partout.

### Espace, ombre, mouvement

- **Espace** : grille de 4 px ; 8 px dans un groupe, 12 à 16 px entre groupes, 16 à 24 px aux bords des panneaux.
- **Ombre** : `--sh-float` (`0 4px 14px` noir 22 %) pour tout ce qui flotte. Les listes et tables n'ont pas d'ombre.
- **Mouvement** : une courbe `--ease`, deux durées `--d1` (0,14 s, retour d'action) et `--d2` (0,2 s, ce qui entre).

### Défilement et noms longs

Barres de défilement fines, de 8 px, sans flèches. Un nom ne passe jamais sur deux lignes et n'a jamais de « … » : il s'estompe à droite (classe `.fade`, variable `--fade`). Les titres des panneaux de détail et les phrases peuvent passer à la ligne.

## Composants communs

Les atomes sont dans `ui/src/components/ui/atoms`, les molécules dans `molecules`, les organismes dans `organisms`.

### Boutons

| Variante | Rôle |
| --- | --- |
| défaut | Bouton de base, fond `--c4` |
| `ghost` | Sans fond, pour les actions secondaires |
| `sm`, `sm-ghost` | Hauteur `--ctl-sm` |
| `action` | Contour de 1 px (`--line2`), fond relevé : les rares actions principales d'une barre |
| `round`, `ghost-round` | Rond, icône seule (par exemple le + et l'envoi du Chat) |
| Jouer | Vert `--green-btn`, un seul par vue ; devient « Arrêter » pendant la partie |

Règles : une bordure seulement sur les actions principales, pas sur chaque contrôle. Le bouton d'icône seule a toujours une infobulle (`data-tip`) et un `aria-label`. Les boutons d'icône discrets font 24 px ; l'action principale bordée fait `--ctl-sm`.

### Badge et Chip

Même recette, « soft dark » : fond = la teinte à 16 % dans `--c1`, texte et icône = la teinte à 68 % vers le blanc, pas de bordure.

- **Badge** (20 px, rayon `--r-tag`) : état d'une ligne. Tons : `win`, `lose`, `both`, `plug`, `upd`, `bad`, `root`, `archived`, `downloading`.
- **Chip** (22 px, tout rond, avec croix) : étiquette retirable, modes du Chat (`plan` bleu, `debug` orange, `multi` violet) et mod cité.

### Sélecteur (`Select`)

Ce n'est pas un `<select>` natif. C'est un déclencheur à contour (icône, libellé, méta facultative, chevron), de hauteur `--ctl-sm`, qui ouvre un `PopoverMenu`. Le sélecteur de branche de la barre de branche est la référence.

### PopoverMenu

Un seul composant pour tous les popovers. Structure fixe : titre (avec croix) ; au plus une zone utilitaire (recherche ou un bouton) ; séparateurs pleine largeur ; onglets facultatifs ; corps. Rayon `--r`, ombre `--sh-float`, fond `--bg-overlay`, de 260 à 360 px de large.

| Mode (`chrome`) | Pour | En-tête |
| --- | --- | --- |
| `dialog` (défaut) | Menus riches : branche, Ajouter un mod, favoris d'URL | Titre et croix |
| `context` | Clic droit, menus d'actions | Aucun, sauf titre compact si utile ; fond plus serré |

Créer une branche se fait hors du popover. Les sous-menus s'ouvrent à côté (flyout).

### Barre de branche (`ShellToolbar`)

Elle se compacte selon sa propre largeur (container queries).

| Largeur de la barre | Effet |
| --- | --- |
| Large | Libellés complets |
| 940 px et moins | Compteurs en icône et chiffre ; sélecteur de branche en icône et `r.N` ; boutons à libellé en icône seule |
| 700 px et moins | Statistiques secondaires masquées, espaces resserrés |

Un bouton texte qui se compacte garde son type : on ne le remplace pas par un `IconButton`. Dès qu'un libellé disparaît, l'infobulle et l'`aria-label` portent l'information.

**Emplacement de l'action principale** (`.tbar-primary-slot`) : même place et même hauteur que le sélecteur de branche. Son contenu change avec l'état (sélecteur normal, ou bouton « Installer le loader … » tant que le loader manque, puis retour au sélecteur) ; on n'empile pas une seconde action à côté. Ce bouton garde son libellé en mode compact.

### En-tête de page (`PageHero`)

Pleine largeur, sans carte ni arrondi : fond `--c2`, filet en bas, nom en 20 px qui s'estompe, puis une grille de faits sur deux colonnes. Un fait modifiable fait de sa valeur un bouton (fond au survol, léger arrondi). La page fournit chaque fait ; le composant n'en invente pas.

### Barre de gauche

- Rangée d'icônes en haut (menu, Explorateur, Recherche, Branches, Outils) ; bouton d'icône de taille `--ctl-sm`.
- Sections repliables avec un titre sans chevron ; leur titre reste visible et cliquable même vides.
- Instances : une ligne de `--row` avec une pastille de canal ; l'étoile de favori apparaît partout où une instance est listée, et les favoris passent toujours en tête.
- Jeux : dossier qui devient chevron au survol, + au survol.
- Carte de l'instance active : nom modifiable, pastille de canal, Jouer, Partager.

### Tableaux et listes

- Ligne = `--row`, rayon `--r-ctl`, texte `--t2`, sélection `--sel`.
- Le tableau des mods est virtualisé. Le tri est un affichage seulement : la priorité ne change pas, le glisser-déposer est coupé pendant un tri, un troisième clic revient à l'ordre de priorité.
- Pas de zébrage.

### États vides

Courts : un titre, une phrase, au plus une action (`.empty`, `SideEmpty`). Le titre dit quoi, la phrase dit pourquoi. Dans une section de barre de gauche : `.sempty`.

### Autres

- **Infobulle** (`Tooltip`) : fond `--bg-overlay`, rayon `--r-ctl`, 11 px, raccourci à droite, apparition unique en fondu.
- **Toast** : message court, centré en bas ; sert aussi à « pas encore branché ».
- **Dialog / ConfirmDialog** : rayon `--r`, une action principale ; une action destructive se confirme.

## Références

Grille de 4 px et petits arrondis (shadcn/ui, Radix Colors, Fluent, Primer, Atlassian), barre latérale et palette (VS Code, Cursor), contraste de 4,5:1 au moins (WCAG).
