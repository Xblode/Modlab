/**
 * Chrome shell de la colonne utilitaire droite (`aside.utility-col`).
 *
 * DOM : `shell-col > shell-row > aside.utility-col` (sœur de `main-stack`, pas dans la feuille).
 * Largeur : `--utility` et poignée `col-resize-utility` dans `LayoutShell`.
 *
 * Le slot `right` de `LayoutShell` reçoit un nœud dont la prop `className` contient
 * `utility-col` (détection directe sur l’élément passé, pas sur un enfant).
 * `layout="column"` pose la classe sur l’`<aside>` rendu par ce composant.
 *
 * Contenu des onglets (Web, Claude, Info, Console…) : le composeur (`App.tsx` → `RightPanel`),
 * pas ce module.
 */
export {
  SideDrawer,
  type SideDrawerLayout,
  type SideDrawerSide,
} from "../components/ui/organisms/SideDrawer";