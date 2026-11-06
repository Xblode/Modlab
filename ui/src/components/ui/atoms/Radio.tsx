import type { ComponentProps, CSSProperties } from "react";

/**
 * A round `.chk` sibling — same overlay-native-input trick as `Checkbox`
 * (real `<input type="radio">` underneath, invisible, so it stays focusable
 * and keyboard-toggleable) but round and dot-filled instead of checked.
 */
const OVERLAY: CSSProperties = {
  position: "absolute",
  top: -1,
  left: -1,
  right: -1,
  bottom: -1,
  margin: 0,
  opacity: 0,
  cursor: "inherit",
};

export function Radio({
  checked,
  onSelect,
  name,
  className,
  ...props
}: Omit<ComponentProps<"input">, "checked" | "onChange" | "type"> & {
  checked: boolean;
  onSelect: () => void;
  name: string;
}) {
  const classNames = ["radio", checked ? "on" : "", className ?? ""]
    .filter(Boolean)
    .join(" ");
  return (
    <span className={classNames} style={{ position: "relative" }}>
      <input
        type="radio"
        name={name}
        checked={checked}
        onChange={onSelect}
        style={OVERLAY}
        {...props}
      />
      {checked ? <span className="radio-dot" /> : null}
    </span>
  );
}
