import { useEffect, useRef, useState } from "react";
import {
  clearDownloadsDirOverride,
  clearGamePathOverride,
  clearInstancePathOverride,
  getGamePaths,
  setDownloadsDirOverride,
  setGamePathOverride,
  setInstancePathOverride,
  type GamePaths,
  type ManagedPath,
} from "../api";

function text(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

/**
 * The four locations `game_paths` returns. Saves go through the command
 * `ManagedPath.update` names, then the list is read again.
 */
export function useGamePaths(gameId: string, loadoutId: number | null = null) {
  const [paths, setPaths] = useState<GamePaths | null>(null);
  const [error, setError] = useState<string | null>(null);
  const requested = useRef(`${gameId}:${loadoutId ?? ""}`);

  useEffect(() => {
    const current = gameId;
    const token = `${current}:${loadoutId ?? ""}`;
    requested.current = token;
    setPaths(null);
    setError(null);
    getGamePaths(current, loadoutId)
      .then((next) => {
        if (requested.current === token) setPaths(next);
      })
      .catch((reason: unknown) => {
        if (requested.current === token) setError(text(reason));
      });
  }, [gameId, loadoutId]);

  async function reload(current: string) {
    const next = await getGamePaths(current, loadoutId);
    if (requested.current === `${current}:${loadoutId ?? ""}`) setPaths(next);
  }

  async function saveManaged(path: ManagedPath, value: string) {
    const current = gameId;
    if (!path.update) return;
    if (path.update.via === "gamePathOverride") {
      await setGamePathOverride(current, path.update.slotId, value);
    } else if (path.update.via === "instancePathOverride") {
      await setInstancePathOverride(path.update.instanceId, path.update.slotId, value);
    } else {
      await setDownloadsDirOverride(value);
    }
    await reload(current);
  }

  async function resetManaged(path: ManagedPath) {
    const current = gameId;
    if (!path.update) return;
    if (path.update.via === "gamePathOverride") {
      await clearGamePathOverride(current, path.update.slotId);
    } else if (path.update.via === "instancePathOverride") {
      await clearInstancePathOverride(path.update.instanceId, path.update.slotId);
    } else {
      await clearDownloadsDirOverride();
    }
    await reload(current);
  }

  async function saveIni(slotId: string, value: string) {
    const current = gameId;
    await setGamePathOverride(current, slotId, value);
    await reload(current);
  }

  async function resetIni(slotId: string) {
    const current = gameId;
    await clearGamePathOverride(current, slotId);
    await reload(current);
  }

  return { paths, error, saveManaged, resetManaged, saveIni, resetIni };
}
