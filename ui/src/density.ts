import { useSyncExternalStore } from "react";

/**
 * UI density. The CSS owns the actual sizes (`--row`, `--ctl`, `--ctl-sm` in
 * `shell.css`, switched by `data-density` on the root element); this module
 * only names the two choices and exposes the active row height to the code
 * that has to do pixel arithmetic (the virtualised mods table, the menu's
 * flip estimate), so no number is duplicated in TypeScript.
 */
export type Density = "compact" | "comfortable";

export const DEFAULT_DENSITY: Density = "comfortable";

export function isDensity(value: unknown): value is Density {
  return value === "compact" || value === "comfortable";
}

/** Put the choice on the root element, where the stylesheet reads it. */
export function applyDensity(density: Density): void {
  document.documentElement.dataset.density = density;
}

/** The active row height in px, read from the `--row` CSS variable. */
export function readRowHeight(): number {
  const raw = getComputedStyle(document.documentElement).getPropertyValue("--row");
  const px = Number.parseFloat(raw);
  return Number.isFinite(px) && px > 0 ? px : 32;
}

function subscribe(onChange: () => void): () => void {
  const observer = new MutationObserver(onChange);
  observer.observe(document.documentElement, {
    attributes: true,
    attributeFilter: ["data-density"],
  });
  return () => observer.disconnect();
}

/** Active row height, re-read whenever the density changes. */
export function useRowHeight(): number {
  return useSyncExternalStore(subscribe, readRowHeight, () => 32);
}
