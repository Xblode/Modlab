import type { Channel } from "../../../api";
import { CHANNEL_LETTER } from "../../../channel";
import { Ico } from "../../../Ico";

/**
 * The sidebar's status line. Left: what concerns the game (its short name and
 * the deployment backend its adapter declares — or that the game is running,
 * which replaces both). Right: the current instance, name then revision.
 */
export function SidebarFooter({
  running,
  gameShort,
  loadoutName,
  channel,
  revision,
  deploymentLabel,
}: {
  running: boolean;
  gameShort: string | undefined;
  loadoutName: string | undefined;
  channel: Channel | undefined;
  /** `undefined` while not loaded yet — rendered as no revision at all,
   * never a guessed number. */
  revision: number | undefined;
  deploymentLabel: string | undefined;
}) {
  return (
    <div className="side-foot">
      {running ? (
        <span className="fi">
          <Ico name="play" />
          Skyrim en cours
        </span>
      ) : (
        <>
          <span className="fi" title="Jeu">
            <Ico name="box" />
            {gameShort ?? "—"}
          </span>
          <span className="fi" title="Déploiement">
            <Ico name="layers" />
            {deploymentLabel ?? "—"}
          </span>
        </>
      )}
      {loadoutName ? (
        <span className="fi end" title="Instance en cours">
          <span className="fade">{loadoutName}</span>
          {channel && revision !== undefined ? (
            <span className="mono">
              {CHANNEL_LETTER[channel]}.{revision}
            </span>
          ) : null}
        </span>
      ) : null}
    </div>
  );
}
