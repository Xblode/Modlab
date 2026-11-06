import type { ReactNode } from "react";
import type { TabDef, TabId } from "../../gameChrome";
import { Titlebar } from "./Titlebar";

/**
 * The shell's frame: our own title bar, then sidebar, main sheet, floating
 * right panel, bottom dock.
 *
 * Pure layout — it holds no state and knows nothing about games, loadouts or
 * mods. Every slot is filled by the caller; the only thing this component
 * decides is where a slot goes and which of the maquette's two layout
 * modifiers (`no-right`, `running`) the root carries.
 */
export function LayoutShell({
  rightOpen,
  running,
  sidebar,
  right,
  dock,
  overlays,
  children,
  webActive,
  aiActive,
  onOpenWeb,
  onOpenAi,
  onOpenConsole,
  hideTabs,
  tabs,
  currentTab,
  modCount,
  webOpen,
  onSelectTab,
}: {
  rightOpen: boolean;
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
  onOpenWeb: () => void;
  onOpenAi: () => void;
  onOpenConsole: () => void;
  /** Hides the title bar's tab strip — for a page not tied to any game tab
   * (Réglages, Profil, Communauté), showing the previous game's tabs would
   * be stale, not helpful. */
  hideTabs?: boolean;
  tabs: TabDef[];
  currentTab: TabId;
  modCount: number;
  webOpen: boolean;
  onSelectTab: (tab: TabId) => void;
}) {
  return (
    <div className={`shell-root ${rightOpen ? "" : "no-right"} ${running ? "running" : ""}`}>
      {sidebar}

      <div className="shell-col">
        <Titlebar
          hideTabs={hideTabs}
          tabs={tabs}
          currentTab={currentTab}
          modCount={modCount}
          webOpen={webOpen}
          onSelectTab={onSelectTab}
          webActive={webActive}
          aiActive={aiActive}
          onOpenWeb={onOpenWeb}
          onOpenAi={onOpenAi}
          onOpenConsole={onOpenConsole}
        />

        <div className="app">
          <div className="main">
            <div className="sheet">{children}</div>

            {right}

            <div className="dock">{dock}</div>
          </div>

          {overlays}
        </div>
      </div>
    </div>
  );
}
