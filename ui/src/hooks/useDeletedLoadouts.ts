import { useCallback, useEffect, useState } from "react";
import { listDeletedLoadouts, type DeletedLoadout } from "../api";

/**
 * The loadout trash — loaded on open and after every restore, same
 * recompute-on-demand rule as `useTransactions`. `null` is "not loaded yet",
 * rendered differently from an empty trash.
 */
export function useDeletedLoadouts(open: boolean): {
  trash: DeletedLoadout[] | null;
  refreshTrash: () => void;
} {
  const [trash, setTrash] = useState<DeletedLoadout[] | null>(null);

  const refreshTrash = useCallback(() => {
    listDeletedLoadouts()
      .then((next) => setTrash(next))
      .catch(() => setTrash(null));
  }, []);

  useEffect(() => {
    if (open) refreshTrash();
  }, [open, refreshTrash]);

  return { trash, refreshTrash };
}
