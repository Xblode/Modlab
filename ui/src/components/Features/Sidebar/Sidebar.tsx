import { useEffect, useRef, useState } from "react";
import type { Channel, DetectedGame, LoadoutDto } from "../../../api";
import { useDeletedLoadouts } from "../../../hooks/useDeletedLoadouts";
import { useTransactions } from "../../../hooks/useTransactions";
import { KIND_LABELS } from "../../../api";
import { Ico } from "../../../Ico";
import {
  Accordion,
  Button,
  IconButton,
  PopoverMenu,
  PopoverMenuEntries,
  SearchInput,
  SideEmpty,
  SideRow,
  type PopoverMenuEntry,
} from "../../ui";
import { CommunityMenu, type CommunityMenuId } from "../Community/CommunityMenu";
import type { SettingsMenu } from "../Settings/SettingsPane";
import { SideMenu } from "../Settings/SideMenu";
import { ActiveInstanceCard } from "./ActiveInstanceCard";
import type { LaunchNotice } from "../../../sessionNotice";
import { APP_MENU_TREE } from "./appMenu";
import { ExplorerTree, type FavoriteEntry } from "./ExplorerTree";
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
  onMenuAction: (item: PopoverMenuEntry) => void;

  activeGame: DetectedGame | undefined;
  activeLoadout: LoadoutDto | undefined;
  games: DetectedGame[];
  loadouts: LoadoutDto[];
  /** Branches of the instance on screen. Not the other instances. */
  branches: readonly LoadoutDto[];
  onPickBranch: (branchId: number) => void;
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
  onRenameLoadout: (name: string) => void;
  onSetLoadoutChannel: (channel: Channel) => void;
  /** Still accepted so the shell can pass them (Ctrl+S, the transactions
   * panel). The sidebar card no longer draws save or rollback. */
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
  /** The refused-launch message or last-session balance sheet, if any. */
  launchNotice?: LaunchNotice | null;
  onDismissLaunchNotice?: () => void;
  /** Distinct categories in the active loadout, with how many mods carry
   * each — see `ExplorerTree`'s own doc on why this isn't a real list. */
  categoryCounts: { name: string; count: number }[];
  /** The active game's categories, so a stored id shows its French label. */
  categoryCatalog: { id: string; label: string; icon: string }[];
  /** Catalog id of the active sidebar category filter, or null. */
  activeCategory: string | null;
  onPickCategory: (name: string) => void;
  showEmptyCategories: boolean;
  onToggleShowEmptyCategories: () => void;
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
  branches,
  onPickBranch,
  favorites,
  expanded,
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
  onRenameLoadout,
  onSetLoadoutChannel,
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
  launchNotice,
  onDismissLaunchNotice,
  categoryCounts,
  categoryCatalog,
  activeCategory,
  onPickCategory,
  showEmptyCategories,
  onToggleShowEmptyCategories,
}: Props) {
  const menuButtonRef = useRef<HTMLButtonElement>(null);
  // `running` is a single session flag. Keep the instance that was selected
  // when the session started, so choosing another row does not move the loader.
  const [runningLoadoutId, setRunningLoadoutId] = useState<number | undefined>(undefined);
  const runningLatched = useRef(false);
  useEffect(() => {
    if (!running) {
      runningLatched.current = false;
      setRunningLoadoutId(undefined);
      return;
    }
    if (!runningLatched.current && activeLoadout) {
      runningLatched.current = true;
      setRunningLoadoutId(activeLoadout.id);
    }
  }, [running, activeLoadout]);
  // A second, independent fetch of the same journal `TransactionsPanel`
  // shows — only loaded while the History activity is actually open, same
  // "recompute on demand, don't share a cache" rule `useConflicts` follows.
  const { txs: historyTxs } = useTransactions(act === "history");
  const { trash, refreshTrash } = useDeletedLoadouts(act === "history");
  const gameTrash = (trash ?? []).filter((l) => l.gameId === activeGame?.id);
  const branchRows = [...branches].sort((a, b) => a.id - b.id);
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
        <PopoverMenu
          chrome="context"
          anchorRef={menuButtonRef}
          onClose={onCloseMenu}
        >
          <PopoverMenuEntries
            items={APP_MENU_TREE}
            onSelect={(item) => {
              onMenuAction(item);
              onCloseMenu();
            }}
          />
        </PopoverMenu>
      ) : null}

      {act === "explorer" && activeLoadout ? (
        <ActiveInstanceCard
          name={activeLoadout.instanceName}
          channel={activeLoadout.channel}
          running={running}
          onPlay={onPlay}
          onShare={onShare}
          onRename={onRenameLoadout}
          onSetChannel={onSetLoadoutChannel}
          notice={launchNotice}
          onDismissNotice={onDismissLaunchNotice}
        />
      ) : act === "explorer" && activeGame ? (
        <div className="side-instance">
          <div className="lo">
            <div className="lo-top">
              <span className="lo-name muted">Aucune instance</span>
            </div>
            <Button variant="sm" onClick={onCreateLoadout}>
              <Ico name="plus" className="sm" />
              Créer une instance
            </Button>
          </div>
        </div>
      ) : null}

      <div className="side-body">
        <div className="side-body-scroll">
        {act === "explorer" ? (
          <ExplorerTree
            favorites={favorites}
            games={games}
            loadouts={loadouts}
            expanded={expanded}
            activeLoadoutId={activeLoadout?.id}
            activeInstanceId={activeLoadout?.instanceId}
            categoryCounts={categoryCounts}
            categoryCatalog={categoryCatalog}
            activeCategory={activeCategory}
            onPickCategory={onPickCategory}
            showEmptyCategories={showEmptyCategories}
            onToggleShowEmptyCategories={onToggleShowEmptyCategories}
            isFavorite={isFavorite}
            onToggleFavorite={onToggleFavorite}
            onPickLoadout={onPickLoadout}
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
            onOpenCategorySettings={() => {
              onSettingsMenu("categories");
              onAct("settings");
            }}
            runningLoadoutId={runningLoadoutId}
          />
        ) : act === "search" ? (
          <div style={{ padding: "8px 8px 6px" }}>
            <SearchInput
              compact
              value={query}
              onChange={onQueryChange}
              placeholder="Filtrer les mods"
            />
          </div>
        ) : act === "history" ? (
          <>
            {!activeLoadout ? (
              <SideEmpty title="Aucune instance">Choisis-en une dans l’Explorateur</SideEmpty>
            ) : (
              <Accordion title={activeLoadout.instanceName} count={branchRows.length}>
                {branchRows.length === 0 ? (
                  <SideEmpty title="Aucune branche">Cette instance n’en a pas</SideEmpty>
                ) : (
                  branchRows.map((branch) => (
                    <SideRow
                      key={branch.id}
                      className="nested"
                      icon="branch"
                      label={branch.name}
                      current={branch.id === activeLoadout.id}
                      onClick={() => {
                        if (branch.id !== activeLoadout.id) onPickBranch(branch.id);
                      }}
                    />
                  ))
                )}
              </Accordion>
            )}
            <Accordion
              title="Révisions"
              count={loadoutRevisions.length}
              action={
                <IconButton
                  name="history"
                  className="sec-act"
                  title="Panneau complet"
                  shortcut="Ctrl J"
                  onClick={onOpenTransactions}
                />
              }
            >
              {!activeLoadout ? (
                <SideEmpty title="Aucune instance">Choisis-en une dans l’Explorateur</SideEmpty>
              ) : historyTxs === null ? (
                <div className="tr muted">
                  Chargement…
                </div>
              ) : loadoutRevisions.length === 0 ? (
                <SideEmpty title="Aucune révision">Rien d’enregistré pour cette instance</SideEmpty>
              ) : (
                loadoutRevisions.map((t, i) => (
                  <button
                    className="tr"
                    type="button"
                    key={t.id}
                    style={{ height: "auto", paddingTop: 4, paddingBottom: 4 }}
                    onClick={onOpenTransactions}
                  >
                    <span className={`hd ${i === 0 ? "" : "idle"}`} />
                    <span className="lbl">
                      {KIND_LABELS[t.kind] ?? t.kind}
                      <div className="muted" style={{ fontSize: "var(--fs-xs)" }}>
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
                <div className="tr muted">
                  Chargement…
                </div>
              ) : gameTrash.length === 0 ? (
                <SideEmpty title="Corbeille vide">Les instances supprimées apparaissent ici</SideEmpty>
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
            <SideEmpty title="Aucun outil">Le Core n’a pas encore de registre d’outils</SideEmpty>
          </Accordion>
        ) : null}
        {act === "settings" ? (
          <SideMenu active={settingsMenu} onSelect={onSettingsMenu} onBack={onSettingsBack} />
        ) : null}
        {act === "community" ? (
          <CommunityMenu active={communityMenu} onSelect={onCommunityMenu} onBack={onCommunityBack} />
        ) : null}
        </div>
      </div>
      <SidebarProfile
        act={act}
        onAct={onAct}
        dataRoot={dataRoot}
        communityEnabled={communityEnabled}
      />
    </aside>
  );
}
