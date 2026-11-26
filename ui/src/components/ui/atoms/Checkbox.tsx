import type { ComponentProps, CSSProperties } from "react";
import { Ico } from "../../../Ico";

/**
 * The maquette's `.chk` square, driven by a real `<input type="checkbox">`.
 *
 * The painted result is the maquette's, to the pixel: the square and its check
 * glyph are unchanged. What changed is underneath — the control is now a
 * native checkbox rather than a `<span role="checkbox">`, so it is focusable,
 * togglable with Space and announced with its real state. The input is laid
 * over the square (border included, hence the -1px) at zero opacity.
 *
 * `default` keeps the DS / settings look (filled accent + check glyph when on).
 * `mod` is the denser ModTable enable control: framed case with a soft accent
 * wash and a small inner accent square when on (Radio-like, no checkmark).
 */
const OVERLAY: CSSProperties = {
  position: "absolute",
  top: -1,
  left: -1,
  right: -1,
  bottom: -1,
  margin: 0,
  opacity: 0,
  // Never impose a cursor of its own: the row it sits in decides.
  cursor: "inherit",
};

export type CheckboxVariant = "default" | "mod";

export function Checkbox({
  checked,
  onToggle,
  className,
  stopPropagation = false,
  variant = "default",
  ...props
}: Omit<ComponentProps<"input">, "checked" | "onChange" | "type"> & {
  checked: boolean;
  onToggle: () => void;
  /** Keep the click from reaching a clickable row underneath. */
  stopPropagation?: boolean;
  variant?: CheckboxVariant;
}) {
  const classNames = [
    "chk",
    variant !== "default" ? variant : "",
    checked ? "on" : "",
    className ?? "",
  ]
    .filter(Boolean)
    .join(" ");
  return (
    <span
      className={classNames}
      style={{ position: "relative" }}
      onClick={stopPropagation ? (e) => e.stopPropagation() : undefined}
    >
      <input
        type="checkbox"
        checked={checked}
        onChange={onToggle}
        style={OVERLAY}
        {...props}
      />
      {variant === "mod" ? (
        <span className="chk-mod-dot" aria-hidden />
      ) : checked ? (
        <Ico name="check" className="sm" />
      ) : null}
    </span>
  );
}
