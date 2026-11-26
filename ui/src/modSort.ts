import type { GameCategory, ModRow } from "./api";
import { presentCategory } from "./categoryIcon";

export const MOD_SORT_KEYS = ["priority", "name", "size", "date", "category"] as const;

export type ModSortKey = (typeof MOD_SORT_KEYS)[number];
export type ModSortDir = "asc" | "desc";

/**
 * Display order of the Mods table. No sort, and priority ascending, both
 * follow the Core's priority order. Priority descending only flips the draw.
 */
export type ModSort = {
  key: ModSortKey;
  dir: ModSortDir;
};

const KEYS = new Set<string>(MOD_SORT_KEYS);

/** Date and size open on the end you usually want: newest, then largest. */
const DESC_FIRST = new Set<ModSortKey>(["size", "date"]);

const TEXT: Intl.CollatorOptions = { numeric: true, sensitivity: "base" };

/**
 * Click cycle for one column. A different column starts at its first
 * direction. The third click (second for date and size) clears the sort.
 */
export function nextModSort(current: ModSort | null, key: ModSortKey): ModSort | null {
  if (current?.key !== key) {
    return { key, dir: DESC_FIRST.has(key) ? "desc" : "asc" };
  }
  if (DESC_FIRST.has(key)) {
    return current.dir === "desc" ? { key, dir: "asc" } : null;
  }
  return current.dir === "asc" ? { key, dir: "desc" } : null;
}

/** Drop anything the stored blob does not describe. Unknown games are kept. */
export function sanitizeModSortMap(value: unknown): Record<string, ModSort> {
  if (typeof value !== "object" || value === null || Array.isArray(value)) return {};
  const out: Record<string, ModSort> = {};
  for (const [gameId, raw] of Object.entries(value)) {
    if (gameId === "" || typeof raw !== "object" || raw === null) continue;
    const key = (raw as { key?: unknown }).key;
    const dir = (raw as { dir?: unknown }).dir;
    if (typeof key !== "string" || !KEYS.has(key)) continue;
    if (dir !== "asc" && dir !== "desc") continue;
    out[gameId] = { key: key as ModSortKey, dir };
  }
  return out;
}

function textCmp(a: string, b: string): number {
  return a.localeCompare(b, "fr", TEXT);
}

function textOf(value: string | null | undefined): string | null {
  const trimmed = value?.trim() ?? "";
  return trimmed === "" ? null : trimmed;
}

function downloadedTime(row: ModRow): number | null {
  if (!row.downloadedAt) return null;
  const time = Date.parse(row.downloadedAt);
  return Number.isNaN(time) ? null : time;
}

/**
 * Empty values stay last in either direction. A tie keeps priority order
 * (`position` ascending — the order the Core already returns).
 */
function finish(
  aEmpty: boolean,
  bEmpty: boolean,
  cmp: number,
  dir: ModSortDir,
  a: ModRow,
  b: ModRow,
): number {
  if (aEmpty || bEmpty) {
    if (aEmpty && bEmpty) return a.position - b.position;
    return aEmpty ? 1 : -1;
  }
  if (cmp === 0) return a.position - b.position;
  return dir === "asc" ? cmp : -cmp;
}

/** Display comparison. Does not change `position`. */
export function compareModRows(
  a: ModRow,
  b: ModRow,
  sort: ModSort,
  catalog: readonly GameCategory[],
): number {
  switch (sort.key) {
    case "name": {
      const left = textOf(a.name);
      const right = textOf(b.name);
      return finish(
        left == null,
        right == null,
        left == null || right == null ? 0 : textCmp(left, right),
        sort.dir,
        a,
        b,
      );
    }
    case "priority":
      return sort.dir === "asc" ? a.position - b.position : b.position - a.position;
    case "size": {
      const left = a.sizeBytes != null && Number.isFinite(a.sizeBytes) ? a.sizeBytes : null;
      const right = b.sizeBytes != null && Number.isFinite(b.sizeBytes) ? b.sizeBytes : null;
      return finish(
        left == null,
        right == null,
        left == null || right == null ? 0 : left - right,
        sort.dir,
        a,
        b,
      );
    }
    case "date": {
      const left = downloadedTime(a);
      const right = downloadedTime(b);
      return finish(
        left == null,
        right == null,
        left == null || right == null ? 0 : left - right,
        sort.dir,
        a,
        b,
      );
    }
    case "category": {
      const left = textOf(a.category);
      const right = textOf(b.category);
      const leftLabel = left == null ? null : presentCategory(left, catalog).label;
      const rightLabel = right == null ? null : presentCategory(right, catalog).label;
      return finish(
        leftLabel == null,
        rightLabel == null,
        leftLabel == null || rightLabel == null ? 0 : textCmp(leftLabel, rightLabel),
        sort.dir,
        a,
        b,
      );
    }
  }
}
