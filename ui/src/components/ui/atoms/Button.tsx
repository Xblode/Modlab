import { type ComponentProps } from "react";


export type ButtonVariant = "default" | "ghost" | "sm" | "sm-ghost";

export function Button({
  className,
  variant,
  children,
  ...props
}: ComponentProps<"button"> & {
  variant?: ButtonVariant;
}) {
  const variants: string[] = variant && variant !== "default" ? variant.split("-") : [];
  const classNames = ["btn", ...variants, className ?? ""].filter(Boolean).join(" ");
  return (
    <button className={classNames} type="button" {...props}>
      {children}
    </button>
  );
}
