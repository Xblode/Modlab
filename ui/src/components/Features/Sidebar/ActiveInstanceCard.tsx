import { Ico } from "../../../Ico";
import { ChannelBadge } from "../../ui";

/**
 * The active instance, pinned at the top of the explorer: what it is, whether
 * it is up to date, and the two things one does with it — play and share.
 * The revision and the save state have no Core source yet, so they stay inert.
 */
export function ActiveInstanceCard({
  name,
  running,
  enabledCount,
  totalCount,
  onPlay,
  onShare,
}: {
  name: string;
  running: boolean;
  enabledCount: number;
  totalCount: number;
  onPlay: () => void;
  onShare: () => void;
}) {
  return (
    <div className="side-instance">
      <div className="lo">
        <div className="lo-top">
          <button className="lo-name" type="button">
            <span className="fade">{name}</span>
          </button>
          <ChannelBadge channel="stable">stable</ChannelBadge>
        </div>
        <div className="lo-row">
          <button className="vsel" type="button">
            r.—<span className="d ok">✓</span>
            <span className="hd ok" />
            <span className="spacer" />
            <Ico name="chev" />
          </button>
          <button className="btn save-btn" type="button" disabled>
            <Ico name="db" className="sm" />À jour
          </button>
        </div>
        <button
          className={`play-btn ${running ? "stop" : ""}`}
          type="button"
          onClick={onPlay}
        >
          <Ico name={running ? "x" : "play"} />
          {running ? "Arrêter" : "Jouer"}
        </button>
        <button className="share-btn" type="button" onClick={onShare}>
          <Ico name="share" />
          Partager
        </button>
        <div className="st">
          <span>
            <b>{enabledCount}</b> mods
          </span>
          <span>
            <b>{totalCount}</b> dans le loadout
          </span>
        </div>
      </div>
    </div>
  );
}
