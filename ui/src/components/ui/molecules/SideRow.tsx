import type { MouseEvent, ReactNode } from "react";
import { Ico } from "../../../Ico";

type SideRowProps = {
  /** A string is an icon name. A node is used as-is (channel dot, folder mark). */
  icon?: ReactNode;
  label: ReactNode;
  /** Same neutral row as `current`. Kept so existing call sites still compile. */
  selected?: boolean;
  /** The row the user is in. Neutral `--c3`, never the blue `--sel` fill. */
  current?: boolean;
  className?: string;
  title?: string;
  /** Folder rows report this on the label button. */
  expanded?: boolean;
  /** Quiet mark at the right of the label (branch icon, short meta). Not a button. */
  end?: ReactNode;
  /**
   * A plus or a star. When set, the row is a `div` and only the label is a
   * button, so those controls are not nested inside it.
   */
  trailing?: ReactNode;
  onClick?: () => void;
  onContextMenu?: (event: MouseEvent<HTMLElement>) => void;
};

function leading(icon: ReactNode) {
  if (icon == null || icon === false) return null;
  if (typeof icon === "string") return <Ico name={icon} />;
  return icon;
}

/**
 * One sidebar row. With no `trailing` control the whole row is the button.
 * With a plus or a star, the row is a div: the name, the plus and the star
 * are the only buttons.
 */
export function SideRow({
  icon,
  label,
  selected = false,
  current = false,
  className,
  title,
  expanded,
  end,
  trailing,
  onClick,
  onContextMenu,
}: SideRowProps) {
  const classNames = ["side-row", selected || current ? "current" : "", className ?? ""]
    .filter(Boolean)
    .join(" ");
  const body = (
    <>
      {leading(icon)}
      {typeof label === "string" ? (
        <span className="side-row-label fade">{label}</span>
      ) : (
        <span className="side-row-label">{label}</span>
      )}
      {end ? <span className="side-row-end">{end}</span> : null}
    </>
  );

  if (trailing) {
    return (
      <div className={classNames} onContextMenu={onContextMenu}>
        {onClick ? (
          <button
            type="button"
            className="side-row-main"
            title={title}
            aria-current={current || undefined}
            aria-pressed={selected || undefined}
            aria-expanded={expanded}
            onClick={onClick}
          >
            {body}
          </button>
        ) : (
          <div className="side-row-main">{body}</div>
        )}
        {trailing}
      </div>
    );
  }

  return (
    <button
      type="button"
      className={classNames}
      title={title}
      aria-current={current || undefined}
      aria-pressed={selected || undefined}
      aria-expanded={expanded}
      onClick={onClick}
      onContextMenu={onContextMenu}
    >
      {body}
    </button>
  );
}
