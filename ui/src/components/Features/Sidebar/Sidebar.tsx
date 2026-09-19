import { useRef } from "react";
import type { DetectedGame, LoadoutDto } from "../../../api";
import { Ico } from "../../../Ico";
import { Accordion, ChannelBadge, SearchInput } from "../../ui";
import { MenuPicker, type MenuItem, type MenuVariant } from "../MenuPicker/MenuPicker";
import type { SettingsMenu } from "../Settings/SettingsPane";
import { ActiveInstanceCard } from "./ActiveInstanceCard";
import { ExplorerTree, type FavoriteEntry } from "./ExplorerTree";
import { SidebarFooter } from "./SidebarFooter";
import { SidebarRail } from "./SidebarRail";

export type SideAct = "explorer" | "search" | "history" | "tools" | "settings";

const SETTINGS_ENTRIES = [
  ["general", "Général", "sliders"],
  ["notifications", "Notifications", "alert"],
  ["models", "Modèles", "box"],
  ["games", "Jeux", "layers"],
] as const;

type Props = {
  act: SideAct;
  onAct: (act: SideAct) => void;

  menuOpen: boolean;
  onToggleMenu: () => void;
  onCloseMenu: () => void;
  menuVariant: MenuVariant;
  onMenuVariantChange: (variant: MenuVariant) => void;
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
  menuVariant,
  onMenuVariantChange,
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
          variant={menuVariant}
          onClose={onCloseMenu}
          onVariantChange={onMenuVariantChange}
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
          <Accordion title="Menu" count={4}>
            {SETTINGS_ENTRIES.map(([id, label, icon]) => (
              <button
                key={id}
                className={`tr ${settingsMenu === id ? "sel" : ""}`}
                type="button"
                onClick={() => onSettingsMenu(id)}
              >
                <Ico name={icon} />
                <span className="lbl">{label}</span>
              </button>
            ))}
          </Accordion>
        ) : null}
      </div>
      {act === "settings" ? (
        <div className="side-profile">
          <div className="lo">
            <div className="lo-top">
              <span className="lo-name">
                <span className="fade">Profils info</span>
              </span>
              <ChannelBadge channel="stable">local</ChannelBadge>
            </div>
            <div className="lo-row muted" style={{ fontSize: 11.5, padding: "0 2px" }}>
              Racine : {dataRoot ?? "—"}
            </div>
          </div>
        </div>
      ) : null}
      <SidebarFooter
        running={running}
        gameShort={activeGame?.short}
        loadoutName={activeLoadout?.name}
        deploymentLabel={activeGame?.capabilities.deploymentLabel}
      />
    </aside>
  );
}
