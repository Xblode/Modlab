import {
  canonicalRightTab,
  isFixedRightTab,
  sanitizePersistedRightTabs,
  type FixedRightTab,
} from "./components/Features/Inspector/RightPanel";
import {
  browseSitesFor,
  defaultFavoriteSiteIdsFor,
  type BrowseSite,
  type TabId,
} from "./gameChrome";
import { DEFAULT_DENSITY, isDensity, type Density } from "./density";
import { sanitizeModSortMap, type ModSort } from "./modSort";
import {
  customSiteIdForUrl,
  labelForUrl,
  sanitizeWebHomepage,
  type WebHomepage,
  DEFAULT_WEB_HOMEPAGE,
} from "./webHomepage";

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
  /** Last focused *fixed* utility tab (Web pages are session-only, not stored). */
  rightTab: FixedRightTab;
  /** Fixed tabs in the right column, in strip order. Empty closes the column.
   *  Web page tabs (`web:<id>`) are never persisted — see {@link legacyRightTabs}. */
  rightTabs: FixedRightTab[];
  llm: string;
  /** Experimental "Profil / Communauté / Économie" screens (see
   * `.bframe/VISION.md`) — not part of the locked architecture
   * yet, so this stays a client-side preference gate rather than a Core
   * flag. Off by default: nobody sees the mockup screens unless they opt in
   * from Réglages → Général. */
  communityUpdate: boolean;
  /** Row and control size (`data-density` on the root element). */
  density: Density;
  /** Left bar width, in px. Stored within {@link COLUMN_WIDTH_MIN}–{@link COLUMN_WIDTH_MAX}.
   *  The shell draws it no wider than half the window — see {@link columnWidthCeiling}. */
  sideWidth: number;
  /** Right column width, in px. Stored within {@link COLUMN_WIDTH_MIN}–{@link UTILITY_WIDTH_MAX}.
   *  The shell draws it no wider than the window minus 320 px — see {@link utilityWidthCeiling}. */
  utilityWidth: number;
  /**
   * Mods table display sort, per game id. Missing key = the Core's priority
   * order, the same screen order as priority ascending. A stored sort never
   * writes that order; it only remembers how the list is drawn. `version` is
   * no longer a sort: an old blob that still names it is dropped.
   */
  modSort: Record<string, ModSort>;
  /** Offline Minecraft pseudo (auth v1). */
  minecraftPlayer: string;
  /** Explorer: list every catalog category, including those with zero mods. */
  showEmptyCategories: boolean;
  /**
   * Favourite browse-site ids per game. Missing key → game defaults from
   * `defaultFavoriteSiteIdsFor`. Present key (even `[]`) → user’s choice.
   */
  browseSiteFavorites: Record<string, string[]>;
  /**
   * User-added browse sites (starred from the Web address bar), per game.
   * Merged with the built-in list from `gameChrome`.
   */
  browseCustomSites: Record<string, BrowseSite[]>;
  /** Embedded browser homepage / search engine. Default Google. */
  webHomepage: WebHomepage;
};

/** Shared floor for the left bar and the right column. */
export const COLUMN_WIDTH_MIN = 200;
/** Stored and drawn ceiling for the left bar. */
export const COLUMN_WIDTH_MAX = 720;
/** Stored ceiling for the right column. The drawn width also stops 320 px short of the window. */
export const UTILITY_WIDTH_MAX = 1400;
/** Pixels kept for the center sheet: the right column is at most the window minus this. */
const UTILITY_CENTER_RESERVE = 320;

/**
 * Widest the left bar may be drawn right now: {@link COLUMN_WIDTH_MAX}, and never
 * more than half the window. When half the window is under the floor, the
 * floor wins.
 */
export function columnWidthCeiling(windowWidth: number): number {
  if (!Number.isFinite(windowWidth)) return COLUMN_WIDTH_MAX;
  const half = Math.floor(windowWidth / 2);
  return Math.min(COLUMN_WIDTH_MAX, Math.max(COLUMN_WIDTH_MIN, half));
}

/**
 * Widest the right column may be drawn right now: {@link UTILITY_WIDTH_MAX},
 * and never wider than the window minus 320 px. When that remainder is under
 * the floor, the floor wins.
 */
export function utilityWidthCeiling(windowWidth: number): number {
  if (!Number.isFinite(windowWidth)) return UTILITY_WIDTH_MAX;
  const room = Math.floor(windowWidth) - UTILITY_CENTER_RESERVE;
  return Math.min(UTILITY_WIDTH_MAX, Math.max(COLUMN_WIDTH_MIN, room));
}

const DEFAULT_SIDE_WIDTH = 280;
const DEFAULT_UTILITY_WIDTH = 320;

/** Every remembered open game, in order. A bare string (older blob) is one id. */
function expandedIds(value: unknown): string[] {
  const source = typeof value === "string" ? [value] : value;
  if (!Array.isArray(source)) return [];
  const ids: string[] = [];
  for (const item of source) {
    if (typeof item === "string" && item !== "" && !ids.includes(item)) ids.push(item);
  }
  return ids;
}

const DEFAULTS: UiMemory = {
  gameId: null,
  loadoutId: null,
  tab: null,
  expanded: [],
  rightOpen: false,
  rightTab: "info",
  rightTabs: [],
  llm: "qwen-1.5b",
  communityUpdate: false,
  density: DEFAULT_DENSITY,
  sideWidth: DEFAULT_SIDE_WIDTH,
  utilityWidth: DEFAULT_UTILITY_WIDTH,
  modSort: {},
  minecraftPlayer: "Player",
  showEmptyCategories: true,
  browseSiteFavorites: {},
  browseCustomSites: {},
  webHomepage: { ...DEFAULT_WEB_HOMEPAGE },
};

/** Pixel width inside the left-bar range. Non-finite input falls back. */
export function clampColumnWidth(value: unknown, fallback = DEFAULT_SIDE_WIDTH): number {
  return clampWidth(value, fallback, COLUMN_WIDTH_MAX);
}

/** Pixel width inside the right-column range. Non-finite input falls back. */
export function clampUtilityWidth(value: unknown, fallback = DEFAULT_UTILITY_WIDTH): number {
  return clampWidth(value, fallback, UTILITY_WIDTH_MAX);
}

function clampWidth(value: unknown, fallback: number, max: number): number {
  const n = typeof value === "number" ? value : Number.NaN;
  if (!Number.isFinite(n)) return fallback;
  return Math.min(max, Math.max(COLUMN_WIDTH_MIN, Math.round(n)));
}

/** Drop non-string ids and duplicates; keep order. */
function sanitizeSiteIdList(value: unknown): string[] {
  if (!Array.isArray(value)) return [];
  const ids: string[] = [];
  for (const item of value) {
    if (typeof item === "string" && item !== "" && !ids.includes(item)) ids.push(item);
  }
  return ids;
}

function sanitizeBrowseSiteFavorites(value: unknown): Record<string, string[]> {
  if (typeof value !== "object" || value === null || Array.isArray(value)) return {};
  const out: Record<string, string[]> = {};
  for (const [gameId, list] of Object.entries(value as Record<string, unknown>)) {
    if (typeof gameId !== "string" || gameId === "") continue;
    out[gameId] = sanitizeSiteIdList(list);
  }
  return out;
}

function sanitizeBrowseSite(value: unknown): BrowseSite | null {
  if (typeof value !== "object" || value === null || Array.isArray(value)) return null;
  const record = value as Record<string, unknown>;
  const id = typeof record.id === "string" ? record.id.trim() : "";
  const label = typeof record.label === "string" ? record.label.trim() : "";
  const url = typeof record.url === "string" ? record.url.trim() : "";
  if (!id || !label || !url) return null;
  return { id, label, url };
}

function sanitizeBrowseCustomSites(value: unknown): Record<string, BrowseSite[]> {
  if (typeof value !== "object" || value === null || Array.isArray(value)) return {};
  const out: Record<string, BrowseSite[]> = {};
  for (const [gameId, list] of Object.entries(value as Record<string, unknown>)) {
    if (typeof gameId !== "string" || gameId === "") continue;
    if (!Array.isArray(list)) continue;
    const sites: BrowseSite[] = [];
    const seen = new Set<string>();
    for (const item of list) {
      const site = sanitizeBrowseSite(item);
      if (!site || seen.has(site.id)) continue;
      seen.add(site.id);
      sites.push(site);
    }
    out[gameId] = sites;
  }
  return out;
}

/**
 * Blobs written before the tab strip: the column was either the floating
 * inspector (`rightOpen` + `rightTab`) or, when the center tab was still
 * `"web"`, the embedded browser. A closed inspector stays closed. Legacy
 * `"web"` no longer restores a drawer page (Web tabs are session-only).
 */
function legacyRightTabs(raw: Record<string, unknown>): FixedRightTab[] {
  const tabs: FixedRightTab[] = [];
  const right = canonicalRightTab(raw.rightTab);
  if (raw.rightOpen === true && right && isFixedRightTab(right) && !tabs.includes(right)) {
    tabs.push(right);
  }
  return sanitizePersistedRightTabs(tabs);
}

export function readUiMemory(): UiMemory {
  try {
    const raw = window.localStorage.getItem(KEY);
    if (!raw) return DEFAULTS;
    const parsed: unknown = JSON.parse(raw);
    if (typeof parsed !== "object" || parsed === null) return DEFAULTS;
    const record = parsed as Record<string, unknown>;
    const merged = { ...DEFAULTS, ...(record as Partial<UiMemory>) };
    const rightTabs = Object.prototype.hasOwnProperty.call(record, "rightTabs")
      ? sanitizePersistedRightTabs(record.rightTabs)
      : legacyRightTabs(record);
    const preferred = canonicalRightTab(record.rightTab);
    const rightTab =
      preferred && isFixedRightTab(preferred) ? preferred : DEFAULTS.rightTab;
    return {
      ...merged,
      expanded: expandedIds(merged.expanded),
      rightTabs,
      rightOpen: rightTabs.length > 0,
      rightTab,
      density: isDensity(merged.density) ? merged.density : DEFAULT_DENSITY,
      sideWidth: clampColumnWidth(merged.sideWidth, DEFAULT_SIDE_WIDTH),
      utilityWidth: clampUtilityWidth(merged.utilityWidth, DEFAULT_UTILITY_WIDTH),
      modSort: sanitizeModSortMap(record.modSort),
      minecraftPlayer:
        typeof merged.minecraftPlayer === "string" && merged.minecraftPlayer.trim() !== ""
          ? merged.minecraftPlayer.trim()
          : DEFAULTS.minecraftPlayer,
      showEmptyCategories: merged.showEmptyCategories !== false,
      browseSiteFavorites: sanitizeBrowseSiteFavorites(record.browseSiteFavorites),
      browseCustomSites: sanitizeBrowseCustomSites(record.browseCustomSites),
      webHomepage: sanitizeWebHomepage(record.webHomepage),
    };
  } catch {
    return DEFAULTS;
  }
}

/**
 * Patch the remembered session and write the whole blob back.
 *
 * Callers may omit `sideWidth` / `utilityWidth` / `modSort` (the shell screen
 * in `App` rewrites every other field). Those stay as last stored, so a
 * screen save does not drop a width the user just dragged or a sort they
 * just picked.
 */
export function writeUiMemory(memory: Partial<UiMemory>): void {
  try {
    const current = readUiMemory();
    const rightTabs =
      memory.rightTabs === undefined
        ? current.rightTabs
        : sanitizePersistedRightTabs(memory.rightTabs);
    const preferred = memory.rightTab === undefined ? current.rightTab : memory.rightTab;
    const canon = canonicalRightTab(preferred);
    const rightTab = canon && isFixedRightTab(canon) ? canon : DEFAULTS.rightTab;
    const merged: UiMemory = {
      ...current,
      ...memory,
      expanded: expandedIds(memory.expanded === undefined ? current.expanded : memory.expanded),
      rightTabs,
      rightTab,
      rightOpen: rightTabs.length > 0,
      density:
        memory.density === undefined
          ? current.density
          : isDensity(memory.density)
            ? memory.density
            : DEFAULT_DENSITY,
      sideWidth: clampColumnWidth(
        memory.sideWidth ?? current.sideWidth,
        DEFAULT_SIDE_WIDTH,
      ),
      utilityWidth: clampUtilityWidth(
        memory.utilityWidth ?? current.utilityWidth,
        DEFAULT_UTILITY_WIDTH,
      ),
      modSort: sanitizeModSortMap(
        memory.modSort === undefined ? current.modSort : memory.modSort,
      ),
      browseSiteFavorites: sanitizeBrowseSiteFavorites(
        memory.browseSiteFavorites === undefined
          ? current.browseSiteFavorites
          : memory.browseSiteFavorites,
      ),
      browseCustomSites: sanitizeBrowseCustomSites(
        memory.browseCustomSites === undefined
          ? current.browseCustomSites
          : memory.browseCustomSites,
      ),
      webHomepage: sanitizeWebHomepage(
        memory.webHomepage === undefined ? current.webHomepage : memory.webHomepage,
      ),
    };
    window.localStorage.setItem(KEY, JSON.stringify(merged));
  } catch {
    /* private mode / blocked storage — the session simply resets each launch */
  }
}

/** The Mods table sort remembered for this game, or null for priority order. */
export function modSortFor(gameId: string | null): ModSort | null {
  if (!gameId) return null;
  return readUiMemory().modSort[gameId] ?? null;
}

/** Remember or clear the Mods table sort for one game. Other games stay. */
export function rememberModSort(gameId: string | null, sort: ModSort | null): void {
  if (!gameId) return;
  const modSort = { ...readUiMemory().modSort };
  if (sort) modSort[gameId] = sort;
  else delete modSort[gameId];
  writeUiMemory({ modSort });
}

/** Built-in sites plus user-starred custom URLs for this game. */
export function allBrowseSitesFor(gameId: string | null | undefined): BrowseSite[] {
  if (!gameId) return [];
  const builtin = browseSitesFor(gameId);
  const custom = readUiMemory().browseCustomSites[gameId] ?? [];
  if (custom.length === 0) return [...builtin];
  const seen = new Set(builtin.map((s) => s.id));
  const merged = [...builtin];
  for (const site of custom) {
    if (seen.has(site.id)) continue;
    seen.add(site.id);
    merged.push(site);
  }
  return merged;
}

/** Favourites first, then the rest — same order as the site picker. */
export function orderedAllBrowseSitesFor(
  gameId: string | null | undefined,
  favoriteIds: readonly string[],
): BrowseSite[] {
  const sites = allBrowseSitesFor(gameId);
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
 * Favourite browse-site ids for this game. Missing memory key → defaults
 * from {@link defaultFavoriteSiteIdsFor}. Ids unknown to the game’s site
 * list (built-in + custom) are dropped.
 */
export function browseSiteFavoritesFor(gameId: string | null | undefined): string[] {
  if (!gameId) return [];
  const known = new Set(allBrowseSitesFor(gameId).map((s) => s.id));
  const stored = readUiMemory().browseSiteFavorites;
  const raw = Object.prototype.hasOwnProperty.call(stored, gameId)
    ? stored[gameId]!
    : [...defaultFavoriteSiteIdsFor(gameId)];
  return raw.filter((id) => known.has(id));
}

/** Star or unstar a browse site for one game; returns the new favourite ids. */
export function toggleBrowseSiteFavorite(
  gameId: string | null | undefined,
  siteId: string,
): string[] {
  if (!gameId || siteId === "") return [];
  const known = new Set(allBrowseSitesFor(gameId).map((s) => s.id));
  if (!known.has(siteId)) return browseSiteFavoritesFor(gameId);
  const current = browseSiteFavoritesFor(gameId);
  const next = current.includes(siteId)
    ? current.filter((id) => id !== siteId)
    : [...current, siteId];
  writeUiMemory({
    browseSiteFavorites: {
      ...readUiMemory().browseSiteFavorites,
      [gameId]: next,
    },
  });
  return next;
}

/**
 * Add (or refresh) a custom URL as a favourite browse site for each game id.
 * Returns the favourite ids for the first game, for callers that refresh one list.
 */
export function addBrowseUrlFavorite(
  gameIds: readonly string[],
  url: string,
): string[] {
  const trimmed = url.trim();
  if (!trimmed || gameIds.length === 0) return [];
  const id = customSiteIdForUrl(trimmed);
  const label = labelForUrl(trimmed);
  const href = /^https?:\/\//i.test(trimmed) ? trimmed : `https://${trimmed}`;
  const memory = readUiMemory();
  const custom = { ...memory.browseCustomSites };
  const favorites = { ...memory.browseSiteFavorites };
  for (const gameId of gameIds) {
    if (!gameId) continue;
    const existing = custom[gameId] ?? [];
    const without = existing.filter((s) => s.id !== id);
    custom[gameId] = [...without, { id, label, url: href }];
    const favBase = Object.prototype.hasOwnProperty.call(favorites, gameId)
      ? favorites[gameId]!
      : [...defaultFavoriteSiteIdsFor(gameId)];
    favorites[gameId] = favBase.includes(id) ? favBase : [...favBase, id];
  }
  writeUiMemory({ browseCustomSites: custom, browseSiteFavorites: favorites });
  return browseSiteFavoritesFor(gameIds[0]);
}

export function webHomepageSetting(): WebHomepage {
  return sanitizeWebHomepage(readUiMemory().webHomepage);
}

export function rememberWebHomepage(homepage: WebHomepage): void {
  writeUiMemory({ webHomepage: sanitizeWebHomepage(homepage) });
}
