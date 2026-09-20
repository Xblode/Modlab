import { useRef } from "react";
import type { DetectedGame, LoadoutDto } from "../../../api";
import { Ico } from "../../../Ico";
import { Accordion, SearchInput } from "../../ui";
import { MenuPicker, type MenuItem } from "../MenuPicker/MenuPicker";
import type { SettingsMenu } from "../Settings/SettingsPane";
import { SideMenu } from "../Settings/SideMenu";
import { ActiveInstanceCard } from "./ActiveInstanceCard";
import { ExplorerTree, type FavoriteEntry } from "./ExplorerTree";
import { SidebarFooter } from "./SidebarFooter";
import { SidebarProfile } from "./SidebarProfile";
import { SidebarRail } from "./SidebarRail";

export type SideAct = "explorer" | "search" | "history" | "tools" | "settings";

type Props = {
  act: SideAct;
  onAct: (act: SideAct) => void;

  menuOpen: boolean;
  onToggleMenu: () => void;
  onCloseMenu: () => void;
  onMenuAction: (item: MenuItem) => void;

  activeGame: DetectedGame | undefined;
  activeLoadout: LoadoutDto | undefined;
  games: DetectedGame[];
  loadouts: LoadoutDto[];
  favorites: FavoriteEntry[];
  expanded: string[];
  enabledCount: number;
  entryCount: number;
  running: boolean;
  dataRoot: string | undefined;

  isFavorite: (loadoutId: number) => boolean;
  onToggleFavorite: (loadoutId: number) => void;
  onPickLoadout: (loadoutId: number) => void;
  onPickGame: (gameId: string) => void;
  onToggleExpanded: (gameId: string) => void;
  onAddGame: () => void;
  onPlay: () => void;
  onShare: () => void;

  query: string;
  onQueryChange: (value: string) => void;

  /** `null` while the journal has not been loaded yet. */
  txCount: number | null;
  onOpenTransactions: () => void;

  settingsMenu: SettingsMenu;
  onSettingsMenu: (menu: SettingsMenu) => void;
  onSettingsBack: () => void;
};

/**
 * The left sidebar: the icon rail and whichever activity it has selected.
 * Every list it shows belongs to the Core's snapshot; this component only
 * routes clicks back up.
 */
export function Sidebar({
  act,
  onAct,
  menuOpen,
  onToggleMenu,
  onCloseMenu,
  onMenuAction,
  activeGame,
  activeLoadout,
  games,
  loadouts,
  favorites,
  expanded,
  enabledCount,
  entryCount,
  running,
  dataRoot,
  isFavorite,
  onToggleFavorite,
  onPickLoadout,
  onPickGame,
  onToggleExpanded,
  onAddGame,
  onPlay,
  onShare,
  query,
  onQueryChange,
  txCount,
  onOpenTransactions,
  settingsMenu,
  onSettingsMenu,
  onSettingsBack,
}: Props) {
  const menuButtonRef = useRef<HTMLButtonElement>(null);

  return (
    <aside className="side">
      <SidebarRail
        act={act}
        onAct={onAct}
        menuButtonRef={menuButtonRef}
        menuOpen={menuOpen}
        onToggleMenu={onToggleMenu}
      />

      {menuOpen ? (
        <MenuPicker
          anchorRef={menuButtonRef}
          onClose={onCloseMenu}
          onItemSelect={onMenuAction}
        />
      ) : null}

      {act === "explorer" && activeLoadout ? (
        <ActiveInstanceCard
          name={activeLoadout.name}
          running={running}
          enabledCount={enabledCount}
          totalCount={entryCount}
          onPlay={onPlay}
          onShare={onShare}
        />
      ) : null}

      <div className="side-body">
        {act === "explorer" ? (
          <ExplorerTree
            favorites={favorites}
            games={games}
            loadouts={loadouts}
            expanded={expanded}
            activeGameId={activeGame?.id}
            activeLoadoutId={activeLoadout?.id}
            entryCount={entryCount}
            isFavorite={isFavorite}
            onToggleFavorite={onToggleFavorite}
            onPickLoadout={onPickLoadout}
            onPickGame={onPickGame}
            onToggleExpanded={onToggleExpanded}
            onAddGame={onAddGame}
          />
        ) : act === "search" ? (
          <div style={{ padding: "8px 8px 6px" }}>
            <SearchInput
              compact
              value={query}
              onChange={onQueryChange}
              placeholder="Filtrer les mods…"
            />
          </div>
        ) : act === "history" ? (
          <Accordion title="Journal" count={txCount ?? 0}>
            <button className="tr" type="button" onClick={onOpenTransactions}>
              <Ico name="history" />
              <span className="lbl">Transactions</span>
              <span className="tail">{txCount ?? "—"}</span>
            </button>
            <div className="tr muted" style={{ fontStyle: "italic" }}>
              Panneau du bas : Ctrl+J.
            </div>
          </Accordion>
        ) : act === "tools" ? (
          <Accordion title="Outils" count={0}>
            <div className="tr muted" style={{ fontStyle: "italic" }}>
              Pas de registre d’outils dans le Core pour l’instant.
            </div>
          </Accordion>
        ) : null}
        {act === "settings" ? (
          <SideMenu active={settingsMenu} onSelect={onSettingsMenu} onBack={onSettingsBack} />
        ) : null}
      </div>
      <SidebarProfile act={act} onAct={onAct} dataRoot={dataRoot} />
      <SidebarFooter
        running={running}
        gameShort={activeGame?.short}
        loadoutName={activeLoadout?.name}
        deploymentLabel={activeGame?.capabilities.deploymentLabel}
      />
    </aside>
  );
}
