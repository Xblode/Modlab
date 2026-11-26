import {
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
  type ReactNode,
  type RefObject,
} from "react";
import { Ico } from "../../../Ico";
import { readRowHeight } from "../../../density";
import { Input } from "../atoms/Input";
import "./popover-menu.css";

const PANEL_WIDTH_DIALOG = 320;
const PANEL_WIDTH_CONTEXT = 220;
const FLYOUT_WIDTH = 220;
const ITEM_GAP = 1;
const OPEN_DELAY_MS = 170;
const CLOSE_DELAY_MS = 220;

export type PopoverMenuUtility =
  | {
      kind: "search";
      value: string;
      onChange: (value: string) => void;
      placeholder?: string;
    }
  | { kind: "action"; label: string; onClick: () => void; icon?: string };

export type PopoverMenuTab = { id: string; label: string };

/** Declarative tree entry (context menus, app menu). */
export type PopoverMenuEntry = {
  id: string;
  label: string;
  sub?: string;
  icon?: string;
  disabled?: boolean;
  danger?: boolean;
  current?: boolean;
  badge?: string;
  shortcut?: string;
  children?: PopoverMenuEntry[];
};

/** `dialog` = title + close (rich menus). `context` = no X; header only if `title` is set. */
export type PopoverMenuChrome = "dialog" | "context";

export type PopoverMenuProps = {
  /** Required for `dialog`. Optional for `context` — omit for a bare right-click menu. */
  title?: string;
  onClose: () => void;
  open?: boolean;
  anchorRef?: RefObject<HTMLElement | null>;
  anchorPoint?: { x: number; y: number };
  utility?: PopoverMenuUtility;
  tabs?: PopoverMenuTab[];
  activeTab?: string;
  onTabChange?: (id: string) => void;
  footer?: ReactNode;
  children: ReactNode;
  className?: string;
  /** Design-system / static preview — not fixed to the viewport. */
  inline?: boolean;
  /** Header chrome. Default `dialog` (title + close). Use `context` for right-click menus. */
  chrome?: PopoverMenuChrome;
  /** Anchor side. `above` sits the panel over the button (chat composer +). Default `below`. */
  placement?: "below" | "above";
};

type Pos = { left: number; top: number };

/**
 * Fixed popover chrome: optional title (+ close in dialog mode), optional
 * search/action, optional tabs, scrollable body, optional footer. Call sites
 * pass body items as children (`PopoverMenuItem` / `PopoverMenuEntries` /
 * `PopoverMenuSep`). Context chrome with no title skips the header entirely.
 */
export function PopoverMenu({
  title,
  onClose,
  open = true,
  anchorRef,
  anchorPoint,
  utility,
  tabs,
  activeTab,
  onTabChange,
  footer,
  children,
  className,
  inline = false,
  chrome = "dialog",
  placement = "below",
}: PopoverMenuProps) {
  const ref = useRef<HTMLDivElement>(null);
  const [pos, setPos] = useState<Pos | null>(null);
  const isContext = chrome === "context";
  const panelWidth = isContext ? PANEL_WIDTH_CONTEXT : PANEL_WIDTH_DIALOG;
  const titleText = title?.trim() ?? "";
  const showHead = !isContext || titleText.length > 0;
  const placeAbove = placement === "above";

  useLayoutEffect(() => {
    if (!open || inline) return;
    if (anchorPoint) {
      setPos({
        left: Math.max(6, Math.min(anchorPoint.x, window.innerWidth - panelWidth - 6)),
        top: Math.max(6, Math.min(anchorPoint.y, window.innerHeight - 6)),
      });
      return;
    }
    const anchor = anchorRef?.current;
    if (!anchor) return;
    const r = anchor.getBoundingClientRect();
    const left = Math.max(6, Math.min(r.left, window.innerWidth - panelWidth - 6));
    if (placeAbove) {
      // Bottom edge of the menu sits 4px above the anchor; height measured after paint.
      setPos({ left, top: Math.max(6, r.top - 4) });
      return;
    }
    setPos({ left, top: r.bottom + 4 });
  }, [anchorRef, anchorPoint, open, inline, panelWidth, placeAbove]);

  useLayoutEffect(() => {
    if (!open || inline || !placeAbove || !pos || anchorPoint) return;
    const panel = ref.current;
    const anchor = anchorRef?.current;
    if (!panel || !anchor) return;
    const h = panel.getBoundingClientRect().height;
    const r = anchor.getBoundingClientRect();
    const nextTop = Math.max(6, r.top - h - 4);
    if (Math.abs(nextTop - pos.top) > 0.5) setPos({ left: pos.left, top: nextTop });
  }, [open, inline, placeAbove, pos, anchorRef, anchorPoint, children]);

  useEffect(() => {
    if (!open || inline) return;
    function onDown(e: MouseEvent) {
      const t = e.target as Node;
      if (ref.current?.contains(t)) return;
      if (anchorRef?.current?.contains(t)) return;
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
  }, [anchorRef, onClose, open, inline]);

  if (!open) return null;
  if (!inline && !pos) return null;

  return (
    <div
      ref={ref}
      className={[
        "popover-menu",
        isContext ? "popover-menu--context" : "popover-menu--dialog",
        inline ? "popover-menu--inline" : "",
        className ?? "",
      ]
        .filter(Boolean)
        .join(" ")}
      role={isContext ? undefined : "dialog"}
      aria-label={isContext ? undefined : titleText || "Menu"}
      style={inline ? undefined : { left: pos!.left, top: pos!.top }}
    >
      {showHead ? (
        <div className="popover-menu-head">
          {titleText ? <div className="popover-menu-title">{titleText}</div> : null}
          {isContext ? null : (
            <button
              type="button"
              className="popover-menu-close"
              aria-label="Fermer"
              title="Fermer"
              onClick={onClose}
            >
              <Ico name="x" />
            </button>
          )}
        </div>
      ) : null}

      {utility ? (
        <div className="popover-menu-utility">
          {utility.kind === "search" ? (
            <label className="popover-menu-search">
              <Ico name="search" />
              <Input
                value={utility.value}
                onChange={(e) => utility.onChange(e.target.value)}
                placeholder={utility.placeholder}
                autoFocus
              />
            </label>
          ) : (
            <button
              type="button"
              className="popover-menu-action"
              onClick={() => {
                utility.onClick();
                onClose();
              }}
            >
              {utility.icon ? <Ico name={utility.icon} className="sm" /> : null}
              {utility.label}
            </button>
          )}
        </div>
      ) : null}

      {utility || (tabs && tabs.length > 0) ? <div className="popover-menu-sep" /> : null}

      {tabs && tabs.length > 0 ? (
        <>
          <div className="popover-menu-tabs" role="tablist">
            {tabs.map((tab) => (
              <button
                key={tab.id}
                type="button"
                className="popover-menu-tab"
                role="tab"
                aria-selected={activeTab === tab.id}
                onClick={() => onTabChange?.(tab.id)}
              >
                {tab.label}
              </button>
            ))}
          </div>
          <div className="popover-menu-sep" />
        </>
      ) : null}

      <div className="popover-menu-body" role="menu" aria-label={titleText || "Menu"}>
        {children}
      </div>

      {footer ? (
        <div className="popover-menu-footer">
          {typeof footer === "string" ? (
            <span className="popover-menu-footer-link">{footer}</span>
          ) : (
            footer
          )}
        </div>
      ) : null}
    </div>
  );
}

export function PopoverMenuSep() {
  return <div className="popover-menu-sep" role="separator" />;
}

export function PopoverMenuItem({
  icon,
  label,
  sub,
  current = false,
  badge,
  danger = false,
  disabled = false,
  shortcut,
  onPick,
  inlineSub = false,
}: {
  icon?: string;
  label: string;
  sub?: string;
  current?: boolean;
  badge?: string;
  danger?: boolean;
  disabled?: boolean;
  shortcut?: string;
  onPick?: () => void;
  /** Subtitle on the same row (menu-tree style) instead of under the label. */
  inlineSub?: boolean;
}) {
  const lead = current ? (
    <Ico name="check" className="check" />
  ) : icon ? (
    <Ico name={icon} />
  ) : null;

  return (
    <button
      type="button"
      role="menuitem"
      className={[
        "popover-menu-item",
        current ? "is-current" : "",
        danger ? "is-danger" : "",
        disabled ? "is-disabled" : "",
        lead ? "" : "popover-menu-item--plain",
      ]
        .filter(Boolean)
        .join(" ")}
      disabled={disabled}
      title={sub ?? label}
      onClick={() => {
        if (disabled) return;
        onPick?.();
      }}
    >
      {lead ? (
        <span className="popover-menu-item-lead" aria-hidden="true">
          {lead}
        </span>
      ) : null}
      {inlineSub ? (
        <>
          <span className="popover-menu-item-label">{label}</span>
          {sub ? <span className="popover-menu-item-sub-inline">{sub}</span> : null}
          {badge ? <span className="popover-menu-item-badge">{badge}</span> : null}
          {shortcut ? <span className="popover-menu-item-shortcut">{shortcut}</span> : null}
        </>
      ) : (
        <span className="popover-menu-item-main">
          <span className="popover-menu-item-row">
            <span className="popover-menu-item-label">{label}</span>
            {badge ? <span className="popover-menu-item-badge">{badge}</span> : null}
            {shortcut ? <span className="popover-menu-item-shortcut">{shortcut}</span> : null}
          </span>
          {sub ? <span className="popover-menu-item-sub">{sub}</span> : null}
        </span>
      )}
    </button>
  );
}

/**
 * Renders a declarative tree with hover flyouts (former MenuPicker behaviour).
 * Selecting a leaf calls `onSelect` and closes via the parent `PopoverMenu`.
 */
export function PopoverMenuEntries({
  items,
  onSelect,
}: {
  items: readonly PopoverMenuEntry[];
  onSelect: (item: PopoverMenuEntry) => void;
}) {
  const [openPath, setOpenPath] = useState<string[]>([]);
  const timer = useRef<number | null>(null);

  function cancel() {
    if (timer.current != null) {
      window.clearTimeout(timer.current);
      timer.current = null;
    }
  }

  function schedule(fn: () => void, ms: number) {
    cancel();
    timer.current = window.setTimeout(() => {
      timer.current = null;
      fn();
    }, ms);
  }

  useEffect(() => cancel, []);

  return (
    <div
      onMouseEnter={cancel}
      onMouseLeave={() => schedule(() => setOpenPath([]), CLOSE_DELAY_MS)}
    >
      <EntryLevel
        items={items}
        depth={0}
        openPath={openPath}
        onOpenPath={setOpenPath}
        onSelect={(item) => {
          cancel();
          setOpenPath([]);
          onSelect(item);
        }}
        schedule={schedule}
        cancel={cancel}
        root
      />
    </div>
  );
}

function EntryLevel({
  items,
  depth,
  openPath,
  onOpenPath,
  onSelect,
  schedule,
  cancel,
  root = false,
  pos,
}: {
  items: readonly PopoverMenuEntry[];
  depth: number;
  openPath: string[];
  onOpenPath: (path: string[]) => void;
  onSelect: (item: PopoverMenuEntry) => void;
  schedule: (fn: () => void, ms: number) => void;
  cancel: () => void;
  root?: boolean;
  pos?: Pos;
}) {
  const panelRef = useRef<HTMLDivElement>(null);
  const rowRefs = useRef(new Map<string, HTMLButtonElement | null>());
  const [childPos, setChildPos] = useState<Pos | null>(null);

  const openId = openPath[depth] ?? null;
  const openItem =
    items.find((i) => i.id === openId && (i.children?.length ?? 0) > 0) ?? null;

  useLayoutEffect(() => {
    if (!openItem) {
      setChildPos(null);
      return;
    }
    const row = rowRefs.current.get(openItem.id);
    const panel = panelRef.current;
    if (!row || !panel) return;
    const r = row.getBoundingClientRect();
    const p = panel.getBoundingClientRect();
    let left = p.right - 2;
    if (left + FLYOUT_WIDTH > window.innerWidth - 6) {
      left = Math.max(6, p.left - FLYOUT_WIDTH + 2);
    }
    const estimatedHeight =
      (openItem.children?.length ?? 0) * (readRowHeight() + ITEM_GAP) + 12;
    // Align flyout top with the triggering row (screen reference).
    const top = Math.max(
      6,
      Math.min(r.top - 4, window.innerHeight - 6 - estimatedHeight),
    );
    setChildPos({ left, top });
  }, [openItem, pos?.left, pos?.top]);

  const list = (
    <>
      {items.map((item) => {
        const disabled = item.disabled ?? false;
        const hasChildren = (item.children?.length ?? 0) > 0;
        const isOpen = openItem?.id === item.id;
        const lead = item.current ? (
          <Ico name="check" className="check" />
        ) : item.icon ? (
          <Ico name={item.icon} />
        ) : null;
        return (
          <div key={item.id} style={{ position: "relative" }}>
            <button
              type="button"
              ref={(el) => {
                rowRefs.current.set(item.id, el);
              }}
              role="menuitem"
              className={[
                "popover-menu-item",
                item.current ? "is-current" : "",
                item.danger ? "is-danger" : "",
                disabled ? "is-disabled" : "",
                isOpen ? "is-open" : "",
                lead ? "" : "popover-menu-item--plain",
              ]
                .filter(Boolean)
                .join(" ")}
              disabled={disabled}
              title={item.sub ?? item.label}
              aria-haspopup={hasChildren || undefined}
              aria-expanded={hasChildren ? isOpen : undefined}
              onMouseEnter={() => {
                if (disabled) return;
                schedule(
                  () =>
                    onOpenPath(
                      hasChildren
                        ? [...openPath.slice(0, depth), item.id]
                        : openPath.slice(0, depth),
                    ),
                  OPEN_DELAY_MS,
                );
              }}
              onClick={() => {
                if (disabled) return;
                cancel();
                if (hasChildren) {
                  onOpenPath([...openPath.slice(0, depth), item.id]);
                  return;
                }
                onSelect(item);
              }}
            >
              {lead ? (
                <span className="popover-menu-item-lead" aria-hidden="true">
                  {lead}
                </span>
              ) : null}
              <span className="popover-menu-item-label">{item.label}</span>
              {item.sub ? <span className="popover-menu-item-sub-inline">{item.sub}</span> : null}
              {item.badge ? <span className="popover-menu-item-badge">{item.badge}</span> : null}
              {item.shortcut ? (
                <span className="popover-menu-item-shortcut">{item.shortcut}</span>
              ) : null}
              {hasChildren ? (
                <Ico name="chevr" className="sm popover-menu-item-chevron" />
              ) : null}
            </button>
          </div>
        );
      })}
      {openItem && childPos ? (
        <EntryLevel
          items={openItem.children ?? []}
          depth={depth + 1}
          openPath={openPath}
          onOpenPath={onOpenPath}
          onSelect={onSelect}
          schedule={schedule}
          cancel={cancel}
          pos={childPos}
        />
      ) : null}
    </>
  );

  if (root) {
    return <div ref={panelRef}>{list}</div>;
  }

  return (
    <div
      ref={panelRef}
      className="popover-menu-flyout"
      style={{ left: pos?.left ?? -9999, top: pos?.top ?? -9999 }}
      role="menu"
    >
      {list}
    </div>
  );
}

export function PopoverMenuFooterLink({
  children,
  onClick,
}: {
  children: ReactNode;
  onClick?: () => void;
}) {
  return (
    <button type="button" className="popover-menu-footer-link" onClick={onClick}>
      {children}
    </button>
  );
}
