import type { ReactNode } from "react";

/**
 * One `.stat` tile: a label over a value. No icon: the label already says what it counts. A tile with an `onClick` is
 * a real button, one without is inert markup — never a button that does
 * nothing.
 */
export function StatTile({
  label,
  value,
  title,
  onClick,
}: {
  label: string;
  value: ReactNode;
  title?: string;
  onClick?: () => void;
}) {
  const content = (
    <>
      <span className="k">{label}</span>
      <span className="v">{value}</span>
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
