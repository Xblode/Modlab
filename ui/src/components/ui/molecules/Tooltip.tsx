import { useEffect, useLayoutEffect, useRef, useState } from "react";

/**
 * Tooltips for controls (`IconButton`, `Button`, `Select`, …), with no dependency and no
 * wrapper element (a wrapper would change the layout of every toolbar it touches).
 *
 * A control opts in with `data-tip` (the short French label) and, only when a
 * real key handler exists in the code, `data-tip-key` (e.g. "Ctrl J"). One
 * `<TooltipLayer />` mounted once listens at the document level and draws the
 * bubble. Use `tip()` to spread the right attributes — it also sets the
 * `aria-label` (needed for icon-only buttons; labeled `Button`s may override).
 */
export function tip(text: string, shortcut?: string) {
  return { "data-tip": text, "data-tip-key": shortcut, "aria-label": text } as const;
}

/** Short on purpose: long enough not to flash while crossing a toolbar. */
const SHOW_DELAY_MS = 350;
const GAP = 6;
const MARGIN = 8;

type Shown = { text: string; keys: string | null; rect: DOMRect };

export function TooltipLayer() {
  const [shown, setShown] = useState<Shown | null>(null);
  const [pos, setPos] = useState<{ left: number; top: number } | null>(null);
  const boxRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    let timer = 0;
    let current: Element | null = null;

    const hide = () => {
      window.clearTimeout(timer);
      current = null;
      setShown(null);
    };
    const arm = (target: EventTarget | null, delay: number) => {
      const el = target instanceof Element ? target.closest("[data-tip]") : null;
      if (el === current) return;
      hide();
      if (!el) return;
      current = el;
      timer = window.setTimeout(() => {
        const text = el.getAttribute("data-tip");
        if (!text || !el.isConnected) return;
        setShown({ text, keys: el.getAttribute("data-tip-key"), rect: el.getBoundingClientRect() });
      }, delay);
    };

    const onOver = (e: PointerEvent) => arm(e.target, SHOW_DELAY_MS);
    // Keyboard focus only: a mouse click also focuses, and must not re-open it.
    const onFocus = (e: FocusEvent) => {
      if (e.target instanceof Element && e.target.matches(":focus-visible")) arm(e.target, 0);
    };
    const onLeaveWindow = () => hide();

    document.addEventListener("pointerover", onOver);
    document.addEventListener("pointerdown", hide);
    document.addEventListener("focusin", onFocus);
    document.addEventListener("focusout", hide);
    document.addEventListener("keydown", hide);
    document.addEventListener("scroll", hide, true);
    document.documentElement.addEventListener("pointerleave", onLeaveWindow);
    return () => {
      window.clearTimeout(timer);
      document.removeEventListener("pointerover", onOver);
      document.removeEventListener("pointerdown", hide);
      document.removeEventListener("focusin", onFocus);
      document.removeEventListener("focusout", hide);
      document.removeEventListener("keydown", hide);
      document.removeEventListener("scroll", hide, true);
      document.documentElement.removeEventListener("pointerleave", onLeaveWindow);
    };
  }, []);

  // Place after measuring: below the control, above it when there is no room.
  useLayoutEffect(() => {
    const box = boxRef.current;
    if (!shown || !box) {
      setPos(null);
      return;
    }
    const { width, height } = box.getBoundingClientRect();
    const { rect } = shown;
    let top = rect.bottom + GAP;
    if (top + height > window.innerHeight - MARGIN) top = rect.top - height - GAP;
    let left = rect.left + rect.width / 2 - width / 2;
    left = Math.max(MARGIN, Math.min(left, window.innerWidth - width - MARGIN));
    setPos({ left, top: Math.max(MARGIN, top) });
  }, [shown]);

  if (!shown) return null;
  return (
    <div
      ref={boxRef}
      className="tip"
      role="tooltip"
      style={{ left: pos?.left ?? 0, top: pos?.top ?? 0, opacity: pos ? 1 : 0 }}
    >
      <span>{shown.text}</span>
      {shown.keys ? <span className="kbd">{shown.keys}</span> : null}
    </div>
  );
}
