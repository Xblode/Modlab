import type { ComponentProps } from "react";
import { Ico } from "../../../Ico";

export function IconButton({
  className,
  name,
  title,
  "aria-label": ariaLabel,
  variant,
  ...props
}: ComponentProps<"button"> & {
  name: string;
  title?: string;
  variant?: "sm" | "ghost" | "sm-ghost";
}) {
  const variants = variant ? [variant === "sm" ? "sm" : variant] : [];
  const classNames = ["tb", ...variants, className ?? ""].filter(Boolean).join(" ");
  return (
    <button
      className={classNames}
      type="button"
      title={title}
      aria-label={ariaLabel ?? title}
      {...props}
    >
      <Ico name={name} className={variant === "sm" ? "sm" : undefined} />
    </button>
  );
}
