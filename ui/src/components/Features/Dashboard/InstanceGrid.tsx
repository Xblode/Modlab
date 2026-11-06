import type { CSSProperties } from "react";
import type { LoadoutDto } from "../../../api";
import { CHANNEL_LETTER } from "../../../channel";
import { initials } from "../../../gameChrome";
import { Ico } from "../../../Ico";
import { Avatar, ChannelBadge, FavoriteButton } from "../../ui";

/**
 * The instance cards of a game's dashboard, plus the card that creates one.
 * Order is decided by the caller — this only renders it.
 */
export function InstanceGrid({
  loadouts,
  activeLoadoutId,
  cardStyle,
  isFavorite,
  onToggleFavorite,
  onPickLoadout,
  onCreateLoadout,
}: {
  loadouts: LoadoutDto[];
  activeLoadoutId: number | null;
  /** Carries the game's `--c1g` / `--c2g` gradient down to each thumbnail. */
  cardStyle: CSSProperties;
  isFavorite: (loadoutId: number) => boolean;
  onToggleFavorite: (loadoutId: number) => void;
  onPickLoadout: (loadoutId: number) => void;
  onCreateLoadout: () => void;
}) {
  return (
    <div className="inst-grid">
      {loadouts.map((l) => {
        const cur = l.id === activeLoadoutId;
        const fav = isFavorite(l.id);
        return (
          <div
            key={l.id}
            className={`inst-card ${cur ? "cur" : ""}`}
            role="button"
            tabIndex={0}
            style={cardStyle}
            onClick={() => onPickLoadout(l.id)}
            onKeyDown={(e) => {
              if (e.key === "Enter" || e.key === " ") {
                e.preventDefault();
                onPickLoadout(l.id);
              }
            }}
          >
            <Avatar variant="thumb" label={initials(l.name)} />
            <span className="ic-body">
              <span className="ic-top">
                <span className="t fade">{l.name}</span>
                <ChannelBadge channel={l.channel} className="xs">
                  {l.channel}
                </ChannelBadge>
              </span>
              <span className="ic-meta fade">
                <span className="mono">
                  {CHANNEL_LETTER[l.channel]}.{l.revision}
                </span>
                <span>{l.modCount} mods</span>
                {cur ? <span className="on">● active</span> : null}
              </span>
            </span>
            <FavoriteButton on={fav} onToggle={() => onToggleFavorite(l.id)} />
          </div>
        );
      })}
      <div
        className="inst-card add"
        role="button"
        tabIndex={0}
        onClick={onCreateLoadout}
        onKeyDown={(e) => {
          if (e.key === "Enter" || e.key === " ") {
            e.preventDefault();
            onCreateLoadout();
          }
        }}
      >
        <Ico name="box" className="sm" />
        <span>Nouvelle instance</span>
      </div>
    </div>
  );
}
