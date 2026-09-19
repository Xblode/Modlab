import type { CSSProperties, ReactNode } from "react";
import { Ico } from "../../../Ico";

/**
 * The shell's two "header + body" containers.
 *
 * `box` is the maquette's inline `.box` card: an icon, a title, and a body
 * whose class is chosen by the caller (`.why` for prose, `.br` for a
 * definition grid). `right` is the floating right-hand `.right` panel, whose
 * header is a tab strip rather than a title — which is why the header is a
 * slot here and not a string.
 */
type PanelProps = {
  children: ReactNode;
  style?: CSSProperties;
} & (
  | {
      variant?: "box";
      icon: string;
      title: ReactNode;
      /** Body class, e.g. `why` for prose. */
      bodyClassName: string;
    }
  | {
      variant: "right";
      header: ReactNode;
    }
);

export function Panel(props: PanelProps) {
  if (props.variant === "right") {
    return (
      <aside className="right" style={props.style}>
        <div className="rhead">{props.header}</div>
        <div className="rbody">{props.children}</div>
      </aside>
    );
  }
  return (
    <div className="box" style={props.style}>
      <div className="bh">
        <Ico name={props.icon} className="sm" />
        {props.title}
      </div>
      <div className={props.bodyClassName}>{props.children}</div>
    </div>
  );
}
