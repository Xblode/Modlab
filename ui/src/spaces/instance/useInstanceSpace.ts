import { useEffect, useRef, useState } from "react";
import {
  createBranch,
  createLoadout,
  deleteLoadout,
  getGameSessionState,
  getLoadoutVersion,
  getShellSnapshot,
  importManualMod,
  isTauriShell,
  launchMinecraft,
  launchSkse,
  testLaunchMinecraft,
  stopGameSession,
  mcLoaderStatus,
  minecraftImportInstance,
  minecraftSyncMods,
  mo2Import,
  noteActiveLoadout,
  onGameEnded,
  onGameLaunchHint,
  onGameLog,
  pickManualModPath,
  renameLoadout,
  restoreLoadout,
  revealInstanceFolder,
  rollbackLoadout,
  setBranchMcGameVersion,
  setBranchMcLoader,
  setBranchMcLoaderVersion,
  setLoadoutChannel,
  snapshotLoadout,
  type Channel,
  type DetectedGame,
  type GameLogChunk,
  type LoadoutDto,
  type McLoaderId,
  type ShellSnapshot,
} from "../../api";
import type { CreateInstancePayload } from "../../components/Features/LoadoutDialogs/CreateLoadoutDialog";
import {
  noticeFromInfo,
  noticeFromRefusal,
  noticeFromReport,
  toastFromReport,
  type LaunchNotice,
} from "../../sessionNotice";
import { useMcLoaderFacts } from "../../app/useMcLoaderBranch";
import type { PopoverMenuEntry } from "../../components/ui";
import type { TabId } from "../../gameChrome";
import { expandGameKeepingOthers } from "../../shell/shellNavHelpers";
import { readUiMemory } from "../../uiMemory";

export type UseInstanceSpaceParams = {
  snap: ShellSnapshot | null;
  activeGame: DetectedGame | undefined;
  activeLoadout: LoadoutDto | undefined;
  loadouts: LoadoutDto[];
  loadoutId: number | null;
  setSnap: React.Dispatch<React.SetStateAction<ShellSnapshot | null>>;
  setGameId: (id: string | null) => void;
  setLoadoutId: (id: number | null) => void;
  setExpanded: React.Dispatch<React.SetStateAction<string[]>>;
  setTab: (tab: TabId) => void;
  setError: (msg: string | null) => void;
  setToast: (msg: string | null) => void;
  refresh: (id: number | null, opts?: { hydrateMetadata?: boolean }) => Promise<number | null>;
  setVersionTick: React.Dispatch<React.SetStateAction<number>>;
  refreshTxs: () => void;
  /** Optional: open / focus the Console drawer when Jouer starts (MC). */
  onFocusConsole?: () => void;
};

export function useInstanceSpace(params: UseInstanceSpaceParams) {
  const {
    snap,
    activeGame,
    activeLoadout,
    loadouts,
    loadoutId,
    setSnap,
    setGameId,
    setLoadoutId,
    setExpanded,
    setTab,
    setError,
    setToast,
    refresh,
    setVersionTick,
    refreshTxs,
    onFocusConsole,
  } = params;

  const [createOpen, setCreateOpen] = useState(false);
  const [branchOpen, setBranchOpen] = useState(false);
  const [mo2Open, setMo2Open] = useState(false);
  const [running, setRunning] = useState(false);
  const [testLaunchBusy, setTestLaunchBusy] = useState(false);
  const [launchNotice, setLaunchNotice] = useState<LaunchNotice | null>(null);
  const [gameLog, setGameLog] = useState<GameLogChunk[]>([]);
  /** Filled below — CTA Installer calls the same Jouer path once after success. */
  const playMinecraftRef = useRef<() => Promise<void>>(async () => {});

  const { facts: mcLoaderFacts, primaryAction: toolbarPrimaryAction } = useMcLoaderFacts({
    loadout: activeLoadout ?? null,
    onRefresh: async () => {
      await refresh(loadoutId ?? null);
    },
    onNotice: setToast,
    onError: setError,
    onAfterLoaderInstall: async () => {
      await playMinecraftRef.current();
    },
  });

  const shellReady = snap != null;
  useEffect(() => {
    if (!shellReady) return;
    void noteActiveLoadout(activeLoadout?.id ?? null);
  }, [shellReady, activeLoadout?.id]);

  useEffect(() => {
    let unlistenEnded: (() => void) | undefined;
    let unlistenLog: (() => void) | undefined;
    let unlistenHint: (() => void) | undefined;
    let cancelled = false;
    void onGameEnded((report) => {
      setRunning(false);
      setLaunchNotice(noticeFromReport(report));
      setToast(toastFromReport(report));
    }).then((off) => {
      if (cancelled) off();
      else unlistenEnded = off;
    });
    void onGameLog((chunk) => {
      setGameLog((prev) => [...prev, chunk]);
    }).then((off) => {
      if (cancelled) off();
      else unlistenLog = off;
    });
    void onGameLaunchHint((hint) => {
      setLaunchNotice(noticeFromInfo(hint.message));
      setToast(hint.message);
    }).then((off) => {
      if (cancelled) off();
      else unlistenHint = off;
    });
    void getGameSessionState().then((s) => {
      if (cancelled) return;
      setRunning(s.running);
      if (s.lastReport) setLaunchNotice(noticeFromReport(s.lastReport));
    });
    return () => {
      cancelled = true;
      unlistenEnded?.();
      unlistenLog?.();
      unlistenHint?.();
    };
  }, []);

  useEffect(() => {
    if (!running) return;
    const tick = window.setInterval(() => {
      void getGameSessionState().then((s) => {
        if (!s.running) setRunning(false);
        if (s.lastReport) setLaunchNotice(noticeFromReport(s.lastReport));
      });
    }, 4000);
    return () => clearInterval(tick);
  }, [running]);

  function unwired(label: string) {
    setToast(`« ${label} » : pas encore branché au Core.`);
  }

  function handleMenuAction(item: PopoverMenuEntry) {
    switch (item.id) {
      case "nouveau":
      case "nouveau-loadout":
        setCreateOpen(true);
        break;
      case "importer":
      case "importer-loadout":
        startImport();
        break;
      default:
        setToast(`« ${item.label} » : pas encore branché au Core.`);
    }
  }

  async function importMo2(profile: string, modsDir: string, name: string): Promise<number> {
    const result = await mo2Import(profile, modsDir, name);
    await refresh(result.loadout.id);
    setGameId(result.loadout.gameId);
    setExpanded((current) => expandGameKeepingOthers(current, result.loadout.gameId));
    setTab("dashboard");
    setMo2Open(false);
    setToast(
      `${result.imported} mod(s) importé(s) dans « ${result.loadout.name} » — les mods pointent vers les dossiers MO2, en place.`,
    );
    return result.imported;
  }

  async function importMinecraftInstance() {
    const name = `${activeGame?.short ?? "Minecraft"} — instance détectée`;
    setToast("Lecture du dossier mods/…");
    try {
      const result = await minecraftImportInstance(name);
      await refresh(result.loadout.id);
      setGameId(result.loadout.gameId);
      setExpanded((current) => expandGameKeepingOthers(current, result.loadout.gameId));
      setTab("dashboard");
      setToast(
        `${result.imported} mod(s) importé(s) dans « ${result.loadout.name} » — les jars restent dans le dossier mods/, rien n'a été déplacé.`,
      );
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }

  function startImport() {
    switch (activeGame?.capabilities.importSource) {
      case "mo2":
        setMo2Open(true);
        break;
      case "minecraft-instance":
        void importMinecraftInstance();
        break;
      default:
        unwired("Importer");
    }
  }

  /**
   * « Importer… » from « + Ajouter un mod »: pick a file/folder and register
   * it on the *current* loadout as a manual source. Distinct from
   * {@link startImport} (full MO2 profile / Minecraft instance).
   */
  async function startManualImport() {
    if (!activeLoadout) {
      setToast("Ouvre une instance avant d’importer un mod.");
      return;
    }
    const gameId = activeGame?.id ?? activeLoadout.gameId;
    const minecraft = gameId === "minecraft";
    try {
      // Minecraft: pick a jar to copy in, or cancel to only register jars
      // already dropped in mods/. Skyrim: pick a mod folder (MO2-shaped).
      const picked = minecraft
        ? await pickManualModPath("file", [{ name: "Mods Minecraft", extensions: ["jar"] }])
        : await pickManualModPath("dir");
      if (!picked && !minecraft) return;
      setToast(minecraft ? "Import…" : "Import du dossier…");
      const result = picked
        ? await importManualMod(activeLoadout.id, picked)
        : await minecraftSyncMods(activeLoadout.id);
      await refresh(activeLoadout.id);
      if (result.added === 0) {
        setToast(
          minecraft && !picked
            ? "Aucun nouveau jar dans mods/."
            : "Rien de nouveau — ce mod est déjà dans la liste.",
        );
      } else if (result.added === 1) {
        setToast("1 mod ajouté (Manuel).");
      } else {
        setToast(`${result.added} mods ajoutés (Manuel).`);
      }
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }

  async function renameLoadoutById(id: number, name: string) {
    try {
      await renameLoadout(id, name);
      await refresh(loadoutId ?? null);
      setToast(`Instance renommée : « ${name} »`);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }

  async function renameActiveLoadout(name: string) {
    if (!activeLoadout) return;
    await renameLoadoutById(activeLoadout.id, name);
  }

  async function setLoadoutChannelById(id: number, channel: Channel) {
    const name = loadouts.find((l) => l.id === id)?.name ?? "";
    try {
      await setLoadoutChannel(id, channel);
      await refresh(loadoutId ?? null);
      setToast(`Canal de « ${name} » : ${channel}`);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }

  async function setActiveLoadoutChannel(channel: Channel) {
    if (!activeLoadout) return;
    await setLoadoutChannelById(activeLoadout.id, channel);
  }

  async function deleteLoadoutById(id: number) {
    const target = loadouts.find((l) => l.id === id);
    const name = target?.instanceName ?? "";
    const wasActive = activeLoadout?.instanceId === target?.instanceId;
    try {
      await deleteLoadout(id);
      await refresh(wasActive ? null : (loadoutId ?? null));
      setToast(`Instance « ${name} » supprimée.`);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }

  async function restoreLoadoutById(id: number, name: string) {
    try {
      await restoreLoadout(id);
      await refresh(loadoutId ?? null);
      setToast(`Instance « ${name} » restaurée.`);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }

  async function createInstance(payload: CreateInstancePayload) {
    if (!activeGame) return;
    const created = await createLoadout(activeGame.id, payload.name);
    if (payload.mc && created.gameId === "minecraft" && isTauriShell()) {
      const { gameVersion, loader, loaderVersion } = payload.mc;
      // Order matters: game version clears loader; then loader; then loader version.
      await setBranchMcGameVersion(created.id, gameVersion);
      await setBranchMcLoader(created.id, loader as McLoaderId);
      if (loader !== "vanilla" && loaderVersion) {
        await setBranchMcLoaderVersion(created.id, loaderVersion);
      }
    }
    const next = await getShellSnapshot(created.id);
    setSnap(
      next.loadouts.some((loadout) => loadout.id === created.id)
        ? next
        : { ...next, loadouts: [...next.loadouts, created], selectedLoadoutId: created.id },
    );
    setLoadoutId(created.id);
    setGameId(created.gameId);
    setTab(created.gameId === "minecraft" ? "mods" : "dashboard");
    setToast(`Instance « ${created.instanceName} » créée.`);
    setCreateOpen(false);
  }

  async function createInstanceBranch(name: string) {
    if (!activeLoadout) return;
    const created = await createBranch(activeLoadout.id, name);
    const next = await getShellSnapshot(created.id);
    setSnap(
      next.loadouts.some((loadout) => loadout.id === created.id)
        ? next
        : { ...next, loadouts: [...next.loadouts, created], selectedLoadoutId: created.id },
    );
    setLoadoutId(created.id);
    setTab("mods");
    setToast(`Branche « ${created.name} » créée.`);
    setBranchOpen(false);
  }

  playMinecraftRef.current = async () => {
    if (!activeLoadout) return;
    if (running) {
      setToast("Une partie est déjà en cours.");
      return;
    }
    try {
      const loader = await mcLoaderStatus(activeLoadout.id);
      if (loader.state === "unset") {
        setLaunchNotice(
          noticeFromRefusal(
            "Choisissez un loader et une version sur l’onglet Mods, puis installez Vanilla (ou un loader).",
          ),
        );
        setToast("Lancement annulé — choisissez la version sur l’onglet Mods.");
        setTab("mods");
        return;
      }
      if (loader.state === "missing") {
        setLaunchNotice(
          noticeFromRefusal(
            "Le loader choisi n’est pas installé — utilisez « Installer … » dans la barre d’instance, puis réessayez Jouer.",
          ),
        );
        setToast("Lancement annulé — installez le loader d’abord.");
        setTab("mods");
        return;
      }
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
      throw e instanceof Error ? e : new Error(String(e));
    }
    setGameLog([]);
    setRunning(true);
    setLaunchNotice(null);
    setToast("Préparation Minecraft…");
    onFocusConsole?.();
    const player = readUiMemory().minecraftPlayer;
    try {
      const notice = await launchMinecraft(activeLoadout.id, player);
      // Session owns the JVM — keep running until game://ended / Arrêter.
      // MSA online when connected; otherwise offline pseudo (unchanged).
      setToast(
        notice?.trim()
          ? notice
          : "Minecraft est lancé — ModLab reste utilisable pendant la partie.",
      );
      if (notice?.trim()) {
        setLaunchNotice(noticeFromInfo(notice));
      }
    } catch (e) {
      setRunning(false);
      const message = e instanceof Error ? e.message : String(e);
      setLaunchNotice(noticeFromRefusal(message));
      setToast("Lancement annulé — voir le message sous « Jouer ».");
      throw e instanceof Error ? e : new Error(message);
    }
  };

  async function testLaunch() {
    if (!activeLoadout) return;
    if (activeGame?.id !== "minecraft") {
      setToast("Le test de lancement n’est disponible que pour Minecraft.");
      return;
    }
    if (running || testLaunchBusy) {
      setToast("Une partie ou un test est déjà en cours.");
      return;
    }
    try {
      const loader = await mcLoaderStatus(activeLoadout.id);
      if (loader.state === "unset") {
        setLaunchNotice(
          noticeFromRefusal(
            "Choisissez un loader et une version sur l’onglet Mods, puis installez Vanilla (ou un loader).",
          ),
        );
        setToast("Test annulé — choisissez la version sur l’onglet Mods.");
        setTab("mods");
        return;
      }
      if (loader.state === "missing") {
        setLaunchNotice(
          noticeFromRefusal(
            "Le loader choisi n’est pas installé — utilisez « Installer … » dans la barre d’instance, puis réessayez.",
          ),
        );
        setToast("Test annulé — installez le loader d’abord.");
        setTab("mods");
        return;
      }
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
      return;
    }
    setGameLog([]);
    setRunning(true);
    setTestLaunchBusy(true);
    setLaunchNotice(null);
    setToast("Test de lancement…");
    onFocusConsole?.();
    const player = readUiMemory().minecraftPlayer;
    try {
      const result = await testLaunchMinecraft(activeLoadout.id, player);
      setToast(result.message);
      if (result.status === "ok") {
        setLaunchNotice(noticeFromInfo(result.message));
      } else {
        setLaunchNotice(noticeFromRefusal(result.message));
      }
    } catch (e) {
      const message = e instanceof Error ? e.message : String(e);
      setLaunchNotice(noticeFromRefusal(message));
      setToast("Test annulé — voir le message sous « Jouer ».");
      setRunning(false);
    } finally {
      setTestLaunchBusy(false);
    }
  }

  async function play() {
    if (!activeLoadout) return;
    if (running) {
      try {
        const stop = await stopGameSession();
        if (stop.message) setToast(stop.message);
        else if (stop.alreadyStopped) {
          setRunning(false);
          setToast("Aucune partie en cours.");
        }
      } catch (e) {
        setError(e instanceof Error ? e.message : String(e));
      }
      return;
    }
    const gameId = activeGame?.id;
    if (gameId === "skyrimse") {
      setRunning(true);
      setLaunchNotice(null);
      setToast("Lancement SKSE via USVFS…");
      try {
        await launchSkse(activeLoadout.id);
        setToast("Skyrim est lancé — ModLab reste utilisable pendant la partie.");
      } catch (e) {
        setRunning(false);
        const message = e instanceof Error ? e.message : String(e);
        setLaunchNotice(noticeFromRefusal(message));
        setToast("Lancement annulé — voir le message sous « Jouer ».");
      }
      return;
    }
    if (gameId === "minecraft") {
      try {
        await playMinecraftRef.current();
      } catch {
        // Toast / notice already set inside playMinecraftRef.
      }
      return;
    }
    setToast("Le lancement n’est pas disponible pour ce jeu.");
  }

  async function doSnapshot() {
    if (!activeLoadout) {
      setToast("Sélectionne une instance d'abord.");
      return;
    }
    try {
      await snapshotLoadout(activeLoadout.id);
      setToast(`Instantané de « ${activeLoadout.name} » pris.`);
      refreshTxs();
      setVersionTick((t) => t + 1);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }

  async function doRollback() {
    if (!activeLoadout) return;
    try {
      await rollbackLoadout(activeLoadout.id);
      const s = await getShellSnapshot(activeLoadout.id);
      setSnap(s);
      setToast("Configuration restaurée depuis l'instantané.");
      refreshTxs();
      setVersionTick((t) => t + 1);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }

  async function saveActive() {
    if (!activeLoadout) {
      setToast("Sélectionne une instance d'abord.");
      return;
    }
    try {
      const version = await getLoadoutVersion(activeLoadout.id);
      if (version.dirtyPlus === 0 && version.dirtyMinus === 0) {
        setToast("Rien à enregistrer.");
        return;
      }
      await doSnapshot();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }

  function revealActiveInstanceFolder() {
    if (!activeLoadout) {
      setError("Aucune instance active.");
      return;
    }
    void revealInstanceFolder(activeLoadout.id).catch((e: unknown) =>
      setError(e instanceof Error ? e.message : String(e)),
    );
  }

  return {
    createOpen,
    setCreateOpen,
    branchOpen,
    setBranchOpen,
    mo2Open,
    setMo2Open,
    running,
    testLaunchBusy,
    gameLog,
    launchNotice,
    setLaunchNotice,
    mcLoaderFacts,
    toolbarPrimaryAction,
    unwired,
    handleMenuAction,
    importMo2,
    startImport,
    startManualImport,
    renameLoadoutById,
    renameActiveLoadout,
    setLoadoutChannelById,
    setActiveLoadoutChannel,
    deleteLoadoutById,
    restoreLoadoutById,
    createInstance,
    createInstanceBranch,
    play,
    testLaunch,
    doSnapshot,
    doRollback,
    saveActive,
    revealActiveInstanceFolder,
  };
}