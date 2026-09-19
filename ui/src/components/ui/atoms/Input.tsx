import type { ComponentProps } from "react";

/**
 * Text input primitive.
 *
 * `field` is the maquette's standalone `.modal-input` box. `bare` is an input
 * that lives inside an already-styled shell — `.composer`, `.cpk .in`, the
 * chat's `.in` — where the surrounding rule is what styles the field, and the
 * input itself must add nothing.
 */
export type InputVariant = "bare" | "field";

export function Input({
  variant = "bare",
  className,
  ...props
}: ComponentProps<"input"> & { variant?: InputVariant }) {
  const classNames = [variant === "field" ? "modal-input" : "", className ?? ""]
    .filter(Boolean)
    .join(" ");
  return <input className={classNames || undefined} {...props} />;
}
