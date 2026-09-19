import type { ReactNode } from "react";
import { Ico } from "../../../Ico";

/**
 * One `.stat` tile: a label over an icon + value. A tile with an `onClick` is
 * a real button, one without is inert markup — never a button that does
 * nothing.
 */
export function StatTile({
  label,
  value,
  icon,
  title,
  onClick,
}: {
  label: string;
  value: ReactNode;
  icon: string;
  title?: string;
  onClick?: () => void;
}) {
  const content = (
    <>
      <span className="k">{label}</span>
      <span className="v">
        <Ico name={icon} />
        {value}
      </span>
    </>
  );
  return onClick ? (
    <button className="stat" type="button" title={title} onClick={onClick}>
      {content}
    </button>
  ) : (
    <div className="stat" title={title}>
      {content}
    </div>
  );
}
