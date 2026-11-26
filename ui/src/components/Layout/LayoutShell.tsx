import {
  isValidElement,
  useEffect,
  useState,
  type CSSProperties,
  type PointerEvent as ReactPointerEvent,
  type ReactElement,
  type ReactNode,
  type RefObject,
} from "react";
import type { TabDef, TabId } from "../../gameChrome";
import {
  COLUMN_WIDTH_MIN,
  clampColumnWidth,
  clampUtilityWidth,
  columnWidthCeiling,
  utilityWidthCeiling,
  readUiMemory,
  writeUiMemory,
} from "../../uiMemory";
import { TooltipLayer } from "../ui";
import { Titlebar } from "./Titlebar";

/**
 * The shell's frame. The sidebar runs the full window height on the left.
 * To its right, one column stacks the title bar (full remaining width) over
 * a row: the left stack (header, toolbar, sheet, dock) and, when open, the
 * utility column at the same height as that stack.
 *
 * Left-bar and right-column widths live here and in `uiMemory`. Every other
 * slot is filled by the caller. The root carries the maquette's layout
 * modifiers (`no-right`, `running`).
 *
 * The right column is the `right` slot when that element itself has the
 * class `utility-col` (the tabbed panel). Content inside it belongs to the
 * caller. A `right` element without that class stays the floating panel
 * inside the sheet, and only the left bar can be dragged. The floating
 * sheet inset stays off while the column is the one on screen — the column
 * already takes its own width.
 */
export function LayoutShell({
  rightOpen,
  /** When true, `right` is laid out in `shell-row` (flex push), not floated in `.main`. */
  utilityColumn = false,
  running,
  sidebar,
  right,
  dock,
  overlays,
  children,
  webActive,
  aiActive,
  consoleActive,
  onOpenWeb,
  onOpenAi,
  onOpenConsole,
  onOpenSearch,
  webButtonRef,
  hideTabs,
  tabs,
  currentTab,
  modCount,
  onSelectTab,
  /** Instance hero (Mods branch header) — first block in `main-stack`, above the toolbar. */
  header,
  toolbar,
}: {
  rightOpen: boolean;
  utilityColumn?: boolean;
  running: boolean;
  sidebar: ReactNode;
  /** The right panel itself, or `null` when it is closed. */
  right?: ReactNode;
  dock: ReactNode;
  /** Toasts, palette, modals — anything that floats above the frame. */
  overlays?: ReactNode;
  /** The sheet's contents: the active page — the tab strip itself now lives
   * in the title bar, not here. */
  children: ReactNode;
  webActive: boolean;
  aiActive: boolean;
  consoleActive: boolean;
  onOpenWeb: () => void;
  onOpenAi: () => void;
  onOpenConsole: () => void;
  onOpenSearch: () => void;
  webButtonRef?: RefObject<HTMLButtonElement | null>;
  /** Hides the title bar's tab strip — for a page not tied to any game tab
   * (Réglages, Profil, Communauté), showing the previous game's tabs would
   * be stale, not helpful. */
  hideTabs?: boolean;
  tabs: TabDef[];
  currentTab: TabId;
  modCount: number;
  onSelectTab: (tab: TabId) => void;
  header?: ReactNode;
  /** Content toolbar — branch bar, etc. In `main-stack` under the header, above the sheet. */
  toolbar?: ReactNode;
}) {
  const [sideWidth, setSideWidth] = useState(() => readUiMemory().sideWidth);
  const [utilityWidth, setUtilityWidth] = useState(() => readUiMemory().utilityWidth);
  const [sideCeiling, setSideCeiling] = useState(() =>
    columnWidthCeiling(typeof window === "undefined" ? Number.POSITIVE_INFINITY : window.innerWidth),
  );
  const [utilityCeiling, setUtilityCeiling] = useState(() =>
    utilityWidthCeiling(
      typeof window === "undefined" ? Number.POSITIVE_INFINITY : window.innerWidth,
    ),
  );
  const columnMode = utilityColumn || isUtilityColumn(right);
  const columnRight = columnMode && right != null ? right : null;
  const floatRight = rightOpen && !columnMode;
  const shownSide = Math.min(sideWidth, sideCeiling);
  const shownUtility = Math.min(utilityWidth, utilityCeiling);

  useEffect(() => {
    const sync = () => {
      const nextSide = columnWidthCeiling(window.innerWidth);
      const nextUtility = utilityWidthCeiling(window.innerWidth);
      setSideCeiling((prev) => (prev === nextSide ? prev : nextSide));
      setUtilityCeiling((prev) => (prev === nextUtility ? prev : nextUtility));
    };
    window.addEventListener("resize", sync);
    return () => window.removeEventListener("resize", sync);
  }, []);

  function onResizeDown(which: "side" | "utility") {
    return (event: ReactPointerEvent<HTMLDivElement>) => {
      event.preventDefault();
      const origin = which === "side" ? shownSide : shownUtility;
      const sign = which === "side" ? 1 : -1;
      const startX = event.clientX;
      const handle = event.currentTarget;
      handle.setPointerCapture(event.pointerId);
      const previousCursor = document.body.style.cursor;
      const previousSelect = document.body.style.userSelect;
      document.body.style.cursor = "col-resize";
      document.body.style.userSelect = "none";

      const apply = (clientX: number) => {
        const dragged = origin + sign * (clientX - startX);
        const next =
          which === "side"
            ? Math.min(clampColumnWidth(dragged, origin), columnWidthCeiling(window.innerWidth))
            : Math.min(clampUtilityWidth(dragged, origin), utilityWidthCeiling(window.innerWidth));
        if (which === "side") setSideWidth(next);
        else setUtilityWidth(next);
        return next;
      };

      let finished = false;
      const end = (ev: PointerEvent) => {
        if (finished) return;
        finished = true;
        handle.removeEventListener("pointermove", move);
        handle.removeEventListener("pointerup", end);
        handle.removeEventListener("pointercancel", end);
        document.body.style.cursor = previousCursor;
        document.body.style.userSelect = previousSelect;
        // A click on the handle must not replace a wider saved width with the
        // value clamped to the current window.
        if (Math.abs(ev.clientX - startX) < 1) return;
        const next = apply(ev.clientX);
        const memory = readUiMemory();
        writeUiMemory(
          which === "side" ? { ...memory, sideWidth: next } : { ...memory, utilityWidth: next },
        );
      };
      const move = (ev: PointerEvent) => {
        apply(ev.clientX);
      };
      handle.addEventListener("pointermove", move);
      handle.addEventListener("pointerup", end);
      handle.addEventListener("pointercancel", end);
    };
  }

  const shellStyle = {
    "--sb": `${shownSide}px`,
    "--utility": `${shownUtility}px`,
  } as CSSProperties;

  return (
    <div
      className={`shell-root ${columnMode ? "utility-column" : ""} ${floatRight ? "float-right" : ""} ${floatRight ? "" : "no-right"} ${running ? "running" : ""}`}
      style={shellStyle}
    >
      {sidebar}

      <div
        className="col-resize"
        role="separator"
        aria-orientation="vertical"
        aria-valuemin={COLUMN_WIDTH_MIN}
        aria-valuemax={sideCeiling}
        aria-valuenow={shownSide}
        aria-label="Redimensionner la barre latérale"
        onPointerDown={onResizeDown("side")}
      />

      <div className="shell-col">
        <Titlebar
          hideTabs={hideTabs}
          tabs={tabs}
          currentTab={currentTab}
          modCount={modCount}
          onSelectTab={onSelectTab}
          webActive={webActive}
          aiActive={aiActive}
          consoleActive={consoleActive}
          onOpenWeb={onOpenWeb}
          onOpenAi={onOpenAi}
          onOpenConsole={onOpenConsole}
          onOpenSearch={onOpenSearch}
          webButtonRef={webButtonRef}
        />

        <div className="shell-row">
          <div className="main-stack">
            <div className="shell-slot-header">{header}</div>
            <div className="shell-slot-toolbar">{toolbar}</div>

            <div className="app">
              <div className="main">
                <div className="sheet">{children}</div>

                {columnMode ? null : right}

                <div className="dock">{dock}</div>
              </div>

              {overlays}
            </div>
          </div>

          {columnRight ? (
            <>
              <div
                className="col-resize col-resize-utility"
                role="separator"
                aria-orientation="vertical"
                aria-valuemin={COLUMN_WIDTH_MIN}
                aria-valuemax={utilityCeiling}
                aria-valuenow={shownUtility}
                aria-label="Redimensionner la colonne de droite"
                onPointerDown={onResizeDown("utility")}
              />
              {columnRight}
            </>
          ) : null}
        </div>
      </div>

      <TooltipLayer />
    </div>
  );
}

/** True when `right` is the utility column element, not a component that
 * paints that class further down. An empty column is never invented here. */
function isUtilityColumn(node: ReactNode): node is ReactElement {
  if (!isValidElement(node)) return false;
  const props = node.props as { className?: unknown };
  if (typeof props.className !== "string") return false;
  return props.className.split(/\s+/).includes("utility-col");
}
