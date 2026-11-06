import { useEffect, useState } from "react";
import { Ico } from "../../Ico";
import { TabStrip } from "../Features/Tabs/TabStrip";
import type { TabDef, TabId } from "../../gameChrome";

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
 * buttons (browser, IA, console — moved here from the tab strip so they
 * stay reachable no matter what page is open) and the three window
 * controls: no app menu (back in the sidebar, its original spot) and no
 * title/logo.
 */
export function Titlebar({
  hideTabs,
  tabs,
  currentTab,
  modCount,
  webOpen,
  onSelectTab,
  webActive,
  aiActive,
  onOpenWeb,
  onOpenAi,
  onOpenConsole,
}: {
  hideTabs?: boolean;
  tabs: TabDef[];
  currentTab: TabId;
  modCount: number;
  webOpen: boolean;
  onSelectTab: (tab: TabId) => void;
  webActive: boolean;
  aiActive: boolean;
  onOpenWeb: () => void;
  onOpenAi: () => void;
  onOpenConsole: () => void;
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
          webOpen={webOpen}
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
          className="ti"
          title="Ouvrir le navigateur intégré"
          aria-pressed={webActive}
          onClick={onOpenWeb}
        >
          <Ico name="globe" />
        </button>
        <button
          type="button"
          className="ti claude"
          aria-pressed={aiActive}
          onClick={onOpenAi}
        >
          <Ico name="sparkle" />
          <span className="n">IA</span>
        </button>
        <button
          type="button"
          className="ti"
          title="Console"
          onClick={onOpenConsole}
        >
          <Ico name="term" />
        </button>
      </div>

      <div className="tb-controls">
        <button
          type="button"
          className="tb-win"
          title="Réduire"
          aria-label="Réduire"
          onClick={minimize}
        >
          <Ico name="minimize" className="sm" />
        </button>
        <button
          type="button"
          className="tb-win"
          title={maximized ? "Restaurer" : "Agrandir"}
          aria-label={maximized ? "Restaurer" : "Agrandir"}
          onClick={toggleMaximize}
        >
          <Ico name={maximized ? "restore" : "maximize"} className="sm" />
        </button>
        <button
          type="button"
          className="tb-win danger"
          title="Fermer"
          aria-label="Fermer"
          onClick={close}
        >
          <Ico name="x" className="sm" />
        </button>
      </div>
    </header>
  );
}
