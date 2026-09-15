import { useVirtualizer } from "@tanstack/react-virtual";
import { useRef } from "react";
import { Ico } from "./Ico";
import type { ModRow } from "./api";

type Props = {
  rows: ModRow[];
  selectedId: number | null;
  onSelect: (id: number) => void;
  onToggle: (row: ModRow) => void;
};

export function ModTable({ rows, selectedId, onSelect, onToggle }: Props) {
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
          <div style={{ justifyContent: "flex-end" }}>#</div>
          <div>Mod</div>
          <div>Indicateurs</div>
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
                <span
                  className={`chk ${m.enabled ? "on" : ""}`}
                  role="checkbox"
                  aria-checked={m.enabled}
                  onClick={(e) => {
                    e.stopPropagation();
                    onToggle(m);
                  }}
                >
                  {m.enabled ? <Ico name="check" className="sm" /> : null}
                </span>
              </div>
              <div className="pri num">{pri}</div>
              <div className="nm">
                <span className="t">{m.name}</span>
                <span className="tools">
                  <button className="tb" title="Ouvrir le dossier" type="button">
                    <Ico name="folder" />
                  </button>
                  <button
                    className="tb"
                    title="Demander à Claude"
                    type="button"
                    style={{ color: "var(--claude)" }}
                  >
                    <Ico name="sparkle" />
                  </button>
                  <button className="tb" title="Plus…" type="button">
                    <Ico name="more" />
                  </button>
                </span>
              </div>
              <div className="flags" />
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
