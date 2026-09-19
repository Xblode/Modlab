import { useEffect, useMemo, useRef, useState } from "react";
import {
  getLoadoutConflicts,
  type LoadoutConflicts,
  type LoadoutDto,
  type ModConflicts,
  type ModRow,
} from "../api";

/**
 * Conflict scan of the active loadout, as told by the Core.
 *
 * Nothing is computed here: the frontend asks the Core and caches the answer.
 * `null` means *not scanned yet* — the table must never render that as "no
 * conflict".
 */
export function useConflicts(
  activeLoadout: LoadoutDto | undefined,
  entries: ModRow[],
): {
  conflicts: LoadoutConflicts | null;
  conflictByMod: Map<number, ModConflicts> | null;
} {
  const [conflicts, setConflicts] = useState<LoadoutConflicts | null>(null);
  const conflictsCache = useRef(new Map<string, LoadoutConflicts>());

  /**
   * The enabled set the current conflict scan describes. Enabling or
   * disabling a mod changes what conflicts exist, so this is what a scan is
   * keyed on — not the entries array identity.
   */
  const enabledKey = useMemo(
    () =>
      entries
        .filter((e) => e.enabled)
        .map((e) => e.modId)
        .join(","),
    [entries],
  );

  /**
   * Scanning a real loadout walks every enabled mod's folder, so this runs
   * once per (loadout, enabled set) instead of on every render: results are
   * memoized by that key, the call is debounced, and a key change cancels the
   * pending one. A stale or missing scan renders as *unknown* (no badge), never
   * as "no conflict".
   */
  useEffect(() => {
    if (!activeLoadout) {
      setConflicts(null);
      return;
    }
    const key = `${activeLoadout.id}|${enabledKey}`;
    const cached = conflictsCache.current.get(key);
    if (cached) {
      setConflicts(cached);
      return;
    }
    setConflicts(null);
    let cancelled = false;
    const timer = window.setTimeout(() => {
      getLoadoutConflicts(activeLoadout.id)
        .then((next) => {
          conflictsCache.current.set(key, next);
          if (!cancelled) setConflicts(next);
        })
        .catch(() => {
          /* stays null: an unscanned loadout is not an empty one */
        });
    }, 250);
    return () => {
      cancelled = true;
      window.clearTimeout(timer);
    };
  }, [activeLoadout, enabledKey]);

  const conflictByMod = useMemo(() => {
    if (!conflicts) return null;
    return new Map(conflicts.mods.map((m) => [m.modId, m]));
  }, [conflicts]);

  return { conflicts, conflictByMod };
}
