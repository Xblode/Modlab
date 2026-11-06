import { Ico } from "../../../Ico";
import { useRef, useLayoutEffect, useState, useEffect, type RefObject } from "react";

export type MenuItem = {
  id: string;
  label: string;
  /** Descriptive subtitle shown on the row — unrelated to `children`. */
  sub?: string;
  icon?: string;
  disabled?: boolean;
  /** Styled as a destructive action (red), e.g. "Supprimer". */
  danger?: boolean;
  /** A nested submenu. Any depth: the renderer recurses. */
  children?: MenuItem[];
};

export type MenuPickerProps = {
  /** The tree to render. Defaults to the app's own `MENU_TREE` — every
   * caller that isn't the hamburger menu passes its own, e.g. a right-click
   * context menu's rename/channel/delete items. */
  items?: MenuItem[];
  /** A dropdown anchored under an element (the hamburger trigger, a "…"
   * button). Ignored when `anchorPoint` is given. */
  anchorRef?: RefObject<HTMLElement | null>;
  /** A context menu anchored at a raw point — where a right-click landed. */
  anchorPoint?: { x: number; y: number };
  onClose: () => void;
  onItemSelect?: (item: MenuItem) => void;
  open?: boolean;
};

/**
 * The whole application menu as one tree. The top level is what used to be
 * the four tabs (Fichier / Loadout / Outils / Affichage); everything below
 * is a flyout, exactly like a native desktop app menu.
 */
export const MENU_TREE: MenuItem[] = [
  {
    id: "fichier",
    label: "Fichier",
    icon: "file",
    children: [
      { id: "nouveau", label: "Nouveau", icon: "plus", sub: "Créer un nouvel élément" },
      { id: "ouvrir", label: "Ouvrir…", icon: "folder", sub: "Ouvrir un fichier existant" },
      { id: "sauvegarder", label: "Enregistrer", icon: "check", sub: "Enregistrer les modifications" },
      { id: "export", label: "Exporter…", icon: "send", sub: "Exporter les données" },
      { id: "importer", label: "Importer…", icon: "download", sub: "Importer depuis un fichier" },
      { id: "imprimer", label: "Imprimer…", icon: "file", sub: "Imprimer le contenu" },
      { id: "fermer", label: "Fermer", icon: "x", sub: "Fermer l'application" },
    ],
  },
  {
    id: "loadout",
    label: "Loadout",
    icon: "layers",
    children: [
      { id: "nouveau-loadout", label: "Nouveau loadout", icon: "plus", sub: "Créer une nouvelle instance" },
      { id: "importer-loadout", label: "Importer un loadout…", icon: "download", sub: "Depuis MO2 ou un autre format" },
      { id: "exporter-loadout", label: "Exporter un loadout…", icon: "upload", sub: "Exporter vers un fichier" },
      { id: "dupliquer-loadout", label: "Dupliquer", icon: "copy", sub: "Créer une copie de l'instance" },
      { id: "supprimer-loadout", label: "Supprimer", icon: "trash", sub: "Supprimer l'instance", disabled: true },
      { id: "renommer-loadout", label: "Renommer", icon: "edit", sub: "Modifier le nom de l'instance" },
    ],
  },
  {
    id: "outils",
    label: "Outils",
    icon: "tool",
    children: [
      { id: "tri-loot", label: "Trier avec LOOT", icon: "wand", sub: "Réordonner les plugins" },
      { id: "xedit", label: "Ouvrir xEdit", icon: "tool", sub: "Lancer l'éditeur de plugins" },
      { id: "lancer-jeu", label: "Lancer le jeu", icon: "play", sub: "Démarrer la session" },
      { id: "terminal", label: "Terminal", icon: "term", sub: "Ouvrir un terminal" },
      { id: "console", label: "Console", icon: "prompt", sub: "Afficher la console" },
      { id: "logs", label: "Voir les logs", icon: "history", sub: "Afficher les journaux" },
      { id: "diagnostic", label: "Diagnostiquer", icon: "alert", sub: "Lancer un diagnostic" },
    ],
  },
  {
    id: "affichage",
    label: "Affichage",
    icon: "settings",
    children: [
      {
        id: "theme",
        label: "Thème",
        icon: "palette",
        sub: "Changer le thème de l'application",
        children: [
          { id: "theme-clair", label: "Clair", icon: "sparkle" },
          { id: "theme-sombre", label: "Sombre", icon: "disc" },
          { id: "theme-auto", label: "Auto", icon: "refresh" },
        ],
      },
      { id: "taille-police", label: "Taille de police", icon: "text", sub: "Ajuster la taille du texte" },
      { id: "langue", label: "Langue", icon: "globe", sub: "Changer la langue de l'interface" },
      { id: "barre-outils", label: "Barre d'outils", icon: "tool", sub: "Montrer/masquer la barre d'outils" },
      { id: "barre-onglets", label: "Barre d'onglets", icon: "tab", sub: "Configuration des onglets" },
      { id: "panneau-droit", label: "Panneau droit", icon: "layers", sub: "Afficher/masquer le panneau droit" },
      { id: "panneau-bas", label: "Panneau du bas", icon: "down", sub: "Afficher/masquer le panneau du bas" },
      { id: "reset", label: "Réinitialiser", icon: "refresh", sub: "Réinitialiser les préférences" },
    ],
  },
];

/** Hover intent: opening late avoids flicker, closing late allows diagonal travel. */
const OPEN_DELAY_MS = 170;
const CLOSE_DELAY_MS = 220;
/** Panel geometry is only needed to decide flips before the panel exists. */
const PANEL_WIDTH = 300;
const ROW_HEIGHT = 27;

type Pos = { x: number; y: number };

/** One popup level. Recurses through `children`, so depth is unbounded. */
function MenuLevel({
  items,
  pos,
  depth,
  openPath,
  onOpenPath,
  onSelect,
  schedule,
  cancel,
}: {
  items: MenuItem[];
  pos: Pos;
  depth: number;
  openPath: string[];
  onOpenPath: (path: string[]) => void;
  onSelect: (item: MenuItem) => void;
  schedule: (fn: () => void, ms: number) => void;
  cancel: () => void;
}) {
  const panelRef = useRef<HTMLDivElement>(null);
  const rowRefs = useRef(new Map<string, HTMLButtonElement | null>());
  const [childPos, setChildPos] = useState<Pos | null>(null);

  const openId = openPath[depth] ?? null;
  const openItem =
    items.find((i) => i.id === openId && (i.children?.length ?? 0) > 0) ?? null;

  // Place the child popup against this panel's right edge, top-aligned with
  // the hovered row — flipping to the left when the viewport would cut it off,
  // the same clamping idea the root popup uses against the anchor.
  useLayoutEffect(() => {
    if (!openItem) {
      setChildPos(null);
      return;
    }
    const row = rowRefs.current.get(openItem.id);
    const panel = panelRef.current;
    if (!row || !panel) return;
    const r = row.getBoundingClientRect();
    const p = panel.getBoundingClientRect();
    let x = p.right + 2;
    if (x + PANEL_WIDTH > window.innerWidth - 6) {
      x = Math.max(6, p.left - PANEL_WIDTH - 2);
    }
    const estimatedHeight = (openItem.children?.length ?? 0) * ROW_HEIGHT + 12;
    const y = Math.max(
      6,
      Math.min(r.top - 5, window.innerHeight - 6 - estimatedHeight),
    );
    setChildPos({ x, y });
  }, [openItem, pos.x, pos.y]);

  return (
    <div
      className={`menu-picker ${depth > 0 ? "menu-sub" : ""}`}
      role="menu"
      ref={panelRef}
      style={{ left: pos.x, top: pos.y }}
    >
      <div className="menu-picker-items">
        {items.map((item) => {
          const disabled = item.disabled ?? false;
          const hasChildren = (item.children?.length ?? 0) > 0;
          const isOpen = openItem?.id === item.id;
          return (
            <div className="menu-row" key={item.id}>
              <button
                type="button"
                ref={(el) => {
                  rowRefs.current.set(item.id, el);
                }}
                className={`menu-item ${disabled ? "disabled" : ""} ${isOpen ? "open" : ""} ${item.danger ? "danger" : ""}`}
                disabled={disabled}
                title={item.sub ?? item.label}
                role="menuitem"
                aria-haspopup={hasChildren || undefined}
                aria-expanded={hasChildren ? isOpen : undefined}
                onMouseEnter={() => {
                  if (disabled) return;
                  // Hovering any row settles the tree at this depth: a parent
                  // opens its children, a leaf closes whatever was open.
                  schedule(
                    () =>
                      onOpenPath(
                        hasChildren
                          ? [...openPath.slice(0, depth), item.id]
                          : openPath.slice(0, depth),
                      ),
                    OPEN_DELAY_MS,
                  );
                }}
                onClick={() => {
                  if (disabled) return;
                  cancel();
                  if (hasChildren) {
                    onOpenPath([...openPath.slice(0, depth), item.id]);
                    return;
                  }
                  onSelect(item);
                }}
              >
                {item.icon ? <Ico name={item.icon} className="sm" /> : null}
                <span className="menu-item-label">{item.label}</span>
                <span className="menu-item-sub">{item.sub}</span>
                {item.id === "fermer" ? (
                  <span className="menu-item-shortcut">⌃Q</span>
                ) : null}
                {hasChildren ? (
                  <Ico name="chevr" className="sm menu-item-chevron" />
                ) : null}
              </button>

              {isOpen && childPos ? (
                <MenuLevel
                  items={item.children ?? []}
                  pos={childPos}
                  depth={depth + 1}
                  openPath={openPath}
                  onOpenPath={onOpenPath}
                  onSelect={onSelect}
                  schedule={schedule}
                  cancel={cancel}
                />
              ) : null}
            </div>
          );
        })}
      </div>
    </div>
  );
}

/**
 * A tree of flyouts, anchored either under an element (`anchorRef` — the
 * hamburger trigger) or at a raw point (`anchorPoint` — a right-click).
 * Defaults to the application's own `MENU_TREE`, but any caller can pass
 * `items` instead — this is also ModLab's context-menu primitive, used for
 * right-clicking a game or an instance in the sidebar. Items open their
 * children to the right on hover, which themselves may open their own
 * submenu — the renderer recurses, so adding a level means adding
 * `children`, nothing else.
 *
 * Selecting anything at any depth closes the whole tree, and so do Escape and
 * a click outside.
 */
export function MenuPicker({
  items = MENU_TREE,
  anchorRef,
  anchorPoint,
  onClose,
  onItemSelect,
  open = true,
}: MenuPickerProps) {
  const ref = useRef<HTMLDivElement>(null);
  const [pos, setPos] = useState<Pos | null>(null);
  const [openPath, setOpenPath] = useState<string[]>([]);
  const timer = useRef<number | null>(null);

  function cancel() {
    if (timer.current != null) {
      window.clearTimeout(timer.current);
      timer.current = null;
    }
  }

  function schedule(fn: () => void, ms: number) {
    cancel();
    timer.current = window.setTimeout(() => {
      timer.current = null;
      fn();
    }, ms);
  }

  useEffect(() => cancel, []);

  useLayoutEffect(() => {
    if (!open) return;
    if (anchorPoint) {
      setPos({
        x: Math.max(6, Math.min(anchorPoint.x, window.innerWidth - 330)),
        y: Math.max(6, Math.min(anchorPoint.y, window.innerHeight - 6)),
      });
      return;
    }
    const anchor = anchorRef?.current;
    if (!anchor) return;
    const r = anchor.getBoundingClientRect();
    setPos({ x: Math.max(6, Math.min(r.left, window.innerWidth - 330)), y: r.bottom + 6 });
  }, [anchorRef, anchorPoint, open]);

  // Reopening starts from a collapsed tree, like any native menu.
  useEffect(() => {
    if (!open) setOpenPath([]);
  }, [open]);

  useEffect(() => {
    function onDown(e: MouseEvent) {
      const t = e.target as Node;
      // Submenus are DOM children of their parent panel, so this one
      // containment check covers every open level.
      if (ref.current?.contains(t)) return;
      if (anchorRef?.current?.contains(t)) return;
      onClose();
    }
    function onKey(e: KeyboardEvent) {
      if (e.key === "Escape") onClose();
    }
    document.addEventListener("mousedown", onDown);
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("mousedown", onDown);
      document.removeEventListener("keydown", onKey);
    };
  }, [anchorRef, onClose]);

  if (!open || !pos) return null;

  return (
    <div
      ref={ref}
      className="menu-tree"
      onMouseEnter={cancel}
      onMouseLeave={() => schedule(() => setOpenPath([]), CLOSE_DELAY_MS)}
    >
      <MenuLevel
        items={items}
        pos={pos}
        depth={0}
        openPath={openPath}
        onOpenPath={setOpenPath}
        onSelect={(item) => {
          cancel();
          setOpenPath([]);
          onItemSelect?.(item);
          onClose();
        }}
        schedule={schedule}
        cancel={cancel}
      />
    </div>
  );
}
