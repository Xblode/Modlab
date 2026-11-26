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
  /** The embedded browser. Not an adapter section: a shell-level view the
   * user opens from the title bar, so it never appears in `TAB_META`, in a
   * game's `capabilities.sections`, or in the tab strip. */
  | "web";

export type TabDef = { id: TabId; label: string; icon: string };

/** Tab vocabulary per adapter section, French labels straight from the maquette. */
const TAB_META: Record<string, TabDef> = {
  dashboard: { id: "dashboard", label: "Mon jeu", icon: "sliders" },
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
  { id: "dashboard", label: "Mon jeu", icon: "sliders" },
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
 * "Mon jeu" is drawn apart from the other tabs. The others keep the adapter's
 * order as one group; the strip draws a rule immediately before this tab.
 */
export function splitGameTab(tabs: TabDef[]): { group: TabDef[]; game: TabDef | null } {
  let game: TabDef | null = null;
  const group: TabDef[] = [];
  for (const tab of tabs) {
    if (tab.id === "dashboard") game = tab;
    else group.push(tab);
  }
  return { group, game };
}

/**
 * A mod site the Mods toolbar can open in the shell’s Web tab.
 *
 * A UI default, not a Core fact: the Core has no Provider and knows nothing
 * about Nexus, CurseForge or Modrinth. The day a real Provider lands, "where
 * this game's mods live" becomes its business and this table goes away.
 */
export type BrowseSite = {
  id: string;
  /** French UI label. Brand names stay as the site writes them. */
  label: string;
  url: string;
};

const SKYRIM_SITES: readonly BrowseSite[] = [
  {
    id: "nexus",
    label: "Nexus Mods",
    url: "https://www.nexusmods.com/skyrimspecialedition",
  },
];

const MINECRAFT_SITES: readonly BrowseSite[] = [
  {
    id: "modrinth",
    label: "Modrinth",
    url: "https://modrinth.com/mods",
  },
  {
    id: "curseforge",
    label: "CurseForge",
    url: "https://www.curseforge.com/minecraft",
  },
];

/**
 * One list per adapter id. A Skyrim id that is not listed yet (VR, LE, …)
 * shares the Special Edition list: the project has no other Nexus home.
 */
const BROWSE_SITES: Record<string, readonly BrowseSite[]> = {
  skyrimse: SKYRIM_SITES,
  minecraft: MINECRAFT_SITES,
};

/** Site ids starred by default the first time a game has no stored favourites. */
const DEFAULT_FAVORITE_IDS: Record<string, readonly string[]> = {
  skyrimse: ["nexus"],
  minecraft: ["modrinth", "curseforge"],
};

const BROWSE_FALLBACK = "https://www.nexusmods.com";

function sitesFor(gameId: string): readonly BrowseSite[] {
  const listed = BROWSE_SITES[gameId];
  if (listed) return listed;
  if (gameId.startsWith("skyrim")) return SKYRIM_SITES;
  return [];
}

/** Sites shown on the Mods toolbar for this game, in button order. */
export function browseSitesFor(gameId: string | null | undefined): readonly BrowseSite[] {
  if (!gameId) return [];
  return sitesFor(gameId);
}

/**
 * Default favourite site ids for this game (before any `uiMemory` override).
 * Skyrim → Nexus; Minecraft → Modrinth + CurseForge.
 */
export function defaultFavoriteSiteIdsFor(gameId: string | null | undefined): readonly string[] {
  if (!gameId) return [];
  const listed = DEFAULT_FAVORITE_IDS[gameId];
  if (listed) return listed;
  if (gameId.startsWith("skyrim")) return DEFAULT_FAVORITE_IDS.skyrimse ?? [];
  return [];
}

/**
 * Sites ordered for the picker: favourites first (in favourite order), then
 * the rest in {@link browseSitesFor} order. Unknown favourite ids are dropped.
 */
export function orderedBrowseSitesFor(
  gameId: string | null | undefined,
  favoriteIds: readonly string[],
): BrowseSite[] {
  const sites = browseSitesFor(gameId);
  if (sites.length === 0) return [];
  const byId = new Map(sites.map((s) => [s.id, s]));
  const seen = new Set<string>();
  const ordered: BrowseSite[] = [];
  for (const id of favoriteIds) {
    const site = byId.get(id);
    if (!site || seen.has(id)) continue;
    ordered.push(site);
    seen.add(id);
  }
  for (const site of sites) {
    if (seen.has(site.id)) continue;
    ordered.push(site);
  }
  return ordered;
}

/**
 * Where the embedded browser lands when it is opened for a given game
 * with no explicit address. The first site of that game, or Nexus.
 */
export function browseHomeFor(gameId: string | null | undefined): string {
  if (!gameId) return BROWSE_FALLBACK;
  return sitesFor(gameId)[0]?.url ?? BROWSE_FALLBACK;
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
