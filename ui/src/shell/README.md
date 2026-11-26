# Porte d'entrée — shell UI

## À quoi sert ce dossier

Logique de **session et navigation** du shell (pas le rendu des écrans jeu). Le cadre visuel est `components/Layout/LayoutShell.tsx` ; `App.tsx` est le **composeur** qui branche sidebar, en-tête instance, toolbar, spaces et appels Tauri.

## Slots `shell-col` (ordre vertical)

1. **Titlebar** — onglets jeu, actions web / IA / console (pleine largeur sous la sidebar).
2. **`shell-row`** — ligne horizontale sous la titlebar (hauteur utile sous la barre d’onglets) :
   - **`main-stack`** (colonne gauche) :
     - **`header`** — hero instance Mods (`BranchHeader` + `PageHero`). Uniquement onglet Mods.
     - **`toolbar`** — barre de branche (`ShellToolbar`). Uniquement onglet Mods avec instance.
     - **`.app` > `main` > `sheet` (`children`)** — contenu métier (tableau mods, archives, packs…).
     - **`dock`** — compositeur / panneau bas, dans `.main`.
   - **`col-resize-utility`** — poignée (colonne utilitaire ouverte).
   - **`right` (colonne utilitaire)** — panneau droit du shell, **hors** de la feuille : DOM `shell-col > shell-row > aside.utility-col` (sœur de `main-stack`), rendu par `LayoutShell` quand le slot `right` porte `className` contenant `utility-col`.

La colonne utilitaire a la **même hauteur** que `main-stack` (header + toolbar + feuille), pas seulement la feuille. En mode colonne (`utilityColumn` sur `LayoutShell`), elle **réduit en flex** la largeur du hero, de la toolbar et de la feuille — pas d’overlay absolu sur ces zones.

**Règle chrome :** tout élément réutilisé sur ≥ 2 onglets ou écrans va dans un slot shell ; une page ne rend que son contenu métier.

## Colonne utilitaire droite (chrome shell)

Panneau réutilisé pour **pages Web** (onglets `web:<id>` dans `.rhead-tabs`), **Chat**, **Info**, **Console**, **Recherche** — une seule barre d’onglets ; ce n’est pas un écran `spaces/`, c’est du **chrome shell** comme la titlebar ou la sidebar.

| Couche | Rôle | Fichier |
| --- | --- | --- |
| Cadre + resize + placement | Slot `right` de `LayoutShell`, variables `--utility`, poignée `col-resize-utility`, modificateur `no-right` sur `shell-root` | `components/Layout/LayoutShell.tsx` |
| Enveloppe `aside.utility-col` | `SideDrawer` en `layout="column"` (`utility-card`, onglets dans `rhead`) | `shell/utilityColumnChrome.ts` (réexport du composant UI) |
| Styles colonne / onglets | Largeur, bordures, `.drawer-tab.is-fill` (navigateur, chat, console) | `ui/src/shell.css` |
| Contenu des onglets | État d’ouverture, corps Web / IA / inspecteur mod… | `App.tsx` passe `RightPanel` dans `right` ; logique dans `spaces/utility/useUtilitySpace.ts` |

**Contrat slot `right` :**

- Élément passé à `LayoutShell` avec `className` incluant **`utility-col`** → colonne shell à droite de `shell-row` (resize actif).
- Même slot **sans** `utility-col` sur l’élément racine → tiroir **flottant** dans `.main` / `.sheet` (inset, ombre) ; seule la barre gauche se redimensionne.
- `App.tsx` ne fait qu’**composer** : brancher `RightPanel` (et le navigateur natif via props) dans `right`, sans recréer le cadre de colonne ailleurs.

**Ne pas** monter un second `aside.utility-col` dans un space ou la feuille : un seul propriétaire du chrome colonne = `LayoutShell` + `SideDrawer` colonne.

## Fichiers clés

- `useShellSession.ts` : snapshot shell, instance/branche courante, rafraîchissements.
- `useShellNavigation.ts`, `shellNavHelpers.ts`, `shellConstants.ts` : arborescence, expansion, ids.
- `ShellMainContent.tsx` : routage du contenu principal selon l'espace.
- `utilityColumnChrome.ts` : point d'entrée shell pour `SideDrawer` (mode colonne `utility-col`).
- **Spaces** (écrans fonctionnels) : `ui/src/spaces/` (`instance`, `my-game`, …).
- Styles globaux shell : `ui/src/shell.css`.

## Un fait = un propriétaire (affichage)

| Fait | Propriétaire unique | Ailleurs |
| --- | --- | --- |
| Canal (édition) | Sidebar : `ActiveInstanceCard` (instance active) ; menu canal `ExplorerTree` (branche ciblée) | Pas dans `BranchHeader` |
| Révision, dirty, snapshot / rollback | `ShellToolbar` (barre shell) | Pas dans `BranchHeader` |
| Compteurs mods (activés / loadout) | `ShellToolbar` | Pas dans `BranchHeader` |
| Nom d'instance (hero Mods) | `BranchHeader` / `PageHero` | Sidebar : navigation, pas doublon de faits Mods |
| SKSE / loaders MC (faits + menus) | `BranchHeader` | — |
| CTA installer loader MC | `ShellToolbar` slot primary (à la place du select branche) | Pas dans les popovers loader |

## Ne pas

- Redessiner le shell sans passage par l'humain (`ui/` = référence visuelle).
- Dupliquer les faits ci-dessus dans plusieurs composants.
- Mettre de la logique métier Rust ici (appels via l'API Tauri / hooks).

## Ensuite

- Instance / Mods : `ui/src/spaces/instance/`.
- Core métier : `crates/modlab-core/README.md`.
- Règles projet : `CLAUDE.md`, `.bframe/ETAT.md`.