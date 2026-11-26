import { formatBytes } from "./formatBytes";

/** A transfer `download://started` named, so later byte counts can find the mod. */
export type TrackedTransfer = {
  modId: number;
  fileName: string;
  loadoutId: number;
};

/** Bytes received for one transfer. `total` is null when the server sent no length. */
export type DownloadBytes = {
  received: number;
  total: number | null;
};

export type LiveDownloads = {
  byMod: Map<number, DownloadBytes>;
  byDownload: Map<number, DownloadBytes>;
};

export function emptyLiveDownloads(): LiveDownloads {
  return { byMod: new Map(), byDownload: new Map() };
}

const NBSP = "\u00a0";

/**
 * The figure that follows « Téléchargement » : `42 %`, or the received size
 * (`12,3 Mo`, `840 octets`, …) when the total is unknown.
 */
export function formatDownloadProgress(progress: DownloadBytes): string {
  const { received, total } = progress;
  if (total != null && total > 0) {
    const pct = Math.min(100, Math.floor((received / total) * 100));
    return `${pct}${NBSP}%`;
  }
  return formatBytes(received);
}

function sameBytes(a: DownloadBytes | undefined, b: DownloadBytes): boolean {
  return a != null && a.received === b.received && a.total === b.total;
}

/** Copy an already-seen byte count onto the mod once `download://started` names it. */
export function attachMod(
  live: LiveDownloads,
  downloadId: number,
  modId: number,
): LiveDownloads {
  const bytes = live.byDownload.get(downloadId);
  if (!bytes || sameBytes(live.byMod.get(modId), bytes)) return live;
  const byMod = new Map(live.byMod);
  byMod.set(modId, bytes);
  return { byMod, byDownload: live.byDownload };
}

/** Record one throttled sample. Returns the same object when nothing changed. */
export function applyProgress(
  live: LiveDownloads,
  downloadId: number,
  modId: number | undefined,
  bytes: DownloadBytes,
): LiveDownloads {
  const current = live.byDownload.get(downloadId);
  if (
    sameBytes(current, bytes) &&
    (modId == null || sameBytes(live.byMod.get(modId), bytes))
  ) {
    return live;
  }
  const byDownload = new Map(live.byDownload);
  const byMod = new Map(live.byMod);
  byDownload.set(downloadId, bytes);
  if (modId != null) byMod.set(modId, bytes);
  return { byMod, byDownload };
}

/** Ids to forget when a transfer ends. `also` covers a finish event whose id was renamed. */
export function trackedIds(
  tracked: ReadonlyMap<number, TrackedTransfer>,
  fileName: string,
  loadoutId: number | null,
  also?: number,
): { downloadIds: number[]; modIds: number[] } {
  const downloadIds: number[] = [];
  const modIds: number[] = [];
  const seen = new Set<number>();
  const consider = (id: number) => {
    if (seen.has(id)) return;
    seen.add(id);
    downloadIds.push(id);
    const modId = tracked.get(id)?.modId;
    if (modId != null) modIds.push(modId);
  };
  if (also != null) consider(also);
  for (const [id, row] of tracked) {
    if (row.fileName !== fileName) continue;
    if (loadoutId != null && row.loadoutId !== loadoutId) continue;
    consider(id);
  }
  return { downloadIds, modIds };
}

/** Drop finished transfers. Returns the same object when none of the ids were tracked. */
export function dropProgress(
  live: LiveDownloads,
  downloadIds: readonly number[],
  modIds: readonly number[],
): LiveDownloads {
  const downloads = downloadIds.filter((id) => live.byDownload.has(id));
  const mods = modIds.filter((id) => live.byMod.has(id));
  if (downloads.length === 0 && mods.length === 0) return live;
  const byDownload = new Map(live.byDownload);
  const byMod = new Map(live.byMod);
  for (const id of downloads) byDownload.delete(id);
  for (const id of mods) byMod.delete(id);
  return { byMod, byDownload };
}

/**
 * Browser preview only: the mock snapshot has a `downloading` row and no
 * Tauri events, so the pill can show a fixed 42 %.
 */
export function withPreviewProgress(
  live: LiveDownloads["byMod"],
  rows: readonly { modId: number; status: string }[],
  preview: boolean,
): LiveDownloads["byMod"] {
  if (!preview) return live;
  let next: Map<number, DownloadBytes> | null = null;
  for (const row of rows) {
    if (row.status !== "downloading" || live.has(row.modId)) continue;
    if (!next) next = new Map(live);
    next.set(row.modId, { received: 42_000_000, total: 100_000_000 });
  }
  return next ?? live;
}
