import { useVirtualizer } from "@tanstack/react-virtual";
import { useEffect, useRef, useState, type KeyboardEvent, type MouseEvent, type PointerEvent } from "react";
import { Ico } from "../../../Ico";
import { Pop, type PopItem } from "../../../Pop";
import { Checkbox, ConfirmDialog, IconButton } from "../../ui";
import { MenuPicker, type MenuItem } from "../MenuPicker/MenuPicker";
import { categoryIcon } from "../../../categoryIcon";
import { useRowHeight } from "../../../density";
import type { ModRow } from "../../../api";
import type { ModConflicts } from "../../../api";
import { ConflictBadge } from "./ConflictBadge";

/** Exploits the shell's own minimum width (`tauri.conf.json`'s 1024px)
 * rather than an arbitrary number: below that, horizontal scroll (already
 * on the container) is the fallback, not a broken layout. `Mod` and
 * `Catégorie` are the two columns that actually benefit from extra room —
 * everything else is icon- or badge-sized and stays put. */
const COLUMNS = "20px 28px 40px minmax(220px,1fr) 130px 84px minmax(100px,170px) 90px";

const ROW_MENU_ITEMS: MenuItem[] = [
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
  /** An archived row has no usable checkbox — its box is replaced by an
   * "Installer" affordance that unpacks the archive first. */
  onInstallArchived: (row: ModRow) => void;
  /** Every tag already in use, for the category field's autocomplete. */
  categories: string[];
  onSetCategory: (row: ModRow, category: string | null) => void;
  /** Other instances of the active game — an archived row can be moved to
   * one of these instead, since nothing has been extracted yet. Empty when
   * there is nowhere else to send it. */
  otherLoadouts: { id: number; name: string }[];
  onReassign: (row: ModRow, toLoadoutId: number) => void;
  /**
   * Per-mod conflict tallies, keyed by mod id. `null` means the loadout has
   * not been scanned yet; a mod missing from the map means the scan covered it
   * and found it in no conflict (which is why the two are kept apart).
   */
  conflicts: Map<number, ModConflicts> | null;
  onRename: (row: ModRow, name: string) => void;
  onDelete: (row: ModRow) => void;
  onUninstall: (row: ModRow) => void;
  onRevealFolder: (row: ModRow) => void;
  onRevealArchive: (row: ModRow) => void;
};

export function ModTable({
  rows,
  selectedId,
  onSelect,
  onToggle,
  onReorder,
  canReorder,
  onInstallArchived,
  categories,
  onSetCategory,
  otherLoadouts,
  onReassign,
  conflicts,
  onRename,
  onDelete,
  onUninstall,
  onRevealFolder,
  onRevealArchive,
}: Props) {
  const parentRef = useRef<HTMLDivElement>(null);
  // Row and header height: the virtualizer's fixed size, so a pointer position maps to a slot with plain arithmetic (no DOM lookup, most rows are not mounted). It follows the density setting.
  const ROW_H = useRowHeight();
  const virtualizer = useVirtualizer({
    count: rows.length,
    getScrollElement: () => parentRef.current,
    estimateSize: () => ROW_H,
    overscan: 12,
  });
  // Cached sizes are stale once the density changes the row height.
  useEffect(() => {
    virtualizer.measure();
  }, [ROW_H]); // eslint-disable-line react-hooks/exhaustive-deps

  const [editingCategory, setEditingCategory] = useState<number | null>(null);
  const [categoryDraft, setCategoryDraft] = useState("");
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
  const rowsRef = useRef(rows);
  rowsRef.current = rows;

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
    const idx = rows.findIndex((r) => r.modId === id);
    if (idx < 0) return;
    focusAfter.current = null;
    virtualizer.scrollToIndex(idx, { align: "auto" });
    requestAnimationFrame(() => {
      parentRef.current?.querySelector<HTMLElement>(`[data-mod-id="${id}"]`)?.focus();
    });
  }, [rows, virtualizer]);

  function slotAt(clientY: number): number {
    const el = parentRef.current;
    if (!el) return 0;
    const y = clientY - el.getBoundingClientRect().top + el.scrollTop - ROW_H;
    return Math.max(0, Math.min(rowsRef.current.length, Math.round(y / ROW_H)));
  }

  function beginDrag(e: PointerEvent, m: ModRow, from: number) {
    if (!canReorder || busy.current || e.button !== 0) return;
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
    else if (e.key === "End") to = rows.length - 1;
    if (to === null) return;
    e.preventDefault();
    if (!canReorder) {
      setAnnouncement("Efface la recherche pour changer l'ordre des mods.");
      return;
    }
    void commitMove(m, to);
  }

  function startEditCategory(m: ModRow) {
    setCategoryDraft(m.category ?? "");
    setEditingCategory(m.modId);
  }

  function commitCategory(m: ModRow) {
    setEditingCategory(null);
    const trimmed = categoryDraft.trim();
    const next = trimmed || null;
    if (next !== m.category) onSetCategory(m, next);
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

  function rowMenuItems(m: ModRow): MenuItem[] {
    const installed = m.status !== "archived";
    return ROW_MENU_ITEMS.filter((it) => {
      if (it.id === "folder") return installed;
      if (it.id === "archive") return m.archivePath != null;
      if (it.id === "uninstall") return installed && m.archivePath != null;
      return true;
    });
  }

  function handleMenuSelect(item: MenuItem, modId: number) {
    const m = rows.find((r) => r.modId === modId);
    if (!m) return;
    if (item.id === "rename") startRename(m);
    else if (item.id === "folder") onRevealFolder(m);
    else if (item.id === "archive") onRevealArchive(m);
    else if (item.id === "uninstall") onUninstall(m);
    else if (item.id === "delete") setConfirmDelete(m);
  }

  const reassignItems: PopItem[] = otherLoadouts.map((l) => ({
    label: l.name,
    icon: "branch",
    onPick: () => {
      const row = rows.find((r) => r.modId === reassignRow);
      if (row) onReassign(row, l.id);
    },
  }));

  return (
    <div className="content" ref={parentRef} style={{ overflow: "auto" }}>
      <div className="sr-only" role="status" aria-live="polite">
        {announcement}
      </div>
      <datalist id="mod-categories">
        {categories.map((c) => (
          <option key={c} value={c} />
        ))}
      </datalist>
      <div
        className="grid virt-grid"
        style={{
          height: virtualizer.getTotalSize(),
          position: "relative",
          display: "block",
          minWidth: 760,
        }}
      >
        <div
          className="gh"
          style={{
            display: "grid",
            gridTemplateColumns: COLUMNS,
            position: "sticky",
            top: 0,
            zIndex: 2,
            background: "var(--c2)",
          }}
        >
          <div />
          <div />
          <div style={{ justifyContent: "flex-end" }}>Pri.</div>
          <div>Mod</div>
          <div>État</div>
          <div>Version</div>
          <div>Catégorie</div>
          <div>Source</div>
        </div>
        {drag ? (
          <div
            className="drop-line"
            aria-hidden="true"
            style={{ transform: `translateY(${ROW_H + drag.slot * ROW_H}px)` }}
          />
        ) : null}
        {virtualizer.getVirtualItems().map((v) => {
          const m = rows[v.index];
          const pri = String(v.index + 1).padStart(3, "0");
          const selected = selectedId === m.modId;
          return (
            <div
              key={m.modId}
              role="row"
              tabIndex={0}
              data-mod-id={m.modId}
              aria-label={`${m.name}, position ${v.index + 1} sur ${rows.length}`}
              className={`row ${selected ? "sel" : ""} ${m.enabled ? "" : "off"} ${drag?.modId === m.modId ? "dragging" : ""}`}
              style={{
                position: "absolute",
                top: 0,
                left: 0,
                width: "100%",
                transform: `translateY(${v.start + ROW_H}px)`,
                display: "grid",
                gridTemplateColumns: COLUMNS,
              }}
              onClick={() => onSelect(m.modId)}
              onKeyDown={(e) => onRowKeyDown(e, m, v.index)}
              onContextMenu={(e: MouseEvent) => {
                e.preventDefault();
                setMenuTarget({ modId: m.modId, point: { x: e.clientX, y: e.clientY } });
              }}
            >
              <div
                className={`grip ${canReorder ? "" : "locked"}`}
                {...(canReorder
                  ? { "data-tip": "Glisser pour changer la priorité", "data-tip-key": "Alt ↑ ↓" }
                  : { "data-tip": "Efface la recherche pour réordonner" })}
                onPointerDown={(e) => beginDrag(e, m, v.index)}
                onClick={(e) => e.stopPropagation()}
              >
                <Ico name="grip" className="sm" />
              </div>
              <div>
                {m.status === "archived" ? (
                  // Never a checkbox here: nothing is extracted yet, so
                  // ticking it could not deploy a single file.
                  <IconButton
                    name="box"
                    title="Installer l’archive"
                    onClick={(e) => {
                      e.stopPropagation();
                      onInstallArchived(m);
                    }}
                  />
                ) : (
                  <Checkbox
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
                  <IconButton
                    name="sparkle"
                    title="Demander à Claude"
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
                {m.status === "archived" ? (
                  <>
                    <span className="tag archived" title={m.archivePath ?? undefined}>
                      <Ico name="box" className="sm" />
                      Archive
                    </span>
                    {otherLoadouts.length > 0 ? (
                      <IconButton
                        name="share"
                        variant="sm"
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
                  </>
                ) : (
                  <ConflictBadge conflicts={conflicts?.get(m.modId)} />
                )}
              </div>
              {/* Real value from the Core when the mod's source declared one
                  (a Minecraft jar manifest does, an MO2 import doesn't) —
                  a dash otherwise, never an invented number. */}
              <div className="ver" title={m.version ?? "Version non déclarée par la source"}>
                {m.version ?? "—"}
              </div>
              {/* A free tag the human typed (no Provider metadata is
                  involved) — still a dash when they haven't. */}
              <div
                className={`cat ${m.category ? "" : "muted"}`}
                onClick={(e) => e.stopPropagation()}
              >
                {editingCategory === m.modId ? (
                  <input
                    className="cat-input"
                    list="mod-categories"
                    autoFocus
                    maxLength={40}
                    value={categoryDraft}
                    onChange={(e) => setCategoryDraft(e.target.value)}
                    onFocus={(e) => e.currentTarget.select()}
                    onBlur={() => commitCategory(m)}
                    onKeyDown={(e) => {
                      e.stopPropagation();
                      if (e.key === "Enter") commitCategory(m);
                      if (e.key === "Escape") setEditingCategory(null);
                    }}
                  />
                ) : (
                  <button
                    className="cat-btn"
                    type="button"
                    title="Modifier la catégorie"
                    onClick={() => startEditCategory(m)}
                  >
                    {m.category ? <Ico name={categoryIcon(m.category)} className="sm" /> : null}
                    {m.category ?? "—"}
                  </button>
                )}
              </div>
              {/* Accurate today rather than a placeholder: every mod the
                  Core holds got there by a local import or install. It
                  starts varying the day a Provider lands. */}
              <div className="src">
                <Ico name="folder" className="sm" />
                local
              </div>
            </div>
          );
        })}
      </div>
      {reassignRow !== null ? (
        <Pop
          anchorRef={{ current: reassignRefs.current.get(reassignRow) ?? null }}
          items={reassignItems}
          onClose={() => setReassignRow(null)}
        />
      ) : null}
      {menuTarget && rows.some((r) => r.modId === menuTarget.modId) ? (
        <MenuPicker
          items={rowMenuItems(rows.find((r) => r.modId === menuTarget.modId)!)}
          anchorPoint={menuTarget.point}
          anchorRef={menuTarget.anchor ? { current: menuTarget.anchor } : undefined}
          onClose={() => setMenuTarget(null)}
          onItemSelect={(item) => handleMenuSelect(item, menuTarget.modId)}
        />
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
