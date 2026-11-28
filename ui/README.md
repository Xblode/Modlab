# Interface React

L'interface de ModLab, en React, TypeScript et Vite. Elle tourne dans l'hôte Tauri `crates/modlab-ui` (voir `crates/README.md`). C'est la référence visuelle du projet ; les règles d'interface sont dans `.bframe/DESIGN.md`, les écrans et concepts dans `.bframe/PRODUIT.md`.

## Lancer et tester

Depuis la racine du dépôt, toujours dans le dossier ModLab (ailleurs, npm peut tomber sur un autre projet) :

- `npm --prefix ui install` : installe les dépendances (la première fois, ou quand `ui/package.json` change).
- `npm --prefix ui run tauri:dev` : application complète (compile l'hôte Tauri, ouvre la fenêtre, recharge l'interface à chaque modification). La première compilation est longue.
- `npm --prefix ui run dev` : l'interface seule dans le navigateur, sur `http://localhost:1420` (sans moteur : les appels à l'hôte Tauri ne répondent pas). Arrêt avec `Ctrl+C`.
- `npm --prefix ui run build` : vérifie les types TypeScript (`tsc --noEmit`) puis construit l'interface. Il n'y a pas encore de tests d'interface.

Prérequis : Node.js avec npm (pas pnpm ni yarn), Rust, et sous Windows les outils C++ de Visual Studio avec WebView2 (requis par Tauri 2).

## Organisation

- `src/app/` : les 10 écrans (`archives`, `dashboard`, `mods`, `packs`, `plugins`, `repertoire`, `sante`, `saves`, `servers`, `worlds`), un `page.tsx` chacun.
- `src/components/` : `ui/` (atomic design : `atoms`, `molecules`, `organisms`), `Layout/` (`LayoutShell`, `Titlebar`) et `Features/` (composants métier : Browser, Chat, Instance, Mods, Sidebar, etc.).
- `src/hooks/` : les hooks d'accès aux données (conflits, dépendances, téléchargements, chemins du jeu, etc.).
- `src/spaces/` : la logique des espaces fonctionnels (`instance`, `mods`, `dock`, `utility`).
- `src/shell/` : session et navigation du shell (voir plus bas).
- `src/api.ts` : le lien vers l'hôte Tauri. `src/shell.css` : les styles du shell.
- Le rendu par jeu suit les capacités déclarées par les adaptateurs (voir `.bframe/DECISIONS.md`).

## Le shell (`src/shell/`)

Logique de **session et navigation** du shell, pas le rendu des écrans de jeu. Le cadre visuel est `components/Layout/LayoutShell.tsx` ; `App.tsx` est le **composeur** qui branche la sidebar, l'en-tête d'instance, la barre d'outils, les spaces et les appels Tauri.

Fichiers clés :

- `useShellSession.ts` : snapshot du shell, instance et branche courantes, rafraîchissements.
- `useShellNavigation.ts`, `shellNavHelpers.ts`, `shellConstants.ts` : arborescence, expansion, identifiants.
- `ShellMainContent.tsx` : routage du contenu principal selon l'espace.
- `utilityColumnChrome.ts` : point d'entrée du shell pour `SideDrawer` (mode colonne `utility-col`).

### Emplacements `shell-col` (ordre vertical)

1. **Titlebar** : onglets de jeu, actions web / IA / console (pleine largeur sous la sidebar).
2. **`shell-row`** : ligne horizontale sous la titlebar.
   - **`main-stack`** (colonne gauche) :
     - **`header`** : hero d'instance Mods (`BranchHeader` + `PageHero`). Onglet Mods uniquement.
     - **`toolbar`** : barre de branche (`ShellToolbar`). Onglet Mods avec instance uniquement.
     - **`.app` > `main` > `sheet` (`children`)** : contenu métier (tableau des mods, archives, packs, etc.).
     - **`dock`** : compositeur / panneau bas, dans `.main`.
   - **`col-resize-utility`** : la poignée (colonne utilitaire ouverte).
   - **`right` (colonne utilitaire)** : panneau droit du shell, **hors** de la feuille. DOM : `shell-col > shell-row > aside.utility-col` (sœur de `main-stack`), rendu par `LayoutShell` quand le slot `right` porte une `className` contenant `utility-col`.

La colonne utilitaire a la **même hauteur** que `main-stack` (header + toolbar + feuille). En mode colonne (`utilityColumn` sur `LayoutShell`), elle **réduit en flex** la largeur du hero, de la toolbar et de la feuille : pas d'overlay absolu sur ces zones.

**Règle chrome :** tout élément réutilisé sur au moins 2 onglets ou écrans va dans un slot du shell ; une page ne rend que son contenu métier.

### Colonne utilitaire droite

Panneau réutilisé pour les **pages Web** (onglets `web:<id>` dans `.rhead-tabs`), le **Chat**, l'**Info**, la **Console** et la **Recherche**, avec une seule barre d'onglets. Ce n'est pas un écran de `spaces/`, c'est du **chrome du shell**, comme la titlebar ou la sidebar.

| Couche | Rôle | Fichier |
| --- | --- | --- |
| Cadre + redimensionnement + placement | Slot `right` de `LayoutShell`, variables `--utility`, poignée `col-resize-utility`, modificateur `no-right` sur `shell-root` | `components/Layout/LayoutShell.tsx` |
| Enveloppe `aside.utility-col` | `SideDrawer` en `layout="column"` (`utility-card`, onglets dans `rhead`) | `shell/utilityColumnChrome.ts` (réexport du composant UI) |
| Styles colonne / onglets | Largeur, bordures, `.drawer-tab.is-fill` (navigateur, chat, console) | `ui/src/shell.css` |
| Contenu des onglets | État d'ouverture, corps Web / IA / inspecteur de mod… | `App.tsx` passe `RightPanel` dans `right` ; logique dans `spaces/utility/useUtilitySpace.ts` |

Contrat du slot `right` :

- Un élément passé à `LayoutShell` avec une `className` incluant **`utility-col`** devient la colonne du shell à droite de `shell-row` (redimensionnement actif).
- Le même slot **sans** `utility-col` sur l'élément racine donne un tiroir **flottant** dans `.main` / `.sheet` (inset, ombre) ; seule la barre gauche se redimensionne.
- `App.tsx` ne fait que **composer** : brancher `RightPanel` (et le navigateur natif via les props) dans `right`, sans recréer le cadre de colonne ailleurs.

**Ne pas** monter un second `aside.utility-col` dans un space ou dans la feuille : un seul propriétaire du chrome de colonne, `LayoutShell` + `SideDrawer` en colonne.

## Un fait = un propriétaire (affichage)

| Fait | Propriétaire unique | Ailleurs |
| --- | --- | --- |
| Canal (édition) | Sidebar : `ActiveInstanceCard` (instance active) ; menu canal `ExplorerTree` (branche ciblée) | Pas dans `BranchHeader` |
| Révision, dirty, snapshot / rollback | `ShellToolbar` (barre du shell) | Pas dans `BranchHeader` |
| Compteurs de mods (activés / loadout) | `ShellToolbar` | Pas dans `BranchHeader` |
| Nom d'instance (hero Mods) | `BranchHeader` / `PageHero` | Sidebar : navigation, pas de doublon des faits Mods |
| SKSE / loaders MC (faits + menus) | `BranchHeader` | n/a |
| CTA d'installation du loader MC | `ShellToolbar`, slot primary (à la place du select de branche) | Pas dans les popovers du loader |

## À ne pas faire

- Redessiner le shell sans passer par l'humain (`ui/` est la référence visuelle).
- Dupliquer les faits ci-dessus dans plusieurs composants.
- Mettre de la logique métier Rust ici : les appels passent par l'API Tauri et les hooks.
