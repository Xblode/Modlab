import type { CSSProperties } from "react";
import { Ico } from "../../../Ico";
import { Input } from "../atoms/Input";

/**
 * A search field built on the maquette's `.composer` pill.
 *
 * `compact` is the small in-panel form of that pill — the maquette has no
 * separate class for it, so the overrides it needs live here, in one place,
 * instead of being retyped at each call site. The inner input uses
 * `variant="ghost"` (parent shell carries the frame).
 */
const COMPACT: CSSProperties = {
  height: "var(--ctl)",
  padding: "0 10px",
  borderRadius: 6,
  fontSize: "var(--fs-md)",
  boxShadow: "none",
  background: "var(--c2)",
};

export function SearchInput({
  value,
  onChange,
  placeholder,
  compact = false,
}: {
  value: string;
  onChange: (value: string) => void;
  placeholder?: string;
  compact?: boolean;
}) {
  return (
    <div className="composer" style={compact ? COMPACT : undefined}>
      <Ico name="search" className="sm" />
      <Input
        variant="ghost"
        value={value}
        onChange={(e) => onChange(e.target.value)}
        placeholder={placeholder}
      />
    </div>
  );
}
