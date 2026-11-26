import { useCallback, useEffect, useState } from "react";
import { getVanillaControl, type VanillaControlView } from "../api";

export type VanillaControlState = VanillaControlView | { status: "pending" };

/**
 * Read-only scan from `game_vanilla_scan`. `refresh` re-reads without
 * clearing the summary. Nothing here deletes a file.
 */
export function useVanillaControl(gameId: string): {
  state: VanillaControlState;
  refresh: () => Promise<VanillaControlView>;
} {
  const [state, setState] = useState<VanillaControlState>({ status: "pending" });

  useEffect(() => {
    let cancelled = false;
    setState({ status: "pending" });
    getVanillaControl(gameId)
      .then((next) => {
        if (!cancelled) setState(next);
      })
      .catch((error: unknown) => {
        if (!cancelled) {
          setState({
            status: "error",
            message: error instanceof Error ? error.message : String(error),
          });
        }
      });
    return () => {
      cancelled = true;
    };
  }, [gameId]);

  const refresh = useCallback(async () => {
    const next = await getVanillaControl(gameId);
    setState(next);
    return next;
  }, [gameId]);

  return { state, refresh };
}
