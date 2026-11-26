import { useEffect, useRef, useState } from "react";
import {
  attachDownload,
  finalizeDownload,
  onDownloadCaptured,
  onDownloadFailed,
  onDownloadProgress,
  onDownloadStarted,
  type CapturedDownload,
  type DownloadFailed,
  type DownloadStarted,
  type LoadoutDto,
  type ShellSnapshot,
} from "../../api";
import {
  applyProgress,
  attachMod,
  dropProgress,
  emptyLiveDownloads,
  trackedIds,
  type LiveDownloads,
  type TrackedTransfer,
} from "../../downloadProgress";
import { useTransactions } from "../../hooks/useTransactions";
import { useDownloads } from "../../hooks/useDownloads";
import type { BottomTab } from "../../components/Features/Transactions/BottomTab";
import type { RightPaneState } from "../../components/Features/Inspector/RightPanel";
import type { TabId } from "../../gameChrome";

/** Download events only need SQLite rows — never re-walk archives (Skyrim freeze). */
const DL_REFRESH = { hydrateMetadata: false } as const;

/**
 * After progress hits 100 % without `captured`: short then longer retries.
 * Last attempt gives up (Core → failed + toast FR).
 */
const AFTER_100_MS = [1_500, 4_000, 12_000, 30_000] as const;

/**
 * From download start when Finished never comes and progress may lack a
 * Content-Length (Mouse Tweaks / Modrinth). Last attempt gives up.
 */
const WATCHDOG_MS = [25_000, 55_000, 90_000] as const;

export type UseDockSpaceParams = {
  activeLoadout: LoadoutDto | undefined;
  activeLoadoutIdRef: React.MutableRefObject<number | null>;
  refresh: (id: number | null, opts?: { hydrateMetadata?: boolean }) => Promise<number | null>;
  setSnap: React.Dispatch<React.SetStateAction<ShellSnapshot | null>>;
  setTab: (tab: TabId) => void;
  setToast: (msg: string | null) => void;
  setError: (msg: string | null) => void;
  pane: RightPaneState;
};

export function useDockSpace(params: UseDockSpaceParams) {
  const {
    activeLoadout,
    activeLoadoutIdRef,
    refresh,
    setSnap,
    setTab,
    setToast,
    setError,
    pane,
  } = params;

  const [txOpen, setTxOpen] = useState(false);
  const [bottomTab, setBottomTab] = useState<BottomTab>("transactions");
  const [downloadsRefreshKey, setDownloadsRefreshKey] = useState(0);
  const modByDownload = useRef(new Map<number, TrackedTransfer>());
  const finalizeTimers = useRef(new Map<number, ReturnType<typeof setTimeout>>());
  const watchdogTimers = useRef(new Map<number, ReturnType<typeof setTimeout>>());
  const [liveDownloads, setLiveDownloads] = useState<LiveDownloads>(emptyLiveDownloads);

  const activeLoadoutRef = useRef(activeLoadout);
  activeLoadoutRef.current = activeLoadout;
  const setTabRef = useRef(setTab);
  setTabRef.current = setTab;
  const setToastRef = useRef(setToast);
  setToastRef.current = setToast;
  const setErrorRef = useRef(setError);
  setErrorRef.current = setError;
  const refreshRef = useRef(refresh);
  refreshRef.current = refresh;
  const setSnapRef = useRef(setSnap);
  setSnapRef.current = setSnap;

  const { txs, refreshTxs } = useTransactions(txOpen || pane.tabs.includes("console"));
  const { downloads } = useDownloads(txOpen, downloadsRefreshKey);

  function clearFinalizeTimer(downloadId: number) {
    const timer = finalizeTimers.current.get(downloadId);
    if (timer != null) {
      clearTimeout(timer);
      finalizeTimers.current.delete(downloadId);
    }
  }

  function clearWatchdogTimer(downloadId: number) {
    const timer = watchdogTimers.current.get(downloadId);
    if (timer != null) {
      clearTimeout(timer);
      watchdogTimers.current.delete(downloadId);
    }
  }

  function forgetTransfer(fileName: string, loadoutId: number | null, alsoId?: number) {
    const { downloadIds, modIds } = trackedIds(
      modByDownload.current,
      fileName,
      loadoutId,
      alsoId,
    );
    for (const id of downloadIds) {
      clearFinalizeTimer(id);
      clearWatchdogTimer(id);
      modByDownload.current.delete(id);
    }
    setLiveDownloads((prev) => dropProgress(prev, downloadIds, modIds));
  }

  /** Flip one row to Archive from Core truth — ephemeral UI only, cleared on captured/failed. */
  function markArchivedLocally(modId: number, displayName: string) {
    setSnapRef.current((snap) => {
      if (!snap) return snap;
      let changed = false;
      const entries = snap.entries.map((row) => {
        if (row.modId !== modId || row.status !== "downloading") return row;
        changed = true;
        return { ...row, status: "archived" as const, name: displayName || row.name };
      });
      return changed ? { ...snap, entries } : snap;
    });
  }

  async function runFinalize(
    downloadId: number,
    giveUp: boolean,
  ): Promise<"completed" | "notReady" | "alreadyDone" | "abandoned" | null> {
    if (!modByDownload.current.has(downloadId)) return null;
    try {
      const outcome = await finalizeDownload(downloadId, giveUp);
      if (outcome === "alreadyDone") {
        const tracked = modByDownload.current.get(downloadId);
        if (tracked) {
          markArchivedLocally(tracked.modId, tracked.fileName);
          forgetTransfer(tracked.fileName, tracked.loadoutId, downloadId);
          if (activeLoadoutIdRef.current === tracked.loadoutId) {
            await refreshRef.current(tracked.loadoutId, DL_REFRESH);
          }
        }
      }
      // completed / abandoned emit captured|failed — listeners refresh the table.
      return outcome;
    } catch (e) {
      setToastRef.current(
        e instanceof Error
          ? e.message
          : "Le téléchargement n’a pas pu être finalisé.",
      );
      return null;
    }
  }

  /** Progress can hit 100 % while Finished/captured never arrives (CDN / WebView2). */
  function scheduleFinalize(downloadId: number, attempt = 0) {
    if (finalizeTimers.current.has(downloadId)) return;
    const delay = AFTER_100_MS[Math.min(attempt, AFTER_100_MS.length - 1)] ?? 30_000;
    const giveUp = attempt >= AFTER_100_MS.length - 1;
    const timer = setTimeout(() => {
      finalizeTimers.current.delete(downloadId);
      if (!modByDownload.current.has(downloadId)) return;
      void (async () => {
        const outcome = await runFinalize(downloadId, giveUp);
        if (outcome === "notReady" && attempt < AFTER_100_MS.length - 1) {
          scheduleFinalize(downloadId, attempt + 1);
        }
      })();
    }, delay);
    finalizeTimers.current.set(downloadId, timer);
  }

  /**
   * Long silence / missing Content-Length: still reconcile disk → archived,
   * or fail with toast. Does not remount the browser.
   */
  function scheduleWatchdog(downloadId: number, attempt = 0) {
    if (watchdogTimers.current.has(downloadId)) return;
    const delay = WATCHDOG_MS[Math.min(attempt, WATCHDOG_MS.length - 1)] ?? 90_000;
    const giveUp = attempt >= WATCHDOG_MS.length - 1;
    const timer = setTimeout(() => {
      watchdogTimers.current.delete(downloadId);
      if (!modByDownload.current.has(downloadId)) return;
      void (async () => {
        const outcome = await runFinalize(downloadId, giveUp);
        if (outcome === "notReady" && attempt < WATCHDOG_MS.length - 1) {
          scheduleWatchdog(downloadId, attempt + 1);
        }
      })();
    }, delay);
    watchdogTimers.current.set(downloadId, timer);
  }

  async function showDownloadStart(event: DownloadStarted) {
    setDownloadsRefreshKey((k) => k + 1);
    if (activeLoadoutIdRef.current === event.loadoutId) {
      await refreshRef.current(event.loadoutId, DL_REFRESH);
      if (activeLoadoutIdRef.current === event.loadoutId) setTabRef.current("mods");
    }
    setToastRef.current(`Téléchargement de « ${event.displayName} »…`);
  }

  async function showDownloadFailed(event: DownloadFailed) {
    setDownloadsRefreshKey((k) => k + 1);
    if (event.loadoutId != null && activeLoadoutIdRef.current === event.loadoutId) {
      await refreshRef.current(event.loadoutId, DL_REFRESH);
    }
    const detail = event.message?.trim();
    setToastRef.current(
      detail
        ? `« ${event.fileName} » : ${detail}`
        : `« ${event.fileName} » : téléchargement interrompu.`,
    );
  }

  async function fileCapture(event: CapturedDownload) {
    setDownloadsRefreshKey((k) => k + 1);
    if (event.modId != null) {
      markArchivedLocally(event.modId, event.displayName);
    }
    if (event.alreadyFiled) {
      if (event.loadoutId != null && activeLoadoutIdRef.current === event.loadoutId) {
        await refreshRef.current(event.loadoutId, DL_REFRESH);
        if (activeLoadoutIdRef.current === event.loadoutId) setTabRef.current("mods");
      }
      const current = activeLoadoutRef.current;
      const where =
        current && event.loadoutId === current.id ? ` → ${current.name}` : "";
      setToastRef.current(`« ${event.displayName} »${where} (archive, à installer)`);
      return;
    }
    const current = activeLoadoutRef.current;
    if (!current) {
      setToastRef.current(`« ${event.fileName} » téléchargé — ouvre une instance pour le classer.`);
      return;
    }
    try {
      await attachDownload(event.downloadId, current.id, null);
      await refreshRef.current(current.id, DL_REFRESH);
      if (activeLoadoutIdRef.current === current.id) setTabRef.current("mods");
      setToastRef.current(`« ${event.fileName} » → ${current.name} (archive, à installer)`);
    } catch (e) {
      setErrorRef.current(e instanceof Error ? e.message : String(e));
    }
  }

  // Subscribe once. Re-binding on every activeLoadout identity change dropped
  // `download://captured` and cleared finalize timers mid-transfer (stuck UI).
  // Never remount BrowserPane from these handlers — refresh Mods only.
  useEffect(() => {
    let unlistens: Array<() => void> = [];
    let cancelled = false;
    void Promise.all([
      onDownloadStarted((e) => {
        modByDownload.current.set(e.downloadId, {
          modId: e.modId,
          fileName: e.fileName,
          loadoutId: e.loadoutId,
        });
        setLiveDownloads((prev) => attachMod(prev, e.downloadId, e.modId));
        scheduleWatchdog(e.downloadId);
        void showDownloadStart(e);
      }),
      onDownloadProgress((e) => {
        if (e.downloadId == null) return;
        const downloadId = e.downloadId;
        const modId = modByDownload.current.get(downloadId)?.modId;
        setLiveDownloads((prev) =>
          applyProgress(prev, downloadId, modId, {
            received: e.received,
            total: e.total,
          }),
        );
        if (e.total != null && e.total > 0 && e.received >= e.total) {
          scheduleFinalize(downloadId);
        }
      }),
      onDownloadCaptured((e) => {
        if (e.modId == null) {
          const tracked = modByDownload.current.get(e.downloadId);
          if (tracked) markArchivedLocally(tracked.modId, e.displayName);
        }
        forgetTransfer(e.fileName, e.loadoutId, e.downloadId);
        void fileCapture(e);
      }),
      onDownloadFailed((e) => {
        forgetTransfer(e.fileName, e.loadoutId);
        void showDownloadFailed(e);
      }),
    ]).then((offs) => {
      if (cancelled) offs.forEach((off) => off());
      else unlistens = offs;
    });
    return () => {
      cancelled = true;
      unlistens.forEach((off) => off());
      for (const timer of finalizeTimers.current.values()) clearTimeout(timer);
      finalizeTimers.current.clear();
      for (const timer of watchdogTimers.current.values()) clearTimeout(timer);
      watchdogTimers.current.clear();
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps -- stable listeners via refs
  }, []);

  return {
    txOpen,
    setTxOpen,
    bottomTab,
    setBottomTab,
    downloadsRefreshKey,
    liveDownloads,
    txs,
    refreshTxs,
    downloads,
  };
}
