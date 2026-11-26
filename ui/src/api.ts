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
  /** Branch name. The sidebar shows `instanceName`. */
  name: string;
  /** Parent instance. Only branches with this id belong to it. */
  instanceId: number;
  instanceName: string;
  /** Real count from the Core. Last-played has no Core backing yet. */
  modCount: number;
  channel: Channel;
  /** How many snapshots this loadout has — `transaction::revision_count`,
   * carried on every listed loadout so a card can show `r.N` without its
   * own `getLoadoutVersion` round-trip. */
  revision: number;
  mcLoader: string | null;
  mcGameVersion: string | null;
  mcLoaderVersion: string | null;
};

export type McLoaderId = "vanilla" | "fabric" | "forge" | "neoforge" | "quilt";

export type McLoaderStatus = {
  state: string;
  message: string;
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
   * `"downloading"` — the transfer has started and the file is not here yet.
   * `"archived"` — it landed, not unpacked yet, so it cannot be enabled.
   * `"installed"` — unpacked. The table shows the difference instead of
   * offering a checkbox that would do nothing.
   */
  status: ModStatus;
  /** The human's own free tag, `null` until they type one. */
  category: string | null;
  archivePath: string | null;
  /**
   * Free loader id from the game adapter (`"skse"`, `"skse-vr"`, `"forge"`,
   * `"fabric"`, `"neoforge"`, …). `null` when this row is not that loader.
   * The UI does not re-decide from the name, and this flag does not add a
   * metadata cell.
   */
  tool: string | null;
  /** Version that goes with `tool`. `null` when the adapter has none. */
  toolVersion: string | null;
  /**
   * Host token stored on the mod (`nexus`, `curseforge`, `modrinth`,
   * `silverlock`, `manual`, `other`, `unknown`). `null` when the row
   * predates it — the UI treats that like a manual disk add.
   */
  sourceHost: string | null;
  /** Page to reopen in the embedded browser. Not the file URL. */
  pageUrl: string | null;
  /** Game versions the file declared. Empty when it declared none. */
  gameVersions: string[];
  /**
   * Loader ids the file declared (`fabric`, `forge`, `neoforge`, `quilt`).
   * Empty when unknown, or when the game has no loader.
   */
  loaders: string[];
  /** Dependency names or ids the file declared. */
  dependencies: string[];
  /**
   * True when the human chose the category. Provenance `user` on the fiche:
   * a later automatic fill does not replace it.
   */
  categoryUserSet: boolean;
  /** Category text as the file wrote it, before the game list translates it. */
  categoryRaw: string | null;
  /** Nexus category number when the file had one. */
  categoryNexusId: string | null;
  nexusModId: string | null;
  author: string | null;
  website: string | null;
  /**
   * Length of the mod file (archive or jar), in bytes. `null` until known.
   * Stays set after the file is removed.
   */
  sizeBytes: number | null;
  /**
   * When the mod became installed, ISO 8601 UTC. `null` for a download or
   * an archive, and when the date could not be recovered.
   */
  installedAt: string | null;
  /**
   * When the mod was downloaded or otherwise obtained, ISO 8601 UTC.
   * Set as soon as a download starts. Stays after the file is removed.
   * `null` when it could not be recovered.
   */
  downloadedAt: string | null;
};

/** One entry of a game's fixed category list. The shell asks the Core for it. */
export type GameCategory = {
  id: string;
  label: string;
  icon: string;
};

export type ModStatus = "downloading" | "archived" | "installed";

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

export type ShellSnapshotOpts = {
  /** When false, skip archive/metadata hydration — for fast instance switches. */
  hydrateMetadata?: boolean;
};

export async function getShellSnapshot(
  loadoutId?: number | null,
  opts?: ShellSnapshotOpts,
): Promise<ShellSnapshot> {
  if (!isTauri()) {
    return mockSnapshot();
  }
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<ShellSnapshot>("get_shell_snapshot", {
    loadoutId: loadoutId ?? null,
    hydrateMetadata: opts?.hydrateMetadata ?? null,
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

/** Preview-only: the order the browser fixture was last dragged into. */
let mockOrder: number[] | null = null;

/**
 * Move `modIds` so the block lands at index `to` of the list. Later rows win
 * file conflicts, so a higher index means a higher priority. The Core does the
 * real work (one transaction); this twin only exists for the browser preview.
 */
function moveBlock(ids: number[], moved: number[], to: number): number[] {
  const block = ids.filter((id) => moved.includes(id));
  const rest = ids.filter((id) => !moved.includes(id));
  const at = Math.max(0, Math.min(to, rest.length));
  return [...rest.slice(0, at), ...block, ...rest.slice(at)];
}

/**
 * Change the priority order of an instance: the mods in `modIds` go to index
 * `to` of the list (the last row wins conflicts). One transaction in the Core,
 * journaled, undone by "Annuler les modifications". Returns the fresh entries.
 */
export async function moveMods(
  loadoutId: number,
  modIds: number[],
  to: number,
): Promise<ModRow[]> {
  if (!isTauri()) {
    const base = mockSnapshot().entries;
    const ids = mockOrder ?? base.map((e) => e.modId);
    mockOrder = moveBlock(ids, modIds, to);
    const byId = new Map(base.map((e) => [e.modId, e]));
    return mockOrder.map((id, position) => ({ ...byId.get(id)!, position }));
  }
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<ModRow[]>("move_mods", { loadoutId, modIds, to });
}

/** Create an empty instance through the Core (or a preview-only fixture). */
export async function createLoadout(
  gameId: string,
  name: string,
): Promise<LoadoutDto> {
  const cleanName = name.trim();
  if (!cleanName) throw new Error("Donne un nom à l’instance.");
  if (!isTauri()) {
    const id = Math.floor(Date.now() / 1000);
    return {
      id,
      gameId,
      name: "principale",
      instanceId: id,
      instanceName: cleanName,
      modCount: 0,
      channel: "stable",
      revision: 0,
      mcLoader: null,
      mcGameVersion: null,
      mcLoaderVersion: null,
    };
  }
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<LoadoutDto>("create_loadout", { gameId, name: cleanName });
}

/** Create a branch of the instance that owns `loadoutId`. Not a new instance. */
export async function createBranch(loadoutId: number, name: string): Promise<LoadoutDto> {
  const cleanName = name.trim();
  if (!cleanName) throw new Error("Donne un nom à la branche.");
  if (!isTauri()) {
    const source = mockSnapshot().loadouts.find((l) => l.id === loadoutId);
    return {
      id: Math.floor(Date.now() / 1000),
      gameId: source?.gameId ?? "skyrimse",
      name: cleanName,
      instanceId: source?.instanceId ?? loadoutId,
      instanceName: source?.instanceName ?? "Instance",
      modCount: source?.modCount ?? 0,
      channel: source?.channel ?? "stable",
      revision: 0,
      mcLoader: source?.mcLoader ?? null,
      mcGameVersion: source?.mcGameVersion ?? null,
      mcLoaderVersion: source?.mcLoaderVersion ?? null,
    };
  }
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<LoadoutDto>("create_branch", { loadoutId, name: cleanName });
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

export async function setBranchMcLoader(loadoutId: number, loader: McLoaderId): Promise<void> {
  if (!isTauri()) throw new Error("Disponible uniquement dans le shell Tauri.");
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke("set_branch_mc_loader", { loadoutId, loader });
}

export async function setBranchMcGameVersion(loadoutId: number, version: string): Promise<void> {
  if (!isTauri()) throw new Error("Disponible uniquement dans le shell Tauri.");
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke("set_branch_mc_game_version", { loadoutId, version });
}

export async function setBranchMcLoaderVersion(loadoutId: number, version: string): Promise<void> {
  if (!isTauri()) throw new Error("Disponible uniquement dans le shell Tauri.");
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke("set_branch_mc_loader_version", { loadoutId, version });
}

export async function listMcGameVersions(): Promise<string[]> {
  if (!isTauri()) return [];
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<string[]>("list_mc_game_versions");
}

export async function listMcLoaderVersions(loader: McLoaderId, mcVersion: string): Promise<string[]> {
  if (!isTauri()) return [];
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<string[]>("list_mc_loader_versions", { loader, mcVersion });
}

/** Loader builds already on disk for this instance + Minecraft version. */
export async function listInstalledMcLoaderVersions(
  loadoutId: number,
  loader: McLoaderId,
  mcVersion: string,
): Promise<string[]> {
  if (!isTauri()) return [];
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<string[]>("list_installed_mc_loader_versions", {
    loadoutId,
    loader,
    mcVersion,
  });
}

/** Vanilla game version folders already on disk for this instance. */
export async function listInstalledMcGameVersions(loadoutId: number): Promise<string[]> {
  if (!isTauri()) return [];
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<string[]>("list_installed_mc_game_versions", { loadoutId });
}

export async function mcLoaderStatus(loadoutId: number): Promise<McLoaderStatus> {
  if (!isTauri()) return { state: "unset", message: "—" };
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<McLoaderStatus>("mc_loader_status", { loadoutId });
}

/**
 * Install (or reinstall) the branch loader. Pass `force: true` to re-run the
 * pipeline even when `versions_installed` already says the build is present.
 */
export async function installBranchMcLoader(
  loadoutId: number,
  options?: { force?: boolean },
): Promise<void> {
  if (!isTauri()) throw new Error("Disponible uniquement dans le shell Tauri.");
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke("install_branch_mc_loader", {
    loadoutId,
    force: options?.force ?? false,
  });
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

/** Progress hit 100 % but Archive never arrived — promote or abandon. */
export type FinalizeDownloadOutcome =
  | "completed"
  | "notReady"
  | "alreadyDone"
  | "abandoned";

export async function finalizeDownload(
  downloadId: number,
  giveUp = false,
): Promise<FinalizeDownloadOutcome> {
  if (!isTauri()) return "alreadyDone";
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<FinalizeDownloadOutcome>("finalize_download", {
    downloadId,
    giveUp,
  });
}

/** Categories this game offers, with their icon names. Empty when the game is unknown. */
export async function gameCategories(gameId: string): Promise<GameCategory[]> {
  if (!isTauri() || !gameId) return [];
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<GameCategory[]>("game_categories", { gameId });
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

/**
 * Replay local metadata (file name, archive, installed folder) on one mod.
 * Info calls this when it opens. Returns the loadout's rows afterwards.
 * `null` in the browser preview, where there is no database to refresh.
 */
export async function refreshModMetadata(
  loadoutId: number,
  modId: number,
): Promise<ModRow[] | null> {
  if (!isTauri()) return null;
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<ModRow[]>("refresh_mod_metadata", { loadoutId, modId });
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
 * Soft-delete from the table. Minecraft also removes the jar from the
 * instance `mods/` (`.jar` / `.jar.disabled` → data-root trash). Skyrim
 * leaves managed folders alone (see `download::remove` / `delete_mod`).
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

/**
 * Pages officielles à ouvrir dans le navigateur du système
 * (`openExternalUrl`). Le shell Tauri sert la même liste
 * (`official_tool_pages`) ; ces deux adresses sont la copie pour l'aperçu
 * navigateur et pour un appel qui ne veut pas attendre le Core.
 * À garder alignées avec `tool_archive::OFFICIAL_TOOL_PAGES`.
 */
export const OFFICIAL_TOOL_PAGES = [
  { id: "skse", name: "SKSE", url: "https://skse.silverlock.org/" },
  { id: "loot", name: "LOOT", url: "https://loot.github.io/" },
] as const;

export type OfficialToolPage = {
  id: string;
  name: string;
  url: string;
};

/** SKSE et LOOT : les pages que l'UI peut ouvrir, pas des téléchargements. */
export async function officialToolPages(): Promise<OfficialToolPage[]> {
  if (!isTauri()) return OFFICIAL_TOOL_PAGES.map((page) => ({ ...page }));
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<OfficialToolPage[]>("official_tool_pages");
}

/**
 * Ouvre une adresse http(s) dans le navigateur du système.
 * Dans le shell, seule la commande `open_external_url` est utilisée :
 * une adresse refusée remonte à l'appelant. Hors Tauri, l'aperçu ouvre un onglet.
 */
export async function openExternalUrl(url: string): Promise<void> {
  if (!isTauri()) {
    const opened = window.open(url, "_blank", "noopener,noreferrer");
    if (!opened) throw new Error("Impossible d’ouvrir le navigateur.");
    return;
  }
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke("open_external_url", { url });
}

/** Open Explorer at `path`. Directories open as folders; files are always
 * shown with `/select,` on Windows. */
export async function revealPath(path: string, select = false): Promise<void> {
  if (!isTauri()) throw new Error("Disponible uniquement dans le shell Tauri.");
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke("reveal_path", { path, select });
}

/** One-shot PowerShell for Console → PowerShell (UI REPL, not MCP). */
export type PowerShellResult = {
  stdout: string;
  stderr: string;
  exitCode: number;
  cwd: string;
};

/** Run `command` via `powershell -NoProfile -Command`. Optional `cwd` sticks across calls. */
export async function runPowerShell(
  command: string,
  cwd?: string | null,
): Promise<PowerShellResult> {
  if (!isTauri()) {
    throw new Error("PowerShell n’est disponible que dans le shell Tauri.");
  }
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<PowerShellResult>("run_powershell", {
    command,
    cwd: cwd ?? null,
  });
}

/** Open the per-instance folder for this branch (Minecraft launcher tree, …). */
export async function revealInstanceFolder(loadoutId: number): Promise<void> {
  if (!isTauri()) throw new Error("Disponible uniquement dans le shell Tauri.");
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke("reveal_instance_folder", { loadoutId });
}

/**
 * Native open dialog. `dir` picks a folder, `file` picks one file.
 * `null` means the user cancelled. The path is whatever the dialog returns;
 * callers save it through the existing override commands.
 */
export async function pickPath(kind: PathKind, current: string | null): Promise<string | null> {
  if (!isTauri()) {
    throw new Error("La sélection d’un chemin n’est disponible que dans le shell Tauri.");
  }
  const { open } = await import("@tauri-apps/plugin-dialog");
  const defaultPath = current?.trim() || undefined;
  const selected = await openDialog(open, kind, defaultPath);
  if (typeof selected === "string") return selected;
  if (Array.isArray(selected)) return selected[0] ?? null;
  return null;
}

async function openDialog(
  open: typeof import("@tauri-apps/plugin-dialog").open,
  kind: PathKind,
  defaultPath: string | undefined,
): Promise<string | string[] | null> {
  const title = kind === "dir" ? "Choisir un dossier" : "Choisir un fichier";
  try {
    if (kind === "dir") {
      return await open({
        directory: true,
        multiple: false,
        title,
        defaultPath,
        canCreateDirectories: true,
      });
    }
    return await open({
      directory: false,
      multiple: false,
      title,
      defaultPath,
      filters: [{ name: "Fichiers INI", extensions: ["ini"] }],
    });
  } catch (error) {
    if (!defaultPath) throw error;
    if (kind === "dir") {
      return await open({
        directory: true,
        multiple: false,
        title,
        canCreateDirectories: true,
      });
    }
    return await open({
      directory: false,
      multiple: false,
      title,
      filters: [{ name: "Fichiers INI", extensions: ["ini"] }],
    });
  }
}

/** What an archive turned out to contain, once unpacked to staging. */
export type ArchivePlan = {
  modId: number;
  name: string;
  shape: string;
  needsWizard: boolean;
  stagedPath: string;
  contentRoot: string;
  /** French sentence when this game cannot install the file. */
  refusal: string | null;
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
 * One settled screen of the FOMOD wizard. Types, visibility and the invalid
 * hint are computed in Rust from the branch and the checked plugin ids.
 */
export type FomodGroupType =
  | "select-exactly-one"
  | "select-at-most-one"
  | "select-at-least-one"
  | "select-any"
  | "select-all";

export type FomodPluginType =
  | "required"
  | "optional"
  | "recommended"
  | "not-usable"
  | "could-be-usable";

export type FomodPlugin = {
  id: string;
  name: string;
  description: string;
  image: string | null;
  typeName: FomodPluginType;
  selected: boolean;
  locked: boolean;
  reason: string | null;
};
export type FomodGroup = {
  id: string;
  name: string;
  groupType: FomodGroupType;
  hint: string | null;
  plugins: FomodPlugin[];
};
export type FomodStep = { id: string; name: string; visible: boolean; groups: FomodGroup[] };
export type FomodImage = { path: string; show: boolean; height: number | null };
export type FomodModule = { name: string; image: FomodImage | null; steps: FomodStep[] };

export async function getFomodModule(contentRoot: string, loadoutId: number): Promise<FomodModule> {
  if (!isTauri()) throw new Error("Disponible uniquement dans le shell Tauri.");
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<FomodModule>("get_fomod_module", { contentRoot, loadoutId });
}

/** Re-settle the same screen after a tick. Recommended options are not put back. */
export async function previewFomod(
  contentRoot: string,
  loadoutId: number,
  picked: string[],
): Promise<FomodModule> {
  if (!isTauri()) throw new Error("Disponible uniquement dans le shell Tauri.");
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<FomodModule>("preview_fomod", { contentRoot, loadoutId, picked });
}

/** Image bytes from the extracted archive, as a data URL. `null` when there is none. */
export async function fomodImage(contentRoot: string, path: string): Promise<string | null> {
  if (!isTauri()) throw new Error("Disponible uniquement dans le shell Tauri.");
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<string | null>("fomod_image", { contentRoot, path });
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

/** Fires the moment a browser download is filed into an instance. */
export type DownloadStarted = {
  downloadId: number;
  modId: number;
  loadoutId: number;
  fileName: string;
  displayName: string;
  /** Listing page to restore after the transfer leaves for a CDN URL. */
  pageUrl?: string | null;
};

/** Fires when a browser download fails or is cancelled. The row is already gone. */
export type DownloadFailed = {
  fileName: string;
  loadoutId: number | null;
  /** French sentence from the shell; may be empty. */
  message?: string;
};

/**
 * Fires when a browser download has landed.
 * `alreadyFiled` — the row was created at start and just became an archive.
 * The shell refreshes; it must not attach a second mod.
 */
export type CapturedDownload = {
  downloadId: number;
  fileName: string;
  url: string;
  path: string;
  alreadyFiled: boolean;
  loadoutId: number | null;
  displayName: string;
  /** Present when begin already filed the mod — UI can leave « Téléchargement » immediately. */
  modId?: number | null;
};

/** Tell the Core which instance is on screen, so a download can be filed at start. */
export async function noteActiveLoadout(loadoutId: number | null): Promise<void> {
  if (!isTauri()) return;
  const { invoke } = await import("@tauri-apps/api/core");
  await invoke("note_active_loadout", { loadoutId });
}

/** A main-frame load in the embedded browser started (`loading`) or finished. */
export type BrowserNavigated = { url: string; loading: boolean };

/** Returns an unsubscribe function; a no-op outside Tauri. */
export async function onBrowserNavigated(
  handler: (event: BrowserNavigated) => void,
): Promise<() => void> {
  if (!isTauri()) return () => {};
  const { listen } = await import("@tauri-apps/api/event");
  return listen<BrowserNavigated>("browser://navigated", (e) => handler(e.payload));
}

/** Returns an unsubscribe function; a no-op outside Tauri. */
export async function onDownloadStarted(
  handler: (event: DownloadStarted) => void,
): Promise<() => void> {
  if (!isTauri()) return () => {};
  const { listen } = await import("@tauri-apps/api/event");
  const unlisten = await listen<DownloadStarted>("download://started", (e) => handler(e.payload));
  return unlisten;
}

/** Returns an unsubscribe function; a no-op outside Tauri. */
export async function onDownloadFailed(
  handler: (event: DownloadFailed) => void,
): Promise<() => void> {
  if (!isTauri()) return () => {};
  const { listen } = await import("@tauri-apps/api/event");
  const unlisten = await listen<DownloadFailed>("download://failed", (e) => handler(e.payload));
  return unlisten;
}

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

/** Live byte count. Not stored: the shell forgets it when the transfer ends. */
export type DownloadProgress = {
  downloadId: number | null;
  url: string;
  received: number;
  total: number | null;
};

/** Returns an unsubscribe function; a no-op outside Tauri. */
export async function onDownloadProgress(
  handler: (event: DownloadProgress) => void,
): Promise<() => void> {
  if (!isTauri()) return () => {};
  const { listen } = await import("@tauri-apps/api/event");
  const unlisten = await listen<DownloadProgress>("download://progress", (e) =>
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

export type MinecraftSyncResult = {
  added: number;
  entries: ModRow[];
};

/** Register jars already in the instance `mods/` that the loadout does not list. */
export async function minecraftSyncMods(loadoutId: number): Promise<MinecraftSyncResult> {
  if (!isTauri()) {
    throw new Error("La synchro mods/ n'est disponible que dans le shell Tauri.");
  }
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<MinecraftSyncResult>("minecraft_sync_mods", { loadoutId });
}

export type ImportManualResult = {
  added: number;
  entries: ModRow[];
};

/**
 * Add a human-picked file or folder to the loadout as a manual mod
 * (`source_host = manual`). Minecraft copies/registers jars; Skyrim takes a
 * mod folder or an archive.
 */
export async function importManualMod(
  loadoutId: number,
  path: string,
): Promise<ImportManualResult> {
  if (!isTauri()) {
    throw new Error("L'import manuel n'est disponible que dans le shell Tauri.");
  }
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<ImportManualResult>("import_manual_mod", { loadoutId, path });
}

/**
 * Native file (or folder) picker for a manual mod add. `mode` chooses the
 * dialog; filters apply only to files.
 */
export async function pickManualModPath(
  mode: "file" | "dir",
  filters?: { name: string; extensions: string[] }[],
): Promise<string | null> {
  if (!isTauri()) {
    throw new Error("La sélection d’un chemin n’est disponible que dans le shell Tauri.");
  }
  const { open } = await import("@tauri-apps/plugin-dialog");
  if (mode === "dir") {
    const selected = await open({
      directory: true,
      multiple: false,
      title: "Choisir un dossier de mod",
      canCreateDirectories: false,
    });
    if (typeof selected === "string") return selected;
    if (Array.isArray(selected)) return selected[0] ?? null;
    return null;
  }
  const selected = await open({
    directory: false,
    multiple: false,
    title: "Choisir un fichier de mod",
    filters: filters ?? [{ name: "Mods", extensions: ["jar", "zip", "7z", "rar"] }],
  });
  if (typeof selected === "string") return selected;
  if (Array.isArray(selected)) return selected[0] ?? null;
  return null;
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

/**
 * Starts the game through SKSE and returns as soon as the game process
 * exists — or rejects with the (French) reason it was refused: SKSE absent
 * or built for another game version, a session already running... It does
 * not wait for the end of the game: `onGameEnded` / `getGameSessionState`
 * tell when that happens.
 */
export async function launchSkse(loadoutId: number): Promise<void> {
  if (!isTauri()) {
    throw new Error("Le lancement SKSE n’est disponible que dans le shell Tauri.");
  }
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke("launch_skse", { loadoutId });
}

/** Start a ModLab-owned JVM session (MSA online when connected; else offline pseudo).
 * Resolves when the process has started; session end is `game://ended`.
 * Live stdout/stderr arrive on `game://log` (`onGameLog`). */
export async function launchMinecraft(
  loadoutId: number,
  playerName?: string | null,
): Promise<string | null> {
  if (!isTauri()) {
    throw new Error("Le lancement Minecraft n’est disponible que dans le shell Tauri.");
  }
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<string | null>("launch_minecraft", {
    loadoutId,
    playerName: playerName ?? null,
  });
}

/** Result of « Tester le lancement » (smoke: prepare → spawn → menu or fail → kill). */
export type MinecraftSmokeResult = {
  status: "ok" | "fail" | "timeout" | "stopped" | string;
  message: string;
  matchedMarker: string | null;
};

/** Same prepare/spawn/log stream as Jouer, but waits for menu / crash / timeout
 * then kills the JVM. Resolves when the test finishes. */
export async function testLaunchMinecraft(
  loadoutId: number,
  playerName?: string | null,
): Promise<MinecraftSmokeResult> {
  if (!isTauri()) {
    throw new Error("Le test de lancement n’est disponible que dans le shell Tauri.");
  }
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<MinecraftSmokeResult>("test_launch_minecraft", {
    loadoutId,
    playerName: playerName ?? null,
  });
}

/** Minecraft Microsoft account (auth-code + PKCE). Tokens stay in the shell store. */
export type MsaAccountStatus = {
  connected: boolean;
  username: string | null;
  uuid: string | null;
  clientConfigured: boolean;
  /** Effective MSA application (client) id (public Minecraft/Prism when unset). */
  clientId: string | null;
  /** `env` = MODLAB_MSA_CLIENT_ID ; `default` = client public Minecraft/Prism ; `ui` = legacy override. */
  clientIdSource: "ui" | "env" | "default" | string | null;
  /** Legacy UI override if any (Settings no longer exposes this). */
  clientIdOverride: string | null;
  mode: "online" | "offline" | "error" | string;
  error: string | null;
};

/** Start of a browser login — open `authUrl`, then poll until connected. */
export type MsaLoginStart = {
  authUrl: string;
  message: string;
  expiresIn: number;
  interval: number;
};

/** @deprecated Use MsaLoginStart — kept as alias during the PKCE switch. */
export type MsaDeviceCode = MsaLoginStart;

export type MsaPollResult = {
  status: "pending" | "slow_down" | "connected" | "declined" | "expired" | "error" | string;
  username: string | null;
  error: string | null;
};

export async function msaAccountStatus(): Promise<MsaAccountStatus> {
  if (!isTauri()) {
    return {
      connected: false,
      username: null,
      uuid: null,
      clientConfigured: false,
      clientId: null,
      clientIdSource: null,
      clientIdOverride: null,
      mode: "offline",
      error: "Disponible uniquement dans le shell Tauri.",
    };
  }
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<MsaAccountStatus>("msa_account_status");
}

export async function msaBeginLogin(): Promise<MsaLoginStart> {
  if (!isTauri()) {
    throw new Error("Disponible uniquement dans le shell Tauri.");
  }
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<MsaLoginStart>("msa_begin_login");
}

export async function msaPollLogin(): Promise<MsaPollResult> {
  if (!isTauri()) {
    throw new Error("Disponible uniquement dans le shell Tauri.");
  }
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<MsaPollResult>("msa_poll_login");
}

export async function msaCancelLogin(): Promise<void> {
  if (!isTauri()) return;
  const { invoke } = await import("@tauri-apps/api/core");
  await invoke("msa_cancel_login");
}

export async function msaDisconnect(): Promise<MsaAccountStatus> {
  if (!isTauri()) {
    throw new Error("Disponible uniquement dans le shell Tauri.");
  }
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<MsaAccountStatus>("msa_disconnect");
}

/** Fired when the PKCE loopback thread finishes exchange (success or error). */
export type MsaLoginFinished = {
  status: "connected" | "declined" | "expired" | "error" | string;
  username: string | null;
  error: string | null;
};

/** Returns an unsubscribe function; a no-op outside Tauri. */
export async function onMsaLoginFinished(
  handler: (event: MsaLoginFinished) => void,
): Promise<() => void> {
  if (!isTauri()) return () => {};
  const { listen } = await import("@tauri-apps/api/event");
  const unlisten = await listen<MsaLoginFinished>("msa://login-finished", (e) =>
    handler(e.payload),
  );
  return unlisten;
}

/**
 * `ok` — loader and the DLL of the game's version are in the game folder;
 * `wrong_version` — SKSE is there but built for another game version;
 * `missing` — no loader, or no runtime DLL; `unverifiable` — the game's own
 * version could not be read, so the match cannot be checked.
 */
export type SkseState = "ok" | "wrong_version" | "missing" | "unverifiable";

/** What the game folder really holds, read from the files — see `skse_status`. */
export type SkseStatus = {
  state: SkseState;
  gameVersion: string | null;
  expectedDll: string | null;
  loaderPresent: boolean;
  dllsFound: string[];
  /** One French sentence saying what is wrong, or that all is well. */
  message: string;
};

/** `null` outside the Tauri shell, or when Skyrim is not installed. */
export async function getSkseStatus(): Promise<SkseStatus | null> {
  if (!isTauri()) return null;
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<SkseStatus | null>("skse_status", {});
}

export type RejectedPlugin = { name: string; reason: string };

/** The balance sheet of one finished game session, from SKSE's own journals. */
export type SessionReport = {
  /** Absent or `skyrim` on older builds; `minecraft` for MC sessions. */
  sessionKind?: "skyrim" | "minecraft" | "minecraft-smoke";
  loadoutId: number;
  /** Unix time, milliseconds. */
  endedAt: number;
  /** A problem while following the session itself, if any. */
  error: string | null;
  logFound: boolean;
  /** `skse64.log` was written during this session (not a leftover). */
  logFresh: boolean;
  skseInitialized: boolean;
  skseVersion: string | null;
  initComplete: boolean;
  loaded: string[];
  rejected: RejectedPlugin[];
  /** Plugins the instance provides that SKSE never even looked at. */
  notSeen: string[];
  loaderLaunched: boolean;
  loaderNotes: string[];
};

export type GameSessionState = {
  running: boolean;
  lastReport: SessionReport | null;
};

export async function getGameSessionState(): Promise<GameSessionState> {
  if (!isTauri()) return { running: false, lastReport: null };
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<GameSessionState>("game_session_state", {});
}

export type StopGameSessionResult = {
  accepted: boolean;
  alreadyStopped: boolean;
  message: string | null;
};

/** Kill the active game process; `game://ended` follows once teardown completes. */
export async function stopGameSession(): Promise<StopGameSessionResult> {
  if (!isTauri()) {
    return { accepted: false, alreadyStopped: true, message: null };
  }
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<StopGameSessionResult>("stop_game_session", {});
}

/** Fired once the game (and everything it started) has fully exited. */
export async function onGameEnded(handler: (report: SessionReport) => void): Promise<() => void> {
  if (!isTauri()) return () => {};
  const { listen } = await import("@tauri-apps/api/event");
  return listen<SessionReport>("game://ended", (e) => handler(e.payload));
}

/** One chunk of Minecraft JVM stdout/stderr during an active session. */
export type GameLogChunk = {
  stream: "stdout" | "stderr" | string;
  text: string;
};

/** Live process log while Jouer owns the JVM (`game://log`). */
export async function onGameLog(handler: (chunk: GameLogChunk) => void): Promise<() => void> {
  if (!isTauri()) return () => {};
  const { listen } = await import("@tauri-apps/api/event");
  return listen<GameLogChunk>("game://log", (e) => handler(e.payload));
}

/** French hint while Minecraft is still running (e.g. Sodium Fatal Startup). */
export type GameLaunchHint = {
  message: string;
};

/** Known mod-mismatch hint from live JVM logs — before `game://ended`. */
export async function onGameLaunchHint(
  handler: (hint: GameLaunchHint) => void,
): Promise<() => void> {
  if (!isTauri()) return () => {};
  const { listen } = await import("@tauri-apps/api/event");
  return listen<GameLaunchHint>("game://launch-hint", (e) => handler(e.payload));
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

export type GamePathId = "game_dir" | "mods_dir" | "downloads_dir" | "config_dir";

/** Badge source for `ManagedPath`. A built-in folder is `default`, not `detected`. */
export type GamePathOrigin = "detected" | "override" | "default" | "missing";

/**
 * Which existing command writes this path back. `null` means the folder is
 * derived and no override is honored.
 * - `gamePathOverride`: `setGamePathOverride(gameId, slotId, editPath)`
 * - `downloadsDirOverride`: `setDownloadsDirOverride(editPath)` — `editPath`
 *   is the shared downloads root, not the per-game folder in `effective`.
 */
export type GamePathUpdate =
  | { via: "gamePathOverride"; slotId: string }
  | { via: "downloadsDirOverride" }
  | { via: "instancePathOverride"; instanceId: number; slotId: string };

/**
 * One location from `getGamePaths`. Show `effective`. Badge from `origin`.
 * Open it only when `exists` is true. An editor loads `editPath` and saves
 * through `update`.
 */
export type ManagedPath = {
  id: GamePathId;
  label: string;
  kind: PathKind;
  detected: string | null;
  overridePath: string | null;
  effective: string | null;
  exists: boolean;
  origin: GamePathOrigin;
  editPath: string | null;
  update: GamePathUpdate | null;
};

/** One `.ini` under `configDir`. `slotId` is set for `prefs_ini` only. */
export type GameIniFile = {
  name: string;
  path: string | null;
  present: boolean;
  slotId: string | null;
};

/**
 * The four paths for one game. `modsDir` for Skyrim is `<data root>/mods`
 * (archive installs). MO2 imports keep each row's `sourcePath`. Minecraft's
 * `modsDir` is `<instance>/mods`. `downloadsDir.effective` is the per-game
 * subfolder; `downloadsDir.editPath` is the shared root.
 */
export type GamePaths = {
  gameId: string;
  gameDir: ManagedPath;
  modsDir: ManagedPath;
  downloadsDir: ManagedPath;
  configDir: ManagedPath;
  iniFiles: GameIniFile[];
  /** Launcher folder, shown when installs go to a per-instance directory. */
  referenceDir: ManagedPath | null;
};

/**
 * Install folder, installed-mods folder, this game's downloads folder, and
 * the INI config folder. Read-only. Lecture tier (§7.2).
 */
export async function getGamePaths(
  gameId: string,
  loadoutId?: number | null,
): Promise<GamePaths> {
  if (!isTauri()) return previewGamePaths(gameId);
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<GamePaths>("game_paths", { gameId, loadoutId: loadoutId ?? null });
}

/** One child of the game's install folder (path slot `root`). */
export type GameInstallEntry = {
  name: string;
  kind: PathKind;
};

/**
 * Immediate children of one folder inside the active game's install
 * directory. `relativePath` is relative to that install folder; empty (the
 * default) lists the folder itself. The Core refuses a path that climbs
 * above it. `null` outside the Tauri shell: the preview has a path from
 * `getGamePathSlots` but no folder to read, and this does not invent names.
 * An empty list is a real empty folder (or a game with no install path —
 * the page checks the slot first and says so itself).
 */
export async function listGameInstallEntries(
  gameId: string,
  relativePath = "",
): Promise<GameInstallEntry[] | null> {
  if (!isTauri()) return null;
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<GameInstallEntry[]>("game_install_entries", { gameId, relativePath });
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

/** Point one slot of one instance at a folder. Nothing already installed is moved. */
export async function setInstancePathOverride(
  instanceId: number,
  slotId: string,
  path: string,
): Promise<void> {
  if (!isTauri()) {
    throw new Error("La modification des chemins n'est disponible que dans le shell Tauri.");
  }
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke("set_instance_path_override", { instanceId, slotId, path });
}

/** Drop a human override for one instance slot. The stored folder stays. */
export async function clearInstancePathOverride(
  instanceId: number,
  slotId: string,
): Promise<void> {
  if (!isTauri()) {
    throw new Error("La modification des chemins n'est disponible que dans le shell Tauri.");
  }
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke("clear_instance_path_override", { instanceId, slotId });
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

/** Where a vanilla config or an unreadable path is relative to. */
export type VanillaLocation = "data" | "documents" | "install";

/** One top-level `Data/` name that is not part of a vanilla install. */
export type VanillaExtra = {
  path: string;
  kind: PathKind;
};

/** A config kept apart from `extras`. `path` is relative to `location`. */
export type VanillaConfig = {
  path: string;
  location: VanillaLocation;
};

/** One path the scan could not read. */
export type VanillaUnreadable = {
  path: string;
  location: VanillaLocation;
  reason: string;
};

/**
 * Read-only vanilla check. `scanned` is false when no `Data/` path is known —
 * not the same as a scan that found nothing. `documentsScanned` is false when
 * the INI folder could not be opened.
 */
export type VanillaScan = {
  scanned: boolean;
  documentsScanned: boolean;
  extras: VanillaExtra[];
  configs: VanillaConfig[];
  unreadable: VanillaUnreadable[];
};

export type VanillaControlView =
  | { status: "ready"; scan: VanillaScan }
  | { status: "error"; message: string };

/**
 * What in this game's `Data/` (and, separately, its INI folder) is not part
 * of a vanilla install. Read-only: nothing is deleted. Invoke `game_vanilla_scan`.
 */
export async function getVanillaControl(gameId: string): Promise<VanillaControlView> {
  if (!isTauri()) return { status: "ready", scan: previewVanillaScan() };
  try {
    const { invoke } = await import("@tauri-apps/api/core");
    const scan = await invoke<VanillaScan>("game_vanilla_scan", { gameId });
    return { status: "ready", scan };
  } catch (error) {
    return { status: "error", message: errorText(error) };
  }
}

function errorText(error: unknown): string {
  if (typeof error === "string" && error.trim()) return error;
  if (error instanceof Error && error.message) return error.message;
  if (error && typeof error === "object" && "message" in error) {
    const message = (error as { message: unknown }).message;
    if (typeof message === "string" && message.trim()) return message;
  }
  return "Lecture impossible.";
}

/** Browser preview only — same rule as `previewPathSlots`. Not a scan. */
function previewVanillaScan(): VanillaScan {
  return {
    scanned: true,
    documentsScanned: true,
    extras: [
      { path: "Meshes", kind: "dir" },
      { path: "Mod.esp", kind: "file" },
    ],
    configs: [
      { path: "foo.ini", location: "data" },
      { path: "SKSE/skse.ini", location: "documents" },
    ],
    unreadable: [],
  };
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

/** `active` is satisfied, `disabled` is present but off, `missing` is absent. */
export type DependencyState = "active" | "disabled" | "missing";

/** How a resolved dependency relates to the mod. Absent → treat as required. */
export type DependencyKind =
  | "required"
  | "optional"
  | "incompatible"
  | "discouraged"
  | "usedBy";

/** One dependency the game adapter could resolve. Silence means it does not know. */
export type ResolvedDependency = {
  name: string;
  state: DependencyState;
  /** Mod of this branch that provides it, when one does. */
  providerModId: number | null;
  providerName: string | null;
  /** Required / optional / … / used-by. Older payloads omit it → required. */
  kind?: DependencyKind;
};

export type ModDependencyReport = {
  modId: number;
  dependencies: ResolvedDependency[];
};

/**
 * Dependencies of the active branch. Same moment as the conflict scan:
 * again when a mod is enabled or disabled. An empty list is a finished
 * scan that found nothing, not a skipped one.
 */
export async function getLoadoutDependencies(
  loadoutId: number,
): Promise<ModDependencyReport[]> {
  if (!isTauri()) return [];
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<ModDependencyReport[]>("get_loadout_dependencies", { loadoutId });
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

/** Resolved paths of the enabled mods' files — the merged Data view. */
export async function getLoadoutDirectory(loadoutId: number): Promise<DirectoryEntry[]> {
  if (!isTauri()) return [];
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<DirectoryEntry[]>("loadout_directory", { loadoutId });
}

/** One plugin at its place in the load order ModLab hands the game. */
export type PluginRow = {
  fileName: string;
  /** Official plugin (base game / DLC / Creation Club): first, always active. */
  official: boolean;
  /** `null` when the file comes from the game's own Data folder. */
  modId: number | null;
  modName: string | null;
  contributorCount: number;
  master: boolean;
  light: boolean;
  /** Written with `*` in plugins.txt. `false` = cannot load (see issues). */
  active: boolean;
  missingMasters: string[];
  readable: boolean;
};

/** Stable codes from the Core (`IssueKind::code`); the page words them. */
export type PluginIssueKind =
  | "missing_master"
  | "inactive_master"
  | "unreadable"
  | "master_cycle"
  | "master_after_non_master"
  | "full_limit"
  | "light_limit"
  | "unencodable_name";

export type PluginIssue = {
  kind: PluginIssueKind;
  plugin: string;
  detail: string | null;
};

export type PluginsView = {
  plugins: PluginRow[];
  issues: PluginIssue[];
  activeFull: number;
  activeLight: number;
  fullLimit: number;
  lightLimit: number;
};

/**
 * Browser-preview load order. Only used outside the Tauri shell, and only so
 * the Plugins tab can show the same pills the real list uses. Ids match
 * `mockSnapshot`'s installed mods (4 USSEP, 5 SkyUI, 6 RaceMenu).
 */
function previewPlugins(): PluginsView {
  const row = (
    fileName: string,
    extra: Pick<PluginRow, "official" | "modId" | "modName" | "master" | "light">,
  ): PluginRow => ({
    fileName,
    contributorCount: 1,
    active: true,
    missingMasters: [],
    readable: true,
    ...extra,
  });
  return {
    plugins: [
      row("Skyrim.esm", { official: true, modId: null, modName: null, master: true, light: false }),
      row("Update.esm", { official: true, modId: null, modName: null, master: true, light: false }),
      row("Unofficial Skyrim Special Edition Patch.esp", {
        official: false,
        modId: 4,
        modName: "Unofficial Skyrim Special Edition Patch",
        master: true,
        light: false,
      }),
      row("SkyUI_SE.esp", {
        official: false,
        modId: 5,
        modName: "SkyUI",
        master: false,
        light: false,
      }),
      row("RaceMenu.esl", {
        official: false,
        modId: 6,
        modName: "RaceMenu",
        master: true,
        light: true,
      }),
    ],
    issues: [],
    activeFull: 4,
    activeLight: 1,
    fullLimit: 255,
    lightLimit: 4096,
  };
}

/** The Plugins tab: the load order computed by the Skyrim adapter from the
 * enabled mods (masters first, missing masters reported). Same computation
 * that writes plugins.txt at launch. LOOT is not run, and this does not read
 * a plugins.txt back. */
export async function getLoadoutPlugins(loadoutId: number): Promise<PluginsView> {
  if (!isTauri()) {
    return previewPlugins();
  }
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<PluginsView>("loadout_plugins", { loadoutId });
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

/** Dependencies of one mod. Same scan as `getLoadoutDependencies`. */
export async function getModDependencies(
  loadoutId: number,
  modId: number,
): Promise<ResolvedDependency[]> {
  if (!isTauri()) return [];
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<ResolvedDependency[]>("mod_dependencies", { loadoutId, modId });
}

/** A config file found in the selected mod's own folder. */
export type ModConfigFile = {
  /** File name only. */
  name: string;
  /** Path relative to the mod folder, `/`-separated. Not a game `Data/` path. */
  relativePath: string;
  /**
   * UTF-8 body when the file is at most 64 KiB and textual.
   * `null` when there is nothing to show — the file is still listed.
   */
  text: string | null;
};

/** Config files this mod placed. Empty when it placed none. Read-only. */
export async function getModConfigFiles(
  loadoutId: number,
  modId: number,
): Promise<ModConfigFile[]> {
  if (!isTauri()) return [];
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<ModConfigFile[]>("mod_config_files", { loadoutId, modId });
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

/** Browser-preview `dataRoot`. Other screens may still show it; the profile row does not. */
export const BROWSER_PREVIEW_DATA_ROOT = "(aperçu navigateur — pas le Core)";

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
  const sizes: (number | null)[] = [
    840,
    Math.round(12.3 * 1024),
    null,
    Math.round(12.3 * 1024 * 1024),
    Math.round(1.2 * 1024 * 1024 * 1024),
    4096,
    88 * 1024,
    Math.round(2.5 * 1024 * 1024),
    640,
    900 * 1024 * 1024,
  ];
  // Several months, on purpose: the cell shows the month, the full instant
  // (day included) is what a sort would use. Archived and in-progress rows
  // have no install date. Every row still has a download date, including
  // the one whose transfer has only just started, and it is not the same
  // instant as the install.
  const installedAt = [
    "2026-11-03T18:42:00.000Z",
    null,
    null,
    "2026-08-14T09:05:00.000Z",
    "2026-01-22T21:10:00.000Z",
    "2025-12-01T08:00:00.000Z",
    "2026-03-19T16:30:00.000Z",
    "2025-06-02T11:15:00.000Z",
    "2026-09-28T07:48:00.000Z",
    "2024-11-11T22:05:00.000Z",
  ];
  const downloadedAt = [
    "2026-10-28T14:12:00.000Z",
    "2026-11-20T09:00:00.000Z",
    "2026-11-25T04:40:00.000Z",
    "2026-07-02T11:20:00.000Z",
    "2025-12-18T19:45:00.000Z",
    "2025-11-03T08:30:00.000Z",
    "2026-02-14T13:05:00.000Z",
    "2025-04-09T16:55:00.000Z",
    "2026-08-21T06:18:00.000Z",
    "2024-09-30T21:40:00.000Z",
  ];
  // Jeux column preview: one loader, several loaders, a version range,
  // a Skyrim build, and a row that declared nothing.
  const gameVersions: string[][] = [
    ["1.20.1"],
    ["1.20.1"],
    ["1.21.1"],
    ["1.20.1"],
    ["1.20", "1.20.1", "1.21"],
    [],
    ["1.6.1170"],
    [],
    [],
    [],
  ];
  const loaders: string[][] = [
    ["fabric"],
    ["forge"],
    ["neoforge"],
    ["fabric", "quilt"],
    ["fabric"],
    [],
    [],
    [],
    [],
    [],
  ];
  return {
    dataRoot: BROWSER_PREVIEW_DATA_ROOT,
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
      {
        id: 1,
        gameId: "skyrimse",
        name: "principale",
        instanceId: 1,
        instanceName: "Main 2026",
        modCount: 10,
        channel: "stable",
        revision: 12,
        mcLoader: null,
        mcGameVersion: null,
        mcLoaderVersion: null,
      },
      {
        id: 5,
        gameId: "skyrimse",
        name: "essai",
        instanceId: 1,
        instanceName: "Main 2026",
        modCount: 10,
        channel: "beta",
        revision: 1,
        mcLoader: null,
        mcGameVersion: null,
        mcLoaderVersion: null,
      },
      {
        id: 2,
        gameId: "skyrimse",
        name: "principale",
        instanceId: 2,
        instanceName: "Labo météo",
        modCount: 7,
        channel: "alpha",
        revision: 3,
        mcLoader: null,
        mcGameVersion: null,
        mcLoaderVersion: null,
      },
      {
        id: 3,
        gameId: "skyrimse",
        name: "principale",
        instanceId: 3,
        instanceName: "Bench FG",
        modCount: 4,
        channel: "beta",
        revision: 5,
        mcLoader: null,
        mcGameVersion: null,
        mcLoaderVersion: null,
      },
      {
        id: 4,
        gameId: "minecraft",
        name: "principale",
        instanceId: 4,
        instanceName: "Survie Fabric 1.21",
        modCount: 14,
        channel: "stable",
        revision: 8,
        mcLoader: "fabric",
        mcGameVersion: "1.21",
        mcLoaderVersion: null,
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
      // One archived row and one in-progress download, so the preview shows
      // both states a capture passes through. Everything else came from an
      // import and is therefore installed.
      status:
        i === 1 ? ("archived" as const) : i === 2 ? ("downloading" as const) : ("installed" as const),
      category: i === 1 ? "Éclairage" : null,
      archivePath: i === 1 ? `F:/ModLab/downloads/${name}.7z` : null,
      tool: i === 0 ? "skse" : null,
      toolVersion: null,
      sourceHost: null,
      pageUrl: null,
      gameVersions: gameVersions[i] ?? [],
      loaders: loaders[i] ?? [],
      dependencies: [],
      categoryUserSet: false,
      categoryRaw: null,
      categoryNexusId: null,
      nexusModId: null,
      author: null,
      website: null,
      sizeBytes: sizes[i] ?? null,
      installedAt: installedAt[i] ?? null,
      downloadedAt: downloadedAt[i] ?? null,
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
 * ids (`skyrimse`: root/exe/data_dir/usvfs_dll/prefs_ini/saves_dir, `minecraft`: root/launcher_exe/mods_dir)
 * so the Réglages → Jeux layout can be built and eyeballed without the Core.
 */
function previewPathSlots(gameId: string): GamePathSlot[] {
  if (gameId === "minecraft") {
    return [
      {
        id: "root",
        label: "Dossier du launcher Minecraft",
        kind: "dir",
        detected: "C:\\Users\\dev\\curseforge\\minecraft\\Install",
        overridePath: null,
        effective: "C:\\Users\\dev\\curseforge\\minecraft\\Install",
      },
      {
        id: "launcher_exe",
        label: "Exécutable du launcher Minecraft",
        kind: "file",
        detected: "C:\\Users\\dev\\curseforge\\minecraft\\Install\\minecraft.exe",
        overridePath: null,
        effective: "C:\\Users\\dev\\curseforge\\minecraft\\Install\\minecraft.exe",
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
    {
      id: "prefs_ini",
      label: "SkyrimPrefs.ini",
      kind: "file",
      detected: "C:\\Users\\dev\\Documents\\My Games\\Skyrim Special Edition\\SkyrimPrefs.ini",
      overridePath: null,
      effective: "C:\\Users\\dev\\Documents\\My Games\\Skyrim Special Edition\\SkyrimPrefs.ini",
    },
    {
      id: "saves_dir",
      label: "Dossier des sauvegardes",
      kind: "dir",
      detected: "C:\\Users\\dev\\Documents\\My Games\\Skyrim Special Edition\\Saves",
      overridePath: null,
      effective: "C:\\Users\\dev\\Documents\\My Games\\Skyrim Special Edition\\Saves",
    },
  ];
}

function previewManagedPath(
  id: GamePathId,
  label: string,
  path: string | null,
  origin: GamePathOrigin,
  update: GamePathUpdate | null,
  editPath: string | null = path,
): ManagedPath {
  return {
    id,
    label,
    kind: "dir",
    detected: origin === "override" ? null : path,
    overridePath: origin === "override" ? path : null,
    effective: path,
    exists: path !== null,
    origin,
    editPath: update ? editPath : null,
    update,
  };
}

/**
 * Browser-preview fixture for `getGamePaths`. Same rule as `previewPathSlots`:
 * only outside the Tauri shell. Paths are illustrative, not this machine.
 */
function previewGamePaths(gameId: string): GamePaths {
  if (gameId === "minecraft") {
    const instance = "F:\\ModLab\\instances\\minecraft\\.decube";
    return {
      gameId,
      gameDir: previewManagedPath(
        "game_dir",
        "Dossier de l'instance",
        instance,
        "default",
        { via: "instancePathOverride", instanceId: 1, slotId: "instance_root" },
      ),
      modsDir: previewManagedPath(
        "mods_dir",
        "Dossier mods/",
        `${instance}\\mods`,
        "default",
        { via: "instancePathOverride", instanceId: 1, slotId: "mods_dir" },
      ),
      downloadsDir: previewManagedPath(
        "downloads_dir",
        "Téléchargements",
        "F:\\ModLab\\downloads\\minecraft",
        "default",
        { via: "downloadsDirOverride" },
        "F:\\ModLab\\downloads",
      ),
      configDir: previewManagedPath(
        "config_dir",
        "Dossier config/",
        `${instance}\\config`,
        "default",
        { via: "instancePathOverride", instanceId: 1, slotId: "config_dir" },
      ),
      iniFiles: [],
      referenceDir: previewManagedPath(
        "game_dir",
        "Dossier du launcher Minecraft",
        "C:\\Users\\dev\\curseforge\\minecraft\\Install",
        "detected",
        { via: "gamePathOverride", slotId: "root" },
      ),
    };
  }
  const root = "C:\\Steam\\Skyrim Special Edition";
  const configs = "C:\\Users\\dev\\Documents\\My Games\\Skyrim Special Edition";
  return {
    gameId,
    gameDir: previewManagedPath("game_dir", "Dossier d'installation", root, "detected", {
      via: "gamePathOverride",
      slotId: "root",
    }),
    modsDir: previewManagedPath("mods_dir", "Mods installés", "F:\\ModLab\\mods", "default", null),
    downloadsDir: previewManagedPath(
      "downloads_dir",
      "Téléchargements",
      "F:\\ModLab\\downloads\\skyrimse",
      "default",
      { via: "downloadsDirOverride" },
      "F:\\ModLab\\downloads",
    ),
    configDir: previewManagedPath("config_dir", "Configs (.ini)", configs, "detected", null),
    iniFiles: [
      { name: "Skyrim.ini", path: `${configs}\\Skyrim.ini`, present: true, slotId: null },
      {
        name: "SkyrimPrefs.ini",
        path: `${configs}\\SkyrimPrefs.ini`,
        present: true,
        slotId: "prefs_ini",
      },
      {
        name: "SkyrimCustom.ini",
        path: `${configs}\\SkyrimCustom.ini`,
        present: false,
        slotId: null,
      },
    ],
    referenceDir: null,
  };
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
