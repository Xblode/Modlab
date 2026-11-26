import { type ComponentProps } from "react";
import { tip } from "../molecules/Tooltip";

export type ButtonVariant =
  | "default"
  | "ghost"
  | "sm"
  | "sm-ghost"
  | "action"
  | "round"
  | "ghost-round";

export function Button({
  className,
  variant,
  children,
  title,
  shortcut,
  "aria-label": ariaLabel,
  ...props
}: ComponentProps<"button"> & {
  variant?: ButtonVariant;
  /** Tooltip text via the shared `data-tip` layer (same as IconButton). */
  title?: string;
  /** Only when a real key handler exists in the code. */
  shortcut?: string;
}) {
  const variants: string[] = variant && variant !== "default" ? variant.split("-") : [];
  const classNames = ["btn", ...variants, className ?? ""].filter(Boolean).join(" ");
  return (
    <button
      className={classNames}
      type="button"
      {...(title ? tip(title, shortcut) : {})}
      {...(ariaLabel ? { "aria-label": ariaLabel } : {})}
      {...props}
    >
      {children}
    </button>
  );
}
