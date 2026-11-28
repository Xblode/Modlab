import type { CSSProperties, ReactNode } from "react";

/**
 * Sheet anchored to one edge of the center area.
 *
 * `float` is the inset card: 8 px inset, `--r` on all four corners, raised
 * surface, float shadow. `column` is the shell's resizable right column
 * (class `utility-col`). See `ui/README.md` (section "Le shell") and `shell/utilityColumnChrome.ts`.
 * The shell only treats the slot as that column when
 * the element it receives carries `utility-col` on its props — pass the
 * class through from the caller, don't rely on this default alone.
 */
export type SideDrawerSide = "left" | "right";
export type SideDrawerLayout = "float" | "column";

export function SideDrawer({
  side = "right",
  layout = "float",
  header,
  children,
  label = "Panneau",
  className,
  style,
}: {
  /** Edge the floating drawer is anchored to. Ignored for `column`. */
  side?: SideDrawerSide;
  /** `float` overlays the sheet. `column` fills the shell's right column. */
  layout?: SideDrawerLayout;
  header: ReactNode;
  children: ReactNode;
  /** Accessible name of the drawer. */
  label?: string;
  className?: string;
  style?: CSSProperties;
}) {
  const column = layout === "column";
  const classNames = (
    column
      ? ["utility-col", className]
      : ["side-drawer", side === "right" ? "right" : "", className]
  )
    .filter(Boolean)
    .join(" ");
  const sheet = (
    <>
      <div className="rhead">{header}</div>
      <div className="rbody">{children}</div>
    </>
  );
  return (
    <aside
      className={classNames}
      {...(column ? {} : { "data-side": side })}
      aria-label={label}
      style={style}
    >
      {column ? <div className="utility-card">{sheet}</div> : sheet}
    </aside>
  );
}
