import { useEffect, useState, type RefObject } from "react";
import { Ico } from "../../Ico";
import { tip } from "../ui";
import { TabStrip } from "../Features/Tabs/TabStrip";
import type { TabDef, TabId } from "../../gameChrome";
import "./titlebar-tools.css";

/** Same guard as `api.ts`: outside the Tauri shell there is no window to drive. */
function isTauri(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

async function appWindow() {
  const { getCurrentWindow } = await import("@tauri-apps/api/window");
  return getCurrentWindow();
}

/**
 * Window state, mirrored from the real window. Outside Tauri it stays `false`
 * and the controls are inert — the browser preview keeps working.
 */
function useMaximized(): [boolean, () => void] {
  const [maximized, setMaximized] = useState(false);

  useEffect(() => {
    if (!isTauri()) return;
    let unlisten: (() => void) | undefined;
    let cancelled = false;
    void (async () => {
      const w = await appWindow();
      const sync = async () => {
        const v = await w.isMaximized();
        if (!cancelled) setMaximized(v);
      };
      await sync();
      // Covers the double-click on the drag region and any OS-side resize
      // (snap, Win+Up, dragging the edge) — not just our own buttons.
      unlisten = await w.onResized(() => void sync());
      if (cancelled) unlisten?.();
    })();
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, []);

  function toggle() {
    if (!isTauri()) return;
    void appWindow().then((w) => w.toggleMaximize());
  }

  return [maximized, toggle];
}

/**
 * The window's own title bar — Windows has none of its own
 * (`decorations: false`). The page's own tab strip lives here too (so it
 * never costs its own row), then the drag region, the shell-wide action
 * buttons (Web, IA, console, search — the first three open tabs in the
 * right column) and
 * the three window controls: no app menu (back in the sidebar, its
 * original spot) and no title/logo.
 */
export function Titlebar({
  hideTabs,
  tabs,
  currentTab,
  modCount,
  onSelectTab,
  webActive,
  aiActive,
  consoleActive,
  onOpenWeb,
  onOpenAi,
  onOpenConsole,
  onOpenSearch,
  webButtonRef,
}: {
  hideTabs?: boolean;
  tabs: TabDef[];
  currentTab: TabId;
  modCount: number;
  onSelectTab: (tab: TabId) => void;
  webActive: boolean;
  aiActive: boolean;
  consoleActive: boolean;
  onOpenWeb: () => void;
  onOpenAi: () => void;
  onOpenConsole: () => void;
  /** Opens the existing command palette (same entry as Ctrl+K). */
  onOpenSearch: () => void;
  /** Optional ref on the Web tool (legacy anchor; picker lives on Ajouter un mod). */
  webButtonRef?: RefObject<HTMLButtonElement | null>;
}) {
  const [maximized, toggleMaximize] = useMaximized();

  function minimize() {
    if (!isTauri()) return;
    void appWindow().then((w) => w.minimize());
  }

  function close() {
    if (!isTauri()) return;
    void appWindow().then((w) => w.close());
  }

  return (
    <header className="titlebar">
      {hideTabs ? null : (
        <TabStrip
          tabs={tabs}
          currentTab={currentTab}
          modCount={modCount}
          onSelect={onSelectTab}
        />
      )}

      {/* Everything not interactive drags the window; the controls
          deliberately do not carry the attribute, so they stay clickable
          instead of starting a drag. Double-click is Tauri's own maximize
          toggle on a drag region. */}
      <div className="tb-drag" data-tauri-drag-region />

      <div className="tb-actions">
        <button
          type="button"
          className="tb-tool"
          ref={webButtonRef}
          {...tip("Web")}
          aria-pressed={webActive}
          onClick={onOpenWeb}
        >
          <Ico name="globe" />
        </button>
        <button
          type="button"
          className="tb-tool"
          {...tip("Chat")}
          aria-pressed={aiActive}
          onClick={onOpenAi}
        >
          <Ico name="sparkle" />
        </button>
        <button
          type="button"
          className="tb-tool"
          {...tip("Console", "Ctrl J")}
          aria-pressed={consoleActive}
          onClick={onOpenConsole}
        >
          <Ico name="term" />
        </button>
        <button
          type="button"
          className="tb-tool"
          {...tip("Rechercher", "Ctrl K")}
          onClick={onOpenSearch}
        >
          <Ico name="search" />
        </button>
      </div>

      <div className="tb-controls">
        <button
          type="button"
          className="tb-win"
          {...tip("Réduire")}
          onClick={minimize}
        >
          <Ico name="minimize" className="sm" />
        </button>
        <button
          type="button"
          className="tb-win"
          {...tip(maximized ? "Restaurer" : "Agrandir")}
          onClick={toggleMaximize}
        >
          <Ico name={maximized ? "restore" : "maximize"} className="sm" />
        </button>
        <button
          type="button"
          className="tb-win danger"
          {...tip("Fermer")}
          onClick={close}
        >
          <Ico name="x" className="sm" />
        </button>
      </div>
    </header>
  );
}
