import { useCallback, useEffect, useState } from "react";
import { listDownloads, type Download } from "../api";

/**
 * Every capture the browser recorded — loaded on open and after any refresh
 * trigger (a new capture landing, per `App.tsx`'s `onDownloadCaptured`
 * listener), same "load on demand, don't cache forever" rule
 * `useTransactions` follows.
 */
export function useDownloads(open: boolean, refreshKey: number): {
  downloads: Download[] | null;
  refreshDownloads: () => void;
} {
  const [downloads, setDownloads] = useState<Download[] | null>(null);

  const refreshDownloads = useCallback(() => {
    listDownloads()
      .then((next) => setDownloads(next))
      .catch(() => setDownloads(null));
  }, []);

  useEffect(() => {
    if (open) refreshDownloads();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open, refreshKey, refreshDownloads]);

  return { downloads, refreshDownloads };
}
