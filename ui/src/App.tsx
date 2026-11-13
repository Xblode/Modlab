import { FormEvent, type ReactNode, useEffect, useMemo, useRef, useState } from "react";
import {
  getShellSnapshot,
  createLoadout,
  mo2Import,
  minecraftImportInstance,
  launchSkse,
  setModEnabled,
  aiAsk,
  snapshotLoadout,
  rollbackLoadout,
  getLoadoutVersion,
  renameLoadout,
  setLoadoutChannel,
  deleteLoadout,
  restoreLoadout,
  attachDownload,
  onDownloadCaptured,
  inspectArchivedMod,
  installArchivedMod,
  installFomod,
  setModCategory,
  modCategories,
  reassignMod,
  renameMod,
  deleteMod,
  uninstallMod,
  revealPath,
  type Channel,
  type DetectedGame,
  type LoadoutDto,
  type ModRow,
  type ShellSnapshot,
} from "./api";
import { useFavorites } from "./favorites";
import { readUiMemory, writeUiMemory } from "./uiMemory";
import { useConflicts } from "./hooks/useConflicts";
import { useTransactions } from "./hooks/useTransactions";
import { useDownloads } from "./hooks/useDownloads";
import { browseHomeFor, tabsFor, type TabId } from "./gameChrome";
import { BrowserPane } from "./components/Features/Browser/BrowserPane";
import { FomodWizard } from "./components/Features/Mods/FomodWizard";
import { StubTab } from "./StubTab";

import { LayoutShell } from "./components/Layout/LayoutShell";
import { Sidebar, type SideAct } from "./components/Features/Sidebar/Sidebar";
import type { EngineCap } from "./components/Features/Sidebar/ActiveInstanceCard";
import { RightPanel, type RightTab } from "./components/Features/Inspector/RightPanel";
import { type ChatMsg } from "./components/Features/Chat/ChatPanel";
import { Composer } from "./components/Features/Composer/Composer";
import { TransactionsPanel } from "./components/Features/Transactions/TransactionsPanel";
import { DownloadsPanel } from "./components/Features/Transactions/DownloadsPanel";
import type { BottomTab } from "./components/Features/Transactions/BottomTab";
import { SettingsPane, type SettingsMenu } from "./components/Features/Settings/SettingsPane";
import { ProfilePage } from "./components/Features/Profile/ProfilePage";
import { type CommunityMenuId } from "./components/Features/Community/CommunityMenu";
import { PatchWorkshopPage } from "./components/Features/Community/PatchWorkshopPage";
import { MarketplacePage } from "./components/Features/Community/MarketplacePage";
import { CommandPalette, type PaletteItem } from "./components/Features/CommandPalette/CommandPalette";
import { CreateLoadoutDialog } from "./components/Features/LoadoutDialogs/CreateLoadoutDialog";
import { Mo2ImportDialog } from "./components/Features/LoadoutDialogs/Mo2ImportDialog";
import type { MenuItem } from "./components/Features/MenuPicker/MenuPicker";
import { Crumbs, Toast } from "./components/ui";

import DashboardPage from "./app/dashboard/page";
import ModsPage from "./app/mods/page";
import PluginsPage from "./app/plugins/page";
import ArchivesPage from "./app/archives/page";
import RepertoirePage from "./app/repertoire/page";
import SavesPage from "./app/saves/page";
import SantePage from "./app/sante/page";
import PacksPage from "./app/packs/page";
import WorldsPage from "./app/worlds/page";
import ServersPage from "./app/servers/page";

const LLM_MODELS = [
  ["qwen-1.5b", "Qwen2.5 1.5B", "défaut · CPU ok"],
  ["llama-3b", "Llama 3.2 3B", "meilleur raisonnement"],
  ["phi-mini", "Phi-3.5 mini", "compact"],
  ["qwen-8b", "Qwen3 8B", "si GPU ≥ 8 Go"],
] as const;

export default function App() {
  // Read once, synchronously, before the first render — every field below
  // that the shell remembers between launches seeds from this same read so
  // there is exactly one moment where "what was on screen last time" enters
  // React state, not one `localStorage` read per field.
  const [initialMemory] = useState(readUiMemory);
  const [snap, setSnap] = useState<ShellSnapshot | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [act, setAct] = useState<SideAct>("explorer");
  // The activity to return to when leaving Réglages — captured the moment we
  // enter it, so `SideBack` can undo whichever icon actually opened it.
  const [prevAct, setPrevAct] = useState<SideAct>("explorer");
  const [menuOpen, setMenuOpen] = useState(false);
  const [settingsMenu, setSettingsMenu] = useState<SettingsMenu>("general");
  const [communityMenu, setCommunityMenu] = useState<CommunityMenuId>("patchWorkshop");
  const [tab, setTab] = useState<TabId>(initialMemory.tab ?? "dashboard");
  const [gameId, setGameId] = useState<string | null>(null);
  const [loadoutId, setLoadoutId] = useState<number | null>(null);
  const [selectedMod, setSelectedMod] = useState<number | null>(null);
  const [expanded, setExpanded] = useState<string[]>(initialMemory.expanded);
  const [rightOpen, setRightOpen] = useState(initialMemory.rightOpen);
  const [rightTab, setRightTab] = useState<RightTab>(initialMemory.rightTab);
  const [running, setRunning] = useState(false);
  const [toast, setToast] = useState<string | null>(null);
  const [llm, setLlm] = useState(initialMemory.llm);
  const [communityUpdate, setCommunityUpdate] = useState(initialMemory.communityUpdate);
  const [createOpen, setCreateOpen] = useState(false);
  const [mo2Open, setMo2Open] = useState(false);
  /** Set once `installArchived` finds a FOMOD, so the wizard replaces the
   * toast-and-return path a plain archive takes. */
  const [fomodTarget, setFomodTarget] = useState<{
    modId: number;
    name: string;
    contentRoot: string;
  } | null>(null);
  const [categories, setCategories] = useState<string[]>([]);
  /** The browser tab only exists once it has been opened at least once —
   * it isn't a game capability, so it never appears on its own. Also, once
   * true, `BrowserPane` renders for the rest of the session (see its render
   * site below) — merely switching tabs away and back must not remount it,
   * or the native webview would re-navigate on every return trip. */
  const [webOpen, setWebOpen] = useState(() => initialMemory.tab === "web");
  /** Bumped only by an explicit "open the browser" click — the one and only
   * trigger `BrowserPane` navigates on. Becoming visible again by switching
   * tabs back to it does not bump this. */
  const [browserOpenToken, setBrowserOpenToken] = useState(0);
  /** Set right before an explicit browser open that must land on a specific
   * page (e.g. "Télécharger/MAJ" on a mod) rather than the active game's
   * home page — read once by the render that bumps `browserOpenToken`, then
   * cleared, so a later open goes back to the game's home page again. */
  const pendingWebUrlRef = useRef<string | null>(null);
  const [query, setQuery] = useState("");
  const [paletteOpen, setPaletteOpen] = useState(false);
  const [txOpen, setTxOpen] = useState(false);
  /** Which tab the bottom panel (Ctrl+J) shows. */
  const [bottomTab, setBottomTab] = useState<BottomTab>("transactions");
  /** Bumped whenever a download is captured, so `useDownloads` refetches
   * even while the panel is already open. */
  const [downloadsRefreshKey, setDownloadsRefreshKey] = useState(0);
  /** Bumped after anything that can move the active loadout's revision or
   * dirty count (a toggle, a snapshot, a rollback) — `ActiveInstanceCard`'s
   * own `useLoadoutVersion` depends on it to know when to refetch. */
  const [versionTick, setVersionTick] = useState(0);

  // Ctrl+K/J/S listener lives below `activeLoadout`'s own declaration, which
  // `saveActive` (Ctrl+S) needs in scope.

  /**
   * Every menu entry lands here. Only the two entries the Core can already
   * serve are wired (new instance, MO2 import); the rest say so out loud
   * instead of pretending to work.
   */
  function handleMenuAction(item: MenuItem) {
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

  /** A "special page" act — takes over the main content and the sidebar's
   * own sub-nav, same as Réglages, but isn't configuration (see
   * `ProfilePage` / `CommunityMenu`'s own docs on why they live outside
   * Réglages). */
  function isSpecialAct(a: SideAct): boolean {
    return a === "settings" || a === "profile" || a === "community";
  }

  /** Every rail click, the avatar, the Communauté icon and the palette's
   * "Ouvrir les réglages" all go through here so `prevAct` always names
   * whatever was active right before one of these special pages opened, not
   * just a hardcoded default. */
  function handleAct(next: SideAct) {
    if (isSpecialAct(next) && !isSpecialAct(act)) {
      setPrevAct(act);
    }
    setAct(next);
  }

  const { isFavorite, toggleFavorite, favoriteIds } = useFavorites();
  const [chat, setChat] = useState<ChatMsg[]>([
    {
      who: "ai",
      text: "Je réponds d'abord par le moteur de règles du Core — conflits, mods actifs, dossiers illisibles. Avec un modèle GGUF déposé dans <racine données>/models, la même réponse est reformulée par le LLM local, sans y ajouter un seul fait.",
    },
  ]);
  const [draft, setDraft] = useState("");

  useEffect(() => {
    let cancelled = false;
    getShellSnapshot(initialMemory.loadoutId)
      .then((s) => {
        if (cancelled) return;
        // The remembered instance may have been deleted since — the Core
        // still returns a snapshot for a dead id (empty entries), which
        // would show one instance's name next to another's mod list. Fall
        // back to the Core's own honest default instead of trusting it.
        const remembered = initialMemory.loadoutId;
        const stillThere = remembered != null && s.loadouts.some((l) => l.id === remembered);
        if (remembered != null && !stillThere) {
          void refresh(null);
          const first = s.games[0]?.id ?? s.loadouts[0]?.gameId ?? null;
          setGameId(first);
          if (first) setExpanded([first]);
          return;
        }
        setSnap(s);
        setLoadoutId(s.selectedLoadoutId);
        const owner = s.loadouts.find((l) => l.id === s.selectedLoadoutId)?.gameId;
        const rememberedGameOk =
          initialMemory.gameId != null && s.games.some((g) => g.id === initialMemory.gameId);
        const first =
          owner ??
          (rememberedGameOk ? initialMemory.gameId : null) ??
          s.games[0]?.id ??
          s.loadouts[0]?.gameId ??
          null;
        setGameId(first);
        if (first) setExpanded([first]);
      })
      .catch((e: unknown) => {
        if (!cancelled) setError(e instanceof Error ? e.message : String(e));
      });
    return () => {
      cancelled = true;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  useEffect(() => {
    if (toast == null) return;
    const t = window.setTimeout(() => setToast(null), 2800);
    return () => window.clearTimeout(t);
  }, [toast]);

  function refreshCategories() {
    void modCategories()
      .then(setCategories)
      .catch(() => setCategories([]));
  }
  useEffect(refreshCategories, []);

  // Remembers what was on screen for the next launch — every write is a
  // full, small blob (not per-field) so a mid-change crash never leaves it
  // half-updated. `currentTab` (below) is deliberately not what's stored;
  // the raw `tab` is, so a tab that only applied to a previous game isn't
  // silently swapped for "dashboard" on the very next save. Gated on `snap`
  // being loaded: before that, `gameId`/`loadoutId` are still the initial
  // `null` placeholders, and writing them would overwrite a perfectly good
  // remembered session with nulls if the very first fetch is slow or fails.
  const hasSnap = snap !== null;
  useEffect(() => {
    if (!hasSnap) return;
    writeUiMemory({ gameId, loadoutId, tab, expanded, rightOpen, rightTab, llm, communityUpdate });
  }, [hasSnap, gameId, loadoutId, tab, expanded, rightOpen, rightTab, llm, communityUpdate]);

  const games = snap?.games ?? [];
  const loadouts = snap?.loadouts ?? [];
  const activeGame: DetectedGame | undefined =
    games.find((g) => g.id === gameId) ?? games[0];
  const gameLoadouts = loadouts.filter(
    (l) => !activeGame || l.gameId === activeGame.id,
  );
  const activeLoadout: LoadoutDto | undefined =
    gameLoadouts.find((l) => l.id === loadoutId) ?? gameLoadouts[0];
  /** Where an archived row's "déplacer vers…" menu can send it — other
   * instances of the same game. Reassigning across games isn't refused by
   * the Core, but a Skyrim archive offered a Minecraft instance is a
   * confusing menu entry, not a real use case. */
  const otherLoadouts = gameLoadouts.filter((l) => l.id !== activeLoadout?.id);

  /** Ctrl+K / Ctrl+Shift+P — the palette is the keyboard entry point (§6.2).
   * Ctrl+J opens the bottom panel (transactions), Ctrl+S saves — same
   * vocabulary. Depends on `activeLoadout` so `saveActive` always closes
   * over whichever instance is actually selected right now, not whichever
   * one was active when the listener was first attached. */
  useEffect(() => {
    function onKey(e: KeyboardEvent) {
      if (!(e.ctrlKey || e.metaKey) || e.altKey) return;
      const k = e.key.toLowerCase();
      if (k === "k" || (e.shiftKey && k === "p")) {
        e.preventDefault();
        setPaletteOpen((v) => !v);
      } else if (k === "j") {
        e.preventDefault();
        setTxOpen((v) => !v);
      } else if (k === "s") {
        e.preventDefault();
        void saveActive();
      }
    }
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [activeLoadout]);

  /** Captures from the embedded browser. Re-subscribes when the active
   * instance changes so a capture always lands in the one on screen. */
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let cancelled = false;
    void onDownloadCaptured((e) => void fileCapture(e.downloadId, e.fileName)).then((off) => {
      if (cancelled) off();
      else unlisten = off;
    });
    return () => {
      cancelled = true;
      unlisten?.();
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [activeLoadout]);

  const entries = snap?.entries ?? [];
  const enabledCount = entries.filter((e) => e.enabled).length;
  const selectedRow = entries.find((e) => e.modId === selectedMod) ?? null;
  /**
   * "Engine & Indicators" for the active instance — a stopgap name-match
   * heuristic, not the real "known mods" registry (SKSE, Forge, Fabric…
   * with actual version-compatibility rules) that ClickUp already tracks as
   * separate, bigger Core work. Skyrim-only for now: Minecraft's loader is
   * already structurally detected server-side (`ModLoader` in
   * modlab-game-minecraft) through a different data path this doesn't
   * touch yet.
   */
  const engineCaps: EngineCap[] = useMemo(() => {
    if (activeGame?.id !== "skyrimse") return [];
    const skse = entries.find((e) => e.enabled && /skse/i.test(e.name));
    return [
      {
        id: "skse",
        label: "SKSE",
        sub: skse ? (skse.version ?? "détecté") : "non installé",
        detected: Boolean(skse),
        icon: "zap",
      },
    ];
  }, [activeGame?.id, entries]);
  const tabs = tabsFor(activeGame?.capabilities);
  // A game switch can land on a tab the new game does not have. The browser
  // is exempt: it belongs to the shell, so it survives changing games.
  const currentTab: TabId =
    tab === "web" ? "web" : tabs.some((t) => t.id === tab) ? tab : "dashboard";
  const tabLabel = tabs.find((t) => t.id === currentTab)?.label ?? "—";

  // Ordered by the Core's own loadout order, not by when the star was clicked.
  const favorites = useMemo(
    () =>
      loadouts
        .filter((l) => favoriteIds.includes(l.id))
        .map((l) => ({
          loadout: l,
          short: games.find((x) => x.id === l.gameId)?.short ?? l.gameId,
        })),
    [favoriteIds, loadouts, games],
  );

  const filtered = useMemo(() => {
    const q = query.trim().toLowerCase();
    if (!q) return entries;
    return entries.filter(
      (e) => e.name.toLowerCase().includes(q) || e.category?.toLowerCase().includes(q),
    );
  }, [entries, query]);

  /** Distinct categories in the active loadout, with counts — what the
   * sidebar's "Catégories" section lists. Alphabetical, not insertion
   * order: the sidebar is a place to scan, not a log. */
  const categoryCounts = useMemo(() => {
    const counts = new Map<string, number>();
    for (const e of entries) {
      if (!e.category) continue;
      counts.set(e.category, (counts.get(e.category) ?? 0) + 1);
    }
    return Array.from(counts, ([name, count]) => ({ name, count })).sort((a, b) =>
      a.name.localeCompare(b.name),
    );
  }, [entries]);

  /** Clicking a category in the sidebar filters the Mods table down to it —
   * reuses the same free-text search the searchbar drives (matches name or
   * category), so there's one filter mechanism, not two. */
  function pickCategory(name: string) {
    setQuery(name);
    setTab("mods");
  }

  const { conflictByMod } = useConflicts(activeLoadout, entries);
  const { txs, refreshTxs } = useTransactions(txOpen);
  const { downloads } = useDownloads(txOpen, downloadsRefreshKey);

  /**
   * The scan only covers enabled mods it could read. A disabled mod, or one
   * whose folder failed to read, is therefore absent from the map — and the
   * inspector stays quiet about it instead of claiming it conflicts with
   * nothing.
   */
  const modConflict =
    selectedRow && conflictByMod ? conflictByMod.get(selectedRow.modId) : undefined;

  async function refresh(id: number | null) {
    const s = await getShellSnapshot(id);
    setSnap(s);
    setLoadoutId(s.selectedLoadoutId);
  }

  async function pickLoadout(id: number, nextTab?: TabId) {
    setLoadoutId(id);
    setSelectedMod(null);
    const owner = loadouts.find((l) => l.id === id);
    if (owner) {
      setGameId(owner.gameId);
      setExpanded([owner.gameId]);
    }
    if (nextTab) setTab(nextTab);
    try {
      await refresh(id);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }

  function pickGame(id: string) {
    setGameId(id);
    setTab("dashboard");
    setSelectedMod(null);
    setExpanded([id]);
    const first = loadouts.find((l) => l.gameId === id);
    if (first) void pickLoadout(first.id, "dashboard");
  }

  function onFavorite(id: number) {
    const name = loadouts.find((l) => l.id === id)?.name ?? "";
    const now = toggleFavorite(id);
    setToast(
      now ? `« ${name} » ajoutée aux favoris` : `« ${name} » retirée des favoris`,
    );
  }

  function unwired(label: string) {
    setToast(`« ${label} » : pas encore branché au Core.`);
  }

  /**
   * Import an MO2 profile into a new loadout, then land on it. The Core
   * validates everything (folders exist, mods match) — the dialog only
   * surfaces its messages. Returns the count so the dialog can also show it
   * if a later refresh fails.
   */
  async function importMo2(
    profile: string,
    modsDir: string,
    name: string,
  ): Promise<number> {
    const result = await mo2Import(profile, modsDir, name);
    await refresh(result.loadout.id);
    setGameId(result.loadout.gameId);
    setExpanded([result.loadout.gameId]);
    setTab("dashboard");
    setMo2Open(false);
    setToast(
      `${result.imported} mod(s) importé(s) dans « ${result.loadout.name} » — les mods pointent vers les dossiers MO2, en place.`,
    );
    return result.imported;
  }

  /**
   * Import the detected Minecraft instance's own `mods/` folder. Unlike the
   * MO2 import there is nothing to ask for — the instance's path already
   * comes from the adapter (or the override set in Réglages → Jeux) — so
   * this needs no dialog, just a name.
   */
  async function importMinecraftInstance() {
    const name = `${activeGame?.short ?? "Minecraft"} — instance détectée`;
    setToast("Lecture du dossier mods/…");
    try {
      const result = await minecraftImportInstance(name);
      await refresh(result.loadout.id);
      setGameId(result.loadout.gameId);
      setExpanded([result.loadout.gameId]);
      setTab("dashboard");
      setToast(
        `${result.imported} mod(s) importé(s) dans « ${result.loadout.name} » — les jars restent dans le dossier mods/, rien n'a été déplacé.`,
      );
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }

  /**
   * One import button, whichever source the active game's adapter declares
   * (ADR 0005) — the shell maps the capability id to an action exactly as it
   * maps tab ids to labels, and never assumes MO2.
   */
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

  async function toggle(row: ModRow) {
    if (!activeLoadout) return;
    try {
      const next = await setModEnabled(activeLoadout.id, row.modId, !row.enabled);
      setSnap((prev) => (prev ? { ...prev, entries: next } : prev));
      setVersionTick((t) => t + 1);
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
    const name = loadouts.find((l) => l.id === id)?.name ?? "";
    const wasActive = activeLoadout?.id === id;
    try {
      await deleteLoadout(id);
      await refresh(wasActive ? null : (loadoutId ?? null));
      setToast(`Instance « ${name} » supprimée.`);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }

  /**
   * A file just landed from the embedded browser. It is already recorded in
   * the Core — this only files it into whichever instance is on screen, so
   * it shows up in the mod table straight away, archived and disabled.
   *
   * Filing into the *displayed* instance is the deliberate design: no
   * blocking dialog, and the row can be reassigned afterwards precisely
   * because nothing has been unpacked yet.
   */
  /** Opens (or brings forward) the browser tab. With `url`, that's where it
   * lands this time instead of the active game's own home page — used by
   * "Télécharger/MAJ" on a mod, which knows exactly which page to send the
   * human to. */
  function openWeb(url?: string) {
    pendingWebUrlRef.current = url ?? null;
    setWebOpen(true);
    setTab("web");
    setBrowserOpenToken((t) => t + 1);
  }

  async function fileCapture(downloadId: number, fileName: string) {
    setDownloadsRefreshKey((k) => k + 1);
    if (!activeLoadout) {
      setToast(`« ${fileName} » téléchargé — ouvre une instance pour le classer.`);
      return;
    }
    try {
      await attachDownload(downloadId, activeLoadout.id, null);
      await refresh(activeLoadout.id);
      setTab("mods");
      setToast(`« ${fileName} » → ${activeLoadout.name} (archive, à installer)`);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }

  /**
   * Unpack an archived mod. Extraction and classification happen first and
   * touch nothing the game can see; only a plain archive is installed
   * outright. A FOMOD stops here with an honest message rather than being
   * installed wrong — its guided installer is the next chunk of work.
   */
  async function installArchived(row: ModRow) {
    if (!activeLoadout) return;
    setToast(`Extraction de « ${row.name} »…`);
    try {
      const plan = await inspectArchivedMod(row.modId);
      if (plan.needsWizard) {
        setFomodTarget({ modId: row.modId, name: row.name, contentRoot: plan.contentRoot });
        return;
      }
      if (plan.shape === "unknown") {
        setToast(
          `Contenu de « ${row.name} » non reconnu — ni dossier Data/, ni plugin. Rien n'a été installé.`,
        );
        return;
      }
      const rows = await installArchivedMod(row.modId, activeLoadout.id, plan.contentRoot);
      setSnap((s) => (s ? { ...s, entries: rows } : s));
      setToast(`« ${row.name} » installé — active-le quand tu veux.`);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }

  /** Finish an archive whose FOMOD wizard the human just answered. */
  async function finishFomodInstall(picked: string[]) {
    if (!fomodTarget || !activeLoadout) return;
    const rows = await installFomod(fomodTarget.modId, activeLoadout.id, fomodTarget.contentRoot, picked);
    setSnap((s) => (s ? { ...s, entries: rows } : s));
    setToast(`« ${fomodTarget.name} » installé — active-le quand tu veux.`);
    setFomodTarget(null);
  }

  async function setCategoryFor(row: ModRow, category: string | null) {
    try {
      await setModCategory(row.modId, category);
      setSnap((s) =>
        s
          ? { ...s, entries: s.entries.map((e) => (e.modId === row.modId ? { ...e, category } : e)) }
          : s,
      );
      refreshCategories();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }

  async function reassignModTo(row: ModRow, toLoadoutId: number) {
    if (!activeLoadout) return;
    const target = otherLoadouts.find((l) => l.id === toLoadoutId)?.name ?? "";
    try {
      const rows = await reassignMod(row.modId, activeLoadout.id, toLoadoutId);
      setSnap((s) => (s ? { ...s, entries: rows } : s));
      setToast(`« ${row.name} » déplacé vers « ${target} ».`);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }

  async function renameModRow(row: ModRow, name: string) {
    if (!activeLoadout) return;
    try {
      const rows = await renameMod(row.modId, activeLoadout.id, name);
      setSnap((s) => (s ? { ...s, entries: rows } : s));
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }

  /** Soft-delete only — see `deleteMod`'s doc: nothing on disk is touched. */
  async function deleteModRow(row: ModRow) {
    if (!activeLoadout) return;
    try {
      const rows = await deleteMod(row.modId, activeLoadout.id);
      setSnap((s) => (s ? { ...s, entries: rows } : s));
      setToast(`« ${row.name} » retiré de la liste.`);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }

  async function uninstallModRow(row: ModRow) {
    if (!activeLoadout) return;
    try {
      const rows = await uninstallMod(row.modId, activeLoadout.id);
      setSnap((s) => (s ? { ...s, entries: rows } : s));
      setToast(`« ${row.name} » désinstallé — revenu à l'état d'archive.`);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }

  function revealModFolder(row: ModRow) {
    void revealPath(row.sourcePath, false).catch((e: unknown) =>
      setError(e instanceof Error ? e.message : String(e)),
    );
  }

  function revealModArchive(row: ModRow) {
    if (!row.archivePath) return;
    void revealPath(row.archivePath, true).catch((e: unknown) =>
      setError(e instanceof Error ? e.message : String(e)),
    );
  }

  /** The trash list itself lives in `Sidebar` (`useDeletedLoadouts`, loaded
   * only while the History activity is open) — `name` comes from there so
   * this doesn't need its own copy of a list that's about to go stale the
   * moment this call succeeds. */
  async function restoreLoadoutById(id: number, name: string) {
    try {
      await restoreLoadout(id);
      await refresh(loadoutId ?? null);
      setToast(`Instance « ${name} » restaurée.`);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }

  async function createInstance(name: string) {
    if (!activeGame) return;
    const created = await createLoadout(activeGame.id, name);
    const next = await getShellSnapshot(created.id);
    // The browser preview deliberately has a static fixture. Keep a newly
    // created preview instance visible there while the real Tauri shell gets
    // its authoritative list straight back from the Core.
    setSnap(
      next.loadouts.some((loadout) => loadout.id === created.id)
        ? next
        : { ...next, loadouts: [...next.loadouts, created], selectedLoadoutId: created.id },
    );
    setLoadoutId(created.id);
    setGameId(created.gameId);
    setTab("dashboard");
    setToast(`Instance « ${created.name} » créée.`);
    setCreateOpen(false);
  }

  async function play() {
    if (!activeLoadout) return;
    if (activeGame?.id !== "skyrimse") {
      setToast("Le lancement n’est câblé que pour Skyrim SE / SKSE pour l’instant.");
      return;
    }
    setRunning(true);
    setToast("Lancement SKSE via USVFS…");
    try {
      await launchSkse(activeLoadout.id);
      setToast("Session terminée.");
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setRunning(false);
    }
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
      // The configuration changed: reload the whole snapshot from the Core
      // rather than guessing which piece of state the restore touched.
      const s = await getShellSnapshot(activeLoadout.id);
      setSnap(s);
      setToast("Configuration restaurée depuis l'instantané.");
      refreshTxs();
      setVersionTick((t) => t + 1);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }

  /**
   * Ctrl+S — mirrors `ActiveInstanceCard`'s own save button rather than
   * duplicating its disabled-while-clean logic: fetches the same dirty
   * summary the button's `useLoadoutVersion` would show, and only snapshots
   * when there is actually something to save.
   */
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

  /** The chat is a real client of the AI layer: rules engine, then — when
   * available — the local LLM phrasing those facts. Source is displayed. */
  async function sendChat(e?: FormEvent) {
    e?.preventDefault();
    const text = draft.trim();
    if (!text) return;
    if (!activeLoadout) {
      setToast("Sélectionne une instance pour donner du contexte à l'IA.");
      return;
    }
    setDraft("");
    setRightOpen(true);
    setRightTab("claude");
    setChat((c) => [
      ...c,
      { who: "me", text },
      { who: "ai", text: "…", source: "pending" },
    ]);
    try {
      const ans = await aiAsk(text, activeLoadout.id);
      setChat((c) => {
        const next = [...c];
        next[next.length - 1] = { who: "ai", text: ans.text, source: ans.source };
        return next;
      });
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      setChat((c) => {
        const next = [...c];
        next[next.length - 1] = { who: "ai", text: `⚠ ${msg}`, source: "error" };
        return next;
      });
    }
  }

  function openMod(id: number) {
    setSelectedMod(id);
    setRightOpen(true);
    setRightTab("info");
  }

  /**
   * Palette contents: only what the shell can actually serve today —
   * commands wired to real actions, the current game's adapter-declared
   * tabs, instances and mods from the snapshot. Nothing fake is listed.
   */
  const paletteItems: PaletteItem[] = useMemo(() => {
    const items: PaletteItem[] = [
      {
        id: "cmd-new",
        group: "Commandes",
        label: "Nouvelle instance…",
        icon: "plus",
        run: () => setCreateOpen(true),
      },
      {
        id: "cmd-import",
        group: "Commandes",
        // Named after whatever the active game can actually import from,
        // rather than offering an MO2 import while Minecraft is selected.
        label:
          activeGame?.capabilities.importSource === "minecraft-instance"
            ? "Importer l’instance Minecraft détectée"
            : "Importer un profil MO2…",
        icon: "download",
        run: startImport,
      },
      {
        id: "cmd-play",
        group: "Commandes",
        label: "Lancer le jeu (SKSE)",
        icon: "play",
        run: () => void play(),
      },
      {
        id: "cmd-save",
        group: "Commandes",
        label: "Enregistrer (Ctrl+S)",
        icon: "db",
        run: () => void saveActive(),
      },
      {
        id: "cmd-tx",
        group: "Commandes",
        label: "Afficher le panneau du bas (transactions)",
        icon: "history",
        run: () => setTxOpen(true),
      },
      {
        id: "cmd-right",
        group: "Commandes",
        label: "Basculer le panneau droit",
        icon: "layers",
        run: () => setRightOpen((v) => !v),
      },
      {
        id: "cmd-settings",
        group: "Commandes",
        label: "Ouvrir les réglages",
        icon: "gear",
        run: () => {
          handleAct("settings");
          setTab("dashboard");
        },
      },
      {
        id: "cmd-ia",
        group: "IA",
        label: "Demander à l'IA locale…",
        icon: "sparkle",
        run: () => {
          setRightOpen(true);
          setRightTab("claude");
        },
      },
    ];
    for (const t of tabs) {
      items.push({
        id: `tab-${t.id}`,
        group: "Onglets",
        label: `Ouvrir : ${t.label}`,
        icon: t.icon,
        run: () => setTab(t.id),
      });
    }
    for (const l of loadouts) {
      items.push({
        id: `lo-${l.id}`,
        group: "Instances",
        label: l.name,
        sub: games.find((g) => g.id === l.gameId)?.short,
        icon: "branch",
        run: () => void pickLoadout(l.id, "mods"),
      });
    }
    for (const e of entries) {
      items.push({
        id: `mod-${e.modId}`,
        group: "Mods",
        label: e.name,
        sub: e.enabled ? "activé" : "désactivé",
        icon: "box",
        run: () => {
          setTab("mods");
          openMod(e.modId);
        },
      });
    }
    return items;
  }, [tabs, loadouts, games, entries, act]);

  /**
   * One page per tab id — the shell's stand-in for a router: the active tab
   * picks a page, and nothing else about it is dynamic. The dashboard is
   * handled above, because it is also where the settings pane opens.
   */
  function pageForTab(): ReactNode {
    switch (currentTab) {
      case "mods":
        return (
          <ModsPage
            rows={filtered}
            entryCount={entries.length}
            selectedId={selectedMod}
            onSelect={openMod}
            onToggle={(row) => void toggle(row)}
            conflicts={conflictByMod}
            onImportSource={startImport}
            onInstallArchived={(row) => void installArchived(row)}
            categories={categories}
            onSetCategory={(row, category) => void setCategoryFor(row, category)}
            otherLoadouts={otherLoadouts}
            onReassign={(row, toLoadoutId) => void reassignModTo(row, toLoadoutId)}
            onRename={(row, name) => void renameModRow(row, name)}
            onDelete={(row) => void deleteModRow(row)}
            onUninstall={(row) => void uninstallModRow(row)}
            onRevealFolder={revealModFolder}
            onRevealArchive={revealModArchive}
          />
        );
      case "plugins":
        return <PluginsPage loadoutId={activeLoadout?.id ?? null} />;
      case "archives":
        return <ArchivesPage loadoutId={activeLoadout?.id ?? null} />;
      case "repertoire":
        return <RepertoirePage loadoutId={activeLoadout?.id ?? null} />;
      case "saves":
        return <SavesPage />;
      case "sante":
        return <SantePage />;
      case "packs":
        return <PacksPage />;
      case "worlds":
        return <WorldsPage />;
      case "servers":
        return <ServersPage />;
      case "web":
        // Rendered separately, always-mounted, as a sibling below — see its
        // render site's own doc for why.
        return null;
      default:
        // Only reachable on the dashboard tab when no game was detected at
        // all: there is no dashboard to draw, so the tab says so itself.
        return <StubTab tab={currentTab} />;
    }
  }

  if (error && !snap) {
    return (
      <div className="empty">
        <h3>Impossible de joindre le Core</h3>
        <p>{error}</p>
      </div>
    );
  }

  return (
    <LayoutShell
      rightOpen={rightOpen}
      running={running}
      webActive={currentTab === "web"}
      aiActive={rightOpen && rightTab === "claude"}
      onOpenWeb={() => openWeb()}
      onOpenAi={() => {
        setRightOpen(true);
        setRightTab("claude");
      }}
      onOpenConsole={() => setTxOpen(true)}
      // Réglages, Profil and Communauté aren't tied to a game's tab set —
      // the strip would show the previous game's tabs for a page that
      // isn't "in" any tab, so it hides instead of showing something stale.
      hideTabs={act === "settings" || act === "profile" || act === "community"}
      tabs={tabs}
      currentTab={currentTab}
      modCount={entries.length}
      webOpen={webOpen}
      onSelectTab={setTab}
      sidebar={
        <Sidebar
          act={act}
          onAct={handleAct}
          menuOpen={menuOpen}
          onToggleMenu={() => setMenuOpen((v) => !v)}
          onCloseMenu={() => setMenuOpen(false)}
          onMenuAction={handleMenuAction}
          activeGame={activeGame}
          activeLoadout={activeLoadout}
          games={games}
          loadouts={loadouts}
          favorites={favorites}
          expanded={expanded}
          enabledCount={enabledCount}
          entryCount={entries.length}
          engineCaps={engineCaps}
          categoryCounts={categoryCounts}
          onPickCategory={pickCategory}
          running={running}
          dataRoot={snap?.dataRoot}
          versionTick={versionTick}
          isFavorite={isFavorite}
          onToggleFavorite={onFavorite}
          onPickLoadout={(id) => void pickLoadout(id, "mods")}
          onPickGame={pickGame}
          onToggleExpanded={(id) =>
            setExpanded((v) => (v.includes(id) ? [] : [id]))
          }
          onAddGame={() => unwired("Ajouter un jeu")}
          onPlay={() => void play()}
          onShare={() => setToast("Partage d’instance : pas encore branché.")}
          onRenameLoadout={(name) => void renameActiveLoadout(name)}
          onSetLoadoutChannel={(channel) => void setActiveLoadoutChannel(channel)}
          onSnapshot={doSnapshot}
          onRollback={doRollback}
          onCreateLoadout={() => setCreateOpen(true)}
          onRenameLoadoutById={(id, name) => void renameLoadoutById(id, name)}
          onSetLoadoutChannelById={(id, channel) => void setLoadoutChannelById(id, channel)}
          onDeleteLoadout={(id) => void deleteLoadoutById(id)}
          onRestoreLoadout={(id, name) => restoreLoadoutById(id, name)}
          query={query}
          onQueryChange={setQuery}
          onOpenTransactions={() => setTxOpen(true)}
          settingsMenu={settingsMenu}
          onSettingsMenu={setSettingsMenu}
          onSettingsBack={() => setAct(prevAct)}
          communityMenu={communityMenu}
          onCommunityMenu={setCommunityMenu}
          onCommunityBack={() => setAct(prevAct)}
          communityEnabled={communityUpdate}
        />
      }
      right={
        rightOpen ? (
          <RightPanel
            tab={rightTab}
            onTab={setRightTab}
            onClose={() => setRightOpen(false)}
            mod={selectedRow}
            conflicts={modConflict}
            loadoutId={activeLoadout?.id ?? null}
            chat={chat}
            llm={llm}
            draft={draft}
            chatContext={`${activeLoadout?.name ?? "—"}${selectedRow ? ` · ${selectedRow.name}` : ""}`}
            onDraftChange={setDraft}
            onSendChat={sendChat}
          />
        ) : null
      }
      dock={
        <>
          {txOpen && bottomTab === "transactions" ? (
            <TransactionsPanel
              txs={txs}
              canSnapshot={Boolean(activeLoadout)}
              onSnapshot={() => void doSnapshot()}
              onRollback={() => void doRollback()}
              onClose={() => setTxOpen(false)}
              downloadsCount={downloads?.length ?? 0}
              onBottomTabChange={setBottomTab}
            />
          ) : null}
          {txOpen && bottomTab === "downloads" ? (
            <DownloadsPanel
              downloads={downloads}
              loadouts={loadouts}
              games={games}
              onClose={() => setTxOpen(false)}
              onBottomTabChange={setBottomTab}
              onOpenLoadout={(id) => void pickLoadout(id, "mods")}
            />
          ) : null}
          <Composer
            draft={draft}
            modelLabel={LLM_MODELS.find((m) => m[0] === llm)?.[1]}
            onDraftChange={setDraft}
            onFocus={() => setPaletteOpen(true)}
            onSubmit={sendChat}
          />
        </>
      }
      overlays={
        <>
          {toast ? <Toast icon="check">{toast}</Toast> : null}
          <CommandPalette
            open={paletteOpen}
            items={paletteItems}
            onClose={() => setPaletteOpen(false)}
          />
          {error && snap ? (
            <Toast icon="alert" style={{ bottom: 80 }}>
              {error}
            </Toast>
          ) : null}
          {createOpen && activeGame ? (
            <CreateLoadoutDialog
              gameName={activeGame.name}
              onClose={() => setCreateOpen(false)}
              onCreate={createInstance}
            />
          ) : null}
          {mo2Open && activeGame ? (
            <Mo2ImportDialog
              gameName={activeGame.name}
              onClose={() => setMo2Open(false)}
              onImport={importMo2}
            />
          ) : null}
          {fomodTarget ? (
            <FomodWizard
              contentRoot={fomodTarget.contentRoot}
              onInstall={finishFomodInstall}
              onClose={() => setFomodTarget(null)}
            />
          ) : null}
        </>
      }
    >
      {act === "settings" ? (
        <SettingsPane
          menu={settingsMenu}
          games={games}
          llm={llm}
          llmModels={LLM_MODELS}
          dataRoot={snap?.dataRoot}
          onSelectModel={(id, name) => {
            setLlm(id);
            setToast(`Modèle local sélectionné : ${name}`);
          }}
          communityUpdate={communityUpdate}
          onToggleCommunityUpdate={() => setCommunityUpdate((v) => !v)}
          onUnwired={unwired}
        />
      ) : act === "profile" ? (
        <ProfilePage
          communityUpdate={communityUpdate}
          onToggleCommunityUpdate={() => setCommunityUpdate((v) => !v)}
        />
      ) : act === "community" ? (
        communityMenu === "marketplace" ? (
          <MarketplacePage onUnwired={unwired} />
        ) : (
          <PatchWorkshopPage onUnwired={unwired} />
        )
      ) : (
        <>
          {currentTab === "dashboard" && activeGame ? (
            <DashboardPage
              game={activeGame}
              loadouts={gameLoadouts}
              activeLoadoutId={activeLoadout?.id ?? null}
              modCount={activeLoadout?.modCount ?? entries.length}
              isFavorite={isFavorite}
              onToggleFavorite={onFavorite}
              onPickLoadout={(id) => void pickLoadout(id, "dashboard")}
              onCreateLoadout={() => setCreateOpen(true)}
              onImportSource={startImport}
              onGoToTab={(t) => setTab(t)}
              onUnwired={unwired}
            />
          ) : currentTab === "web" ? null : (
            <>
              <Crumbs
                path={[activeGame?.short ?? "—", activeLoadout?.name ?? "—", tabLabel]}
                tail={snap?.dataRoot ?? ""}
              />
              {pageForTab()}
            </>
          )}
        </>
      )}
      {webOpen ? (
        // Always mounted from the first open onward, positioned to cover
        // the same area the rest of the tab content would — see
        // `BrowserPane`'s own doc for why it must not unmount on every tab
        // switch. No breadcrumb even when visible: the browser isn't
        // "inside" an instance, and the native webview needs every pixel of
        // the content area.
        <div
          style={{
            position: "absolute",
            inset: 0,
            display: currentTab === "web" && act !== "settings" ? "flex" : "none",
            flexDirection: "column",
          }}
        >
          <BrowserPane
            homeUrl={(() => {
              // Read-then-clear, both within this same render: `openWebUrl`
              // sets this right before bumping `browserOpenToken`, so it's
              // only ever non-null for the one render that opens on it — a
              // later open (no override set) falls back to the active
              // game's own home page again.
              const url = pendingWebUrlRef.current ?? browseHomeFor(activeGame?.id);
              pendingWebUrlRef.current = null;
              return url;
            })()}
            gameId={activeGame?.id ?? null}
            visible={currentTab === "web" && act !== "settings"}
            openToken={browserOpenToken}
            onOpenDownloads={() => setTxOpen(true)}
          />
        </div>
      ) : null}
    </LayoutShell>
  );
}
