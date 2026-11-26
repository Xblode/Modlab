const NBSP = "\u00a0";

const UNITS = ["Ko", "Mo", "Go"] as const;

/**
 * File length in French, base 1024.
 * `840 octets`, `12,3 Ko`, `12,3 Mo`, `1,2 Go`. The space before the unit
 * does not break.
 */
export function formatBytes(bytes: number | null | undefined): string {
  if (bytes == null || !Number.isFinite(bytes) || bytes < 0) return "—";
  if (bytes < 1024) return `${Math.round(bytes)}${NBSP}octets`;
  let value = bytes / 1024;
  let unit = 0;
  while (value >= 1024 && unit < UNITS.length - 1) {
    value /= 1024;
    unit += 1;
  }
  const text = value.toLocaleString("fr-FR", {
    minimumFractionDigits: 1,
    maximumFractionDigits: 1,
  });
  return `${text}${NBSP}${UNITS[unit]}`;
}

/** Exact byte count, for a tooltip next to the shortened figure. */
export function formatExactBytes(bytes: number): string {
  const text = Math.round(bytes).toLocaleString("fr-FR", { maximumFractionDigits: 0 });
  return `${text}${NBSP}octets`;
}
