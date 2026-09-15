import { useCallback, useEffect, useState } from "react";

/**
 * Favourite instances — **browser-local only**.
 *
 * `modlab_core::loadout` has no favourite field, and inventing a Core API for
 * a starred-in-the-sidebar flag would be a schema change behind Benjamin's
 * back. So this lives in `localStorage`, per WebView profile: it does not sync,
 * the CLI and MCP cannot see it, and it disappears if the WebView data is
 * cleared. When the Core grows a real per-loadout favourite flag, this module
 * is the single place to rewire.
 */
const KEY = "modlab.favorites.loadouts";

function read(): number[] {
  try {
    const raw = window.localStorage.getItem(KEY);
    if (!raw) return [];
    const parsed: unknown = JSON.parse(raw);
    if (!Array.isArray(parsed)) return [];
    return parsed.filter((v): v is number => typeof v === "number");
  } catch {
    return [];
  }
}

function write(ids: number[]): void {
  try {
    window.localStorage.setItem(KEY, JSON.stringify(ids));
  } catch {
    /* private mode / blocked storage — favourites simply don't persist */
  }
}

export function useFavorites(): {
  isFavorite: (loadoutId: number) => boolean;
  toggleFavorite: (loadoutId: number) => boolean;
  favoriteIds: number[];
} {
  const [ids, setIds] = useState<number[]>([]);

  useEffect(() => {
    setIds(read());
  }, []);

  const toggleFavorite = useCallback((loadoutId: number) => {
    const current = read();
    const next = current.includes(loadoutId)
      ? current.filter((id) => id !== loadoutId)
      : [...current, loadoutId];
    write(next);
    setIds(next);
    return next.includes(loadoutId);
  }, []);

  const isFavorite = useCallback(
    (loadoutId: number) => ids.includes(loadoutId),
    [ids],
  );

  return { isFavorite, toggleFavorite, favoriteIds: ids };
}
