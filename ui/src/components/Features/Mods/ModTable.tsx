import { useVirtualizer } from "@tanstack/react-virtual";
import { useRef } from "react";
import { Ico } from "../../../Ico";
import { Checkbox, IconButton } from "../../ui";
import type { ModRow } from "../../../api";
import type { ModConflicts } from "../../../api";
import { ConflictBadge } from "./ConflictBadge";

type Props = {
  rows: ModRow[];
  selectedId: number | null;
  onSelect: (id: number) => void;
  onToggle: (row: ModRow) => void;
  /**
   * Per-mod conflict tallies, keyed by mod id. `null` means the loadout has
   * not been scanned yet; a mod missing from the map means the scan covered it
   * and found it in no conflict (which is why the two are kept apart).
   */
  conflicts: Map<number, ModConflicts> | null;
};

export function ModTable({
  rows,
  selectedId,
  onSelect,
  onToggle,
  conflicts,
}: Props) {
  const parentRef = useRef<HTMLDivElement>(null);
  const virtualizer = useVirtualizer({
    count: rows.length,
    getScrollElement: () => parentRef.current,
    estimateSize: () => 26,
    overscan: 12,
  });

  return (
    <div className="content" ref={parentRef} style={{ overflow: "auto" }}>
      <div
        className="grid virt-grid"
        style={{
          height: virtualizer.getTotalSize(),
          position: "relative",
          display: "block",
          minWidth: 820,
        }}
      >
        <div
          className="gh"
          style={{
            display: "grid",
            gridTemplateColumns:
              "20px 28px 44px minmax(210px,36%) 200px 92px 118px minmax(80px,1fr)",
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
                gridTemplateColumns:
                  "20px 28px 44px minmax(210px,36%) 200px 92px 118px minmax(80px,1fr)",
              }}
              onClick={() => onSelect(m.modId)}
            >
              <div className="grip">
                <Ico name="grip" className="sm" />
              </div>
              <div>
                <Checkbox
                  checked={m.enabled}
                  onToggle={() => onToggle(m)}
                  stopPropagation
                  // The row's own name — the checked state is announced by the input itself.
                  aria-label={m.name}
                />
              </div>
              <div className="pri num">{pri}</div>
              <div className="nm">
                <span className="t">{m.name}</span>
                <span className="tools">
                  <IconButton name="folder" title="Ouvrir le dossier" />
                  <IconButton
                    name="sparkle"
                    title="Demander à Claude"
                    style={{ color: "var(--claude)" }}
                  />
                  <IconButton name="more" title="Plus…" />
                </span>
              </div>
              <div className="flags">
                <ConflictBadge conflicts={conflicts?.get(m.modId)} />
              </div>
              <div className="ver">—</div>
              <div className="cat">Mods</div>
              <div className="src">
                <Ico name="folder" className="sm" />
                local
              </div>
            </div>
          );
        })}
      </div>
    </div>
  );
}
