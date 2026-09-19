import type { ReactNode } from "react";

/**
 * The shell's frame: sidebar, main sheet, floating right panel, bottom dock.
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
}: {
  rightOpen: boolean;
  running: boolean;
  sidebar: ReactNode;
  /** The right panel itself, or `null` when it is closed. */
  right?: ReactNode;
  dock: ReactNode;
  /** Toasts, palette, modals — anything that floats above the frame. */
  overlays?: ReactNode;
  /** The sheet's contents: the tab strip and the active page. */
  children: ReactNode;
}) {
  return (
    <div className={`app ${rightOpen ? "" : "no-right"} ${running ? "running" : ""}`}>
      {sidebar}

      <div className="main">
        <div className="sheet">{children}</div>

        {right}

        <div className="dock">{dock}</div>
      </div>

      {overlays}
    </div>
  );
}
