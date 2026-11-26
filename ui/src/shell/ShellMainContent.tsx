import type { ReactNode } from "react";
import type {
  DetectedGame,
  GameCategory,
  LoadoutDto,
  ModConflicts,
  ModRow,
  ResolvedDependency,
} from "../api";
import type { DownloadBytes } from "../downloadProgress";
import type { BrowseSite, TabId } from "../gameChrome";
import type { SideAct } from "../components/Features/Sidebar/Sidebar";
import type { CommunityMenuId } from "../components/Features/Community/CommunityMenu";
import type { SettingsMenu } from "../components/Features/Settings/SettingsPane";
import type { Density } from "../density";
import { SettingsPane } from "../components/Features/Settings/SettingsPane";
import { ProfilePage } from "../components/Features/Profile/ProfilePage";
import { PatchWorkshopPage } from "../components/Features/Community/PatchWorkshopPage";
import { MarketplacePage } from "../components/Features/Community/MarketplacePage";
import DashboardPage from "../app/dashboard/page";
import ModsPage from "../app/mods/page";
import PluginsPage from "../app/plugins/page";
import ArchivesPage from "../app/archives/page";
import RepertoirePage from "../app/repertoire/page";
import SavesPage from "../app/saves/page";
import SantePage from "../app/sante/page";
import PacksPage from "../app/packs/page";
import WorldsPage from "../app/worlds/page";
import ServersPage from "../app/servers/page";
import { StubTab } from "../StubTab";
import { LLM_MODELS } from "./shellConstants";

export type ShellMainContentProps = {
  act: SideAct;
  settingsMenu: SettingsMenu;
  games: DetectedGame[];
  llm: string;
  dataRoot: string | undefined;
  onSelectModel: (id: string, name: string) => void;
  communityUpdate: boolean;
  onToggleCommunityUpdate: () => void;
  density: Density;
  onDensityChange: (d: Density) => void;
  onUnwired: (label: string) => void;
  loadoutId: number | null;
  communityMenu: CommunityMenuId;
  currentTab: TabId;
  activeGame: DetectedGame | undefined;
  gameInstances: LoadoutDto[];
  activeLoadout: LoadoutDto | undefined;
  filtered: ModRow[];
  entries: ModRow[];
  selectedMod: number | null;
  onSelectMod: (id: number) => void;
  onToggle: (row: ModRow) => void;
  onReorder: (row: ModRow, to: number) => Promise<void>;
  conflicts: Map<number, ModConflicts> | null;
  dependencies: Map<number, ResolvedDependency[]> | null;
  onImportSource: () => void;
  onInstallArchived: (row: ModRow) => void;
  installingIds: ReadonlySet<number>;
  downloadProgress: ReadonlyMap<number, DownloadBytes>;
  catalog: GameCategory[];
  otherLoadouts: LoadoutDto[];
  onReassign: (row: ModRow, toLoadoutId: number) => void;
  onRenameMod: (row: ModRow, name: string) => void;
  onDeleteMod: (row: ModRow) => void;
  onUninstallMod: (row: ModRow) => void;
  onRevealFolder: (row: ModRow) => void;
  onRevealArchive: (row: ModRow) => void;
  onOpenPage: (url: string) => void;
  onChatMod: (row: ModRow) => void;
  browseSites: readonly BrowseSite[];
  onOpenSite: (url: string) => void;
  onOpenInstanceFolder: () => void;
};

function pageForTab(props: ShellMainContentProps): ReactNode {
  const {
    currentTab,
    filtered,
    entries,
    selectedMod,
    onSelectMod,
    onToggle,
    onReorder,
    activeGame,
    conflicts,
    dependencies,
    onImportSource,
    onInstallArchived,
    installingIds,
    downloadProgress,
    catalog,
    otherLoadouts,
    onReassign,
    onRenameMod,
    onDeleteMod,
    onUninstallMod,
    onRevealFolder,
    onRevealArchive,
    onOpenPage,
    onChatMod,
    activeLoadout,
    browseSites,
    onOpenSite,
    onOpenInstanceFolder,
  } = props;

  switch (currentTab) {
    case "mods":
      return (
        <ModsPage
          rows={filtered}
          entryCount={entries.length}
          selectedId={selectedMod}
          onSelect={onSelectMod}
          onToggle={(row) => void onToggle(row)}
          onReorder={onReorder}
          gameId={activeGame?.id ?? null}
          conflicts={conflicts}
          dependencies={dependencies}
          onImportSource={onImportSource}
          onInstallArchived={(row) => void onInstallArchived(row)}
          installingIds={installingIds}
          downloadProgress={downloadProgress}
          catalog={catalog}
          otherLoadouts={otherLoadouts}
          onReassign={(row, toLoadoutId) => void onReassign(row, toLoadoutId)}
          onRename={(row, name) => void onRenameMod(row, name)}
          onDelete={(row) => void onDeleteMod(row)}
          onUninstall={(row) => void onUninstallMod(row)}
          onRevealFolder={onRevealFolder}
          onRevealArchive={onRevealArchive}
          onOpenPage={onOpenPage}
          onChat={onChatMod}
          browseSites={browseSites}
          onOpenSite={onOpenSite}
          onOpenInstanceFolder={onOpenInstanceFolder}
          canOpenInstanceFolder={activeLoadout != null}
        />
      );
    case "plugins":
      return (
        <PluginsPage
          loadoutId={activeLoadout?.id ?? null}
          mods={entries}
          onToggle={(row) => void onToggle(row)}
          onReorder={onReorder}
        />
      );
    case "archives":
      return <ArchivesPage loadoutId={activeLoadout?.id ?? null} />;
    case "repertoire":
      return <RepertoirePage gameId={activeGame?.id ?? null} />;
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
      return null;
    default:
      return <StubTab tab={currentTab} />;
  }
}

export function ShellMainContent(props: ShellMainContentProps) {
  const {
    act,
    settingsMenu,
    games,
    llm,
    dataRoot,
    onSelectModel,
    communityUpdate,
    onToggleCommunityUpdate,
    density,
    onDensityChange,
    onUnwired,
    loadoutId,
    communityMenu,
    currentTab,
    activeGame,
    gameInstances,
    activeLoadout,
    catalog,
  } = props;

  if (act === "settings") {
    return (
      <SettingsPane
        menu={settingsMenu}
        games={games}
        activeGame={activeGame}
        categoryCatalog={catalog}
        llm={llm}
        llmModels={LLM_MODELS}
        dataRoot={dataRoot}
        onSelectModel={onSelectModel}
        communityUpdate={communityUpdate}
        onToggleCommunityUpdate={onToggleCommunityUpdate}
        density={density}
        onDensityChange={onDensityChange}
        onUnwired={onUnwired}
        loadoutId={loadoutId}
      />
    );
  }
  if (act === "profile") {
    return (
      <ProfilePage
        communityUpdate={communityUpdate}
        onToggleCommunityUpdate={onToggleCommunityUpdate}
      />
    );
  }
  if (act === "community") {
    return communityMenu === "marketplace" ? (
      <MarketplacePage onUnwired={onUnwired} />
    ) : (
      <PatchWorkshopPage onUnwired={onUnwired} />
    );
  }
  if (currentTab === "dashboard" && activeGame) {
    return (
      <DashboardPage
        game={activeGame}
        loadouts={gameInstances}
        loadoutId={activeLoadout?.gameId === activeGame.id ? activeLoadout.id : null}
        onUnwired={onUnwired}
      />
    );
  }
  return pageForTab(props);
}