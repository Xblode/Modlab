import type { ReactNode } from "react";

/**
 * A short empty state for a narrow area (sidebar section, bottom panel): a
 * title, one phrase, and an action only when the app already has one. The
 * full-page version is the `.empty` block.
 */
export function SideEmpty({
  title,
  children,
  action,
}: {
  title: string;
  children?: ReactNode;
  action?: ReactNode;
}) {
  return (
    <div className="sempty">
      <span className="st">{title}</span>
      {children ? <span>{children}</span> : null}
      {action}
    </div>
  );
}
