import type { ComponentProps } from "react";

/**
 * Status pellet (Flags / table). Soft dark fill: tint of the tone into `--c1`,
 * foreground a lighter mix of the same hue — same recipe as `Chip`. No border.
 *
 * `variant="ghost"` is a quieter pellet for category icons and neutral markers.
 */

/** One tone per `.badge.*` rule in `shell.css`. */
export type BadgeTone =
  | "win"
  | "lose"
  | "both"
  | "plug"
  | "upd"
  | "bad"
  | "root"
  | "archived"
  | "downloading";

export type BadgeVariant = "ghost";

export function Badge({
  tone,
  variant,
  className,
  children,
  ...props
}: ComponentProps<"span"> & { tone?: BadgeTone; variant?: BadgeVariant }) {
  const classNames = ["badge", variant, tone, className ?? ""].filter(Boolean).join(" ");
  return (
    <span className={classNames} {...props}>
      {children}
    </span>
  );
}
