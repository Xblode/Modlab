import { useVirtualizer } from "@tanstack/react-virtual";
import { useEffect, useMemo, useRef, useState, type KeyboardEvent, type MouseEvent, type PointerEvent } from "react";
import { Ico } from "../../../Ico";
import {
  Badge,
  Checkbox,
  ConfirmDialog,
  IconButton,
  PopoverMenu,
  PopoverMenuEntries,
  PopoverMenuItem,
  type PopoverMenuEntry,
} from "../../ui";
import { useRowHeight } from "../../../density";
import {
  modTableCatTrack,
  modTableColumnTemplate,
  useModTableCatContentPx,
  useModTableWidthTier,
} from "../../../modTableWidth";
import type { GameCategory, ModRow, ResolvedDependency } from "../../../api";
import { presentCategory } from "../../../categoryIcon";
import type { ModConflicts } from "../../../api";
import { compareModRows, nextModSort, type ModSort, type ModSortKey } from "../../../modSort";
import { modSortFor, rememberModSort } from "../../../uiMemory";
import { ConflictBadge } from "./ConflictBadge";
import { DependencyBadge } from "./DependencyBadge";
import { formatDownloadProgress, type DownloadBytes } from "../../../downloadProgress";
import { formatBytes, formatExactBytes } from "../../../formatBytes";
import { formatDateTime, formatMonthYear } from "../../../format";
import { modSourceDisplay } from "../../../modSource";

/** Column widths live in `shell.css` (`.virt-grid { --mod-cols* }`, tiers via `data-width-tier`). */

const ROW_MENU_ITEMS: PopoverMenuEntry[] = [
  { id: "chat", label: "Chat", icon: "sparkle" },
  { id: "rename", label: "Renommer", icon: "edit" },
  { id: "folder", label: "Voir le dossier", icon: "folder" },
  { id: "archive", label: "Voir l'archive", icon: "box" },
  { id: "uninstall", label: "Désinstaller (retour à l'archive)", icon: "history" },
  { id: "delete", label: "Supprimer…", icon: "trash", danger: true },
];

/** Distance from the top/bottom edge where a drag starts scrolling the list. */
const EDGE = 36;
/** Pointer travel before a press on the handle counts as a drag, not a click. */
const DRAG_THRESHOLD = 4;
/** Spoken and shown while a display sort is on: drag would rewrite priority from the wrong indexes. */
const REORDER_SORTED = "Revenir à l'ordre de priorité pour réordonner";

/**
 * État kinds, in the order `modStateKind` tests them. The header cycles the
 * kinds that actually occur; it does not keep a second list of labels.
 */
const MOD_STATE_ORDER = [
  "installing",
  "downloading",
  "archived",
  "dependency",
  "conflict",
  "installed",
] as const;

type ModStateKind = (typeof MOD_STATE_ORDER)[number];

const MOD_STATE_LABEL: Record<ModStateKind, string> = {
  installing: "Installation",
  downloading: "Téléchargement",
  archived: "Archive",
  dependency: "Dépendance",
  conflict: "Conflit",
  installed: "Installé",
};

/** Required deps that are disabled or missing. Optional / used-by never badge. */
function dependencyProblems(
  row: ModRow,
  dependencies: Map<number, ResolvedDependency[]> | null,
): ResolvedDependency[] {
  if (!dependencies || !row.enabled) return [];
  return (dependencies.get(row.modId) ?? []).filter((dep) => {
    const kind = dep.kind ?? "required";
    if (kind !== "required") return false;
    return dep.state === "disabled" || dep.state === "missing";
  });
}

/** Nexus / Curse / Modrinth page, or homepage when that is all we have. */
function modWebPage(row: ModRow): string {
  return row.pageUrl?.trim() || row.website?.trim() || "";
}

/** What the État cell draws for this row. Installing wins over the stored status. */
function modStateKind(
  row: ModRow,
  installing: boolean,
  problems: readonly ResolvedDependency[],
  conflicts: ModConflicts | undefined,
): ModStateKind {
  if (installing) return "installing";
  if (row.status === "downloading") return "downloading";
  if (row.status === "archived") return "archived";
  if (problems.length > 0) return "dependency";
  if (conflicts && (conflicts.wins > 0 || conflicts.loses > 0)) return "conflict";
  return "installed";
}

/**
 * Why a drag must not run. Priority ascending is the Core's order, so a
 * screen index is still a real slot. Any other sort, a state filter, or a
 * search is not.
 */
function reorderLockedReason(
  canReorder: boolean,
  sort: ModSort | null,
  stateFilter: ModStateKind | null,
): string | null {
  if (sort && !(sort.key === "priority" && sort.dir === "asc")) return `${REORDER_SORTED}.`;
  if (stateFilter) return "Affiche tous les états pour changer l'ordre des mods.";
  if (!canReorder) return "Efface la recherche pour changer l'ordre des mods.";
  return null;
}

function SortHeader({
  label,
  column,
  sort,
  end,
  colClass,
  onSort,
}: {
  label: string;
  column: ModSortKey;
  sort: ModSort | null;
  end?: boolean;
  colClass?: string;
  onSort: (column: ModSortKey) => void;
}) {
  const active = sort?.key === column ? sort.dir : null;
  return (
    <div
      role="columnheader"
      className={`gh-sortable${colClass ? ` ${colClass}` : ""}${end ? " end" : ""}`}
      aria-sort={active === "asc" ? "ascending" : active === "desc" ? "descending" : "none"}
    >
      <button
        type="button"
        className={`gh-sort${end ? " end" : ""}${active ? " on" : ""}`}
        onClick={() => onSort(column)}
      >
        <span className="gh-label">{label}</span>
        {active ? <Ico name={active === "asc" ? "up" : "down"} className="gh-arrow" /> : null}
      </button>
    </div>
  );
}

function StateHeader({
  filter,
  onCycle,
}: {
  filter: ModStateKind | null;
  onCycle: () => void;
}) {
  const label = filter ? `État : ${MOD_STATE_LABEL[filter]}` : "État";
  return (
    <div role="columnheader" className="gh-sortable col-flags">
      <button
        type="button"
        className={`gh-sort${filter ? " on" : ""}`}
        data-tip="Cliquer pour filtrer par état"
        aria-label={
          filter ? `${label}. Cliquer pour filtrer par état` : "État. Cliquer pour filtrer par état"
        }
        onClick={onCycle}
      >
        <span className="gh-label">{label}</span>
      </button>
    </div>
  );
}

type Drag = { modId: number; from: number; slot: number };

type Props = {
  rows: ModRow[];
  selectedId: number | null;
  onSelect: (id: number) => void;
  onToggle: (row: ModRow) => void;
  /** Move a mod to index `to` of the list (a later row wins conflicts). Resolves once the Core has answered. */
  onReorder: (row: ModRow, to: number) => Promise<void>;
  /** False while a search hides rows: an index in a partial list is not a place in the real order. */
  canReorder: boolean;
  /** Active game. The display sort is remembered for this id. */
  gameId: string | null;
  /** An archived row has no usable checkbox — its box is replaced by an
   * "Installer" affordance that unpacks the archive first. */
  onInstallArchived: (row: ModRow) => void;
  /** Mods whose install is in flight. The button stays disabled. */
  installingIds: ReadonlySet<number>;
  /** Live byte counts for rows in `downloading`, keyed by mod id. */
  downloadProgress?: ReadonlyMap<number, DownloadBytes>;
  /** This game's categories, from the adapter. */
  catalog: GameCategory[];
  /** Other instances of the active game — an inactive or archived row can
   * be moved here. Empty when there is nowhere else to send it. */
  otherLoadouts: { id: number; name: string }[];
  onReassign: (row: ModRow, toLoadoutId: number) => void;
  /**
   * Per-mod conflict tallies, keyed by mod id. `null` means the loadout has
   * not been scanned yet; a mod missing from the map means the scan covered it
   * and found it in no conflict (which is why the two are kept apart).
   */
  conflicts: Map<number, ModConflicts> | null;
  /**
   * Per-mod dependencies from the game adapter. `null` means not scanned
   * yet. A mod missing from the map was scanned and has nothing to report.
   */
  dependencies: Map<number, ResolvedDependency[]> | null;
  onRename: (row: ModRow, name: string) => void;
  onDelete: (row: ModRow) => void;
  onUninstall: (row: ModRow) => void;
  onRevealFolder: (row: ModRow) => void;
  onRevealArchive: (row: ModRow) => void;
  /** Open the mod's Nexus/Curse/Modrinth/homepage URL in the SideDrawer browser. */
  onOpenPage: (url: string) => void;
  /** Right-click / ⋯ → Chat: open the SideDrawer Chat tab and tag this mod. */
  onChat: (row: ModRow) => void;
};

export function ModTable({
  rows,
  selectedId,
  onSelect,
  onToggle,
  onReorder,
  canReorder,
  gameId,
  onInstallArchived,
  installingIds,
  downloadProgress,
  catalog,
  otherLoadouts,
  onReassign,
  conflicts,
  dependencies,
  onRename,
  onDelete,
  onUninstall,
  onRevealFolder,
  onRevealArchive,
  onOpenPage,
  onChat,
}: Props) {
  const parentRef = useRef<HTMLDivElement>(null);
  const widthTier = useModTableWidthTier(parentRef);
  const columns = modTableColumnTemplate(widthTier);
  const showTailCols = widthTier < 2;
  /** Minecraft shows Load.; Skyrim (and others) hide it — no empty track. */
  const showLoaderCol = showLoaderColumn(gameId);
  const catMeasureItems = useMemo(() => {
    const seen = new Map<string, { icon: string; label: string }>();
    for (const item of catalog) {
      seen.set(`${item.icon}\0${item.label}`, { icon: item.icon, label: item.label });
    }
    for (const row of rows) {
      const presented = presentCategory(row.category, catalog);
      if (!presented.icon) continue;
      seen.set(`${presented.icon}\0${presented.label}`, {
        icon: presented.icon,
        label: presented.label,
      });
    }
    return Array.from(seen.values());
  }, [catalog, rows]);
  const catMeasureKey = useMemo(
    () => catMeasureItems.map((item) => `${item.icon}:${item.label}`).join("|"),
    [catMeasureItems],
  );
  const { measureRef: catMeasureRef, contentPx: catContentPx } =
    useModTableCatContentPx(catMeasureKey);
  const catTrack = modTableCatTrack(widthTier, catContentPx);
  // Display order only. Priority (`position`) stays whatever the Core stored.
  const [sort, setSort] = useState<ModSort | null>(() => modSortFor(gameId));
  // Session only: a state filter is not written next to the remembered sort.
  const [stateFilter, setStateFilter] = useState<ModStateKind | null>(null);
  useEffect(() => {
    const next = modSortFor(gameId);
    setSort((current) =>
      current?.key === next?.key && current?.dir === next?.dir ? current : next,
    );
    setStateFilter(null);
  }, [gameId]);
  const presentStates = useMemo(() => {
    const seen = new Set<ModStateKind>();
    for (const row of rows) {
      seen.add(
        modStateKind(
          row,
          installingIds.has(row.modId),
          dependencyProblems(row, dependencies),
          conflicts?.get(row.modId),
        ),
      );
    }
    return MOD_STATE_ORDER.filter((kind) => seen.has(kind));
  }, [rows, installingIds, dependencies, conflicts]);
  const filtered = useMemo(() => {
    if (!stateFilter) return rows;
    return rows.filter(
      (row) =>
        modStateKind(
          row,
          installingIds.has(row.modId),
          dependencyProblems(row, dependencies),
          conflicts?.get(row.modId),
        ) === stateFilter,
    );
  }, [rows, stateFilter, installingIds, dependencies, conflicts]);
  const shown = useMemo(() => {
    if (!sort) return filtered;
    return filtered.slice().sort((a, b) => compareModRows(a, b, sort, catalog));
  }, [filtered, sort, catalog]);
  const lockedReason = reorderLockedReason(canReorder, sort, stateFilter);
  const reorderable = lockedReason == null;

  function onSortColumn(key: ModSortKey) {
    const next = nextModSort(sort, key);
    setSort(next);
    rememberModSort(gameId, next);
    parentRef.current?.scrollTo({ top: 0 });
  }

  function onStateColumn() {
    const cycle: (ModStateKind | null)[] = [null, ...presentStates];
    const at = cycle.indexOf(stateFilter);
    const next = cycle[at < 0 ? 0 : (at + 1) % cycle.length];
    setStateFilter(next);
    parentRef.current?.scrollTo({ top: 0 });
  }

  // Row and header height: the virtualizer's fixed size, so a pointer position maps to a slot with plain arithmetic (no DOM lookup, most rows are not mounted). It follows the density setting.
  const ROW_H = useRowHeight();
  const virtualizer = useVirtualizer({
    count: shown.length,
    getScrollElement: () => parentRef.current,
    estimateSize: () => ROW_H,
    overscan: 12,
  });
  // Cached sizes are stale once the density changes the row height.
  useEffect(() => {
    virtualizer.measure();
  }, [ROW_H]); // eslint-disable-line react-hooks/exhaustive-deps
  useEffect(() => {
    if (selectedId == null) return;
    const index = shown.findIndex((row) => row.modId === selectedId);
    if (index >= 0) virtualizer.scrollToIndex(index, { align: "auto" });
  }, [selectedId]); // eslint-disable-line react-hooks/exhaustive-deps

  const [reassignRow, setReassignRow] = useState<number | null>(null);
  const reassignRefs = useRef(new Map<number, HTMLButtonElement | null>());
  const moreRefs = useRef(new Map<number, HTMLButtonElement | null>());

  const [menuTarget, setMenuTarget] = useState<
    | { modId: number; point: { x: number; y: number }; anchor?: undefined }
    | { modId: number; anchor: HTMLButtonElement; point?: undefined }
    | null
  >(null);
  const [renamingId, setRenamingId] = useState<number | null>(null);
  const [nameDraft, setNameDraft] = useState("");
  const [confirmDelete, setConfirmDelete] = useState<ModRow | null>(null);

  // ---- Reordering: pointer drag on the grip, or Alt+Up/Down/Home/End on a row ----
  const [drag, setDrag] = useState<Drag | null>(null);
  const [announcement, setAnnouncement] = useState("");
  const busy = useRef(false);
  const focusAfter = useRef<number | null>(null);
  const pointerY = useRef(0);
  const rowsRef = useRef(shown);
  rowsRef.current = shown;

  /** Run one move and tell assistive tech where the mod ended up. */
  async function commitMove(m: ModRow, to: number) {
    const n = rowsRef.current.length;
    const target = Math.max(0, Math.min(to, n - 1));
    const from = rowsRef.current.findIndex((r) => r.modId === m.modId);
    if (busy.current || from < 0 || target === from) return;
    busy.current = true;
    focusAfter.current = m.modId;
    try {
      await onReorder(m, target);
      setAnnouncement(`« ${m.name} » déplacé en position ${target + 1} sur ${n}.`);
    } finally {
      busy.current = false;
    }
  }

  // After a keyboard move the row may have left the rendered window: bring it
  // back into view and give it focus again so the next keypress keeps working.
  useEffect(() => {
    const id = focusAfter.current;
    if (id === null) return;
    const idx = shown.findIndex((r) => r.modId === id);
    if (idx < 0) return;
    focusAfter.current = null;
    virtualizer.scrollToIndex(idx, { align: "auto" });
    requestAnimationFrame(() => {
      parentRef.current?.querySelector<HTMLElement>(`[data-mod-id="${id}"]`)?.focus();
    });
  }, [shown, virtualizer]);

  function slotAt(clientY: number): number {
    const el = parentRef.current;
    if (!el) return 0;
    const y = clientY - el.getBoundingClientRect().top + el.scrollTop - ROW_H;
    return Math.max(0, Math.min(rowsRef.current.length, Math.round(y / ROW_H)));
  }

  function beginDrag(e: PointerEvent, m: ModRow, from: number) {
    if (!reorderable || busy.current || e.button !== 0) return;
    e.preventDefault();
    e.stopPropagation();
    const startY = e.clientY;
    pointerY.current = startY;
    let active = false;
    let scrollTimer = 0;

    const move = (ev: globalThis.PointerEvent) => {
      pointerY.current = ev.clientY;
      if (!active && Math.abs(ev.clientY - startY) < DRAG_THRESHOLD) return;
      if (!active) {
        active = true;
        // Keep the grabbed row selected, as a plain click would.
        onSelect(m.modId);
        // The list scrolls itself while the pointer sits near an edge.
        scrollTimer = window.setInterval(() => {
          const el = parentRef.current;
          if (!el) return;
          const r = el.getBoundingClientRect();
          const y = pointerY.current;
          if (y < r.top + ROW_H + EDGE) el.scrollTop -= 14;
          else if (y > r.bottom - EDGE) el.scrollTop += 14;
          else return;
          setDrag((d) => (d ? { ...d, slot: slotAt(pointerY.current) } : d));
        }, 16);
      }
      setDrag({ modId: m.modId, from, slot: slotAt(ev.clientY) });
    };
    const finish = (commit: boolean) => {
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", up);
      window.removeEventListener("pointercancel", cancel);
      window.removeEventListener("keydown", key, true);
      window.clearInterval(scrollTimer);
      if (!active) return;
      const slot = slotAt(pointerY.current);
      setDrag(null);
      // `slot` counts gaps in the current list; the target is an index in the final one.
      if (commit) void commitMove(m, slot > from ? slot - 1 : slot);
    };
    const up = () => finish(true);
    const cancel = () => finish(false);
    const key = (ev: globalThis.KeyboardEvent) => {
      if (ev.key === "Escape") {
        ev.stopPropagation();
        finish(false);
      }
    };
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", up);
    window.addEventListener("pointercancel", cancel);
    window.addEventListener("keydown", key, true);
  }

  function onRowKeyDown(e: KeyboardEvent, m: ModRow, index: number) {
    if (!e.altKey || e.ctrlKey || e.metaKey || e.target !== e.currentTarget) return;
    let to: number | null = null;
    if (e.key === "ArrowUp") to = index - 1;
    else if (e.key === "ArrowDown") to = index + 1;
    else if (e.key === "Home") to = 0;
    else if (e.key === "End") to = rowsRef.current.length - 1;
    if (to === null) return;
    e.preventDefault();
    if (!reorderable) {
      if (lockedReason) setAnnouncement(lockedReason);
      return;
    }
    void commitMove(m, to);
  }

  function startRename(m: ModRow) {
    setNameDraft(m.name);
    setRenamingId(m.modId);
  }

  function commitRename(m: ModRow) {
    setRenamingId(null);
    const trimmed = nameDraft.trim();
    if (trimmed && trimmed !== m.name) onRename(m, trimmed);
  }

  function rowMenuItems(m: ModRow): PopoverMenuEntry[] {
    const installed = m.status === "installed";
    const downloading = m.status === "downloading";
    return ROW_MENU_ITEMS.filter((it) => {
      if (it.id === "folder") return installed;
      if (it.id === "archive") return m.archivePath != null && !downloading;
      if (it.id === "uninstall") return installed && m.archivePath != null;
      return true;
    });
  }

  function handleMenuSelect(item: PopoverMenuEntry, modId: number) {
    const m = rows.find((r) => r.modId === modId);
    if (!m) return;
    if (item.id === "chat") onChat(m);
    else if (item.id === "rename") startRename(m);
    else if (item.id === "folder") onRevealFolder(m);
    else if (item.id === "archive") onRevealArchive(m);
    else if (item.id === "uninstall") onUninstall(m);
    else if (item.id === "delete") setConfirmDelete(m);
  }

  return (
    <div className="content" ref={parentRef} style={{ overflow: "auto" }}>
      <div className="sr-only" role="status" aria-live="polite">
        {announcement}
      </div>
      <div
        className="grid virt-grid"
        data-width-tier={widthTier}
        {...(showLoaderCol ? { "data-loader-col": "1" } : {})}
        style={{
          height: virtualizer.getTotalSize(),
          position: "relative",
          display: "block",
          ["--mod-col-cat" as string]: catTrack,
        }}
      >
        <div className="mod-cat-measure" ref={catMeasureRef} aria-hidden="true">
          <span className="cat-face cat-face-measure">
            <span className="lbl">Cat.</span>
          </span>
          {catMeasureItems.map((item) => (
            <span key={`${item.icon}:${item.label}`} className="cat-face cat-face-measure">
              <Badge variant="ghost">
                <Ico name={item.icon} />
              </Badge>
              <span className="lbl">{item.label}</span>
            </span>
          ))}
        </div>
        <div
          className="gh"
          style={{
            display: "grid",
            gridTemplateColumns: columns,
            position: "sticky",
            top: 0,
            zIndex: 2,
            background: "var(--c2)",
          }}
        >
          <div />
          <div />
          <SortHeader label="Pri." column="priority" sort={sort} end onSort={onSortColumn} />
          <SortHeader label="Mod" column="name" sort={sort} onSort={onSortColumn} />
          <StateHeader filter={stateFilter} onCycle={onStateColumn} />
          <SortHeader
            label="Cat."
            column="category"
            sort={sort}
            colClass="col-cat"
            onSort={onSortColumn}
          />
          <div className="col-ver">Version</div>
          <div className="col-game">Jeux</div>
          {showLoaderCol ? <div className="col-loader">Load.</div> : null}
          {showTailCols ? (
            <>
              <SortHeader
                label="Taille"
                column="size"
                sort={sort}
                end
                colClass="col-sz"
                onSort={onSortColumn}
              />
              <SortHeader
                label="Date"
                column="date"
                sort={sort}
                end
                colClass="col-when"
                onSort={onSortColumn}
              />
              <div className="col-src">Source</div>
            </>
          ) : null}
        </div>
        {drag ? (
          <div
            className="drop-line"
            aria-hidden="true"
            style={{ transform: `translateY(${ROW_H + drag.slot * ROW_H}px)` }}
          />
        ) : null}
        {virtualizer.getVirtualItems().map((v) => {
          const m = shown[v.index];
          // `position` is the Core's 0-based priority. The column keeps that
          // rank (1-based), never the row's place on screen after a sort.
          const priority = m.position + 1;
          const pri = String(priority).padStart(3, "0");
          const selected = selectedId === m.modId;
          const installing = installingIds.has(m.modId);
          const live =
            m.status === "downloading" ? downloadProgress?.get(m.modId) : undefined;
          const downloadLabel = live
            ? `Téléchargement ${formatDownloadProgress(live)}`
            : "Téléchargement…";
          const problems = dependencyProblems(m, dependencies);
          const kind = modStateKind(m, installing, problems, conflicts?.get(m.modId));
          const game = gameVersionCell(m);
          const loader = showLoaderCol ? loaderLetterCell(m) : null;
          const src = modSourceDisplay(m);
          const category = presentCategory(m.category, catalog);
          const categoryTitle = m.category
            ? m.categoryUserSet
              ? `${category.label} — choisie à la main`
              : category.label
            : undefined;
          const webPage = modWebPage(m);
          // Inactive or archived: offer move when another instance exists.
          const canMoveToInstance =
            (m.status === "archived" || !m.enabled) &&
            m.status !== "downloading" &&
            !installing &&
            otherLoadouts.length > 0;
          return (
            <div
              key={m.modId}
              role="row"
              tabIndex={0}
              data-mod-id={m.modId}
              aria-label={`${m.name}, priorité ${priority}`}
              className={`row ${selected ? "sel" : ""} ${m.enabled ? "" : "off"} ${drag?.modId === m.modId ? "dragging" : ""}`}
              style={{
                position: "absolute",
                top: 0,
                left: 0,
                width: "100%",
                transform: `translateY(${v.start + ROW_H}px)`,
                display: "grid",
                gridTemplateColumns: columns,
              }}
              onClick={() => onSelect(m.modId)}
              onKeyDown={(e) => onRowKeyDown(e, m, v.index)}
              onContextMenu={(e: MouseEvent) => {
                e.preventDefault();
                setMenuTarget({ modId: m.modId, point: { x: e.clientX, y: e.clientY } });
              }}
            >
              <div
                className={`grip ${reorderable ? "" : "locked"}`}
                {...(reorderable
                  ? { "data-tip": "Glisser pour changer la priorité", "data-tip-key": "Alt ↑ ↓" }
                  : { "data-tip": lockedReason ?? REORDER_SORTED })}
                onPointerDown={(e) => beginDrag(e, m, v.index)}
                onClick={(e) => e.stopPropagation()}
              >
                <Ico name="grip" className="sm" />
              </div>
              <div>
                {m.status === "downloading" ? (
                  <span title="Téléchargement en cours">
                    <Ico name="download" className="sm" />
                  </span>
                ) : m.status === "archived" ? (
                  // Never a checkbox here: nothing is extracted yet, so
                  // ticking it could not deploy a single file.
                  <IconButton
                    name="box"
                    title={installing ? "Installation en cours" : "Installer l’archive"}
                    disabled={installing}
                    onClick={(e) => {
                      e.stopPropagation();
                      if (!installing) onInstallArchived(m);
                    }}
                  />
                ) : (
                  <Checkbox
                    variant="mod"
                    checked={m.enabled}
                    onToggle={() => onToggle(m)}
                    stopPropagation
                    // The row's own name — the checked state is announced by the input itself.
                    aria-label={m.name}
                  />
                )}
              </div>
              <div className="pri num">{pri}</div>
              <div className="nm">
                {renamingId === m.modId ? (
                  <input
                    className="cat-input"
                    autoFocus
                    maxLength={100}
                    value={nameDraft}
                    onClick={(e) => e.stopPropagation()}
                    onChange={(e) => setNameDraft(e.target.value)}
                    onFocus={(e) => e.currentTarget.select()}
                    onKeyDown={(e) => {
                      e.stopPropagation();
                      if (e.key === "Enter") commitRename(m);
                      if (e.key === "Escape") setRenamingId(null);
                    }}
                    onBlur={() => commitRename(m)}
                  />
                ) : (
                  <span className="t">{m.name}</span>
                )}
                <span className="tools">
                  <IconButton
                    name="folder"
                    title="Ouvrir le dossier"
                    onClick={(e) => {
                      e.stopPropagation();
                      onRevealFolder(m);
                    }}
                  />
                  {canMoveToInstance ? (
                    <IconButton
                      name="share"
                      title="Déplacer vers une autre instance"
                      ref={(el) => {
                        reassignRefs.current.set(m.modId, el);
                      }}
                      onClick={(e) => {
                        e.stopPropagation();
                        setReassignRow(m.modId);
                      }}
                    />
                  ) : null}
                  <IconButton
                    name="globe"
                    title={webPage ? "Ouvrir sur le Web" : "Pas de page Web"}
                    disabled={!webPage}
                    onClick={(e) => {
                      e.stopPropagation();
                      if (webPage) onOpenPage(webPage);
                    }}
                  />
                  <IconButton
                    name="sparkle"
                    title="Demander à Chat"
                    style={{ color: "var(--claude)" }}
                  />
                  <IconButton
                    name="more"
                    title="Plus d’actions"
                    ref={(el) => {
                      moreRefs.current.set(m.modId, el);
                    }}
                    onClick={(e) => {
                      e.stopPropagation();
                      const anchor = moreRefs.current.get(m.modId);
                      if (anchor) setMenuTarget({ modId: m.modId, anchor });
                    }}
                  />
                </span>
              </div>
              <div className="flags">
                {kind === "installing" ? (
                  <Badge
                    tone="downloading"
                    data-tip="Installation en cours"
                    aria-label="Installation en cours"
                  >
                    <Ico name="box" />
                  </Badge>
                ) : kind === "downloading" ? (
                  <Badge
                    tone="downloading"
                    data-tip={live ? downloadLabel : "Le fichier n’est pas encore arrivé"}
                    aria-label={live ? downloadLabel : "Le fichier n’est pas encore arrivé"}
                  >
                    <Ico name="download" />
                  </Badge>
                ) : kind === "archived" ? (
                  <Badge
                    tone="archived"
                    data-tip={m.archivePath ?? "Archive"}
                    aria-label={m.archivePath ?? "Archive"}
                  >
                    <Ico name="box" />
                  </Badge>
                ) : kind === "dependency" ? (
                  <DependencyBadge deps={problems} modId={m.modId} onOpen={onSelect} />
                ) : kind === "conflict" ? (
                  <ConflictBadge conflicts={conflicts?.get(m.modId)} />
                ) : null}
              </div>
              <div className={`cat ${m.category ? "" : "muted"}`}>
                <span className="cat-face">
                  {category.icon ? (
                    <Badge
                      variant="ghost"
                      data-tip={categoryTitle}
                      aria-label={categoryTitle}
                    >
                      <Ico name={category.icon} />
                    </Badge>
                  ) : null}
                  <span className="lbl">{category.label}</span>
                </span>
              </div>
              {/* Real value from the Core when the mod's source declared one
                  (a Minecraft jar manifest does, an MO2 import doesn't) —
                  a dash otherwise, never an invented number. */}
              <div className="ver" title={m.version ?? "Version non déclarée par la source"}>
                <span className="lbl">{m.version ?? "—"}</span>
              </div>
              <div className="game" title={game.title}>
                <span className="lbl">{game.text}</span>
              </div>
              {loader ? (
                <div className="loader" title={loader.title}>
                  <span className="lbl">{loader.text}</span>
                </div>
              ) : null}
              {showTailCols ? (
                <>
                  <div className="sz num" title={sizeTitle(m.sizeBytes)}>
                    {m.sizeBytes == null ? "—" : formatBytes(m.sizeBytes)}
                  </div>
                  <div className="when num" title={downloadedTitle(m)}>
                    {downloadedMonth(m)}
                  </div>
                  <div className="src" title={src.title}>
                    <Ico name={src.icon} className="sm" />
                    {src.label}
                  </div>
                </>
              ) : null}
            </div>
          );
        })}
      </div>
      {reassignRow !== null ? (
        <PopoverMenu
          chrome="context"
          title="Déplacer vers…"
          anchorRef={{ current: reassignRefs.current.get(reassignRow) ?? null }}
          onClose={() => setReassignRow(null)}
        >
          {otherLoadouts.map((l) => (
            <PopoverMenuItem
              key={l.id}
              icon="branch"
              label={l.name}
              onPick={() => {
                const row = rows.find((r) => r.modId === reassignRow);
                if (row) onReassign(row, l.id);
                setReassignRow(null);
              }}
            />
          ))}
        </PopoverMenu>
      ) : null}
      {menuTarget && rows.some((r) => r.modId === menuTarget.modId) ? (
        <PopoverMenu
          chrome="context"
          anchorPoint={menuTarget.point}
          anchorRef={menuTarget.anchor ? { current: menuTarget.anchor } : undefined}
          onClose={() => setMenuTarget(null)}
        >
          <PopoverMenuEntries
            items={rowMenuItems(rows.find((r) => r.modId === menuTarget.modId)!)}
            onSelect={(item) => {
              handleMenuSelect(item, menuTarget.modId);
              setMenuTarget(null);
            }}
          />
        </PopoverMenu>
      ) : null}
      {confirmDelete ? (
        <ConfirmDialog
          title="Supprimer le mod"
          body={`« ${confirmDelete.name} » sera retiré de la liste. Ses fichiers et son archive restent sur le disque — il n'y a juste pas encore d'écran pour le restaurer.`}
          confirmLabel="Supprimer"
          danger
          onConfirm={() => onDelete(confirmDelete)}
          onClose={() => setConfirmDelete(null)}
        />
      ) : null}
    </div>
  );
}

function sizeTitle(bytes: number | null): string {
  return bytes == null ? "Taille inconnue" : formatExactBytes(bytes);
}

function downloadedMonth(row: ModRow): string {
  if (!row.downloadedAt) return "—";
  return formatMonthYear(row.downloadedAt) ?? "—";
}

function downloadedTitle(row: ModRow): string {
  if (!row.downloadedAt) return "Date de téléchargement inconnue";
  const full = formatDateTime(row.downloadedAt);
  return full ? `Téléchargé le ${full}` : "Date de téléchargement inconnue";
}

const LOADER_LABELS: Record<string, string> = {
  fabric: "Fabric",
  forge: "Forge",
  neoforge: "NeoForge",
  quilt: "Quilt",
};

/** Single-letter (or short) cell codes for the Load. column. */
const LOADER_LETTERS: Record<string, string> = {
  forge: "F",
  neoforge: "N",
  fabric: "Fb",
  quilt: "Q",
};

/** Minecraft shows Load.; Skyrim* and others keep it off until a real loader exists. */
function showLoaderColumn(gameId: string | null): boolean {
  return gameId === "minecraft";
}

function declaredList(values: readonly string[] | null | undefined): string[] {
  if (!values) return [];
  const out: string[] = [];
  for (const value of values) {
    if (typeof value !== "string") continue;
    const trimmed = value.trim();
    if (trimmed && !out.some((item) => item.toLowerCase() === trimmed.toLowerCase())) {
      out.push(trimmed);
    }
  }
  return out;
}

function loaderName(id: string): string {
  const known = LOADER_LABELS[id.toLowerCase()];
  if (known) return known;
  return id.charAt(0).toUpperCase() + id.slice(1);
}

function loaderLetter(id: string): string {
  const known = LOADER_LETTERS[id.toLowerCase()];
  if (known) return known;
  return id.charAt(0).toUpperCase();
}

/** Natural order, then a single version or a min–max span. */
function versionSpan(versions: readonly string[]): string | null {
  if (versions.length === 0) return null;
  const sorted = versions.slice().sort((a, b) =>
    a.localeCompare(b, "en", { numeric: true, sensitivity: "base" }),
  );
  const min = sorted[0];
  const max = sorted[sorted.length - 1];
  return min === max ? min : `${min}–${max}`;
}

/** Jeux cell: game versions only (no loader names). Tip may still list loaders. */
function gameVersionCell(row: ModRow): { text: string; title: string } {
  const versions = declaredList(row.gameVersions);
  const loaders = declaredList(row.loaders).map(loaderName);
  const parts: string[] = [];
  if (versions.length > 0) parts.push(versions.join(", "));
  if (loaders.length > 0) parts.push(loaders.join(", "));
  const title = parts.length > 0 ? parts.join(" · ") : "Aucune version de jeu déclarée";
  return { text: versionSpan(versions) ?? "—", title };
}

/** Load. cell: letter code(s). Tip = full loader name(s). */
function loaderLetterCell(row: ModRow): { text: string; title: string } {
  const loaders = declaredList(row.loaders);
  if (loaders.length === 0) {
    return { text: "—", title: "Aucun loader déclaré" };
  }
  const letters = loaders.map(loaderLetter);
  const names = loaders.map(loaderName);
  return { text: letters.join(""), title: names.join(", ") };
}
