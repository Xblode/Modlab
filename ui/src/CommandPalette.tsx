import { useEffect, useMemo, useRef, useState } from "react";
import { Ico } from "./Ico";

/**
 * Command palette (Ctrl+K). Presentation follows the maquette's `.cp`
 * vocabulary — a floating `c3` card with a `.list` of `.it` rows — and the
 * items are owned by the caller: the shell builds them from what the Core
 * and the adapters can actually serve, this component only filters,
 * navigates and runs.
 */
export type PaletteItem = {
  id: string;
  /** Free-form section label, shown on the row's right. */
  group: string;
  label: string;
  sub?: string;
  icon?: string;
  run: () => void;
};

export function CommandPalette({
  open,
  items,
  onClose,
}: {
  open: boolean;
  items: PaletteItem[];
  onClose: () => void;
}) {
  const [query, setQuery] = useState("");
  const [cursor, setCursor] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);
  const activeRef = useRef<HTMLButtonElement>(null);

  const filtered = useMemo(() => {
    const q = query.trim().toLowerCase();
    if (!q) return items;
    return items.filter((it) =>
      `${it.label} ${it.sub ?? ""} ${it.group}`.toLowerCase().includes(q),
    );
  }, [items, query]);

  // A fresh open starts a fresh search; the input takes focus immediately.
  useEffect(() => {
    if (open) {
      setQuery("");
      setCursor(0);
      // Focus after mount — the palette can be rendered inside a backdrop.
      window.setTimeout(() => inputRef.current?.focus(), 0);
    }
  }, [open]);

  useEffect(() => {
    if (!open) return;
    function onKey(e: KeyboardEvent) {
      if (e.key === "Escape") {
        e.preventDefault();
        onClose();
      } else if (e.key === "ArrowDown") {
        e.preventDefault();
        setCursor((c) => (filtered.length ? (c + 1) % filtered.length : 0));
      } else if (e.key === "ArrowUp") {
        e.preventDefault();
        setCursor((c) =>
          filtered.length ? (c - 1 + filtered.length) % filtered.length : 0,
        );
      } else if (e.key === "Enter") {
        e.preventDefault();
        const it = filtered[cursor];
        if (it) {
          onClose();
          it.run();
        }
      }
    }
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, [open, filtered, cursor, onClose]);

  // Keep the highlighted row in view while navigating with the keyboard.
  useEffect(() => {
    activeRef.current?.scrollIntoView({ block: "nearest" });
  }, [cursor]);

  if (!open) return null;

  return (
    <div
      className="cpk-ov"
      role="presentation"
      onMouseDown={(e) => {
        if (e.target === e.currentTarget) onClose();
      }}
    >
      <div className="cpk" role="dialog" aria-label="Palette de commandes">
        <div className="in">
          <Ico name="search" className="sm" />
          <input
            ref={inputRef}
            value={query}
            onChange={(e) => {
              setQuery(e.target.value);
              setCursor(0);
            }}
            placeholder="Commande, instance, mod…"
            spellCheck={false}
          />
          <span className="kbd">Échap</span>
        </div>
        <div className="cp open">
          <div className="list" role="listbox">
            {filtered.length === 0 ? (
              <div className="hint" style={{ border: 0 }}>
                Aucun résultat pour « {query} »
              </div>
            ) : (
              filtered.map((it, i) => (
                <button
                  key={it.id}
                  ref={i === cursor ? activeRef : undefined}
                  className={`it ${i === cursor ? "cur" : ""}`}
                  type="button"
                  role="option"
                  aria-selected={i === cursor}
                  title={it.sub ?? it.label}
                  onMouseEnter={() => setCursor(i)}
                  onClick={() => {
                    onClose();
                    it.run();
                  }}
                >
                  <Ico name={it.icon ?? "box"} className="sm" />
                  <span>
                    {it.label}
                    {it.sub ? (
                      <span className="muted" style={{ marginLeft: 6, fontSize: 11 }}>
                        {it.sub}
                      </span>
                    ) : null}
                  </span>
                  <span className="grp">{it.group}</span>
                </button>
              ))
            )}
          </div>
          <div className="hint">
            <span>↑↓ naviguer</span>
            <span>↵ exécuter</span>
            <span>Échap fermer</span>
          </div>
        </div>
      </div>
    </div>
  );
}
