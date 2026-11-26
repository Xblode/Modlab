import type { ShellSnapshot } from "../api";
import type {
  FixedRightTab,
  RightPaneState,
  RightTab,
} from "../components/Features/Inspector/RightPanel";
import { sanitizePersistedRightTabs } from "../components/Features/Inspector/RightPanel";

/**
 * Open or close this game only. Every other id in `expanded` stays as it is,
 * so one folder never closes another.
 */
export function toggleExpandedGame(expanded: readonly string[], gameId: string): string[] {
  if (expanded.includes(gameId)) return expanded.filter((id) => id !== gameId);
  return [...expanded, gameId];
}

/**
 * Make sure `gameId` is open. Other open games stay open.
 * Returns the same array when that id is already present.
 */
export function expandGameKeepingOthers(expanded: readonly string[], gameId: string): string[] {
  if (expanded.includes(gameId)) return expanded as string[];
  return [...expanded, gameId];
}

/** Drop ids the snapshot no longer knows, then open `activeGameId` if we have one. */
export function reconcileExpanded(
  expanded: readonly string[],
  activeGameId: string | null,
  knownGameIds: ReadonlySet<string>,
): string[] {
  const kept = expanded.filter((id) => knownGameIds.has(id));
  const base = kept.length === expanded.length ? (expanded as string[]) : kept;
  if (activeGameId && knownGameIds.has(activeGameId)) return expandGameKeepingOthers(base, activeGameId);
  return base;
}

export function gameIdsIn(snapshot: Pick<ShellSnapshot, "games" | "loadouts">): Set<string> {
  const ids = new Set<string>();
  for (const game of snapshot.games) ids.add(game.id);
  for (const loadout of snapshot.loadouts) ids.add(loadout.gameId);
  return ids;
}

/** Empty strip when nothing was open. A restored tab that is not in the strip is ignored.
 *  Web page tabs are never in memory — only fixed utility tabs come back. */
export function initialRightPane(memory: {
  rightTabs: readonly FixedRightTab[];
  rightTab: FixedRightTab;
}): RightPaneState {
  const tabs = sanitizePersistedRightTabs(memory.rightTabs) as RightTab[];
  if (tabs.length === 0) return { tabs: [], active: null };
  const active = tabs.includes(memory.rightTab) ? memory.rightTab : tabs[0];
  return { tabs, active };
}