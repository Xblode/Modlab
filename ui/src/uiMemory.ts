import type { RightTab } from "./components/Features/Inspector/RightPanel";
import type { TabId } from "./gameChrome";

/**
 * What the shell remembers between launches — **browser-local only**, same
 * rule and same reasoning as `favorites.ts`: none of this is Core state (an
 * active game/instance is a client-side cursor over the Core's data, not a
 * fact the Core itself holds), so a `localStorage` blob is the honest place
 * for it rather than inventing a Core API for "what was on screen last
 * time". Per WebView profile: does not sync, disappears if the profile's
 * storage is cleared.
 */
const KEY = "modlab.ui.session";

export type UiMemory = {
  gameId: string | null;
  loadoutId: number | null;
  tab: TabId | null;
  expanded: string[];
  rightOpen: boolean;
  rightTab: RightTab;
  llm: string;
  /** Experimental "Profil / Communauté / Économie" screens (see
   * `docs/deep-research-report.md`) — not part of the locked architecture
   * yet, so this stays a client-side preference gate rather than a Core
   * flag. Off by default: nobody sees the mockup screens unless they opt in
   * from Réglages → Général. */
  communityUpdate: boolean;
};

const DEFAULTS: UiMemory = {
  gameId: null,
  loadoutId: null,
  tab: null,
  expanded: [],
  rightOpen: false,
  rightTab: "info",
  llm: "qwen-1.5b",
  communityUpdate: false,
};

export function readUiMemory(): UiMemory {
  try {
    const raw = window.localStorage.getItem(KEY);
    if (!raw) return DEFAULTS;
    const parsed: unknown = JSON.parse(raw);
    if (typeof parsed !== "object" || parsed === null) return DEFAULTS;
    return { ...DEFAULTS, ...(parsed as Partial<UiMemory>) };
  } catch {
    return DEFAULTS;
  }
}

export function writeUiMemory(memory: UiMemory): void {
  try {
    window.localStorage.setItem(KEY, JSON.stringify(memory));
  } catch {
    /* private mode / blocked storage — the session simply resets each launch */
  }
}
