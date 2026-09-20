import type { ComponentProps, CSSProperties } from "react";

/**
 * A toggle switch, driven by a real `<input type="checkbox">` — same pattern
 * as `Checkbox`: the painted pill/thumb is CSS, the input underneath is what
 * makes it focusable, togglable with Space and announced with its real state.
 */
const OVERLAY: CSSProperties = {
  position: "absolute",
  inset: -1,
  margin: 0,
  opacity: 0,
  cursor: "inherit",
};

export function Switch({
  checked,
  onToggle,
  disabled,
  className,
  ...props
}: Omit<ComponentProps<"input">, "checked" | "onChange" | "type"> & {
  checked: boolean;
  onToggle: () => void;
}) {
  const classNames = [
    "switch",
    checked ? "on" : "",
    disabled ? "disabled" : "",
    className ?? "",
  ]
    .filter(Boolean)
    .join(" ");
  return (
    <span className={classNames} style={{ position: "relative" }}>
      <input
        type="checkbox"
        checked={checked}
        disabled={disabled}
        onChange={onToggle}
        style={OVERLAY}
        {...props}
      />
      <span className="thumb" />
    </span>
  );
}
