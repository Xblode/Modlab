import { useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { isTauriShell } from "./api";
import { withPreviewProgress } from "./downloadProgress";
import { useFavorites } from "./favorites";
import {
  allBrowseSitesFor,
  browseSiteFavoritesFor,
  orderedAllBrowseSitesFor,
  readUiMemory,
  toggleBrowseSiteFavorite,
} from "./uiMemory";
import { applyDensity, type Density } from "./density";
import { type TabId } from "./gameChrome";
import { BrowserPane } from "./components/Features/Browser/BrowserPane";
import { NEW_TAB_URL } from "./webHomepage";
import { FomodWizard } from "./components/Features/Mods/FomodWizard";
import { LayoutShell } from "./components/Layout/LayoutShell";
import { Sidebar } from "./components/Features/Sidebar/Sidebar";
import { TransactionsPanel } from "./components/Features/Transactions/TransactionsPanel";
import { DownloadsPanel } from "./components/Features/Transactions/DownloadsPanel";
import {
  isWebTab,
  RightPanel,
  withRightTabs,
} from "./components/Features/Inspector/RightPanel";
import { CommandPalette, type PaletteItem } from "./components/Features/CommandPalette/CommandPalette";
import { CreateLoadoutDialog } from "./components/Features/LoadoutDialogs/CreateLoadoutDialog";
import { CreateBranchDialog } from "./components/Features/LoadoutDialogs/CreateBranchDialog";
import { Mo2ImportDialog } from "./components/Features/LoadoutDialogs/Mo2ImportDialog";
import { Toast } from "./components/ui";
import { BranchHeader } from "./components/Features/Instance/BranchHeader";
import { ShellToolbar } from "./components/Features/Instance/ShellToolbar";
import { ShellMainContent } from "./shell/ShellMainContent";
import { useShellSession } from "./shell/useShellSession";
import { useShellNavigation } from "./shell/useShellNavigation";
import { toggleExpandedGame } from "./shell/shellNavHelpers";
import { useModsSpace } from "./spaces/mods/useModsSpace";
import { useInstanceSpace } from "./spaces/instance/useInstanceSpace";
import { useUtilitySpace } from "./spaces/utility/useUtilitySpace";
import { useDockSpace } from "./spaces/dock/useDockSpace";

export default function App() {
  const [initialMemory] = useState(readUiMemory);
  const [tab, setTab] = useState<TabId>(() => {
    const remembered = initialMemory.tab;
    if (!remembered || remembered === "web") return "dashboard";
    if ((remembered as string) === "instance") return "mods";
    return remembered;
  });
  const [gameId, setGameId] = useState<string | null>(null);
  const [loadoutId, setLoadoutId] = useState<number | null>(null);
  const [expanded, setExpanded] = useState<string[]>(initialMemory.expanded);
  const [toast, setToast] = useState<string | null>(null);
  const [communityUpdate, setCommunityUpdate] = useState(initialMemory.communityUpdate);
  const [showEmptyCategories, setShowEmptyCategories] = useState(initialMemory.showEmptyCategories);
  const [density, setDensity] = useState<Density>(initialMemory.density);
  const [paletteOpen, setPaletteOpen] = useState(false);
  const [siteFavoriteIds, setSiteFavoriteIds] = useState<string[]>(() =>
    browseSiteFavoritesFor(initialMemory.gameId),
  );
  const refreshTxsRef = useRef<() => void>(() => {});

  useLayoutEffect(() => {
    applyDensity(density);
  }, [density]);

  const session = useShellSession({
    initialMemory,
    gameId,
    loadoutId,
    setGameId,
    setLoadoutId,
    setExpanded,
  });

  const mods = useModsSpace({
    activeLoadout: session.activeLoadout,
    entries: session.entries,
    catalog: session.catalog,
    otherLoadouts: session.otherLoadouts,
    setSnap: session.setSnap,
    setError: session.setError,
    setToast,
    setVersionTick: session.setVersionTick,
    refreshTxs: () => refreshTxsRef.current(),
    setTab,
    showEmptyCategories,
  });

  const utility = useUtilitySpace({
    initialMemory,
    activeLoadout: session.activeLoadout,
    selectedMod: mods.selectedMod,
    setSnap: session.setSnap,
    setToast,
    setSelectedMod: mods.setSelectedMod,
  });

  const dock = useDockSpace({
    activeLoadout: session.activeLoadout,
    activeLoadoutIdRef: session.activeLoadoutIdRef,
    refresh: session.refresh,
    setSnap: session.setSnap,
    setTab,
    setToast,
    setError: session.setError,
    pane: utility.pane,
  });
  refreshTxsRef.current = dock.refreshTxs;

  const instance = useInstanceSpace({
    snap: session.snap,
    activeGame: session.activeGame,
    activeLoadout: session.activeLoadout,
    loadouts: session.loadouts,
    loadoutId,
    setSnap: session.setSnap,
    setGameId,
    setLoadoutId,
    setExpanded,
    setTab,
    setError: session.setError,
    setToast,
    refresh: session.refresh,
    setVersionTick: session.setVersionTick,
    refreshTxs: dock.refreshTxs,
    onFocusConsole: () => utility.openConsole(),
  });

  const nav = useShellNavigation({
    hasSnap: session.hasSnap,
    gameId,
    loadoutId,
    setGameId,
    setLoadoutId,
    tab,
    setTab,
    expanded,
    setExpanded,
    pane: utility.pane,
    llm: utility.llm,
    communityUpdate,
    showEmptyCategories,
    density,
    loadouts: session.loadouts,
    activeLoadout: session.activeLoadout,
    activeGame: session.activeGame,
    refresh: session.refresh,
    setSelectedMod: mods.setSelectedMod,
    setError: session.setError,
    setPaletteOpen,
    setTxOpen: dock.setTxOpen,
    saveActive: instance.saveActive,
  });

  const { isFavorite, toggleFavorite, favoriteIds } = useFavorites();

  const activeGameId = session.activeGame?.id ?? null;
  const orderedSites = useMemo(
    () => orderedAllBrowseSitesFor(activeGameId, siteFavoriteIds),
    [activeGameId, siteFavoriteIds],
  );

  useEffect(() => {
    setSiteFavoriteIds(browseSiteFavoritesFor(activeGameId));
  }, [activeGameId]);

  function handleToggleSiteFavorite(siteId: string) {
    setSiteFavoriteIds(toggleBrowseSiteFavorite(activeGameId, siteId));
  }

  function handleTitleWebClick() {
    // Titlebar Web opens the utility drawer on a new-tab page — never the
    // favourite-site picker (that stays on « + Ajouter un mod » only).
    utility.openWeb(NEW_TAB_URL);
  }

  function refreshSiteFavorites() {
    setSiteFavoriteIds(browseSiteFavoritesFor(activeGameId));
  }

  useEffect(() => {
    if (toast == null) return;
    const t = window.setTimeout(() => setToast(null), 2800);
    return () => window.clearTimeout(t);
  }, [toast]);

  const downloadProgressByMod = withPreviewProgress(
    dock.liveDownloads.byMod,
    session.entries,
    !isTauriShell(),
  );

  const favorites = useMemo(
    () =>
      session.instances
        .filter((l) => favoriteIds.includes(l.id) || favoriteIds.includes(l.instanceId))
        .map((l) => ({
          loadout: l,
          short: session.games.find((x) => x.id === l.gameId)?.short ?? l.gameId,
        })),
    [favoriteIds, session.instances, session.games],
  );

  function onFavorite(id: number) {
    const name = session.loadouts.find((l) => l.id === id)?.instanceName ?? "";
    const now = toggleFavorite(id);
    setToast(
      now ? `« ${name} » ajoutée aux favoris` : `« ${name} » retirée des favoris`,
    );
  }

  const paletteItems: PaletteItem[] = useMemo(() => {
    const items: PaletteItem[] = [
      {
        id: "cmd-new",
        group: "Commandes",
        label: "Nouvelle instance…",
        icon: "plus",
        run: () => instance.setCreateOpen(true),
      },
      {
        id: "cmd-import",
        group: "Commandes",
        label:
          session.activeGame?.capabilities.importSource === "minecraft-instance"
            ? "Importer l’instance Minecraft détectée"
            : "Importer un profil MO2…",
        icon: "download",
        run: instance.startImport,
      },
      {
        id: "cmd-play",
        group: "Commandes",
        label: "Lancer le jeu (SKSE)",
        icon: "play",
        run: () => void instance.play(),
      },
      {
        id: "cmd-save",
        group: "Commandes",
        label: "Enregistrer (Ctrl+S)",
        icon: "db",
        run: () => void instance.saveActive(),
      },
      {
        id: "cmd-tx",
        group: "Commandes",
        label: "Afficher le panneau du bas (transactions)",
        icon: "history",
        run: () => dock.setTxOpen(true),
      },
      {
        id: "cmd-right",
        group: "Commandes",
        label: "Basculer le panneau droit",
        icon: "layers",
        run: () => {
          if (utility.paneRef.current.tabs.length > 0) utility.closeRightPane();
          else utility.setPane({ tabs: ["info"], active: "info" });
        },
      },
      {
        id: "cmd-settings",
        group: "Commandes",
        label: "Ouvrir les réglages",
        icon: "gear",
        run: () => {
          nav.handleAct("settings");
          setTab("dashboard");
        },
      },
      {
        id: "cmd-ia",
        group: "IA",
        label: "Demander à l'IA locale…",
        icon: "sparkle",
        run: () => {
          utility.setPane((prev) => ({
            tabs: withRightTabs(prev.tabs, ["claude"]),
            active: "claude",
          }));
        },
      },
    ];
    for (const t of nav.tabs) {
      items.push({
        id: `tab-${t.id}`,
        group: "Onglets",
        label: `Ouvrir : ${t.label}`,
        icon: t.icon,
        run: () => setTab(t.id),
      });
    }
    for (const l of session.instances) {
      items.push({
        id: `lo-${l.instanceId}`,
        group: "Instances",
        label: l.instanceName,
        sub: session.games.find((g) => g.id === l.gameId)?.short,
        icon: "layers",
        run: () => nav.openInstance(l.id, "mods"),
      });
    }
    for (const e of session.entries) {
      items.push({
        id: `mod-${e.modId}`,
        group: "Mods",
        label: e.name,
        sub: e.enabled ? "activé" : "désactivé",
        icon: "box",
        run: () => {
          setTab("mods");
          utility.openMod(e.modId);
        },
      });
    }
    return items;
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [nav.tabs, session.instances, session.games, session.entries]);

  if (session.error && !session.snap) {
    return (
      <div className="empty">
        <h3>Impossible de joindre le Core</h3>
        <p>{session.error}</p>
      </div>
    );
  }

  return (
    <LayoutShell
      rightOpen={utility.pane.tabs.length > 0}
      utilityColumn={utility.pane.tabs.length > 0}
      running={instance.running}
      webActive={utility.pane.active != null && isWebTab(utility.pane.active)}
      aiActive={utility.pane.active === "claude"}
      consoleActive={utility.pane.active === "console"}
      onOpenWeb={handleTitleWebClick}
      onOpenAi={() => utility.toggleRight("claude")}
      onOpenConsole={() => utility.toggleRight("console")}
      onOpenSearch={() => setPaletteOpen(true)}
      hideTabs={nav.act === "settings" || nav.act === "profile" || nav.act === "community"}
      tabs={nav.tabs}
      currentTab={nav.currentTab}
      modCount={session.entries.length}
      onSelectTab={setTab}
      header={
        session.activeGame &&
        nav.currentTab === "mods" &&
        nav.act !== "settings" &&
        nav.act !== "profile" &&
        nav.act !== "community" ? (
          <BranchHeader
            game={session.activeGame}
            loadout={session.activeLoadout ?? null}
            entries={session.entries}
            mcLoaderFacts={instance.mcLoaderFacts}
            onToggleMod={(row) => void mods.toggle(row)}
            onInstallArchived={(row) => void mods.installArchived(row)}
            onOpenWeb={(url) => utility.openWeb(url)}
            onUnwired={instance.unwired}
            onCreateInstance={() => instance.setCreateOpen(true)}
          />
        ) : null
      }
      toolbar={
        session.activeGame &&
        session.activeLoadout &&
        nav.currentTab === "mods" &&
        nav.act !== "settings" &&
        nav.act !== "profile" &&
        nav.act !== "community" ? (
          <ShellToolbar
            activeId={session.activeLoadout.id}
            name={session.activeLoadout.name}
            channel={session.activeLoadout.channel}
            revision={session.loadoutVersion?.revision ?? session.activeLoadout.revision}
            version={session.loadoutVersion}
            branches={session.instanceBranches}
            enabledCount={session.enabledCount}
            totalCount={session.entries.length}
            onPickBranch={(id) => void nav.pickLoadout(id, "mods")}
            onCreateBranch={() => instance.setBranchOpen(true)}
            onSnapshot={instance.doSnapshot}
            onRollback={instance.doRollback}
            onNotice={setToast}
            browseSites={orderedSites}
            favoriteSiteIds={siteFavoriteIds}
            onToggleSiteFavorite={handleToggleSiteFavorite}
            onOpenSite={(url) => utility.openWeb(url)}
            onImport={() => void instance.startManualImport()}
            onOpenInstanceFolder={instance.revealActiveInstanceFolder}
            primaryAction={instance.toolbarPrimaryAction}
            showTestLaunch={session.activeGame?.id === "minecraft"}
            testLaunchBusy={instance.testLaunchBusy}
            onTestLaunch={() => void instance.testLaunch()}
          />
        ) : null
      }
      sidebar={
        <Sidebar
          act={nav.act}
          onAct={nav.handleAct}
          menuOpen={nav.menuOpen}
          onToggleMenu={() => nav.setMenuOpen((v) => !v)}
          onCloseMenu={() => nav.setMenuOpen(false)}
          onMenuAction={instance.handleMenuAction}
          activeGame={session.activeGame}
          activeLoadout={session.activeLoadout}
          games={session.games}
          loadouts={session.instances}
          branches={session.instanceBranches}
          onPickBranch={(id) => void nav.pickLoadout(id)}
          favorites={favorites}
          expanded={expanded}
          enabledCount={session.enabledCount}
          entryCount={session.entries.length}
          launchNotice={instance.launchNotice}
          onDismissLaunchNotice={() => instance.setLaunchNotice(null)}
          categoryCounts={mods.categoryCounts}
          categoryCatalog={session.catalog}
          activeCategory={mods.activeCategory}
          onPickCategory={mods.pickCategory}
          showEmptyCategories={showEmptyCategories}
          onToggleShowEmptyCategories={() => setShowEmptyCategories((v) => !v)}
          running={instance.running}
          dataRoot={session.snap?.dataRoot}
          isFavorite={isFavorite}
          onToggleFavorite={onFavorite}
          onPickLoadout={(id) => nav.openInstance(id, "mods")}
          onPickGame={nav.pickGame}
          onToggleExpanded={(id) => setExpanded((current) => toggleExpandedGame(current, id))}
          onAddGame={() => instance.unwired("Ajouter un jeu")}
          onPlay={() => void instance.play()}
          onShare={() => setToast("Partage d’instance : pas encore branché.")}
          onRenameLoadout={(name) => void instance.renameActiveLoadout(name)}
          onSetLoadoutChannel={(channel) => void instance.setActiveLoadoutChannel(channel)}
          onSnapshot={instance.doSnapshot}
          onRollback={instance.doRollback}
          onCreateLoadout={() => instance.setCreateOpen(true)}
          onRenameLoadoutById={(id, name) => void instance.renameLoadoutById(id, name)}
          onSetLoadoutChannelById={(id, channel) => void instance.setLoadoutChannelById(id, channel)}
          onDeleteLoadout={(id) => void instance.deleteLoadoutById(id)}
          onRestoreLoadout={(id, name) => instance.restoreLoadoutById(id, name)}
          query={mods.query}
          onQueryChange={mods.setQuery}
          onOpenTransactions={() => dock.setTxOpen(true)}
          settingsMenu={nav.settingsMenu}
          onSettingsMenu={nav.setSettingsMenu}
          onSettingsBack={() => nav.setAct(nav.prevAct)}
          communityMenu={nav.communityMenu}
          onCommunityMenu={nav.setCommunityMenu}
          onCommunityBack={() => nav.setAct(nav.prevAct)}
          communityEnabled={communityUpdate}
        />
      }
      right={
        utility.pane.tabs.length > 0 ? (
          <RightPanel
            className="utility-col"
            tabs={utility.pane.tabs}
            active={utility.pane.active}
            onFocus={(id) => utility.setPane((prev) => ({ ...prev, active: id }))}
            onCloseTab={utility.closeRight}
            onClosePane={utility.closeRightPane}
            mod={mods.selectedRow}
            conflicts={mods.modConflict}
            dependencies={
              mods.selectedRow == null
                ? undefined
                : mods.dependencyByMod
                  ? (mods.dependencyByMod.get(mods.selectedRow.modId) ?? [])
                  : undefined
            }
            loadoutId={session.activeLoadout?.id ?? null}
            chat={utility.chat}
            llm={utility.llm}
            draft={utility.draft}
            chatTags={utility.chatTags}
            chatFocusNonce={utility.chatFocusNonce}
            onDraftChange={utility.setDraft}
            onRemoveChatTag={utility.removeChatTag}
            onSendChat={utility.sendChat}
            onOpenPage={(url) => utility.openWeb(url)}
            webPages={utility.webPages}
            browser={
              // Keep BrowserPane mounted across Mods/Chat focus (`displayWebPage`).
              // Download start/progress/captured must not null this out or change key.
              utility.displayWebPage ? (
                <BrowserPane
                  pageId={utility.displayWebPage.id}
                  url={utility.displayWebPage.url}
                  onNavigate={(url, title) =>
                    utility.updateWebPage(utility.displayWebPage!.id, url, title)
                  }
                  gameId={session.activeGame?.id ?? null}
                  games={session.games}
                  visible={utility.pane.active != null && isWebTab(utility.pane.active)}
                  favoriteSiteIds={siteFavoriteIds}
                  onFavoriteChange={refreshSiteFavorites}
                />
              ) : null
            }
            transactions={dock.txs}
            gameLog={instance.gameLog}
            catalog={session.catalog}
            onSetCategory={(category) => {
              if (mods.selectedRow) void mods.setCategoryFor(mods.selectedRow, category);
            }}
            onOpenWeb={() => utility.openWeb(NEW_TAB_URL)}
            onOpenConsole={() => utility.toggleRight("console")}
            onOpenChat={utility.openChat}
            running={instance.running}
            gameName={session.activeGame?.name ?? null}
          />
        ) : null
      }
      dock={
        <>
          {dock.txOpen && dock.bottomTab === "transactions" ? (
            <TransactionsPanel
              txs={dock.txs}
              canSnapshot={Boolean(session.activeLoadout)}
              onSnapshot={() => void instance.doSnapshot()}
              onRollback={() => void instance.doRollback()}
              onClose={() => dock.setTxOpen(false)}
              downloadsCount={dock.downloads?.length ?? 0}
              onBottomTabChange={dock.setBottomTab}
            />
          ) : null}
          {dock.txOpen && dock.bottomTab === "downloads" ? (
            <DownloadsPanel
              downloads={dock.downloads}
              progressById={dock.liveDownloads.byDownload}
              loadouts={session.loadouts}
              games={session.games}
              onClose={() => dock.setTxOpen(false)}
              onBottomTabChange={dock.setBottomTab}
              onOpenLoadout={(id) => void nav.pickLoadout(id, "mods")}
            />
          ) : null}
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
          {session.error && session.snap ? (
            <Toast icon="alert" style={{ bottom: 80 }}>
              {session.error}
            </Toast>
          ) : null}
          {instance.createOpen && session.activeGame ? (
            <CreateLoadoutDialog
              gameName={session.activeGame.name}
              gameId={session.activeGame.id}
              existingInstanceNames={session.gameInstances.map((row) => row.instanceName)}
              onClose={() => instance.setCreateOpen(false)}
              onCreate={instance.createInstance}
            />
          ) : null}
          {instance.branchOpen && session.activeLoadout ? (
            <CreateBranchDialog
              instanceName={session.activeLoadout.instanceName}
              onClose={() => instance.setBranchOpen(false)}
              onCreate={instance.createInstanceBranch}
            />
          ) : null}
          {instance.mo2Open && session.activeGame ? (
            <Mo2ImportDialog
              gameName={session.activeGame.name}
              onClose={() => instance.setMo2Open(false)}
              onImport={instance.importMo2}
            />
          ) : null}
          {mods.fomodTarget && session.activeLoadout ? (
            <FomodWizard
              contentRoot={mods.fomodTarget.contentRoot}
              loadoutId={session.activeLoadout.id}
              onInstall={mods.finishFomodInstall}
              onClose={() => {
                if (mods.fomodTarget) mods.markInstalling(mods.fomodTarget.modId, false);
                mods.setFomodTarget(null);
              }}
            />
          ) : null}
        </>
      }
    >
      <ShellMainContent
        act={nav.act}
        settingsMenu={nav.settingsMenu}
        games={session.games}
        llm={utility.llm}
        dataRoot={session.snap?.dataRoot}
        onSelectModel={(id, name) => {
          utility.setLlm(id);
          setToast(`Modèle local sélectionné : ${name}`);
        }}
        communityUpdate={communityUpdate}
        onToggleCommunityUpdate={() => setCommunityUpdate((v) => !v)}
        density={density}
        onDensityChange={setDensity}
        onUnwired={instance.unwired}
        loadoutId={session.activeLoadout?.id ?? null}
        communityMenu={nav.communityMenu}
        currentTab={nav.currentTab}
        activeGame={session.activeGame}
        gameInstances={session.gameInstances}
        activeLoadout={session.activeLoadout}
        filtered={mods.filtered}
        entries={session.entries}
        selectedMod={mods.selectedMod}
        onSelectMod={utility.openMod}
        onToggle={mods.toggle}
        onReorder={mods.reorder}
        conflicts={mods.conflictByMod}
        dependencies={mods.dependencyByMod}
        onImportSource={instance.startImport}
        onInstallArchived={mods.installArchived}
        installingIds={mods.installingIds}
        downloadProgress={downloadProgressByMod}
        catalog={session.catalog}
        otherLoadouts={session.otherLoadouts}
        onReassign={mods.reassignModTo}
        onRenameMod={mods.renameModRow}
        onDeleteMod={mods.deleteModRow}
        onUninstallMod={mods.uninstallModRow}
        onRevealFolder={mods.revealModFolder}
        onRevealArchive={mods.revealModArchive}
        onOpenPage={(url) => utility.openWeb(url)}
        onChatMod={utility.tagModInChat}
        browseSites={allBrowseSitesFor(session.activeGame?.id)}
        onOpenSite={(url) => utility.openWeb(url)}
        onOpenInstanceFolder={instance.revealActiveInstanceFolder}
      />
    </LayoutShell>
  );
}