/**
 * Per-game *presentation chrome* — the purely visual constants the maquette
 * carries in its `games` array: hero art, gradient colours and glyph.
 *
 * This is design data, not state: nothing here is a fact about the machine or
 * about a loadout. Every *fact* on screen still comes from the Core through
 * `api.ts`, and every *capability* (sections, deployment backend, tools)
 * comes from the GameAdapter through `api.ts` as well. This file never
 * infers what a game can do — it only describes how it looks.
 */

import type { GameCapabilities } from "./api";

export type TabId =
  | "dashboard"
  | "mods"
  | "plugins"
  | "archives"
  | "repertoire"
  | "saves"
  | "sante"
  | "packs"
  | "worlds"
  | "servers"
  /** The embedded browser. Not an adapter section: a shell-level tab the
   * user opens from the strip's own globe button, so it never appears in
   * `TAB_META` or in a game's `capabilities.sections`. */
  | "web";

export type TabDef = { id: TabId; label: string; icon: string };

/** Tab vocabulary per adapter section, French labels straight from the maquette. */
const TAB_META: Record<string, TabDef> = {
  dashboard: { id: "dashboard", label: "Tableau de bord", icon: "sliders" },
  mods: { id: "mods", label: "Mods", icon: "layers" },
  plugins: { id: "plugins", label: "Plugins", icon: "file" },
  archives: { id: "archives", label: "Archives", icon: "db" },
  directory: { id: "repertoire", label: "Répertoire", icon: "folder" },
  saves: { id: "saves", label: "Sauvegardes", icon: "disc" },
  health: { id: "sante", label: "Santé", icon: "alert" },
  packs: { id: "packs", label: "Packs & shaders", icon: "image" },
  worlds: { id: "worlds", label: "Mondes", icon: "map" },
  servers: { id: "servers", label: "Serveurs", icon: "globe" },
};

const FALLBACK_TABS: TabDef[] = [
  { id: "dashboard", label: "Tableau de bord", icon: "sliders" },
  { id: "mods", label: "Mods", icon: "layers" },
  { id: "sante", label: "Santé", icon: "alert" },
];

/** Tabs for a game come from its adapter capabilities, in adapter order. */
export function tabsFor(capabilities: GameCapabilities | undefined | null): TabDef[] {
  if (!capabilities) return FALLBACK_TABS;
  return capabilities.sections
    .map((section) => TAB_META[section])
    .filter((tab): tab is TabDef => tab !== undefined);
}

/**
 * Where the embedded browser lands when it is opened for a given game.
 *
 * A UI default, not a Core fact: the Core has no Provider and knows nothing
 * about Nexus or CurseForge. The day a real Provider lands, "where this
 * game's mods live" becomes its business and this table goes away.
 */
const BROWSE_HOME: Record<string, string> = {
  skyrimse: "https://www.nexusmods.com/skyrimspecialedition",
  minecraft: "https://www.curseforge.com/minecraft",
};

const BROWSE_FALLBACK = "https://www.nexusmods.com";

export function browseHomeFor(gameId: string | null | undefined): string {
  if (!gameId) return BROWSE_FALLBACK;
  return BROWSE_HOME[gameId] ?? BROWSE_FALLBACK;
}

export type GameChrome = {
  glyph: string;
  /** Hero gradient, matching the maquette's `--c1g` / `--c2g`. */
  c1: string;
  c2: string;
  /** Wide hero art, served from `ui/public`. Undefined → same wide frame with glyph. */
  art?: string;
};

const CHROME: Record<string, GameChrome> = {
  skyrimse: {
    glyph: "SK",
    c1: "#3b6ea5",
    c2: "#1a2f4d",
    art: "img/header_skyrim.jpg",
  },
  minecraft: {
    glyph: "MC",
    c1: "#5c8f3a",
    c2: "#2b3f1a",
  },
};

const UNKNOWN: GameChrome = {
  glyph: "??",
  c1: "#4a4844",
  c2: "#2a2928",
};

export function chromeFor(gameId: string | null | undefined): GameChrome {
  if (!gameId) return UNKNOWN;
  return CHROME[gameId] ?? UNKNOWN;
}

/** Two-letter thumbnail initials, same rule as the maquette's instance cards. */
export function initials(name: string): string {
  return name
    .split(/\s+/)
    .slice(0, 2)
    .map((w) => w[0] ?? "")
    .join("")
    .toUpperCase();
}
