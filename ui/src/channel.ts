import type { Channel } from "./api";

/** Stable, then beta, then alpha — the order every channel menu uses. */
export const CHANNEL_ORDER = ["stable", "beta", "alpha"] as const satisfies readonly Channel[];

/** Label, subtitle and icon for one loadout channel. One table for every menu. */
export const CHANNELS: Record<Channel, { label: string; sub: string; icon: string }> = {
  stable: { label: "Stable", sub: "jouable, ne bouge plus", icon: "shield" },
  beta: { label: "Beta", sub: "en test, peut casser", icon: "zap" },
  alpha: { label: "Alpha", sub: "expérimental, jetable", icon: "tri" },
};

/**
 * The one-letter version prefix a channel renders as — `r.12`, `b.3`, `a.1`.
 * Shared by every place that prints a revision, so they agree on the letter.
 */
export const CHANNEL_LETTER: Record<Channel, string> = {
  stable: "r",
  beta: "b",
  alpha: "a",
};

/** The channel's label, read from `CHANNELS` so menus and facts stay in step. */
export const CHANNEL_NAME: Record<Channel, string> = {
  stable: CHANNELS.stable.label,
  beta: CHANNELS.beta.label,
  alpha: CHANNELS.alpha.label,
};

/** `r.12`, `b.3`, `a.0` — the revision token every surface prints. */
export function revisionLabel(channel: Channel, revision: number): string {
  return `${CHANNEL_LETTER[channel]}.${revision}`;
}
