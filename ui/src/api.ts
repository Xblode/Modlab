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

/** The three channels a loadout can be on — see `modlab_core::loadout::CHANNELS`. */
export type Channel = "stable" | "beta" | "alpha";

export type LoadoutDto = {
  id: number;
  gameId: string;
  name: string;
  /** Real count from the Core. Last-played has no Core backing yet. */
  modCount: number;
  channel: Channel;
  /** How many snapshots this loadout has — `transaction::revision_count`,
   * carried on every listed loadout so a card can show `r.N` without its
   * own `getLoadoutVersion` round-trip. */
  revision: number;
};

export type ModRow = {
  modId: number;
  name: string;
  sourcePath: string;
  enabled: boolean;
  position: number;
  /**
   * The version the mod's own source declared — a Minecraft jar manifest
   * does, an MO2 import doesn't. `null` renders as a dash: the table shows
   * what the Core knows and never invents a number.
   */
  version: string | null;
  /**
   * `"archived"` — captured by the browser, archive not unpacked yet, so it
   * cannot be enabled — or `"installed"`. The table shows the difference
   * instead of offering a checkbox that would do nothing.
   */
  status: ModStatus;
  /** The human's own free tag, `null` until they type one. */
  category: string | null;
  archivePath: string | null;
};

export type ModStatus = "archived" | "installed";

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
    return {
      id: Math.floor(Date.now() / 1000),
      gameId,
      name: cleanName,
      modCount: 0,
      channel: "stable",
      revision: 0,
    };
  }
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<LoadoutDto>("create_loadout", { gameId, name: cleanName });
}

/** Rename a loadout. */
export async function renameLoadout(loadoutId: number, name: string): Promise<void> {
  if (!isTauri()) {
    throw new Error("Le renommage n'est disponible que dans le shell Tauri.");
  }
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke("rename_loadout", { loadoutId, name });
}

/** Move a loadout to a different channel. */
export async function setLoadoutChannel(
  loadoutId: number,
  channel: Channel,
): Promise<void> {
  if (!isTauri()) {
    throw new Error("Le changement de canal n'est disponible que dans le shell Tauri.");
  }
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke("set_loadout_channel", { loadoutId, channel });
}

/**
 * Soft-delete a loadout — reversible in the Core (its row and entries
 * survive). `listDeletedLoadouts`/`restoreLoadout` are the trash screen.
 */
export async function deleteLoadout(loadoutId: number): Promise<void> {
  if (!isTauri()) {
    throw new Error("La suppression n'est disponible que dans le shell Tauri.");
  }
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke("delete_loadout", { loadoutId });
}

/** Where the embedded browser sits, in CSS pixels. */
export type BrowserRect = { x: number; y: number; width: number; height: number };

/**
 * The embedded browser is a native child WebView2 layered over the shell —
 * it has no existence at all outside Tauri, so in the browser preview these
 * are no-ops rather than errors: the web tab simply shows its own
 * explanation instead of a page.
 */
/**
 * `gameId` is only used the very first time the browser is shown after this
 * call — see `browser::show`'s "navigates on the very first open" doc —
 * and records which game's downloads subfolder a capture made during this
 * session should land in (Rust-side `AppState.active_game`).
 */
export async function browserShow(
  url: string,
  rect: BrowserRect,
  gameId: string | null,
): Promise<void> {
  if (!isTauri()) return;
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke("browser_show", { url, rect, gameId });
}

export async function browserHide(): Promise<void> {
  if (!isTauri()) return;
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke("browser_hide");
}

export async function browserNavigate(url: string): Promise<void> {
  if (!isTauri()) return;
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke("browser_navigate", { url });
}

/** `-1` back, `1` forward. */
export async function browserGo(delta: number): Promise<void> {
  if (!isTauri()) return;
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke("browser_go", { delta });
}

/**
 * Whether this is the real shell rather than the Vite preview. Exported for
 * the one case where a component must *say* something different rather than
 * silently fall back: the browser tab, whose whole content is a native
 * webview that simply does not exist outside Tauri.
 */
export function isTauriShell(): boolean {
  return isTauri();
}

/** The downloads folder, and whether it's the default or a chosen override. */
export type DownloadsDirSetting = {
  defaultPath: string;
  overridePath: string | null;
  effective: string;
};

export async function downloadsDirSetting(): Promise<DownloadsDirSetting | null> {
  if (!isTauri()) return null;
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<DownloadsDirSetting>("downloads_dir_setting");
}

export async function setDownloadsDirOverride(path: string): Promise<DownloadsDirSetting> {
  if (!isTauri()) throw new Error("Disponible uniquement dans le shell Tauri.");
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<DownloadsDirSetting>("set_downloads_dir_override", { path });
}

export async function clearDownloadsDirOverride(): Promise<DownloadsDirSetting> {
  if (!isTauri()) throw new Error("Disponible uniquement dans le shell Tauri.");
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<DownloadsDirSetting>("clear_downloads_dir_override");
}

/** A file the embedded browser captured, as the Core recorded it. */
export type Download = {
  id: number;
  url: string;
  fileName: string;
  path: string;
  sizeBytes: number | null;
  capturedAt: string;
  loadoutId: number | null;
  modId: number | null;
};

export async function listDownloads(): Promise<Download[]> {
  if (!isTauri()) return [];
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<Download[]>("list_downloads");
}

/** File a capture into a loadout — it appears archived and disabled. */
export async function attachDownload(
  downloadId: number,
  loadoutId: number,
  category?: string | null,
): Promise<number> {
  if (!isTauri()) throw new Error("Disponible uniquement dans le shell Tauri.");
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<number>("attach_download", {
    downloadId,
    loadoutId,
    category: category ?? null,
  });
}

export async function setModCategory(
  modId: number,
  category: string | null,
): Promise<void> {
  if (!isTauri()) throw new Error("Disponible uniquement dans le shell Tauri.");
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke("set_mod_category", { modId, category });
}

/**
 * Move an archived mod (not yet extracted) to a different instance. Refused
 * once the mod is installed — its files are already committed to somewhere.
 */
export async function reassignMod(
  modId: number,
  fromLoadoutId: number,
  toLoadoutId: number,
): Promise<ModRow[]> {
  if (!isTauri()) throw new Error("Disponible uniquement dans le shell Tauri.");
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<ModRow[]>("reassign_mod", { modId, fromLoadoutId, toLoadoutId });
}

/** Every tag already in use — what the category input autocompletes from. */
export async function modCategories(): Promise<string[]> {
  if (!isTauri()) return [];
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<string[]>("mod_categories");
}

export async function renameMod(
  modId: number,
  loadoutId: number,
  name: string,
): Promise<ModRow[]> {
  if (!isTauri()) throw new Error("Disponible uniquement dans le shell Tauri.");
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<ModRow[]>("rename_mod", { modId, loadoutId, name });
}

/**
 * Soft-delete: the mod disappears from the table but nothing on disk (its
 * extracted files, its archive) is touched — see `download::remove`'s doc
 * on the Rust side.
 */
export async function deleteMod(modId: number, loadoutId: number): Promise<ModRow[]> {
  if (!isTauri()) throw new Error("Disponible uniquement dans le shell Tauri.");
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<ModRow[]>("delete_mod", { modId, loadoutId });
}

/** Revert an installed mod back to its archive — the extracted copy is
 * cleared, the archive stays untouched and can be reinstalled any time. */
export async function uninstallMod(modId: number, loadoutId: number): Promise<ModRow[]> {
  if (!isTauri()) throw new Error("Disponible uniquement dans le shell Tauri.");
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<ModRow[]>("uninstall_mod", { modId, loadoutId });
}

/** Open Explorer at `path` — the folder itself, or (`select`) its parent
 * folder with `path` highlighted, for a single file like an archive. */
export async function revealPath(path: string, select = false): Promise<void> {
  if (!isTauri()) throw new Error("Disponible uniquement dans le shell Tauri.");
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke("reveal_path", { path, select });
}

/** What an archive turned out to contain, once unpacked to staging. */
export type ArchivePlan = {
  modId: number;
  name: string;
  shape: "fomod" | "bare-data" | "wrapped-data" | "unknown";
  needsWizard: boolean;
  stagedPath: string;
  contentRoot: string;
};

export async function inspectArchivedMod(modId: number): Promise<ArchivePlan> {
  if (!isTauri()) throw new Error("Disponible uniquement dans le shell Tauri.");
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<ArchivePlan>("inspect_archived_mod", { modId });
}

export async function installArchivedMod(
  modId: number,
  loadoutId: number,
  contentRoot: string,
): Promise<ModRow[]> {
  if (!isTauri()) throw new Error("Disponible uniquement dans le shell Tauri.");
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<ModRow[]>("install_archived_mod", { modId, loadoutId, contentRoot });
}

/**
 * The FOMOD wizard's questions — see `modlab_game_skyrim::fomod` for what
 * is and isn't parsed (no visibility conditions, no `conditionalFileInstalls`).
 */
export type FomodGroupType =
  | "select-exactly-one"
  | "select-at-most-one"
  | "select-at-least-one"
  | "select-any"
  | "select-all";

export type FomodPlugin = { id: string; name: string; description: string };
export type FomodGroup = {
  id: string;
  name: string;
  groupType: FomodGroupType;
  plugins: FomodPlugin[];
};
export type FomodStep = { id: string; name: string; groups: FomodGroup[] };
export type FomodModule = { name: string; steps: FomodStep[] };

export async function getFomodModule(contentRoot: string): Promise<FomodModule> {
  if (!isTauri()) throw new Error("Disponible uniquement dans le shell Tauri.");
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<FomodModule>("get_fomod_module", { contentRoot });
}

/** `picked` is every plugin id the wizard's answers checked. */
export async function installFomod(
  modId: number,
  loadoutId: number,
  contentRoot: string,
  picked: string[],
): Promise<ModRow[]> {
  if (!isTauri()) throw new Error("Disponible uniquement dans le shell Tauri.");
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<ModRow[]>("install_fomod", { modId, loadoutId, contentRoot, picked });
}

/** Fires the moment the browser finishes capturing a file. */
export type CapturedDownload = {
  downloadId: number;
  fileName: string;
  url: string;
  path: string;
};

/** Returns an unsubscribe function; a no-op outside Tauri. */
export async function onDownloadCaptured(
  handler: (event: CapturedDownload) => void,
): Promise<() => void> {
  if (!isTauri()) return () => {};
  const { listen } = await import("@tauri-apps/api/event");
  const unlisten = await listen<CapturedDownload>("download://captured", (e) =>
    handler(e.payload),
  );
  return unlisten;
}

/** One soft-deleted loadout, as the trash screen lists it. */
export type DeletedLoadout = {
  id: number;
  gameId: string;
  name: string;
};

export async function listDeletedLoadouts(): Promise<DeletedLoadout[]> {
  if (!isTauri()) return [];
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<DeletedLoadout[]>("list_deleted_loadouts");
}

export async function restoreLoadout(loadoutId: number): Promise<void> {
  if (!isTauri()) {
    throw new Error("La restauration n'est disponible que dans le shell Tauri.");
  }
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke("restore_loadout", { loadoutId });
}

/**
 * What the version badge (`r.3`, `b.1`…) shows: how many revisions this
 * loadout has and whether the live state has drifted from the latest one.
 * The channel-letter prefix is picked by the UI from `LoadoutDto.channel` —
 * this only ever carries the number and the dirty counts.
 */
export type LoadoutVersion = {
  revision: number;
  dirtyPlus: number;
  dirtyMinus: number;
};

export async function getLoadoutVersion(loadoutId: number): Promise<LoadoutVersion> {
  if (!isTauri()) return previewLoadoutVersion();
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<LoadoutVersion>("get_loadout_version", { loadoutId });
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

/**
 * Import the detected Minecraft instance's `mods/` folder into a new
 * loadout — the Minecraft counterpart of `mo2Import`, and like it read-only
 * on the game's side: not a jar is moved.
 */
export async function minecraftImportInstance(name: string): Promise<Mo2ImportResult> {
  if (!isTauri()) {
    throw new Error(
      "L'import d'instance Minecraft n'est disponible que dans le shell Tauri (aperçu navigateur : lecture seule).",
    );
  }
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<Mo2ImportResult>("minecraft_import_instance", { name });
}

/**
 * Copy one downloaded `.jar` into the instance's `mods/` folder and register
 * it. A real write into the game's own folder (unlike Skyrim's USVFS
 * overlay), so the Core stages the copy and refuses to overwrite an existing
 * jar. Returns the refreshed entry list.
 */
export async function minecraftInstallJar(
  loadoutId: number,
  jarPath: string,
): Promise<ModRow[]> {
  if (!isTauri()) {
    throw new Error("L'installation de mod n'est disponible que dans le shell Tauri.");
  }
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<ModRow[]>("minecraft_install_jar", { loadoutId, jarPath });
}

/** Resource packs, shader packs and worlds of the detected instance. */
export type MinecraftContent = {
  resourcePacks: string[];
  shaderPacks: string[];
  worlds: string[];
  /**
   * `servers.dat` exists on disk. Its contents are deliberately not read —
   * it is binary NBT and ModLab has no parser for it, so the Servers view
   * says that instead of showing an empty list that would read as
   * "you have no servers".
   */
  serversDatPresent: boolean;
};

export async function getMinecraftContent(): Promise<MinecraftContent> {
  if (!isTauri()) return previewMinecraftContent();
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<MinecraftContent>("minecraft_content", {});
}

export async function launchSkse(loadoutId: number): Promise<void> {
  if (!isTauri()) {
    throw new Error("Le lancement SKSE n’est disponible que dans le shell Tauri.");
  }
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke("launch_skse", { loadoutId });
}

/** A field left `null` means `SkyrimPrefs.ini` doesn't set that key yet —
 * not that it's off/zero. The dialog shows the game's own default in that
 * case rather than a blank field. */
export type GraphicsSettings = {
  width: number | null;
  height: number | null;
  fullscreen: boolean | null;
  vsync: boolean | null;
  antialiasing: number | null;
  fxaa: boolean | null;
};

export async function getSkyrimGraphicsSettings(): Promise<GraphicsSettings> {
  if (!isTauri()) throw new Error("Disponible uniquement dans le shell Tauri.");
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<GraphicsSettings>("skyrim_graphics_settings");
}

/** A field left `null` leaves that key's current line in `SkyrimPrefs.ini`
 * untouched rather than clearing it — a partial update, not a replace. */
export async function setSkyrimGraphicsSettings(
  settings: GraphicsSettings,
): Promise<GraphicsSettings> {
  if (!isTauri()) throw new Error("Disponible uniquement dans le shell Tauri.");
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<GraphicsSettings>("set_skyrim_graphics_settings", { settings });
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
 * One resolved path from the merged view — the Core's own "what the game
 * would see in Data/" answer (`modlab_core::conflicts::merged_view`),
 * computed from the enabled mods' real files without needing a live USVFS
 * session. `contributorCount > 1` means more than one enabled mod provides
 * this same path; `modName` is whichever one wins.
 */
export type DirectoryEntry = {
  relativePath: string;
  modId: number;
  modName: string;
  contributorCount: number;
};

/** The Répertoire tab: every resolved path. */
export async function getLoadoutDirectory(loadoutId: number): Promise<DirectoryEntry[]> {
  if (!isTauri()) return [];
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<DirectoryEntry[]>("loadout_directory", { loadoutId });
}

/** The Plugins tab: the merged view filtered to .esp/.esm/.esl. Mod-position
 * order, not a validated LOOT load order — that doesn't exist yet. */
export async function getLoadoutPlugins(loadoutId: number): Promise<DirectoryEntry[]> {
  if (!isTauri()) return [];
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<DirectoryEntry[]>("loadout_plugins", { loadoutId });
}

/** The Archives tab: the merged view filtered to .bsa/.ba2. Which files
 * exist, not what's packed inside them — BSA/BA2 parsing doesn't exist yet. */
export async function getLoadoutArchives(loadoutId: number): Promise<DirectoryEntry[]> {
  if (!isTauri()) return [];
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<DirectoryEntry[]>("loadout_archives", { loadoutId });
}

/** One `.ess` save file found on disk. `modifiedAt` is Unix seconds.
 * `revision` doesn't exist: the Core can't yet tell which loadout revision
 * was active when a save was written. */
export type SaveFile = {
  fileName: string;
  sizeBytes: number;
  modifiedAt: number | null;
};

export async function getSkyrimSaves(): Promise<SaveFile[]> {
  if (!isTauri()) return [];
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<SaveFile[]>("skyrim_saves");
}

/** A dependency the shell knows to look for on a mod — a stopgap ahead of
 * a real "known mods" registry (see the Rust side's own doc). */
export type ModDependency = {
  name: string;
  satisfied: boolean;
};

export async function getModDependencies(
  loadoutId: number,
  modId: number,
): Promise<ModDependency[]> {
  if (!isTauri()) return [];
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<ModDependency[]>("mod_dependencies", { loadoutId, modId });
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
  /**
   * Which loadout this is about — only populated for snapshot/rollback
   * entries. `null` for everything else (`loadout.create`, `mod.import`…) —
   * their payload has no structural loadout id, not a claim it isn't about one.
   */
  loadoutId: number | null;
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
      { id: 1, gameId: "skyrimse", name: "Main 2026", modCount: 10, channel: "stable", revision: 12 },
      { id: 2, gameId: "skyrimse", name: "Labo météo", modCount: 7, channel: "alpha", revision: 3 },
      { id: 3, gameId: "skyrimse", name: "Bench FG", modCount: 4, channel: "beta", revision: 5 },
      {
        id: 4,
        gameId: "minecraft",
        name: "Survie Fabric 1.21",
        modCount: 14,
        channel: "stable",
        revision: 8,
      },
    ],
    entries: names.map((name, i) => ({
      modId: i + 1,
      name,
      sourcePath: `mods/${name}`,
      enabled: i !== 2,
      position: i,
      // Skyrim mods imported from MO2 genuinely carry no version — the
      // preview shows the same dashes the real shell does.
      version: null,
      // One archived row so the preview shows what a fresh capture looks
      // like before it is unpacked; everything else came from an import and
      // is therefore installed.
      status: i === 1 ? ("archived" as const) : ("installed" as const),
      category: i === 1 ? "Éclairage" : null,
      archivePath: i === 1 ? `F:/ModLab/downloads/${name}.7z` : null,
    })),
    selectedLoadoutId: 1,
    usvfsDll: null,
  };
}

/**
 * Browser-preview fixture for a Minecraft instance's content — same rule as
 * `mockSnapshot`: only reachable outside the Tauri shell, where there is no
 * instance on disk to list.
 */
function previewMinecraftContent(): MinecraftContent {
  return {
    resourcePacks: ["Faithful 32x.zip"],
    shaderPacks: ["ComplementaryReimagined_r5.zip"],
    worlds: ["New World", "Survie 1.21"],
    serversDatPresent: true,
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

/**
 * Browser-preview fixture for a loadout's version info — same rule as
 * `mockSnapshot`. Reports a couple of unsaved toggles so the version badge
 * and the "Enregistrer" button have something real to render in preview.
 */
function previewLoadoutVersion(): LoadoutVersion {
  return { revision: 12, dirtyPlus: 1, dirtyMinus: 0 };
}
