import { forwardRef, type ComponentProps } from "react";
import { Ico } from "../../../Ico";
import { tip } from "../molecules/Tooltip";

/**
 * `forwardRef` on purpose: a `Pop`/`MenuPicker` anchored to one of these
 * needs a real DOM node to measure, and a plain function component would
 * silently drop a `ref` passed to it rather than forward it to the button.
 */
export const IconButton = forwardRef<
  HTMLButtonElement,
  ComponentProps<"button"> & {
    name: string;
    /** Tooltip text, and the accessible name (the icon carries no text). */
    title?: string;
    /** Only when a real key handler exists in the code. */
    shortcut?: string;
    variant?: "sm" | "ghost" | "sm-ghost";
  }
>(function IconButton(
  { className, name, title, shortcut, "aria-label": ariaLabel, variant, ...props },
  ref,
) {
  const variants = variant ? [variant === "sm" ? "sm" : variant] : [];
  const classNames = ["tb", ...variants, className ?? ""].filter(Boolean).join(" ");
  return (
    <button
      ref={ref}
      className={classNames}
      type="button"
      {...(title ? tip(title, shortcut) : {})}
      {...(ariaLabel ? { "aria-label": ariaLabel } : {})}
      {...props}
    >
      <Ico name={name} className={variant === "sm" ? "sm" : undefined} />
    </button>
  );
});
