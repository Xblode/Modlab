import { FormEvent, type ReactNode, useEffect, useMemo, useState } from "react";
import {
  getShellSnapshot,
  createLoadout,
  mo2Import,
  launchSkse,
  setModEnabled,
  aiAsk,
  snapshotLoadout,
  rollbackLoadout,
  type DetectedGame,
  type LoadoutDto,
  type ModRow,
  type ShellSnapshot,
} from "./api";
import { useFavorites } from "./favorites";
import { useConflicts } from "./hooks/useConflicts";
import { useTransactions } from "./hooks/useTransactions";
import { tabsFor, type TabId } from "./gameChrome";
import { StubTab } from "./StubTab";

import { LayoutShell } from "./components/Layout/LayoutShell";
import { Sidebar, type SideAct } from "./components/Features/Sidebar/Sidebar";
import { TabStrip } from "./components/Features/Tabs/TabStrip";
import { RightPanel, type RightTab } from "./components/Features/Inspector/RightPanel";
import { type ChatMsg } from "./components/Features/Chat/ChatPanel";
import { Composer } from "./components/Features/Composer/Composer";
import { TransactionsPanel } from "./components/Features/Transactions/TransactionsPanel";
import { SettingsPane, type SettingsMenu } from "./components/Features/Settings/SettingsPane";
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
  const [snap, setSnap] = useState<ShellSnapshot | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [act, setAct] = useState<SideAct>("explorer");
  // The activity to return to when leaving Réglages — captured the moment we
  // enter it, so `SideBack` can undo whichever icon actually opened it.
  const [prevAct, setPrevAct] = useState<SideAct>("explorer");
  const [menuOpen, setMenuOpen] = useState(false);
  const [settingsMenu, setSettingsMenu] = useState<SettingsMenu>("general");
  const [tab, setTab] = useState<TabId>("dashboard");
  const [gameId, setGameId] = useState<string | null>(null);
  const [loadoutId, setLoadoutId] = useState<number | null>(null);
  const [selectedMod, setSelectedMod] = useState<number | null>(null);
  const [expanded, setExpanded] = useState<string[]>([]);
  const [rightOpen, setRightOpen] = useState(false);
  const [rightTab, setRightTab] = useState<RightTab>("info");
  const [running, setRunning] = useState(false);
  const [toast, setToast] = useState<string | null>(null);
  const [llm, setLlm] = useState("qwen-1.5b");
  const [createOpen, setCreateOpen] = useState(false);
  const [mo2Open, setMo2Open] = useState(false);
  const [query, setQuery] = useState("");
  const [paletteOpen, setPaletteOpen] = useState(false);
  const [txOpen, setTxOpen] = useState(false);

  /** Ctrl+K / Ctrl+Shift+P — the palette is the keyboard entry point (§6.2).
   * Ctrl+J opens the bottom panel (transactions), same vocabulary. */
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
      }
    }
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, []);

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
        setMo2Open(true);
        break;
      default:
        setToast(`« ${item.label} » : pas encore branché au Core.`);
    }
  }

  /** Every rail click and the palette's "Ouvrir les réglages" go through
   * here so `prevAct` always names whatever was active right before
   * Réglages opened, not just a hardcoded default. */
  function handleAct(next: SideAct) {
    if (next === "settings" && act !== "settings") {
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
    getShellSnapshot()
      .then((s) => {
        if (cancelled) return;
        setSnap(s);
        setLoadoutId(s.selectedLoadoutId);
        const first = s.games[0]?.id ?? s.loadouts[0]?.gameId ?? null;
        setGameId(first);
        if (first) setExpanded([first]);
      })
      .catch((e: unknown) => {
        if (!cancelled) setError(e instanceof Error ? e.message : String(e));
      });
    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    if (toast == null) return;
    const t = window.setTimeout(() => setToast(null), 2800);
    return () => window.clearTimeout(t);
  }, [toast]);

  const games = snap?.games ?? [];
  const loadouts = snap?.loadouts ?? [];
  const activeGame: DetectedGame | undefined =
    games.find((g) => g.id === gameId) ?? games[0];
  const gameLoadouts = loadouts.filter(
    (l) => !activeGame || l.gameId === activeGame.id,
  );
  const activeLoadout: LoadoutDto | undefined =
    gameLoadouts.find((l) => l.id === loadoutId) ?? gameLoadouts[0];
  const entries = snap?.entries ?? [];
  const enabledCount = entries.filter((e) => e.enabled).length;
  const selectedRow = entries.find((e) => e.modId === selectedMod) ?? null;
  const tabs = tabsFor(activeGame?.capabilities);
  // A game switch can land on a tab the new game does not have.
  const currentTab: TabId = tabs.some((t) => t.id === tab) ? tab : "dashboard";
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
    return entries.filter((e) => e.name.toLowerCase().includes(q));
  }, [entries, query]);

  const { conflictByMod } = useConflicts(activeLoadout, entries);
  const { txs, refreshTxs } = useTransactions(txOpen);

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
      setExpanded((e) => (e.includes(owner.gameId) ? e : [...e, owner.gameId]));
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
    setExpanded((e) => (e.includes(id) ? e : [...e, id]));
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
    setExpanded((e) =>
      e.includes(result.loadout.gameId) ? e : [...e, result.loadout.gameId],
    );
    setTab("dashboard");
    setMo2Open(false);
    setToast(
      `${result.imported} mod(s) importé(s) dans « ${result.loadout.name} » — les mods pointent vers les dossiers MO2, en place.`,
    );
    return result.imported;
  }

  async function toggle(row: ModRow) {
    if (!activeLoadout) return;
    try {
      const next = await setModEnabled(activeLoadout.id, row.modId, !row.enabled);
      setSnap((prev) => (prev ? { ...prev, entries: next } : prev));
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
        id: "cmd-mo2",
        group: "Commandes",
        label: "Importer un profil MO2…",
        icon: "download",
        run: () => setMo2Open(true),
      },
      {
        id: "cmd-play",
        group: "Commandes",
        label: "Lancer le jeu (SKSE)",
        icon: "play",
        run: () => void play(),
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
          />
        );
      case "plugins":
        return <PluginsPage />;
      case "archives":
        return <ArchivesPage />;
      case "repertoire":
        return <RepertoirePage />;
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
          running={running}
          dataRoot={snap?.dataRoot}
          isFavorite={isFavorite}
          onToggleFavorite={onFavorite}
          onPickLoadout={(id) => void pickLoadout(id, "mods")}
          onPickGame={pickGame}
          onToggleExpanded={(id) =>
            setExpanded((v) => (v.includes(id) ? v.filter((x) => x !== id) : [...v, id]))
          }
          onAddGame={() => unwired("Ajouter un jeu")}
          onPlay={() => void play()}
          onShare={() => setToast("Partage d’instance : pas encore branché.")}
          query={query}
          onQueryChange={setQuery}
          txCount={txs?.length ?? null}
          onOpenTransactions={() => setTxOpen(true)}
          settingsMenu={settingsMenu}
          onSettingsMenu={setSettingsMenu}
          onSettingsBack={() => setAct(prevAct)}
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
          {txOpen ? (
            <TransactionsPanel
              txs={txs}
              canSnapshot={Boolean(activeLoadout)}
              onSnapshot={() => void doSnapshot()}
              onRollback={() => void doRollback()}
              onClose={() => setTxOpen(false)}
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
          onUnwired={unwired}
        />
      ) : (
        <>
          <TabStrip
            tabs={tabs}
            currentTab={currentTab}
            modCount={entries.length}
            aiActive={rightOpen && rightTab === "claude"}
            onSelect={setTab}
            onOpenAi={() => {
              setRightOpen(true);
              setRightTab("claude");
            }}
          />

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
              onImportSource={() => setMo2Open(true)}
              onGoToTab={(t) => setTab(t)}
              onUnwired={unwired}
            />
          ) : (
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
    </LayoutShell>
  );
}
