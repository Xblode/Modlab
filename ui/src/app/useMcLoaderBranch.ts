import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { LoadoutDto, McLoaderId, McLoaderStatus } from "../api";
import {
  installBranchMcLoader,
  listInstalledMcGameVersions,
  listInstalledMcLoaderVersions,
  listMcGameVersions,
  listMcLoaderVersions,
  mcLoaderStatus,
  setBranchMcGameVersion,
  setBranchMcLoader,
  setBranchMcLoaderVersion,
} from "../api";
import type { FactMenuItem, HeaderFact } from "../components/Features/Dashboard/PageHero";
import { isTauriShell } from "../api";

const LOADERS: { id: McLoaderId; label: string }[] = [
  { id: "vanilla", label: "Vanilla" },
  { id: "fabric", label: "Fabric" },
  { id: "forge", label: "Forge" },
  { id: "neoforge", label: "NeoForge" },
  { id: "quilt", label: "Quilt" },
];

const LOADER_LABELS: Record<string, string> = Object.fromEntries(
  LOADERS.map((l) => [l.id, l.label]),
);

const INSTALLED_BADGE = "Installé";

type Deps = {
  loadout: LoadoutDto | null;
  onRefresh: () => Promise<void>;
  onNotice: (message: string) => void;
  onError: (message: string) => void;
  /**
   * After a successful CTA « Installer … », run once the same Jouer path
   * (JVM session). Failures toast via onError — no retry loop.
   */
  onAfterLoaderInstall?: () => Promise<void>;
};

/** Primary toolbar CTA that temporarily replaces the branch select. */
export type ToolbarPrimaryAction = {
  kind: "install-loader";
  /** French label, e.g. « Installer NeoForge » / « Réinstaller NeoForge ». */
  label: string;
  installing: boolean;
  onInstall: () => void;
};

export type McLoaderBranchState = {
  facts: HeaderFact[];
  /** When set, ShellToolbar shows this instead of the branch select. */
  primaryAction: ToolbarPrimaryAction | null;
};

export function useMcLoaderFacts({
  loadout,
  onRefresh,
  onNotice,
  onError,
  onAfterLoaderInstall,
}: Deps): McLoaderBranchState {
  const [status, setStatus] = useState<McLoaderStatus | null>(null);
  const [installing, setInstalling] = useState(false);
  const [gameVersions, setGameVersions] = useState<string[] | null>(null);
  const [loaderVersions, setLoaderVersions] = useState<string[]>([]);
  /** Key the published loader-version list currently matches (`loader|mc`). */
  const [loaderVersionsKey, setLoaderVersionsKey] = useState<string | null>(null);
  const [installedGameVersions, setInstalledGameVersions] = useState<string[]>([]);
  const [installedLoaderVersions, setInstalledLoaderVersions] = useState<string[]>([]);
  /** Loaders that already have at least one build for the current MC version. */
  const [installedLoaders, setInstalledLoaders] = useState<Set<McLoaderId>>(
    () => new Set(),
  );

  const loadoutId = loadout?.id ?? null;
  const loader = (loadout?.mcLoader ?? null) as McLoaderId | null;
  const mcVersion = loadout?.mcGameVersion ?? null;
  const loaderVersion = loadout?.mcLoaderVersion ?? null;

  const selectionKey = `${loader ?? ""}|${mcVersion ?? ""}|${loaderVersion ?? ""}`;
  const prevSelectionKey = useRef(selectionKey);

  const refreshStatus = useCallback(async () => {
    if (!loadoutId) {
      setStatus(null);
      return;
    }
    try {
      setStatus(await mcLoaderStatus(loadoutId));
    } catch {
      setStatus({ state: "error", message: "État indisponible." });
    }
  }, [loadoutId]);

  const refreshInstalled = useCallback(async () => {
    if (!loadoutId || !isTauriShell()) {
      setInstalledGameVersions([]);
      setInstalledLoaderVersions([]);
      setInstalledLoaders(new Set());
      return;
    }
    try {
      const games = await listInstalledMcGameVersions(loadoutId);
      setInstalledGameVersions(games);
      if (!mcVersion) {
        setInstalledLoaderVersions([]);
        setInstalledLoaders(new Set());
        return;
      }
      const nextLoaders = new Set<McLoaderId>();
      if (games.includes(mcVersion)) {
        nextLoaders.add("vanilla");
      }
      const nonVanilla = LOADERS.filter((l) => l.id !== "vanilla");
      const perLoader = await Promise.all(
        nonVanilla.map(async (row) => {
          const vers = await listInstalledMcLoaderVersions(loadoutId, row.id, mcVersion);
          return { id: row.id, vers };
        }),
      );
      for (const row of perLoader) {
        if (row.vers.length > 0) nextLoaders.add(row.id);
      }
      setInstalledLoaders(nextLoaders);
      if (loader && loader !== "vanilla") {
        const match = perLoader.find((r) => r.id === loader);
        setInstalledLoaderVersions(match?.vers ?? []);
      } else {
        setInstalledLoaderVersions([]);
      }
    } catch {
      setInstalledGameVersions([]);
      setInstalledLoaderVersions([]);
      setInstalledLoaders(new Set());
    }
  }, [loadoutId, loader, mcVersion]);

  useEffect(() => {
    if (prevSelectionKey.current !== selectionKey) {
      prevSelectionKey.current = selectionKey;
      // Drop stale « Installé » / CTA while the new selection is resolving.
      setStatus(null);
    }
    void refreshStatus();
  }, [selectionKey, refreshStatus, installing]);

  useEffect(() => {
    void refreshInstalled();
  }, [refreshInstalled, installing, status?.state]);

  useEffect(() => {
    if (!isTauriShell() || loadout?.gameId !== "minecraft") return;
    let cancelled = false;
    void listMcGameVersions().then((rows) => {
      if (!cancelled) setGameVersions(rows);
    });
    return () => {
      cancelled = true;
    };
  }, [loadout?.gameId]);

  const versionsKey =
    loader && mcVersion && loader !== "vanilla" ? `${loader}|${mcVersion}` : null;

  useEffect(() => {
    if (!versionsKey || !loader || !mcVersion) {
      // Keep the last list until a new loader list arrives — avoids an empty flash.
      // Only clear the key so the menu shows « Chargement… » for a new pair.
      if (!versionsKey) {
        setLoaderVersionsKey(null);
      }
      return;
    }
    let cancelled = false;
    void listMcLoaderVersions(loader, mcVersion).then((rows) => {
      if (!cancelled) {
        setLoaderVersions(rows);
        setLoaderVersionsKey(versionsKey);
      }
    });
    return () => {
      cancelled = true;
    };
  }, [versionsKey, loader, mcVersion]);

  useEffect(() => {
    if (!isTauriShell() || !loadoutId) return;
    let cleanup: (() => void) | undefined;
    void import("@tauri-apps/api/event").then(({ listen }) =>
      listen<{ loadoutId?: number; done?: boolean; message?: string }>(
        "loader://progress",
        (event) => {
          if (event.payload.loadoutId !== loadoutId) return;
          setInstalling(!event.payload.done);
          if (event.payload.message) onNotice(event.payload.message);
          if (event.payload.done) {
            void refreshStatus();
            void refreshInstalled();
          }
        },
      ).then((unlisten) => {
        cleanup = unlisten;
      }),
    );
    return () => cleanup?.();
  }, [loadoutId, onNotice, refreshStatus, refreshInstalled]);

  const installedGameSet = useMemo(
    () => new Set(installedGameVersions),
    [installedGameVersions],
  );
  const installedLoaderSet = useMemo(
    () => new Set(installedLoaderVersions),
    [installedLoaderVersions],
  );

  const run = useCallback(
    async (action: () => Promise<void>) => {
      try {
        await action();
        await onRefresh();
        await refreshStatus();
        await refreshInstalled();
      } catch (e) {
        onError(e instanceof Error ? e.message : String(e));
      }
    },
    [onError, onRefresh, refreshStatus, refreshInstalled],
  );

  const runInstall = useCallback(
    (force: boolean) => {
      if (!loadout) return;
      void run(async () => {
        setInstalling(true);
        try {
          await installBranchMcLoader(loadout.id, { force });
          onNotice(force ? "Loader réinstallé — lancement…" : "Loader installé — lancement…");
          if (onAfterLoaderInstall) {
            try {
              await onAfterLoaderInstall();
            } catch (e) {
              // One-shot only: surface error, do not re-install or retry.
              onError(e instanceof Error ? e.message : String(e));
            }
          }
        } finally {
          setInstalling(false);
        }
      });
    },
    [loadout, run, onNotice, onAfterLoaderInstall, onError],
  );

  if (!loadout || loadout.gameId !== "minecraft") {
    return { facts: [], primaryAction: null };
  }

  const ready =
    Boolean(loader) &&
    Boolean(mcVersion) &&
    (loader === "vanilla" || Boolean(loaderVersion));
  const statusKnown = status !== null;
  const isInstalled = status?.state === "installed";
  /** Slot replaces branch select while install is needed or in flight — not for reinstall. */
  const showInstallSlot =
    ready && statusKnown && (installing || !isInstalled);

  const loaderLabel = loader ? (LOADER_LABELS[loader] ?? loader) : "loader";
  const primaryAction: ToolbarPrimaryAction | null = showInstallSlot
    ? {
        kind: "install-loader",
        label: installing ? "Installation…" : `Installer ${loaderLabel}`,
        installing,
        onInstall: () => runInstall(false),
      }
    : null;

  const loaderVersionItems: FactMenuItem[] = (() => {
    if (!versionsKey) return [];
    if (loaderVersionsKey !== versionsKey) {
      return [{ label: "Chargement…", icon: "git-branch" }];
    }
    const rows: FactMenuItem[] = loaderVersions.slice(0, 40).map((ver) => ({
      label: ver,
      icon: "git-branch" as const,
      badge: installedLoaderSet.has(ver) ? INSTALLED_BADGE : undefined,
      current: loaderVersion === ver,
      onPick: () => void run(() => setBranchMcLoaderVersion(loadout.id, ver)),
    }));
    if (isInstalled && loaderVersion) {
      rows.push("-");
      rows.push({
        label: "Réinstaller",
        sub: loaderVersion,
        icon: "refresh",
        onPick: () => runInstall(true),
      });
    }
    return rows;
  })();

  const gameVersionItems: FactMenuItem[] = (() => {
    const rows: FactMenuItem[] = (gameVersions ?? []).slice(0, 40).map((ver) => ({
      label: ver,
      icon: "box" as const,
      badge: installedGameSet.has(ver) ? INSTALLED_BADGE : undefined,
      current: mcVersion === ver,
      onPick: () => void run(() => setBranchMcGameVersion(loadout.id, ver)),
    }));
    if (loader === "vanilla" && isInstalled && mcVersion) {
      rows.push("-");
      rows.push({
        label: "Réinstaller Vanilla",
        sub: mcVersion,
        icon: "refresh",
        onPick: () => runInstall(true),
      });
    }
    return rows;
  })();

  const facts: HeaderFact[] = [
    {
      id: "mc-loader",
      icon: "cpu",
      label: "Loader",
      value: loader ? (LOADER_LABELS[loader] ?? loader) : "—",
      title: "Loader Minecraft",
      muted: !loader,
      edit: {
        mode: "menu",
        label: "Choisir le loader",
        items: LOADERS.map((row) => ({
          label: row.label,
          icon: "cpu" as const,
          badge:
            mcVersion && installedLoaders.has(row.id) ? INSTALLED_BADGE : undefined,
          current: loader === row.id,
          onPick: () => void run(() => setBranchMcLoader(loadout.id, row.id)),
        })),
      },
    },
    {
      id: "mc-version",
      icon: "box",
      label: "Minecraft",
      value: mcVersion ?? "—",
      title: "Version Minecraft",
      muted: !mcVersion,
      edit: {
        mode: "menu",
        label: "Choisir la version",
        items: gameVersionItems,
      },
    },
    // Always keep the third fact so Vanilla ↔ loader swaps do not jump the grid.
    {
      id: "mc-loader-version",
      icon: "git-branch",
      label: "Version du loader",
      value:
        loader === "vanilla" || !loader
          ? "—"
          : installing
            ? "Installation…"
            : (loaderVersion ?? "—"),
      title:
        loader === "vanilla" || !loader
          ? "Vanilla n’a pas de version de loader"
          : "Version du mod loader",
      muted: loader === "vanilla" || !loader || (!loaderVersion && !installing),
      edit:
        loader && loader !== "vanilla"
          ? {
              mode: "menu" as const,
              label: "Choisir la version du loader",
              items: loaderVersionItems,
            }
          : undefined,
    },
  ];

  return { facts, primaryAction };
}
