import { forwardRef, type ComponentProps } from "react";
import { Ico } from "../../../Ico";

/**
 * `forwardRef` on purpose: a `Pop`/`MenuPicker` anchored to one of these
 * needs a real DOM node to measure, and a plain function component would
 * silently drop a `ref` passed to it rather than forward it to the button.
 */
export const IconButton = forwardRef<
  HTMLButtonElement,
  ComponentProps<"button"> & {
    name: string;
    title?: string;
    variant?: "sm" | "ghost" | "sm-ghost";
  }
>(function IconButton(
  { className, name, title, "aria-label": ariaLabel, variant, ...props },
  ref,
) {
  const variants = variant ? [variant === "sm" ? "sm" : variant] : [];
  const classNames = ["tb", ...variants, className ?? ""].filter(Boolean).join(" ");
  return (
    <button
      ref={ref}
      className={classNames}
      type="button"
      title={title}
      aria-label={ariaLabel ?? title}
      {...props}
    >
      <Ico name={name} className={variant === "sm" ? "sm" : undefined} />
    </button>
  );
});
