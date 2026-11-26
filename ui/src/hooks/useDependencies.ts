import { useEffect, useMemo, useRef, useState } from "react";
import {
  getLoadoutDependencies,
  type LoadoutDto,
  type ModDependencyReport,
  type ModRow,
  type ResolvedDependency,
} from "../api";

/**
 * What the dependency scan is keyed on. Enabling a mod, disabling the mod
 * that provides a master, or a newly stored dependency list all change it.
 */
function dependencyKey(entries: ModRow[]): string {
  return entries
    .map((entry) =>
      [
        entry.modId,
        entry.enabled ? 1 : 0,
        entry.position,
        entry.status,
        entry.tool ?? "",
        entry.sourcePath,
        entry.dependencies.join("\n"),
      ].join(":"),
    )
    .join("|");
}

/**
 * Dependency scan of the active loadout, as told by the game adapter.
 *
 * `null` means not scanned yet. An empty map means the scan ran and found
 * nothing to report — the table must not draw a badge for that.
 */
export function useDependencies(
  activeLoadout: LoadoutDto | undefined,
  entries: ModRow[],
): {
  dependencyByMod: Map<number, ResolvedDependency[]> | null;
} {
  const [reports, setReports] = useState<ModDependencyReport[] | null>(null);
  const cache = useRef(new Map<string, ModDependencyReport[]>());
  const key = useMemo(() => dependencyKey(entries), [entries]);

  useEffect(() => {
    if (!activeLoadout) {
      setReports(null);
      return;
    }
    const cacheKey = `${activeLoadout.id}|${key}`;
    const cached = cache.current.get(cacheKey);
    if (cached) {
      setReports(cached);
      return;
    }
    setReports(null);
    let cancelled = false;
    const timer = window.setTimeout(() => {
      getLoadoutDependencies(activeLoadout.id)
        .then((next) => {
          cache.current.set(cacheKey, next);
          if (!cancelled) setReports(next);
        })
        .catch(() => {
          /* stays null: an unscanned loadout is not an empty one */
        });
    }, 250);
    return () => {
      cancelled = true;
      window.clearTimeout(timer);
    };
  }, [activeLoadout, key]);

  const dependencyByMod = useMemo(() => {
    if (!reports) return null;
    return new Map(reports.map((report) => [report.modId, report.dependencies]));
  }, [reports]);

  return { dependencyByMod };
}
