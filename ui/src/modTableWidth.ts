import { useEffect, useLayoutEffect, useRef, useState, type RefObject } from "react";

/**
 * Mod table responsiveness from the `.content` scrollport width (ResizeObserver).
 * T1 (compact): Cat. icon-only (badge), tighter Jeux. État is icon-only at every tier.
 * T2 (narrow): hide Taille, Date, Source — Load. stays when `data-loader-col="1"` (Minecraft).
 *
 * Jeux is version-only (~76px). Load. (~32px) is a separate track toggled in CSS via
 * `data-loader-col` on `.virt-grid` (Skyrim omits the cells entirely — no ghost column).
 *
 * Cat. track: T0 = max content width (measured badge+label / header); T1/T2 = badge pellet.
 */

/** Scrollport width at which T1 starts (still all columns). */
export const MOD_TABLE_T1_PX = 1120;

/** Below this, T2 hides the last three columns. */
export const MOD_TABLE_T2_PX = 920;

/** Cat. column when reduced (T1/T2): icon-only ghost badge, same ballpark as État. */
export const MOD_TABLE_CAT_BADGE_PX = 40;

export type ModTableWidthTier = 0 | 1 | 2;

export function modTableWidthTier(width: number): ModTableWidthTier {
  if (width < MOD_TABLE_T2_PX) return 2;
  if (width < MOD_TABLE_T1_PX) return 1;
  return 0;
}

/** Recomputes when the mods list scrollport is resized (sheet narrowed, utility column, window). */
export function useModTableWidthTier(scrollRef: RefObject<HTMLDivElement | null>): ModTableWidthTier {
  const [tier, setTier] = useState<ModTableWidthTier>(0);

  useEffect(() => {
    const el = scrollRef.current;
    if (!el) return;
    const sync = () => setTier(modTableWidthTier(el.clientWidth));
    sync();
    const observer = new ResizeObserver(sync);
    observer.observe(el);
    return () => observer.disconnect();
  }, [scrollRef]);

  return tier;
}

/**
 * Measures the widest Cat. face (badge + label) so every virtualized row can
 * share one track width in T0. Host must render one child per candidate face.
 */
export function useModTableCatContentPx(
  measureKey: string,
): { measureRef: RefObject<HTMLDivElement | null>; contentPx: number | null } {
  const measureRef = useRef<HTMLDivElement | null>(null);
  const [contentPx, setContentPx] = useState<number | null>(null);

  useLayoutEffect(() => {
    const root = measureRef.current;
    if (!root) return;
    let max = 0;
    for (const child of Array.from(root.children)) {
      max = Math.max(max, (child as HTMLElement).offsetWidth);
    }
    // `.row > div` / `.gh > div` horizontal padding is 8px each side.
    const next = Math.ceil(max + 16);
    setContentPx((prev) => (prev === next ? prev : next));
  }, [measureKey]);

  return { measureRef, contentPx };
}

/** CSS length for `--mod-col-cat` on `.virt-grid`. */
export function modTableCatTrack(
  tier: ModTableWidthTier,
  contentPx: number | null,
): string {
  if (tier >= 1) return `${MOD_TABLE_CAT_BADGE_PX}px`;
  if (contentPx != null && contentPx > 0) return `${contentPx}px`;
  return "max-content";
}

const MOD_COLS = ["var(--mod-cols)", "var(--mod-cols-t1)", "var(--mod-cols-t2)"] as const;

export function modTableColumnTemplate(tier: ModTableWidthTier): string {
  return MOD_COLS[tier];
}
