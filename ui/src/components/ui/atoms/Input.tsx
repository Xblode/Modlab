import type { ComponentProps } from "react";

/**
 * Text input primitive.
 *
 * `field` is the maquette's standalone `.modal-input` box. `ghost` is an input
 * that lives inside an already-styled shell — `.composer`, `.cpk .in`, the
 * chat's `.in` — where the surrounding rule is what styles the field, and the
 * input itself must add nothing (transparent, no border/background).
 */
export type InputVariant = "ghost" | "field";

export function Input({
  variant = "ghost",
  className,
  ...props
}: ComponentProps<"input"> & { variant?: InputVariant }) {
  const classNames = [
    "input",
    variant === "field" ? "modal-input" : "ghost",
    className ?? "",
  ]
    .filter(Boolean)
    .join(" ");
  return <input className={classNames} {...props} />;
}
