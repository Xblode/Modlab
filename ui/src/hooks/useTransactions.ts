import { useCallback, useEffect, useState } from "react";
import { getTransactions, type Transaction } from "../api";

/**
 * The transaction journal lives in the Core; the panel loads it on open and
 * after every snapshot/rollback so it never shows a stale history. `null` is
 * "not loaded", which the panel renders differently from an empty journal.
 */
export function useTransactions(open: boolean): {
  txs: Transaction[] | null;
  refreshTxs: () => void;
} {
  const [txs, setTxs] = useState<Transaction[] | null>(null);

  const refreshTxs = useCallback(() => {
    getTransactions(50)
      .then((next) => setTxs(next))
      .catch(() => setTxs(null));
  }, []);

  useEffect(() => {
    if (open) refreshTxs();
  }, [open, refreshTxs]);

  return { txs, refreshTxs };
}
