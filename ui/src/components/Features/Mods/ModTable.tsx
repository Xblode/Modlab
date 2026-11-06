import { useVirtualizer } from "@tanstack/react-virtual";
import { useRef, useState, type MouseEvent } from "react";
import { Ico } from "../../../Ico";
import { Pop, type PopItem } from "../../../Pop";
import { Checkbox, ConfirmDialog, IconButton } from "../../ui";
import { MenuPicker, type MenuItem } from "../MenuPicker/MenuPicker";
import { categoryIcon } from "../../../categoryIcon";
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

type Props = {
  rows: ModRow[];
  selectedId: number | null;
  onSelect: (id: number) => void;
  onToggle: (row: ModRow) => void;
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
  const virtualizer = useVirtualizer({
    count: rows.length,
    getScrollElement: () => parentRef.current,
    estimateSize: () => 26,
    overscan: 12,
  });

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
        {virtualizer.getVirtualItems().map((v) => {
          const m = rows[v.index];
          const pri = String(v.index + 1).padStart(3, "0");
          const selected = selectedId === m.modId;
          return (
            <div
              key={m.modId}
              role="row"
              tabIndex={0}
              className={`row ${selected ? "sel" : ""} ${m.enabled ? "" : "off"}`}
              style={{
                position: "absolute",
                top: 0,
                left: 0,
                width: "100%",
                transform: `translateY(${v.start + 26}px)`,
                display: "grid",
                gridTemplateColumns: COLUMNS,
              }}
              onClick={() => onSelect(m.modId)}
              onContextMenu={(e: MouseEvent) => {
                e.preventDefault();
                setMenuTarget({ modId: m.modId, point: { x: e.clientX, y: e.clientY } });
              }}
            >
              <div className="grip">
                <Ico name="grip" className="sm" />
              </div>
              <div>
                {m.status === "archived" ? (
                  // Never a checkbox here: nothing is extracted yet, so
                  // ticking it could not deploy a single file.
                  <IconButton
                    name="box"
                    title={`Archive non extraite — installer « ${m.name} »`}
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
                    title="Plus…"
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
