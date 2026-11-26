import { Ico } from "../../../Ico";

/**
 * Soft dark chip (composer modes, @mod tags, etc.) — not a status Badge.
 * Fill: tint of `--chip-base` into `--c1`; label, icon and dismiss share a
 * lighter mix of the same hue toward white.
 */
export type ChipTone = "plan" | "debug" | "multi" | "mod";

const TONE_ICON: Record<ChipTone, string> = {
  plan: "note",
  debug: "tool",
  multi: "layers",
  mod: "box",
};

export function Chip({
  tone,
  label,
  icon,
  title,
  onDismiss,
}: {
  tone: ChipTone;
  label: string;
  /** Defaults to a tone-specific glyph. */
  icon?: string;
  /** Native tooltip; defaults to `label`. */
  title?: string;
  onDismiss?: () => void;
}) {
  const glyph = icon ?? TONE_ICON[tone];
  return (
    <span className={`chip chip--${tone}`} title={title ?? label}>
      <Ico name={glyph} className="sm" aria-hidden />
      <span className="chip-label">{label}</span>
      {onDismiss ? (
        <button
          type="button"
          className="chip-x"
          aria-label={`Retirer ${label}`}
          onClick={onDismiss}
        >
          <Ico name="x" className="sm" />
        </button>
      ) : null}
    </span>
  );
}
