import type { CSSProperties } from "react";
import { Ico } from "../../../Ico";
import { Input } from "../atoms/Input";

/**
 * A search field built on the maquette's `.composer` pill.
 *
 * `compact` is the small in-panel form of that pill — the maquette has no
 * separate class for it, so the overrides it needs live here, in one place,
 * instead of being retyped at each call site. The input's own reset repeats what
 * `.composer input` already declares; it is kept verbatim so the field is
 * byte-identical to what the shell rendered before, and is the one thing to
 * drop if `ui-designer` ever gives the compact pill a class of its own.
 */
const COMPACT: CSSProperties = {
  height: 28,
  padding: "0 10px",
  borderRadius: 6,
  fontSize: 12,
  boxShadow: "none",
  background: "var(--c2)",
};

const INPUT_RESET: CSSProperties = {
  background: "none",
  border: 0,
  color: "var(--t1)",
  outline: "none",
  flex: 1,
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
        value={value}
        onChange={(e) => onChange(e.target.value)}
        placeholder={placeholder}
        style={compact ? INPUT_RESET : undefined}
      />
    </div>
  );
}
