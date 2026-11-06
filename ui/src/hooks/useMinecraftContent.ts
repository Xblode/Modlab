import { useEffect, useState } from "react";
import { getMinecraftContent, type MinecraftContent } from "../api";

/**
 * Resource packs, shader packs and worlds of the detected Minecraft
 * instance, loaded once per mount. `null` is "not loaded yet" — the views
 * render that differently from an instance that genuinely has none, the same
 * distinction `useTransactions` and `useConflicts` keep.
 */
export function useMinecraftContent(): {
  content: MinecraftContent | null;
  error: string | null;
} {
  const [content, setContent] = useState<MinecraftContent | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    getMinecraftContent()
      .then((next) => {
        if (!cancelled) setContent(next);
      })
      .catch((e: unknown) => {
        if (!cancelled) setError(e instanceof Error ? e.message : String(e));
      });
    return () => {
      cancelled = true;
    };
  }, []);

  return { content, error };
}
