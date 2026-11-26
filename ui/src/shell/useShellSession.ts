import { startTransition, useEffect, useMemo, useRef, useState } from "react";
import {
  gameCategories,
  getShellSnapshot,
  type DetectedGame,
  type GameCategory,
  type LoadoutDto,
  type ShellSnapshot,
} from "../api";
import { useLoadoutVersion } from "../hooks/useLoadoutVersion";
import { instanceRows } from "../instanceRows";
import type { UiMemory } from "../uiMemory";
import { gameIdsIn, reconcileExpanded } from "./shellNavHelpers";

export type UseShellSessionParams = {
  initialMemory: UiMemory;
  gameId: string | null;
  loadoutId: number | null;
  setGameId: (id: string | null) => void;
  setLoadoutId: (id: number | null) => void;
  setExpanded: React.Dispatch<React.SetStateAction<string[]>>;
};

export function useShellSession({
  initialMemory,
  gameId,
  loadoutId,
  setGameId,
  setLoadoutId,
  setExpanded,
}: UseShellSessionParams) {
  const [snap, setSnap] = useState<ShellSnapshot | null>(null);
  const [error, setError] = useState<string | null>(null);
  /** Latest `refresh` wins. A download start and its finish both refresh,
   * and the slower snapshot must not paint the in-progress row back over
   * the archived one. */
  const refreshGen = useRef(0);
  const activeLoadoutIdRef = useRef<number | null>(null);
  const [catalog, setCatalog] = useState<GameCategory[]>([]);
  const [versionTick, setVersionTick] = useState(0);

  useEffect(() => {
    let cancelled = false;
    getShellSnapshot(initialMemory.loadoutId)
      .then((s) => {
        if (cancelled) return;
        const remembered = initialMemory.loadoutId;
        const stillThere = remembered != null && s.loadouts.some((l) => l.id === remembered);
        if (remembered != null && !stillThere) {
          void refresh(null);
          const first = s.games[0]?.id ?? s.loadouts[0]?.gameId ?? null;
          setGameId(first);
          setExpanded((current) => reconcileExpanded(current, first, gameIdsIn(s)));
          return;
        }
        setSnap(s);
        setLoadoutId(s.selectedLoadoutId);
        const owner = s.loadouts.find((l) => l.id === s.selectedLoadoutId)?.gameId;
        const rememberedGameOk =
          initialMemory.gameId != null && s.games.some((g) => g.id === initialMemory.gameId);
        const first =
          owner ??
          (rememberedGameOk ? initialMemory.gameId : null) ??
          s.games[0]?.id ??
          s.loadouts[0]?.gameId ??
          null;
        setGameId(first);
        setExpanded((current) => reconcileExpanded(current, first, gameIdsIn(s)));
      })
      .catch((e: unknown) => {
        if (!cancelled) setError(e instanceof Error ? e.message : String(e));
      });
    return () => {
      cancelled = true;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  useEffect(() => {
    if (!gameId) {
      setCatalog([]);
      return;
    }
    let cancel = false;
    gameCategories(gameId)
      .then((list) => {
        if (!cancel) setCatalog(list);
      })
      .catch(() => {
        if (!cancel) setCatalog([]);
      });
    return () => {
      cancel = true;
    };
  }, [gameId]);

  function applySnapshot(s: ShellSnapshot) {
    startTransition(() => {
      setSnap(s);
      setLoadoutId(s.selectedLoadoutId);
    });
  }

  async function refresh(
    id: number | null,
    opts?: { hydrateMetadata?: boolean },
  ): Promise<number | null> {
    const gen = ++refreshGen.current;
    const s = await getShellSnapshot(id, opts);
    if (gen !== refreshGen.current) return null;
    applySnapshot(s);
    return gen;
  }

  const games = snap?.games ?? [];
  const loadouts = snap?.loadouts ?? [];
  const activeGame: DetectedGame | undefined = games.find((g) => g.id === gameId) ?? games[0];
  const gameLoadouts = loadouts.filter((l) => !activeGame || l.gameId === activeGame.id);
  const activeLoadout: LoadoutDto | undefined =
    gameLoadouts.find((l) => l.id === loadoutId) ?? gameLoadouts[0];
  activeLoadoutIdRef.current = activeLoadout?.id ?? null;

  const instances = useMemo(
    () =>
      instanceRows(loadouts).map((row) =>
        activeLoadout && row.instanceId === activeLoadout.instanceId
          ? { ...row, channel: activeLoadout.channel }
          : row,
      ),
    [loadouts, activeLoadout],
  );
  const gameInstances = instances.filter((l) => !activeGame || l.gameId === activeGame.id);
  const instanceBranches = activeLoadout
    ? loadouts.filter((l) => l.instanceId === activeLoadout.instanceId)
    : [];
  const otherLoadouts = gameInstances.filter((l) => l.instanceId !== activeLoadout?.instanceId);

  const snapshotForSelection =
    snap != null &&
    (loadoutId == null
      ? snap.selectedLoadoutId == null
      : snap.selectedLoadoutId === loadoutId);
  const entries = snapshotForSelection ? snap.entries : [];
  const enabledCount = entries.filter((e) => e.enabled).length;

  const { version: loadoutVersion } = useLoadoutVersion(
    activeLoadout?.id,
    `${versionTick}:${enabledCount}`,
  );

  const hasSnap = snap !== null;

  return {
    snap,
    setSnap,
    error,
    setError,
    refresh,
    applySnapshot,
    refreshGen,
    activeLoadoutIdRef,
    games,
    loadouts,
    activeGame,
    activeLoadout,
    instances,
    gameInstances,
    instanceBranches,
    otherLoadouts,
    entries,
    snapshotForSelection,
    enabledCount,
    catalog,
    versionTick,
    setVersionTick,
    loadoutVersion,
    hasSnap,
  };
}