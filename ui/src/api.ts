export type DetectedGame = {
  id: string;
  name: string;
  root: string;
  short: string;
};

export type LoadoutDto = {
  id: number;
  gameId: string;
  name: string;
  /** Real count from the Core. Revision / last-played have no Core backing yet. */
  modCount: number;
};

export type ModRow = {
  modId: number;
  name: string;
  sourcePath: string;
  enabled: boolean;
  position: number;
};

export type ShellSnapshot = {
  dataRoot: string;
  games: DetectedGame[];
  loadouts: LoadoutDto[];
  entries: ModRow[];
  selectedLoadoutId: number | null;
  usvfsDll: string | null;
};

function isTauri(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

export async function getShellSnapshot(
  loadoutId?: number | null,
): Promise<ShellSnapshot> {
  if (!isTauri()) {
    return mockSnapshot();
  }
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<ShellSnapshot>("get_shell_snapshot", {
    loadoutId: loadoutId ?? null,
  });
}

export async function setModEnabled(
  loadoutId: number,
  modId: number,
  enabled: boolean,
): Promise<ModRow[]> {
  if (!isTauri()) {
    const snap = mockSnapshot();
    return snap.entries.map((e) =>
      e.modId === modId ? { ...e, enabled } : e,
    );
  }
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<ModRow[]>("set_mod_enabled", { loadoutId, modId, enabled });
}

export async function launchSkse(loadoutId: number): Promise<void> {
  if (!isTauri()) {
    throw new Error("Le lancement SKSE n’est disponible que dans le shell Tauri.");
  }
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke("launch_skse", { loadoutId });
}

/**
 * Browser-preview fixture. Only reachable outside the Tauri shell (`npm run dev`
 * in a plain browser), so it can never be mistaken for Core data at runtime —
 * `dataRoot` says so out loud in the status bar.
 */
function mockSnapshot(): ShellSnapshot {
  const names = [
    "SKSE64",
    "Address Library for SKSE Plugins",
    "SSE Engine Fixes",
    "Unofficial Skyrim Special Edition Patch",
    "SkyUI",
    "RaceMenu",
    "Cathedral Weathers and Seasons",
    "Lux",
    "Static Mesh Improvement Mod",
    "Skyland AIO",
  ];
  return {
    dataRoot: "(aperçu navigateur — pas le Core)",
    games: [
      {
        id: "skyrimse",
        name: "Skyrim Special Edition",
        root: "C:\\Steam\\Skyrim Special Edition",
        short: "Skyrim SE",
      },
      {
        id: "minecraft",
        name: "Minecraft",
        root: "C:\\Users\\dev\\AppData\\Roaming\\.minecraft",
        short: "Minecraft",
      },
    ],
    loadouts: [
      { id: 1, gameId: "skyrimse", name: "Main 2026", modCount: 10 },
      { id: 2, gameId: "skyrimse", name: "Labo météo", modCount: 7 },
      { id: 3, gameId: "skyrimse", name: "Bench FG", modCount: 4 },
      { id: 4, gameId: "minecraft", name: "Survie Fabric 1.21", modCount: 14 },
    ],
    entries: names.map((name, i) => ({
      modId: i + 1,
      name,
      sourcePath: `mods/${name}`,
      enabled: i !== 2,
      position: i,
    })),
    selectedLoadoutId: 1,
    usvfsDll: null,
  };
}
