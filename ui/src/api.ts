export type GameSection = "dashboard" | "mods" | "plugins" | "archives" | "directory" | "saves" | "health" | "packs" | "worlds" | "servers";

/** Mirrors the serialized GameAdapter contract; declarations are not execution permissions. */
export type GameCapabilities = {
  shortName: string;
  sections: GameSection[];
  deploymentLabel: string;
  scriptExtender: string | null;
  loadOrderTool: { id: string; name: string } | null;
  /**
   * An external mod manager this game can be migrated from, as an opaque id
   * (e.g. "mo2") that the UI maps to a label — the same way section ids map
   * to tab labels. `null` = no import entry point for this game.
   */
  importSource: string | null;
};


export type DetectedGame = {
  id: string;
  name: string;
  root: string;
  short: string;
  capabilities: GameCapabilities;
};

export type PathKind = "dir" | "file";

/**
 * One folder/executable a Game Adapter cares about, merged with any human
 * override the Core holds for it. `effective` is `overridePath ?? detected`
 * — the value anything downstream should actually use.
 */
export type GamePathSlot = {
  id: string;
  label: string;
  kind: PathKind;
  detected: string | null;
  overridePath: string | null;
  effective: string | null;
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

/** Create an empty instance through the Core (or a preview-only fixture). */
export async function createLoadout(
  gameId: string,
  name: string,
): Promise<LoadoutDto> {
  const cleanName = name.trim();
  if (!cleanName) throw new Error("Donne un nom à l’instance.");
  if (!isTauri()) {
    return { id: Math.floor(Date.now() / 1000), gameId, name: cleanName, modCount: 0 };
  }
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<LoadoutDto>("create_loadout", { gameId, name: cleanName });
}

export type Mo2ImportResult = {
  loadout: LoadoutDto;
  /** Mods actually imported — DLC/CC entries in the profile are skipped. */
  imported: number;
};

/**
 * Import an MO2 profile into a new Skyrim loadout. Only reachable inside the
 * Tauri shell: this writes to the Core's database, and a plain browser has
 * none — faking a write here would be worse than an error.
 */
export async function mo2Import(
  profile: string,
  modsDir: string,
  name: string,
): Promise<Mo2ImportResult> {
  if (!isTauri()) {
    throw new Error(
      "L’import MO2 n’est disponible que dans le shell Tauri (aperçu navigateur : lecture seule).",
    );
  }
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<Mo2ImportResult>("mo2_import", { profile, modsDir, name });
}

export async function launchSkse(loadoutId: number): Promise<void> {
  if (!isTauri()) {
    throw new Error("Le lancement SKSE n’est disponible que dans le shell Tauri.");
  }
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke("launch_skse", { loadoutId });
}

/**
 * The path slots one game's adapter declares (§3.1 — each Game Adapter
 * decides its own list, e.g. an `.exe`-anchored root for Skyrim vs an
 * `%APPDATA%`-anchored one for Minecraft), merged with whatever override the
 * Core has on file. Read-only: Lecture tier (§7.2), no confirmation needed.
 */
export async function getGamePathSlots(gameId: string): Promise<GamePathSlot[]> {
  if (!isTauri()) return previewPathSlots(gameId);
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<GamePathSlot[]>("game_path_slots", { gameId });
}

/**
 * Point one path slot at a folder/file chosen by hand — Mutation réversible
 * tier (§7.2): nothing on disk moves, and `clearGamePathOverride` undoes it
 * instantly. Returns the merged slot list so the caller refreshes from the
 * Core's own answer instead of guessing what changed.
 */
export async function setGamePathOverride(
  gameId: string,
  slotId: string,
  path: string,
): Promise<GamePathSlot[]> {
  if (!isTauri()) {
    throw new Error("La modification des chemins n'est disponible que dans le shell Tauri.");
  }
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<GamePathSlot[]>("set_game_path_override", { gameId, slotId, path });
}

/** Drop a human override for one slot, falling back to auto-detection again. */
export async function clearGamePathOverride(
  gameId: string,
  slotId: string,
): Promise<GamePathSlot[]> {
  if (!isTauri()) {
    throw new Error("La modification des chemins n'est disponible que dans le shell Tauri.");
  }
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<GamePathSlot[]>("clear_game_path_override", { gameId, slotId });
}

export type ModConflicts = {
  modId: number;
  name: string;
  /** Distinct paths this mod wins: it is last in the priority order scanned. */
  wins: number;
  /** Distinct paths another mod wins over this one. */
  loses: number;
};

export type LoadoutConflicts = {
  mods: ModConflicts[];
  /**
   * Enabled mods whose folder could not be read, as `id: reason`.
   *
   * A short `mods` list next to a non-empty `unreadable` list means the scan
   * *does not know* about those mods — never that they conflict with nothing.
   */
  unreadable: string[];
};

/**
 * Which mod was scanned as winning is decided by the loadout's stored import
 * order, and MO2's real on-screen priority direction is still unconfirmed
 * (see `modlab_core::loadout::LoadoutEntryRow`). Every win/lose mark the UI
 * shows carries this as its tooltip rather than presenting a guess as a fact.
 */
export const CONFLICT_PRIORITY_CAVEAT =
  "Ordre de priorité = ordre d’import — direction MO2 pas encore confirmée";

/**
 * File conflicts of a loadout, scanned by the Core over the real mod folders.
 * Same computation the CLI's `loadout-conflicts` command runs.
 */
export async function getLoadoutConflicts(
  loadoutId: number,
): Promise<LoadoutConflicts> {
  if (!isTauri()) {
    return previewConflicts();
  }
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<LoadoutConflicts>("get_loadout_conflicts", { loadoutId });
}

/**
 * Answer from the local AI layer (§6): the Core's rules engine diagnoses,
 * and — when a GGUF model is available and the shell was built with the
 * `local-llm` feature — the local LLM phrases the same facts. `source` says
 * which of the two produced this answer.
 */
export type AiAnswer = {
  text: string;
  /** "rules" (moteur de règles seul) ou "local-llm" (phrasé par le modèle). */
  source: string;
  model: string | null;
};

export async function aiAsk(question: string, loadoutId: number): Promise<AiAnswer> {
  if (!isTauri()) {
    return {
      text: "Aperçu navigateur : le Core n'est pas joignable, donc pas de diagnostic réel ici. Lance le shell Tauri pour parler au moteur de règles.",
      source: "rules",
      model: null,
    };
  }
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<AiAnswer>("ai_ask", { question, loadoutId });
}

/** One journaled mutation, newest first as the Core returns them. */
export type Transaction = {
  id: number;
  occurredAt: string;
  /** The Core's own vocabulary (`loadout.snapshot`, `mod.import`, …). */
  kind: string;
  steps: string[];
};

/** French labels for the journal's kinds; unknown kinds fall back to the raw id. */
export const KIND_LABELS: Record<string, string> = {
  "loadout.snapshot": "Instantané",
  "loadout.rollback": "Restauration",
  "mod.import": "Import de mods",
  "loadout.import": "Import de mods",
  "loadout.create": "Création d'instance",
  "mo2.import": "Import MO2",
};

export async function getTransactions(limit = 50): Promise<Transaction[]> {
  if (!isTauri()) return [];
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<Transaction[]>("get_transactions", { limit });
}

/** Capture the loadout's configuration as an immutable journal event (§5). */
export async function snapshotLoadout(loadoutId: number): Promise<number> {
  if (!isTauri()) return 0;
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<number>("snapshot_loadout", { loadoutId });
}

/** Restore the loadout to its most recent snapshot; returns the fresh entries. */
export async function rollbackLoadout(loadoutId: number): Promise<ModRow[]> {
  if (!isTauri()) {
    throw new Error("La restauration n'est disponible que dans le shell Tauri (le navigateur n'a pas de Core à restaurer).");
  }
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<ModRow[]>("rollback_loadout", { loadoutId });
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
        capabilities: {
          shortName: "Skyrim SE",
          sections: ["dashboard", "mods", "plugins", "archives", "directory", "saves", "health"],
          deploymentLabel: "USVFS",
          scriptExtender: "SKSE",
          loadOrderTool: { id: "loot", name: "LOOT" },
          importSource: "mo2",
        },
      },
      {
        id: "minecraft",
        name: "Minecraft",
        root: "C:\\Users\\dev\\AppData\\Roaming\\.minecraft",
        short: "Minecraft",
        capabilities: {
          shortName: "Minecraft",
          sections: ["dashboard", "mods", "packs", "worlds", "servers", "health"],
          deploymentLabel: "dossier mods/",
          scriptExtender: null,
          loadOrderTool: null,
          importSource: null,
        },
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

/**
 * Browser-preview fixture for path slots — same rule as `mockSnapshot`: only
 * reachable outside the Tauri shell. Mirrors each real adapter's actual slot
 * ids (`skyrimse`: root/exe/data_dir/usvfs_dll, `minecraft`: root/mods_dir)
 * so the Réglages → Jeux layout can be built and eyeballed without the Core.
 */
function previewPathSlots(gameId: string): GamePathSlot[] {
  if (gameId === "minecraft") {
    return [
      {
        id: "root",
        label: "Dossier .minecraft / launcher",
        kind: "dir",
        detected: "C:\\Users\\dev\\AppData\\Roaming\\.minecraft",
        overridePath: null,
        effective: "C:\\Users\\dev\\AppData\\Roaming\\.minecraft",
      },
      {
        id: "mods_dir",
        label: "Dossier mods/",
        kind: "dir",
        detected: null,
        overridePath: null,
        effective: null,
      },
    ];
  }
  return [
    {
      id: "root",
      label: "Dossier d'installation",
      kind: "dir",
      detected: "C:\\Steam\\Skyrim Special Edition",
      overridePath: null,
      effective: "C:\\Steam\\Skyrim Special Edition",
    },
    {
      id: "exe",
      label: "Exécutable du jeu (SkyrimSE.exe)",
      kind: "file",
      detected: "C:\\Steam\\Skyrim Special Edition\\SkyrimSE.exe",
      overridePath: null,
      effective: "C:\\Steam\\Skyrim Special Edition\\SkyrimSE.exe",
    },
    {
      id: "data_dir",
      label: "Dossier Data",
      kind: "dir",
      detected: "C:\\Steam\\Skyrim Special Edition\\Data",
      overridePath: null,
      effective: "C:\\Steam\\Skyrim Special Edition\\Data",
    },
    {
      id: "usvfs_dll",
      label: "usvfs_x64.dll (Mod Organizer 2)",
      kind: "file",
      detected: null,
      overridePath: null,
      effective: null,
    },
  ];
}

/**
 * Browser-preview fixture for conflicts — same rule as `mockSnapshot`: only
 * reachable outside the Tauri shell, with the status bar spelling out
 * "(aperçu navigateur — pas le Core)" next to it. The real scan walks the real
 * mod folders; a plain browser has no folder to walk, so these numbers are the
 * maquette's own example, not a measurement. Mod 3 (SSE Engine Fixes) is the
 * fixture's disabled mod: it is absent here on purpose, exactly as the Core
 * leaves it out of a scan.
 */
function previewConflicts(): LoadoutConflicts {
  return {
    mods: [
      { modId: 5, name: "SkyUI", wins: 2, loses: 0 },
      { modId: 8, name: "Lux", wins: 1, loses: 1 },
      { modId: 9, name: "Static Mesh Improvement Mod", wins: 0, loses: 27 },
      { modId: 10, name: "Skyland AIO", wins: 27, loses: 0 },
    ],
    unreadable: [],
  };
}
