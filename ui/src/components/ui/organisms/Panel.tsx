import type { CSSProperties, ReactNode } from "react";
import { Ico } from "../../../Ico";

/**
 * Inline card: an icon, a title, and a body whose class is chosen by the
 * caller (`.why` for prose, `.br` for a definition grid).
 *
 * The floating mod drawer is `SideDrawer`, not this card.
 */
export function Panel({
  icon,
  title,
  bodyClassName,
  children,
  style,
}: {
  children: ReactNode;
  style?: CSSProperties;
  icon: string;
  title: ReactNode;
  /** Body class, e.g. `why` for prose. */
  bodyClassName: string;
}) {
  return (
    <div className="box" style={style}>
      <div className="bh">
        <Ico name={icon} className="sm" />
        {title}
      </div>
      <div className={bodyClassName}>{children}</div>
    </div>
  );
}
