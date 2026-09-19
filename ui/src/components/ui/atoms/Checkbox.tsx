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

export function Checkbox({
  checked,
  onToggle,
  className,
  stopPropagation = false,
  ...props
}: Omit<ComponentProps<"input">, "checked" | "onChange" | "type"> & {
  checked: boolean;
  onToggle: () => void;
  /** Keep the click from reaching a clickable row underneath. */
  stopPropagation?: boolean;
}) {
  const classNames = ["chk", checked ? "on" : "", className ?? ""]
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
      {checked ? <Ico name="check" className="sm" /> : null}
    </span>
  );
}
