import { useRef } from "react";
import type { Channel, DetectedGame, LoadoutDto } from "../../../api";
import { useDeletedLoadouts } from "../../../hooks/useDeletedLoadouts";
import { useLoadoutVersion } from "../../../hooks/useLoadoutVersion";
import { useTransactions } from "../../../hooks/useTransactions";
import { CHANNEL_LETTER } from "../../../channel";
import { KIND_LABELS } from "../../../api";
import { Ico } from "../../../Ico";
import { Accordion, ChannelBadge, IconButton, SearchInput } from "../../ui";
import { CommunityMenu, type CommunityMenuId } from "../Community/CommunityMenu";
import { MenuPicker, type MenuItem } from "../MenuPicker/MenuPicker";
import type { SettingsMenu } from "../Settings/SettingsPane";
import { SideMenu } from "../Settings/SideMenu";
import { ActiveInstanceCard, type EngineCap } from "./ActiveInstanceCard";
import { ExplorerTree, type FavoriteEntry } from "./ExplorerTree";
import { SidebarFooter } from "./SidebarFooter";
import { SidebarProfile } from "./SidebarProfile";
import { SidebarRail } from "./SidebarRail";

export type SideAct =
  | "explorer"
  | "search"
  | "history"
  | "tools"
  | "settings"
  /** The user's own profile — identity, not configuration, so it isn't a
   * Réglages subsection. See `ProfilePage`. */
  | "profile"
  /** Atelier de patchs / Boutique — the "Communauté" preview from
   * `docs/deep-research-report.md`. Also not configuration. */
  | "community";

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
  /** Bumped after anything that can move the active loadout's revision or
   * dirty state (a toggle, a snapshot, a rollback) — see `ActiveInstanceCard`. */
  versionTick: number;

  isFavorite: (loadoutId: number) => boolean;
  onToggleFavorite: (loadoutId: number) => void;
  onPickLoadout: (loadoutId: number) => void;
  onPickGame: (gameId: string) => void;
  onToggleExpanded: (gameId: string) => void;
  onAddGame: () => void;
  onPlay: () => void;
  onShare: () => void;
  onRenameLoadout: (name: string) => void;
  onSetLoadoutChannel: (channel: Channel) => void;
  onSnapshot: () => Promise<void>;
  onRollback: () => Promise<void>;
  onCreateLoadout: () => void;
  /** By-id variants for the explorer tree's context menu, which can act on
   * any instance — not just the active one `onRenameLoadout` above targets. */
  onRenameLoadoutById: (loadoutId: number, name: string) => void;
  onSetLoadoutChannelById: (loadoutId: number, channel: Channel) => void;
  onDeleteLoadout: (loadoutId: number) => void;
  onRestoreLoadout: (loadoutId: number, name: string) => Promise<void>;

  query: string;
  onQueryChange: (value: string) => void;

  onOpenTransactions: () => void;

  settingsMenu: SettingsMenu;
  onSettingsMenu: (menu: SettingsMenu) => void;
  onSettingsBack: () => void;
  communityMenu: CommunityMenuId;
  onCommunityMenu: (menu: CommunityMenuId) => void;
  onCommunityBack: () => void;
  /** Gates the "Communauté" icon in `SidebarProfile` — see `SettingsPane`'s
   * `"general"` branch, where the toggle actually lives. */
  communityEnabled: boolean;
  /** "Engine & Indicators" for the active instance — empty for a game with
   * no known engine mod to watch for yet. */
  engineCaps?: EngineCap[];
  /** Distinct categories in the active loadout, with how many mods carry
   * each — see `ExplorerTree`'s own doc on why this isn't a real list. */
  categoryCounts: { name: string; count: number }[];
  onPickCategory: (name: string) => void;
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
  versionTick,
  isFavorite,
  onToggleFavorite,
  onPickLoadout,
  onPickGame,
  onToggleExpanded,
  onAddGame,
  onPlay,
  onShare,
  onRenameLoadout,
  onSetLoadoutChannel,
  onSnapshot,
  onRollback,
  onCreateLoadout,
  onRenameLoadoutById,
  onSetLoadoutChannelById,
  onDeleteLoadout,
  onRestoreLoadout,
  query,
  onQueryChange,
  onOpenTransactions,
  settingsMenu,
  onSettingsMenu,
  onSettingsBack,
  communityMenu,
  onCommunityMenu,
  onCommunityBack,
  communityEnabled,
  engineCaps,
  categoryCounts,
  onPickCategory,
}: Props) {
  const menuButtonRef = useRef<HTMLButtonElement>(null);
  const { version, refresh: refreshVersion } = useLoadoutVersion(
    activeLoadout?.id,
    `${versionTick}:${enabledCount}`,
  );
  // A second, independent fetch of the same journal `TransactionsPanel`
  // shows — only loaded while the History activity is actually open, same
  // "recompute on demand, don't share a cache" rule `useConflicts` follows.
  const { txs: historyTxs } = useTransactions(act === "history");
  const { trash, refreshTrash } = useDeletedLoadouts(act === "history");
  const gameLoadouts = loadouts.filter((l) => l.gameId === activeGame?.id);
  const gameTrash = (trash ?? []).filter((l) => l.gameId === activeGame?.id);
  const loadoutRevisions = (historyTxs ?? []).filter(
    (t) =>
      t.loadoutId === activeLoadout?.id &&
      (t.kind === "loadout.snapshot" || t.kind === "loadout.rollback"),
  );

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
          channel={activeLoadout.channel}
          running={running}
          enabledCount={enabledCount}
          totalCount={entryCount}
          version={version}
          onRefreshVersion={refreshVersion}
          onPlay={onPlay}
          onShare={onShare}
          onRename={onRenameLoadout}
          onSetChannel={onSetLoadoutChannel}
          onSnapshot={onSnapshot}
          onRollback={onRollback}
          onOpenHistory={onOpenTransactions}
          caps={engineCaps}
        />
      ) : act === "explorer" && activeGame ? (
        <div className="side-instance">
          <div className="lo">
            <div className="lo-top">
              <span className="lo-name muted">Instance sans branche</span>
            </div>
            <button className="btn sm" type="button" onClick={onCreateLoadout}>
              <Ico name="branch" className="sm" />
              Créer une branche
            </button>
          </div>
        </div>
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
            categoryCounts={categoryCounts}
            onPickCategory={onPickCategory}
            isFavorite={isFavorite}
            onToggleFavorite={onToggleFavorite}
            onPickLoadout={onPickLoadout}
            onPickGame={onPickGame}
            onToggleExpanded={onToggleExpanded}
            onAddGame={onAddGame}
            onRenameLoadout={onRenameLoadoutById}
            onSetLoadoutChannel={onSetLoadoutChannelById}
            onDeleteLoadout={onDeleteLoadout}
            onCreateLoadoutForGame={(gameId) => {
              onPickGame(gameId);
              onCreateLoadout();
            }}
            onOpenGameSettings={() => {
              onSettingsMenu("games");
              onAct("settings");
            }}
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
          <>
            <Accordion title="Branches" count={gameLoadouts.length}>
              {gameLoadouts.length === 0 ? (
                <div className="tr muted" style={{ fontStyle: "italic" }}>
                  Aucune branche pour ce jeu.
                </div>
              ) : (
                gameLoadouts.map((l) => (
                  <div
                    key={l.id}
                    className={`tr-row ${l.id === activeLoadout?.id ? "sel" : ""}`}
                  >
                    <button
                      className="tr-label"
                      type="button"
                      onClick={() => onPickLoadout(l.id)}
                    >
                      <Ico name="branch" />
                      <span className="lbl">{l.name}</span>
                      <span className="tail">
                        {l.id === activeLoadout?.id && version ? (
                          <span className="mono">
                            {CHANNEL_LETTER[l.channel]}.{version.revision}
                          </span>
                        ) : null}
                        <ChannelBadge channel={l.channel} className="xs">
                          {l.channel}
                        </ChannelBadge>
                      </span>
                    </button>
                  </div>
                ))
              )}
            </Accordion>
            <Accordion
              title="Révisions"
              count={loadoutRevisions.length}
              action={
                <IconButton
                  name="history"
                  className="sec-act"
                  title="Panneau complet (Ctrl+J)"
                  onClick={onOpenTransactions}
                />
              }
            >
              {!activeLoadout ? (
                <div className="tr muted" style={{ fontStyle: "italic" }}>
                  Sélectionne une instance.
                </div>
              ) : historyTxs === null ? (
                <div className="tr muted" style={{ fontStyle: "italic" }}>
                  Chargement…
                </div>
              ) : loadoutRevisions.length === 0 ? (
                // Not `.tr` here: that class hard-clips at one line with no
                // ellipsis, right for a mod name and wrong for a sentence
                // whose length depends on the instance's own name.
                <p
                  className="muted"
                  style={{
                    margin: "2px 8px",
                    fontSize: 12.5,
                    fontStyle: "italic",
                    whiteSpace: "normal",
                  }}
                >
                  Aucune révision enregistrée pour « {activeLoadout.name} ».
                </p>
              ) : (
                loadoutRevisions.map((t, i) => (
                  <button
                    className="tr"
                    type="button"
                    key={t.id}
                    style={{ height: "auto", paddingTop: 3, paddingBottom: 3 }}
                    onClick={onOpenTransactions}
                  >
                    <span className={`hd ${i === 0 ? "" : "idle"}`} />
                    <span className="lbl">
                      {KIND_LABELS[t.kind] ?? t.kind}
                      <div className="muted" style={{ fontSize: 11 }}>
                        {t.steps[0] ?? ""} ·{" "}
                        {new Date(t.occurredAt).toLocaleString("fr-FR")}
                      </div>
                    </span>
                  </button>
                ))
              )}
            </Accordion>
            <Accordion title="Corbeille" count={gameTrash.length}>
              {trash === null ? (
                <div className="tr muted" style={{ fontStyle: "italic" }}>
                  Chargement…
                </div>
              ) : gameTrash.length === 0 ? (
                <div className="tr muted" style={{ fontStyle: "italic" }}>
                  Rien à restaurer pour ce jeu.
                </div>
              ) : (
                gameTrash.map((l) => (
                  <div key={l.id} className="tr-row">
                    <span className="tr-label" style={{ cursor: "default" }}>
                      <Ico name="trash" />
                      <span className="lbl muted">{l.name}</span>
                    </span>
                    <IconButton
                      name="refresh"
                      className="sec-act"
                      title="Restaurer cette instance"
                      onClick={() => {
                        void onRestoreLoadout(l.id, l.name).then(refreshTrash);
                      }}
                    />
                  </div>
                ))
              )}
            </Accordion>
          </>
        ) : act === "tools" ? (
          <Accordion title="Outils" count={0} defaultOpen={false}>
            <div className="tr muted" style={{ fontStyle: "italic" }}>
              Pas de registre d’outils dans le Core pour l’instant.
            </div>
          </Accordion>
        ) : null}
        {act === "settings" ? (
          <SideMenu active={settingsMenu} onSelect={onSettingsMenu} onBack={onSettingsBack} />
        ) : null}
        {act === "community" ? (
          <CommunityMenu active={communityMenu} onSelect={onCommunityMenu} onBack={onCommunityBack} />
        ) : null}
      </div>
      <SidebarProfile
        act={act}
        onAct={onAct}
        dataRoot={dataRoot}
        communityEnabled={communityEnabled}
      />
      <SidebarFooter
        running={running}
        gameShort={activeGame?.short}
        loadoutName={activeLoadout?.name}
        channel={activeLoadout?.channel}
        revision={version?.revision}
        deploymentLabel={activeGame?.capabilities.deploymentLabel}
      />
    </aside>
  );
}
