import type { Channel } from "./api";

/**
 * The one-letter version prefix a channel renders as — `r.12`, `b.3`, `a.1`.
 * Shared between `ActiveInstanceCard` and `SidebarFooter` so the two can
 * never disagree about what a given channel is called.
 */
export const CHANNEL_LETTER: Record<Channel, string> = {
  stable: "r",
  beta: "b",
  alpha: "a",
};
