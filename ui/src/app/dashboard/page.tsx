import { useEffect, useState } from "react";
import {
  getSkseStatus,
  type DetectedGame,
  type GamePaths,
  type LoadoutDto,
  type SkseStatus,
} from "../../api";
import {
  PageHero,
  type HeaderFact,
} from "../../components/Features/Dashboard/PageHero";
import { GamePathsSection } from "../../components/Features/Dashboard/GamePathsSection";
import { VanillaControlSection } from "../../components/Features/Dashboard/VanillaControlSection";
import { useGamePaths } from "../../hooks/useGamePaths";

type Props = {
  game: DetectedGame;
  loadouts: LoadoutDto[];
  loadoutId: number | null;
  onUnwired: (label: string) => void;
};

/**
 * What to show next to the game name.
 *
 * Skyrim's build comes from `skse_status.gameVersion`, which is
 * `GetFileVersionInfo` on `SkyrimSE.exe` (`major.minor.build`).
 * Minecraft has no instance version on the snapshot, so it stays a dash.
 * No install path is a short empty state — never a made-up number.
 */
type GameVersionView =
  | { kind: "pending" }
  | { kind: "empty" }
  | { kind: "value"; text: string; title: string };

function describeGameVersion(
  game: DetectedGame,
  paths: GamePaths | null,
  pathsError: string | null,
  skse: SkseStatus | null | undefined,
): GameVersionView {
  const read = skse?.gameVersion?.trim() ?? "";
  if (read) {
    return {
      kind: "value",
      text: read,
      title: "Version lue sur l’exécutable du jeu",
    };
  }

  if (paths === null && !pathsError) return { kind: "pending" };
  if (pathsError) return { kind: "value", text: "—", title: "Version non lue" };
  if (!paths?.gameDir.effective) return { kind: "empty" };

  if (game.capabilities.scriptExtender === "SKSE") {
    if (skse === undefined) return { kind: "pending" };
    return {
      kind: "value",
      text: "—",
      title: skse ? "Version de l’exécutable non lue" : "Version non lue",
    };
  }
  return { kind: "value", text: "—", title: "Version d’instance non connue" };
}

function versionFact(view: GameVersionView): HeaderFact {
  if (view.kind === "pending") {
    return {
      id: "version",
      icon: "disc",
      label: "Version",
      value: "lecture…",
      title: "Lecture de la version du jeu",
      muted: true,
    };
  }
  if (view.kind === "empty") {
    return {
      id: "version",
      icon: "disc",
      label: "Version",
      value: "introuvable",
      title: "Aucun chemin de jeu",
      muted: true,
    };
  }
  const unknown = view.text === "—";
  return {
    id: "version",
    icon: "disc",
    label: "Version",
    value: view.text,
    title: view.title,
    muted: unknown,
  };
}

/** Last folder name. The full path stays on the fact’s title. */
function summarizeInstallPath(path: string): string {
  const parts = path.split(/[\\/]/).filter((part) => part.length > 0);
  return parts[parts.length - 1] ?? path;
}

function installFact(paths: GamePaths | null, pathsError: string | null): HeaderFact {
  if (paths === null && !pathsError) {
    return {
      id: "install",
      icon: "folder",
      label: "Installation",
      value: "lecture…",
      title: "Lecture du chemin d’installation",
      muted: true,
    };
  }
  if (pathsError) {
    return {
      id: "install",
      icon: "folder",
      label: "Installation",
      value: "non lu",
      title: pathsError,
      muted: true,
    };
  }
  const path = paths?.gameDir.effective ?? null;
  if (!path) {
    return {
      id: "install",
      icon: "folder",
      label: "Installation",
      value: "introuvable",
      title: "Aucun chemin d’installation",
      muted: true,
    };
  }
  return {
    id: "install",
    icon: "folder",
    label: "Installation",
    value: <span className="fade">{summarizeInstallPath(path)}</span>,
    title: path,
  };
}

export default function DashboardPage({
  game,
  loadouts,
  loadoutId,
  onUnwired,
}: Props) {
  const gamePaths = useGamePaths(game.id, loadoutId);
  const tracksSkse = game.capabilities.scriptExtender === "SKSE";
  const [skse, setSkse] = useState<SkseStatus | null | undefined>(tracksSkse ? undefined : null);

  useEffect(() => {
    if (!tracksSkse) {
      setSkse(null);
      return;
    }
    let cancelled = false;
    setSkse(undefined);
    getSkseStatus()
      .then((status) => {
        if (!cancelled) setSkse(status);
      })
      .catch(() => {
        if (!cancelled) setSkse(null);
      });
    return () => {
      cancelled = true;
    };
  }, [tracksSkse, game.id]);

  const gameVersion = describeGameVersion(game, gamePaths.paths, gamePaths.error, skse);
  const facts: HeaderFact[] = [
    versionFact(gameVersion),
    {
      id: "instances",
      icon: "layers",
      label: "Instances",
      value: String(loadouts.length),
      title: loadouts.length === 1 ? "1 instance" : `${loadouts.length} instances`,
    },
    {
      id: "deploy",
      icon: "branch",
      label: "Déploiement",
      value: game.capabilities.deploymentLabel,
    },
    installFact(gamePaths.paths, gamePaths.error),
  ];

  return (
    <div className="content">
      <div className="dash">
        <PageHero name={game.name} facts={facts} />

        <div className="dpad">
          <GamePathsSection
            gameId={game.id}
            paths={gamePaths.paths}
            error={gamePaths.error}
            saveManaged={gamePaths.saveManaged}
            resetManaged={gamePaths.resetManaged}
            saveIni={gamePaths.saveIni}
            resetIni={gamePaths.resetIni}
            onError={onUnwired}
          />
          <VanillaControlSection gameId={game.id} />
        </div>
      </div>
    </div>
  );
}
