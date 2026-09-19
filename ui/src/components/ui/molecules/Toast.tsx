import type { CSSProperties, ReactNode } from "react";
import { Ico } from "../../../Ico";

/** The maquette's bottom toast. Its lifetime belongs to the caller. */
export function Toast({
  icon,
  style,
  children,
}: {
  icon: string;
  style?: CSSProperties;
  children: ReactNode;
}) {
  return (
    <div className="toast show" style={style}>
      <Ico name={icon} className="sm" />
      <span>{children}</span>
    </div>
  );
}
