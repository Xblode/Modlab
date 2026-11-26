import type { ComponentProps } from "react";
import "../../Features/Sidebar/sidebar-rows.css";

/**
 * Nine squares, three by three, lighting up in sequence. It knows nothing
 * about a game or an instance: a caller shows it only while something is in
 * progress, in place of the status mark it stands in for.
 */
export function PixelLoader({ className, ...props }: ComponentProps<"span">) {
  const classNames = ["pixel-loader", className ?? ""].filter(Boolean).join(" ");
  return (
    <span className={classNames} aria-hidden="true" {...props}>
      <span />
      <span />
      <span />
      <span />
      <span />
      <span />
      <span />
      <span />
      <span />
    </span>
  );
}
