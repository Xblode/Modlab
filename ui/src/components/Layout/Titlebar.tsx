import { useEffect, useRef, useState } from "react";
import { Ico } from "../../Ico";
import { IconButton } from "../ui";
import { MenuPicker, type MenuItem } from "../Features/MenuPicker/MenuPicker";

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
 * (`decorations: false`). Left: the application menu and the app name.
 * Middle: the drag region. Right: minimize / maximize / close.
 */
export function Titlebar({ onMenuAction }: { onMenuAction: (item: MenuItem) => void }) {
  const menuButtonRef = useRef<HTMLButtonElement>(null);
  const [menuOpen, setMenuOpen] = useState(false);
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
      <div className="tb-left">
        <IconButton
          name="menu"
          title="Menu"
          variant="sm"
          ref={menuButtonRef}
          aria-pressed={menuOpen}
          aria-haspopup="menu"
          onClick={() => setMenuOpen((v) => !v)}
        />
        <Ico name="layers" className="sm tb-logo" />
        <span className="tb-title">ModLab</span>
      </div>

      {/* Everything not interactive drags the window; the clusters on either
          side deliberately do not carry the attribute, so their buttons stay
          clickable instead of starting a drag. Double-click is Tauri's own
          maximize toggle on a drag region. */}
      <div className="tb-drag" data-tauri-drag-region />

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

      {menuOpen ? (
        <MenuPicker
          anchorRef={menuButtonRef}
          onClose={() => setMenuOpen(false)}
          onItemSelect={onMenuAction}
        />
      ) : null}
    </header>
  );
}
