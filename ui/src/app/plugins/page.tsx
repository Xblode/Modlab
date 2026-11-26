import { useEffect, useMemo, useRef, useState, type KeyboardEvent, type PointerEvent } from "react";
import { getLoadoutPlugins } from "../../api";
import type { ModRow, PluginIssue, PluginRow, PluginsView } from "../../api";
import { Ico } from "../../Ico";
import { Badge, Checkbox } from "../../components/ui";

const COLUMNS = "20px 28px 52px minmax(160px,1.4fr) minmax(140px,1fr) minmax(120px,176px) 84px";
const DRAG_THRESHOLD = 4;
const EDGE = 36;

function issueText(i: PluginIssue, view: PluginsView): string {
  switch (i.kind) {
    case "missing_master":
      return `${i.plugin} : master manquant « ${i.detail} » — désactivé`;
    case "inactive_master":
      return `${i.plugin} : désactivé car son master « ${i.detail} » est désactivé`;
    case "unreadable":
      return `${i.plugin} : en-tête illisible — désactivé`;
    case "master_cycle":
      return `${i.plugin} : dépendance circulaire entre masters, aucun ordre valide`;
    case "master_after_non_master":
      return `${i.plugin} : master chargé après un plugin non-master`;
    case "full_limit":
      return `${i.plugin} : limite de ${view.fullLimit} plugins actifs dépassée — désactivé`;
    case "light_limit":
      return `${i.plugin} : limite de ${view.lightLimit} plugins ESL actifs dépassée — désactivé`;
    case "unencodable_name":
      return `${i.plugin} : le nom contient des caractères que le jeu ne sait pas lire — désactivé`;
  }
}

/**
 * Type column: where the plugin comes from, then what the header says it is.
 * Provenance is Officiel or Mods — same `root` pill, the label is the
 * difference. Kind is only the flags the Core already stored (`master` /
 * `light` from esplugin: light flag or `.esl`, master flag or `.esm`/`.esl`).
 * Light wins over master, because an `.esl` is light even when the header
 * also marks it as a master. Nothing here is inferred from the file name.
 */
function PluginTypeBadges({ plugin }: { plugin: PluginRow }) {
  return (
    <>
      {plugin.official ? (
        <Badge tone="root" title="Plugin officiel : chargé en premier, toujours actif" style={{ flex: "none" }}>
          Officiel
        </Badge>
      ) : plugin.modId != null ? (
        <Badge tone="root" title="Plugin fourni par un mod" style={{ flex: "none" }}>
          Mods
        </Badge>
      ) : null}
      {plugin.light ? (
        <Badge tone="plug" title="Plugin léger (ESL)" style={{ flex: "none" }}>
          ESL
        </Badge>
      ) : plugin.master ? (
        <Badge tone="plug" title="Plugin maître : chargé avant les autres" style={{ flex: "none" }}>
          Maître
        </Badge>
      ) : null}
    </>
  );
}

function modColumn(plugin: PluginRow, mod: ModRow | null): string {
  if (plugin.official) return "Jeu de base";
  return mod?.name ?? plugin.modName ?? "Dossier Data du jeu";
}

type PluginLine = {
  key: string;
  kind: "plugin";
  index: number;
  plugin: PluginRow;
  mod: ModRow | null;
};

type ModOnlyLine = {
  key: string;
  kind: "mod";
  mod: ModRow;
};

type ListedRow = PluginLine | ModOnlyLine;

function rowMod(row: ListedRow): ModRow | null {
  return row.mod;
}

type DropHint = { key: string; side: "before" | "after"; anchorId: number };

/**
 * Where `modId` should land in the priority list (`move_mods`' index: the
 * place in the list once the moved mod is taken out). `anchorId` is another
 * mod of that same list.
 */
function insertionIndex(
  all: ModRow[],
  modId: number,
  anchorId: number | null,
  side: "before" | "after" | "start" | "end",
): number {
  const order = [...all].sort((a, b) => a.position - b.position || a.modId - b.modId);
  const without = order.map((m) => m.modId).filter((id) => id !== modId);
  if (side === "start") return 0;
  if (side === "end" || anchorId == null) return without.length;
  const idx = without.indexOf(anchorId);
  if (idx < 0) return without.length;
  return side === "before" ? idx : idx + 1;
}

/**
 * Load order the Skyrim adapter computes — the order written to plugins.txt
 * at launch. One row per plugin, in that order, with no extra sort.
 * Installed mods that ship no plugin follow, so they stay visible.
 *
 * LOOT is not launched and plugins.txt is not read back. Drag and Alt+arrows
 * reuse the Mods priority move; the list then re-reads the computed order.
 */
export default function PluginsPage({
  loadoutId,
  mods,
  onToggle,
  onReorder,
}: {
  loadoutId: number | null;
  mods: ModRow[];
  onToggle: (row: ModRow) => void;
  onReorder: (row: ModRow, to: number) => Promise<void>;
}) {
  const [view, setView] = useState<PluginsView | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [dragKey, setDragKey] = useState<string | null>(null);
  const [hint, setHint] = useState<DropHint | null>(null);
  const [announcement, setAnnouncement] = useState("");

  const installed = useMemo(() => mods.filter((m) => m.status === "installed"), [mods]);
  const orderKey = installed.map((m) => `${m.modId}:${m.enabled ? 1 : 0}:${m.position}`).join("|");

  const requestedLoadout = useRef<number | null>(null);
  const scrollerRef = useRef<HTMLDivElement>(null);
  const modsRef = useRef(mods);
  modsRef.current = mods;
  const busy = useRef(false);
  const focusAfter = useRef<number | null>(null);
  const hintRef = useRef<DropHint | null>(null);

  useEffect(() => {
    if (loadoutId == null) {
      setView(null);
      setError(null);
      return;
    }
    let cancelled = false;
    if (requestedLoadout.current !== loadoutId) {
      setView(null);
      requestedLoadout.current = loadoutId;
    }
    setError(null);
    getLoadoutPlugins(loadoutId)
      .then((v) => {
        if (!cancelled) setView(v);
      })
      .catch((e: unknown) => {
        if (!cancelled) {
          setView(null);
          setError(e instanceof Error ? e.message : String(e));
        }
      });
    return () => {
      cancelled = true;
    };
  }, [loadoutId, orderKey]);

  const rows = useMemo(() => {
    const byId = new Map(installed.map((m) => [m.modId, m]));
    if (view == null) {
      return installed
        .slice()
        .sort((a, b) => a.position - b.position || a.modId - b.modId)
        .map((mod): ModOnlyLine => ({ key: `mod-${mod.modId}`, kind: "mod", mod }));
    }
    const seen = new Set<number>();
    const pluginRows: PluginLine[] = view.plugins.map((plugin, index) => {
      const mod = plugin.modId != null ? (byId.get(plugin.modId) ?? null) : null;
      if (mod) seen.add(mod.modId);
      return {
        key: `plugin-${index}-${plugin.fileName}`,
        kind: "plugin",
        index,
        plugin,
        mod,
      };
    });
    const tail: ModOnlyLine[] = installed
      .filter((m) => !seen.has(m.modId))
      .sort((a, b) => a.position - b.position || a.modId - b.modId)
      .map((mod) => ({ key: `mod-${mod.modId}`, kind: "mod", mod }));
    return [...pluginRows, ...tail];
  }, [installed, view]);

  const rowsRef = useRef(rows);
  rowsRef.current = rows;

  useEffect(() => {
    const id = focusAfter.current;
    if (id == null) return;
    focusAfter.current = null;
    requestAnimationFrame(() => {
      scrollerRef.current?.querySelector<HTMLElement>(`[data-mod-id="${id}"]`)?.focus();
    });
  }, [rows]);

  const loadingOrder = loadoutId != null && view === null && error == null;

  function showHint(next: DropHint | null) {
    hintRef.current = next;
    setHint(next);
  }

  function hintAt(clientY: number, draggedId: number): DropHint | null {
    const nodes = scrollerRef.current?.querySelectorAll<HTMLElement>("[data-anchor]") ?? [];
    let last: DropHint | null = null;
    for (const el of nodes) {
      const anchorId = Number(el.dataset.modId);
      if (!Number.isFinite(anchorId) || anchorId === draggedId) continue;
      const rect = el.getBoundingClientRect();
      const side = clientY < rect.top + rect.height / 2 ? "before" : "after";
      const candidate: DropHint = { key: el.dataset.rowKey ?? "", side, anchorId };
      if (side === "before") return candidate;
      last = candidate;
    }
    return last;
  }

  async function commitPlace(mod: ModRow, to: number) {
    const order = [...modsRef.current].sort((a, b) => a.position - b.position || a.modId - b.modId);
    const from = order.findIndex((m) => m.modId === mod.modId);
    const n = order.length;
    const target = Math.max(0, Math.min(to, Math.max(0, n - 1)));
    if (busy.current || from < 0 || target === from) return;
    busy.current = true;
    focusAfter.current = mod.modId;
    try {
      await onReorder(mod, target);
      setAnnouncement(`« ${mod.name} » déplacé en position ${target + 1} sur ${n}.`);
    } finally {
      busy.current = false;
    }
  }

  function beginDrag(e: PointerEvent, row: ListedRow) {
    const mod = rowMod(row);
    if (!mod || busy.current || e.button !== 0) return;
    e.preventDefault();
    e.stopPropagation();
    const startY = e.clientY;
    const pointerY = { current: startY };
    let active = false;
    let scrollTimer = 0;

    const move = (ev: globalThis.PointerEvent) => {
      pointerY.current = ev.clientY;
      if (!active && Math.abs(ev.clientY - startY) < DRAG_THRESHOLD) return;
      if (!active) {
        active = true;
        setDragKey(row.key);
        scrollTimer = window.setInterval(() => {
          const el = scrollerRef.current;
          if (!el) return;
          const rect = el.getBoundingClientRect();
          const y = pointerY.current;
          if (y < rect.top + EDGE) el.scrollTop -= 14;
          else if (y > rect.bottom - EDGE) el.scrollTop += 14;
          else return;
          showHint(hintAt(y, mod.modId));
        }, 16);
      }
      showHint(hintAt(ev.clientY, mod.modId));
    };
    const finish = (commit: boolean) => {
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", up);
      window.removeEventListener("pointercancel", cancel);
      window.removeEventListener("keydown", key, true);
      window.clearInterval(scrollTimer);
      const dropped = hintRef.current;
      setDragKey(null);
      showHint(null);
      if (!active || !commit || !dropped) return;
      void commitPlace(mod, insertionIndex(modsRef.current, mod.modId, dropped.anchorId, dropped.side));
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

  function onRowKeyDown(e: KeyboardEvent, row: ListedRow) {
    const mod = rowMod(row);
    if (!mod || !e.altKey || e.ctrlKey || e.metaKey || e.target !== e.currentTarget) return;
    if (e.key === "Home") {
      e.preventDefault();
      void commitPlace(mod, insertionIndex(modsRef.current, mod.modId, null, "start"));
      return;
    }
    if (e.key === "End") {
      e.preventDefault();
      void commitPlace(mod, insertionIndex(modsRef.current, mod.modId, null, "end"));
      return;
    }
    const dir = e.key === "ArrowUp" ? -1 : e.key === "ArrowDown" ? 1 : 0;
    if (dir === 0) return;
    e.preventDefault();
    const list = rowsRef.current;
    const index = list.findIndex((r) => r.key === row.key);
    let anchor: number | null = null;
    for (let j = index + dir; j >= 0 && j < list.length; j += dir) {
      const other = rowMod(list[j]);
      if (other && other.modId !== mod.modId) {
        anchor = other.modId;
        break;
      }
    }
    const side = anchor == null ? (dir < 0 ? "start" : "end") : dir < 0 ? "before" : "after";
    void commitPlace(mod, insertionIndex(modsRef.current, mod.modId, anchor, side));
  }

  if (loadoutId == null) {
    return (
      <EmptyState
        icon="file"
        title="Aucun plugin"
        body="Choisis une instance pour voir ses plugins"
      />
    );
  }

  if (rows.length === 0) {
    if (error) return <EmptyState icon="alert" title="Lecture impossible" body={error} />;
    if (loadingOrder) return <EmptyState icon="info" title="Lecture en cours…" />;
    return (
      <EmptyState
        icon="file"
        title="Aucun plugin"
        body="Aucun .esp, .esm ou .esl dans les mods activés"
      />
    );
  }

  return (
    <div className="content" ref={scrollerRef} style={{ overflow: "auto" }}>
      <div className="sr-only" role="status" aria-live="polite">
        {announcement}
      </div>
      <div className="grid" style={{ display: "block", minWidth: 760 }}>
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
          <div>N°</div>
          <div>Plugin</div>
          <div>Mod</div>
          <div>Type</div>
          <div>État</div>
        </div>
        {rows.map((row) => (
          <PluginLineView
            key={row.key}
            row={row}
            loadingOrder={loadingOrder}
            pluginsKnown={view != null}
            dragging={dragKey === row.key}
            hint={hint?.key === row.key ? hint.side : null}
            onToggle={onToggle}
            onRowKeyDown={onRowKeyDown}
            onGripDown={beginDrag}
          />
        ))}
      </div>
      <div style={{ padding: "8px 12px", fontSize: 12, color: "var(--t2)" }}>
        {view ? (
          <div>
            {view.activeFull} plugins actifs sur {view.fullLimit} · {view.activeLight} ESL sur{" "}
            {view.lightLimit} — ordre de chargement envoyé au jeu au lancement.
          </div>
        ) : loadingOrder ? (
          <div>Ordre de chargement en cours…</div>
        ) : null}
        {error ? (
          <div style={{ color: "var(--warn)", marginTop: 4 }}>
            <Ico name="alert" /> {error}
          </div>
        ) : null}
        {view?.issues.map((i, n) => (
          <div key={`${i.kind}-${i.plugin}-${n}`} style={{ color: "var(--warn)", marginTop: 4 }}>
            <Ico name="alert" /> {issueText(i, view)}
          </div>
        ))}
      </div>
    </div>
  );
}

function PluginLineView({
  row,
  loadingOrder,
  pluginsKnown,
  dragging,
  hint,
  onToggle,
  onRowKeyDown,
  onGripDown,
}: {
  row: ListedRow;
  loadingOrder: boolean;
  pluginsKnown: boolean;
  dragging: boolean;
  hint: "before" | "after" | null;
  onToggle: (row: ModRow) => void;
  onRowKeyDown: (e: KeyboardEvent, row: ListedRow) => void;
  onGripDown: (e: PointerEvent, row: ListedRow) => void;
}) {
  const mod = rowMod(row);
  const off = row.kind === "plugin" ? !row.plugin.active : !row.mod.enabled;
  const shadow =
    hint === "before"
      ? "inset 0 2px 0 var(--accent)"
      : hint === "after"
        ? "inset 0 -2px 0 var(--accent)"
        : undefined;
  return (
    <div
      role="row"
      tabIndex={mod ? 0 : undefined}
      data-row-key={row.key}
      data-mod-id={mod ? mod.modId : undefined}
      data-anchor={mod ? "" : undefined}
      aria-label={
        row.kind === "plugin"
          ? `${row.plugin.fileName}, ordre ${row.index}`
          : `${row.mod.name}, hors ordre de chargement`
      }
      className={`row${off ? " off" : ""}${dragging ? " dragging" : ""}`}
      style={{
        display: "grid",
        gridTemplateColumns: COLUMNS,
        boxShadow: shadow,
      }}
      onKeyDown={(e) => onRowKeyDown(e, row)}
      onClick={(e) => {
        if ((e.target as HTMLElement).closest("button, a, input")) return;
        e.currentTarget.focus();
      }}
    >
      <div
        className={mod ? "grip" : undefined}
        {...(mod
          ? { "data-tip": "Glisser pour changer la priorité", "data-tip-key": "Alt ↑ ↓" }
          : {})}
        onPointerDown={mod ? (e) => onGripDown(e, row) : undefined}
        onClick={(e) => e.stopPropagation()}
      >
        {mod ? <Ico name="grip" className="sm" /> : null}
      </div>
      <div>
        {mod ? (
          <Checkbox
            checked={mod.enabled}
            onToggle={() => onToggle(mod)}
            stopPropagation
            aria-label={mod.name}
          />
        ) : null}
      </div>
      {row.kind === "plugin" ? (
        <PluginCells plugin={row.plugin} mod={mod} index={row.index} />
      ) : (
        <ModOnlyCells mod={row.mod} pluginsKnown={pluginsKnown} loadingOrder={loadingOrder} />
      )}
    </div>
  );
}

function PluginCells({
  plugin,
  mod,
  index,
}: {
  plugin: PluginRow;
  mod: ModRow | null;
  index: number;
}) {
  return (
    <>
      <div className="pri" title="Ordre de chargement">
        {index}
      </div>
      <div className="nm">
        <span className="t mono">{plugin.fileName}</span>
        {plugin.missingMasters.map((m) => (
          <span key={m} className="badge bad" title="Ce master n'est fourni par aucun mod activé">
            <Ico name="alert" />
            manque {m}
          </span>
        ))}
      </div>
      <div>{modColumn(plugin, mod)}</div>
      <div style={{ display: "flex", gap: 4, minWidth: 0 }}>
        <PluginTypeBadges plugin={plugin} />
      </div>
      <div>
        {plugin.active ? (
          <span title="Écrit avec « * » dans plugins.txt (les officiels sont actifs d'office)">
            Actif
          </span>
        ) : (
          <Badge tone="upd" title="Ce plugin ne sera pas chargé — voir les alertes">
            Inactif
          </Badge>
        )}
      </div>
    </>
  );
}

function ModOnlyCells({
  mod,
  pluginsKnown,
  loadingOrder,
}: {
  mod: ModRow;
  pluginsKnown: boolean;
  loadingOrder: boolean;
}) {
  return (
    <>
      <div className="pri">{pluginsKnown ? "—" : loadingOrder ? "…" : "—"}</div>
      <div className="nm">
        <span className="t">—</span>
      </div>
      <div className="nm">
        <span className="t">{mod.name}</span>
      </div>
      <div style={{ display: "flex", gap: 4, minWidth: 0 }}>
        <ModOnlyType enabled={mod.enabled} pluginsKnown={pluginsKnown} loading={loadingOrder} />
      </div>
      <div />
    </>
  );
}

function ModOnlyType({
  enabled,
  pluginsKnown,
  loading,
}: {
  enabled: boolean;
  pluginsKnown: boolean;
  loading: boolean;
}) {
  if (!pluginsKnown) return loading ? "…" : "—";
  if (!enabled) {
    return <span title="Mod désactivé : on ne sait pas s'il contient un plugin">—</span>;
  }
  return (
    <Badge tone="root" title="Aucun plugin .esp, .esm ou .esl" style={{ flex: "none" }}>
      Mods
    </Badge>
  );
}

function EmptyState({ icon, title, body }: { icon: string; title: string; body?: string }) {
  return (
    <div className="content">
      <div className="empty">
        <div>
          <Ico name={icon} />
          <h3>{title}</h3>
          {body ? <p>{body}</p> : null}
        </div>
      </div>
    </div>
  );
}
