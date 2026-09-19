import type { ComponentProps } from "react";

/**
 * The maquette's two pill families, as one primitive each. Both are pure
 * presentation: the caller decides *what* the pill says, the tone only picks
 * the colour rule already defined in `shell.css`.
 */

/** One tone per `.badge.*` rule in `shell.css`. */
export type BadgeTone = "win" | "lose" | "both" | "plug" | "upd" | "bad" | "root";

export function Badge({
  tone,
  className,
  children,
  ...props
}: ComponentProps<"span"> & { tone: BadgeTone }) {
  const classNames = ["badge", tone, className ?? ""].filter(Boolean).join(" ");
  return (
    <span className={classNames} {...props}>
      {children}
    </span>
  );
}

/** One channel per `.chan.*` rule in `shell.css`. */
export type Channel = "stable" | "beta" | "alpha";

export function ChannelBadge({
  channel,
  className,
  children,
  ...props
}: ComponentProps<"span"> & { channel: Channel }) {
  const classNames = ["chan", channel, className ?? ""].filter(Boolean).join(" ");
  return (
    <span className={classNames} {...props}>
      {children}
    </span>
  );
}
