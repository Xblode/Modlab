# Design ModLab

Référence courte des tokens UI. Source live : `ui/src/shell.css` (`:root`) et page **Réglages → Design system**.

## Rayons

| Token | Valeur | Usage |
| --- | --- | --- |
| `--r` | 12px | Cartes, panels, dialogs, surfaces flottantes, **popovers / menus** (`PopoverMenu`) |
| `--r-ctl` | 8px | Boutons, selects, inputs |
| `--r-tag` | 6px | Badges, pastilles |

Les popovers utilisent **`--r`** (12px), comme les surfaces type utility card — pas `--r-ctl`.

## Badge & Chip (soft dark)

`Badge` (Flags / table) et `Chip` (modes composer : Plan / Debug / Multi-tâches) partagent la même palette soft dark :

- **Fond** : `color-mix(base 16%, var(--c1))` — teinte dans le charcoal, pas un wash pastel transparent
- **Foreground** : `color-mix(base 68%, #fff)` — plus clair que le fill (icône, label, croix du Chip)
- **Pas de bordure** type toolbar

Source live : `.badge*` / `.chip*` dans `shell.css` ; page **Réglages → Design system** (sections Badge et Chip).

## Bordures

| Token | Rôle |
| --- | --- |
| `--line` | Séparateur / contour discret (`1px solid`) |
| `--line2` | Contour de contrôle / champ |
| `--sel-line` | Focus et sélection |

Épaisseur standard : **1px**. Pas de bordure multi-px hors cas exceptionnels.

## Surfaces & couleur (dark-first)

**Mode sombre = défaut produit** (`:root` dans `shell.css`). Le design system se pense en charcoal ; `data-theme="light"` n’est qu’une surcharge optionnelle, pas la référence.

| Famille | Tokens (défaut) |
| --- | --- |
| Surfaces | `--c1` `#1b1a19` → `--c5` `#3c3a38` (plus clair = plus haut) ; `--bg-app/content/raised/utility` alias ; `--bg-overlay` `#2e2d2b` |
| Texte | `--t1` `#e8e6e2` … `--t4` `#4a4844` |
| Lignes | `--line` / `--line2` blanc alpha ; `--sel` / `--sel-line` accent alpha |
| Sémantique | `--accent` `#5a9cf5`, `--on-accent` `#0f1620`, `--green` / `--warn` / `--danger` / `--info`, overrides `--win` `--lose` `--gen` `--claude` `--run` |

Soft Badge / Chip : voir section ci-dessus (16 % dans `--c1`, fg 68 % vers blanc).

Page live : **Réglages → Design system → Couleurs**.

## Densité

| Token | Comfort | Compact |
| --- | --- | --- |
| `--row` | 32px | 28px |
| `--ctl` | 36px | 32px |
| `--ctl-sm` | 32px | 28px |

## Button `round`

Variante ronde icône seule (`.btn.round`, optionnellement `.ghost`) — bouton + / envoyer du composer Chat. Taille `--ctl-sm`, `border-radius: 50%`.

## Toolbar compaction

Règles pour `ShellToolbar` (et toute barre d’outils dense du même genre). Source live : `shell-toolbar.css` (container queries) + page **Réglages → Design system**.

| Largeur (conteneur) | Comportement |
| --- | --- |
| Large | Libellés complets (nom de branche, « N branches », « Ajouter un mod », …) |
| ≤ 940 px | Compaction : counts en icône + chiffre (tip au survol) ; select branche → **icône + `r.N`** (pas le nom long) ; boutons libellés → **icône seule** (`data-tip` / `aria-label`) |
| ≤ 700 px | Stats secondaires du lead masquées ; gaps resserrés |

Principes :

- **Texte → icône** : le contrôle garde son type (`Button` `action` / `sm-ghost`, select branche). On ne transforme pas un bouton texte en `IconButton`.
- **Référence visuelle** des boutons icône seule : les `IconButton` déjà présents (dossier, +) — empreinte 24×24 pour les ghost ; l’action principale bordée reste un carré `--ctl-sm`.
- **Borders** : réservées aux **rares actions principales** (select branche / CTA du slot, « Ajouter un mod »). Pas de bordure sur chaque contrôle.
- **Accessibilité** : dès qu’un libellé disparaît, le tip (`data-tip`) et/ou `aria-label` portent l’info.

## Slot action principale

Emplacement unique dans `ShellToolbar` (`.tbar-primary-slot`) : **même place et même hauteur** (`--ctl-sm`) que le select de branche. Le contenu change selon l’état ; on n’empile pas une seconde action à côté.

| État | Contenu |
| --- | --- |
| Normal | `Select` branche + `PopoverMenu` |
| Loader à installer | `Button` `action` bordé — « Installer NeoForge … » (libellé FR du loader choisi) |
| Après install OK | Retour au select |

Le CTA du slot **garde son libellé** en compaction (≠ « Ajouter un mod » → icône seule). Source : `useMcLoaderBranch` → `primaryAction` → `ShellToolbar`.

## Select

Référence visuelle = select branche de la toolbar Mods (`ShellToolbar` / slot primaire).

| Pièce | Rôle |
| --- | --- |
| Atome `Select` | Déclencheur bordé : icône + libellé (+ méta optionnelle) + chevron ; hauteur `--ctl-sm` ; bordure action (`--line2` / `--bg-raised`) |
| `PopoverMenu` | Panneau (recherche, onglets, items) — le contenu reste côté Features |

Ce n’est **pas** un `<select>` natif. Source live : `components/ui/atoms/Select.tsx` + page **Réglages → Design system → Select**.

## PopoverMenu

Structure fixe (équivalent GitHub) : titre ; au plus une utilitaire (recherche *ou* un bouton) ; séparateurs pleine largeur ; onglets optionnels ; corps. Remplace `Pop` et `MenuPicker`.

Deux modes de chrome (`chrome`) :

| Mode | Usage | Header |
| --- | --- | --- |
| `dialog` (défaut) | Menus riches (branche, Ajouter un mod, favoris URL) | Titre + croix |
| `context` | Clic droit / menus d’actions | Pas de croix ; **pas de header** si `title` est omis (défaut pour Actions) ; titre compact optionnel si utile |
