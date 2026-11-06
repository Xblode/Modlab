import { useCallback, useEffect, useState } from "react";
import { getLoadoutVersion, type LoadoutVersion } from "../api";

/**
 * The active loadout's revision count and dirty state, refetched whenever
 * the loadout changes or `refreshKey` changes — the caller bumps that after
 * anything that could move either number (a mod toggle, a snapshot, a
 * rollback), the same "recompute on demand" rule `useConflicts` follows
 * rather than trying to keep a cache in sync by hand. `null` is "not loaded
 * yet", not "no revisions" — `ActiveInstanceCard` renders those differently.
 */
export function useLoadoutVersion(
  loadoutId: number | undefined,
  refreshKey: string | number,
): { version: LoadoutVersion | null; refresh: () => void } {
  const [version, setVersion] = useState<LoadoutVersion | null>(null);

  const load = useCallback(() => {
    if (loadoutId === undefined) {
      setVersion(null);
      return;
    }
    getLoadoutVersion(loadoutId)
      .then((v) => setVersion(v))
      .catch(() => setVersion(null));
  }, [loadoutId]);

  // `refreshKey` is a deliberate extra trigger, not part of the fetch
  // itself — it has no bearing on *what* to load, only *when* to reload.
  useEffect(() => {
    load();
  }, [load, refreshKey]);

  return { version, refresh: load };
}
