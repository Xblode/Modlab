import { useEffect, useLayoutEffect, useRef, useState, type RefObject } from "react";
import { Ico } from "./Ico";

export type PopItem =
  | "-"
  | {
      label: string;
      sub?: string;
      icon?: string;
      current?: boolean;
      onPick?: () => void;
    };

/**
 * The maquette's `.pop` menu, as a component. Same markup and classes as
 * `openPop()` in `shell-maquette.html`: fixed position under its anchor,
 * closes on outside mousedown or Escape.
 */
export function Pop({
  anchorRef,
  items,
  onClose,
}: {
  anchorRef: RefObject<HTMLElement | null>;
  items: PopItem[];
  onClose: () => void;
}) {
  const ref = useRef<HTMLDivElement>(null);
  const [pos, setPos] = useState<{ left: number; top: number } | null>(null);

  useLayoutEffect(() => {
    const anchor = anchorRef.current;
    if (!anchor) return;
    const r = anchor.getBoundingClientRect();
    setPos({
      left: Math.max(6, Math.min(r.left, window.innerWidth - 260)),
      top: r.bottom + 4,
    });
  }, [anchorRef]);

  useEffect(() => {
    function onDown(e: MouseEvent) {
      const t = e.target as Node;
      if (ref.current?.contains(t)) return;
      if (anchorRef.current?.contains(t)) return;
      onClose();
    }
    function onKey(e: KeyboardEvent) {
      if (e.key === "Escape") onClose();
    }
    document.addEventListener("mousedown", onDown);
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("mousedown", onDown);
      document.removeEventListener("keydown", onKey);
    };
  }, [anchorRef, onClose]);

  return (
    <div
      className="pop open"
      role="menu"
      ref={ref}
      style={{ left: pos?.left ?? -9999, top: pos?.top ?? -9999 }}
    >
      {items.map((it, i) =>
        it === "-" ? (
          <div className="sep" key={`sep-${i}`} />
        ) : (
          <button
            key={it.label}
            className={`it ${it.current ? "cur" : ""}`}
            type="button"
            role="menuitem"
            onClick={() => {
              onClose();
              it.onPick?.();
            }}
          >
            {it.icon ? <Ico name={it.icon} /> : null}
            <span>
              {it.label}
              {it.sub ? <span className="sub">{it.sub}</span> : null}
            </span>
          </button>
        ),
      )}
    </div>
  );
}
