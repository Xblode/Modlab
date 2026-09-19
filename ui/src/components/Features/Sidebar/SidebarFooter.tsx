import { Ico } from "../../../Ico";

/**
 * The sidebar's status line: what the shell is pointed at, and which
 * deployment backend the game's adapter declares — or that the game is
 * running, which replaces both.
 */
export function SidebarFooter({
  running,
  gameShort,
  loadoutName,
  deploymentLabel,
}: {
  running: boolean;
  gameShort: string | undefined;
  loadoutName: string | undefined;
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
          <span className="fi fade" title="Contexte">
            <Ico name="box" />
            {gameShort ?? "—"}
            {loadoutName ? ` · ${loadoutName}` : ""}
          </span>
          <span className="spacer" />
          <span className="fi" title="Backend de déploiement">
            <Ico name="layers" />
            {deploymentLabel ?? "—"}
          </span>
        </>
      )}
    </div>
  );
}
