import { useRef, useState, type MouseEvent, type ReactNode } from "react";
import type { Channel, DetectedGame, LoadoutDto } from "../../../api";
import { presentCategory } from "../../../categoryIcon";
import { Ico } from "../../../Ico";
import {
  Accordion,
  ChannelDot,
  ConfirmDialog,
  FavoriteButton,
  IconButton,
  PixelLoader,
  PopoverMenu,
  PopoverMenuEntries,
  SideEmpty,
  SideRow,
  Switch,
  type PopoverMenuEntry,
} from "../../ui";
import "./sidebar-rows.css";

/** 24px line-start column. The glyph is centered; the label begins after it. */
function RowMark({ children }: { children?: ReactNode }) {
  return (
    <span className="side-mark" aria-hidden="true">
      {children}
    </span>
  );
}

export type FavoriteEntry = { loadout: LoadoutDto; short: string };

type CtxTarget = { kind: "game"; gameId: string } | { kind: "loadout"; loadoutId: number };

const LOADOUT_MENU_ITEMS: PopoverMenuEntry[] = [
  { id: "activate", label: "Activer", icon: "check" },
  { id: "rename", label: "Renommer", icon: "edit" },
  {
    id: "channel",
    label: "Canal",
    icon: "layers",
    children: [
      { id: "channel-stable", label: "Stable", icon: "shield" },
      { id: "channel-beta", label: "Beta", icon: "zap" },
      { id: "channel-alpha", label: "Alpha", icon: "tri" },
    ],
  },
  { id: "fav", label: "Favoris", icon: "star" },
  { id: "delete", label: "Supprimer…", icon: "trash", danger: true },
];

const GAME_MENU_ITEMS: PopoverMenuEntry[] = [
  { id: "create", label: "Créer une instance", icon: "plus" },
  { id: "settings", label: "Réglages du jeu…", icon: "gear" },
];

/** Folder glyph that turns into a disclosure chevron while the pointer is on it. */
function GameFolderMark({ open }: { open: boolean }) {
  return (
    <span className="game-folder side-mark" aria-hidden="true">
      <Ico name="folder" className="game-folder-ico" />
      <Ico name={open ? "chev" : "chevr"} className="game-folder-chev" />
    </span>
  );
}

/**
 * One instance row, in either the Favoris or the per-game section. A plain
 * top-level component (not defined inside `ExplorerTree`) on purpose: an
 * inline component redefined on every parent render would remount every row
 * on each keystroke of the rename input and drop focus after one character.
 */
function LoadoutRow({
  l,
  current,
  short,
  fav,
  renaming,
  draft,
  onDraftChange,
  onCommitRename,
  onCancelRename,
  onPick,
  onToggleFav,
  onContextMenu,
  running,
}: {
  l: LoadoutDto;
  current: boolean;
  short?: string;
  fav: boolean;
  renaming: boolean;
  draft: string;
  onDraftChange: (value: string) => void;
  onCommitRename: () => void;
  onCancelRename: () => void;
  onPick: () => void;
  onToggleFav: () => void;
  onContextMenu: (e: MouseEvent<HTMLElement>) => void;
  /** This instance is the one whose game session is running. */
  running: boolean;
}) {
  return (
    <SideRow
      className="explorer-line"
      current={current}
      title={running ? "Jeu en cours" : undefined}
      icon={
        <RowMark>
          {running ? <PixelLoader /> : <ChannelDot channel={l.channel} />}
        </RowMark>
      }
      label={
        renaming ? (
          <input
            className="tr-rename-input"
            autoFocus
            maxLength={100}
            value={draft}
            onChange={(e) => onDraftChange(e.target.value)}
            onFocus={(e) => e.currentTarget.select()}
            onKeyDown={(e) => {
              e.stopPropagation();
              if (e.key === "Enter") {
                e.preventDefault();
                onCommitRename();
              }
              if (e.key === "Escape") {
                e.preventDefault();
                onCancelRename();
              }
            }}
            onBlur={onCommitRename}
          />
        ) : (
          l.name
        )
      }
      end={!renaming && short ? <span className="side-row-meta">{short}</span> : undefined}
      trailing={
        <span className="instance-tail">
          <FavoriteButton on={fav} onToggle={onToggleFav} />
        </span>
      }
      onClick={renaming ? undefined : onPick}
      onContextMenu={onContextMenu}
    />
  );
}

/**
 * The explorer's three sections: categories in use, starred instances, and
 * the games tree with their instances. Branches are not listed here — they
 * live in the Mods toolbar. Everything listed comes from the Core's snapshot;
 * the stars are the one exception, and they live in the browser.
 *
 * A game name opens or closes its folder. An instance name calls
 * `onPickLoadout`. Right-clicking a game or an instance opens a `PopoverMenu`;
 * the mutations themselves are routed back up through callback props.
 */
export function ExplorerTree({
  favorites,
  games,
  loadouts,
  expanded,
  activeLoadoutId,
  activeInstanceId,
  categoryCounts,
  categoryCatalog,
  isFavorite,
  onToggleFavorite,
  onPickLoadout,
  onToggleExpanded,
  onAddGame,
  onRenameLoadout,
  onSetLoadoutChannel,
  onDeleteLoadout,
  onCreateLoadoutForGame,
  onOpenGameSettings,
  onOpenCategorySettings,
  activeCategory,
  onPickCategory,
  showEmptyCategories,
  onToggleShowEmptyCategories,
  runningLoadoutId,
}: {
  favorites: FavoriteEntry[];
  games: DetectedGame[];
  loadouts: LoadoutDto[];
  expanded: string[];
  activeLoadoutId: number | undefined;
  /** Instance that owns the branch on screen. Rows highlight on this, not on
   * a branch id — a second branch must not add a second sidebar row. */
  activeInstanceId: number | undefined;
  /** Distinct categories in the active loadout, with how many mods carry
   * each — a free tag on `mod_`, not a first-class list a human can create
   * ahead of tagging a mod with it. */
  categoryCounts: { name: string; count: number }[];
  categoryCatalog: { id: string; label: string; icon: string }[];
  isFavorite: (loadoutId: number) => boolean;
  onToggleFavorite: (loadoutId: number) => void;
  onPickLoadout: (loadoutId: number) => void;
  onToggleExpanded: (gameId: string) => void;
  onAddGame: () => void;
  onRenameLoadout: (loadoutId: number, name: string) => void;
  onSetLoadoutChannel: (loadoutId: number, channel: Channel) => void;
  onDeleteLoadout: (loadoutId: number) => void;
  onCreateLoadoutForGame: (gameId: string) => void;
  onOpenGameSettings: () => void;
  onOpenCategorySettings: () => void;
  /** Catalog id of the active category filter, or null when none. */
  activeCategory: string | null;
  /** Toggle category filter (same id clears) and switch to Mods. */
  onPickCategory: (name: string) => void;
  showEmptyCategories: boolean;
  onToggleShowEmptyCategories: () => void;
  /** The instance whose game is running, if any. Only that row swaps its
   * channel dot for the pixel loader; every other instance keeps the dot. */
  runningLoadoutId: number | undefined;
}) {
  const [ctx, setCtx] = useState<{ x: number; y: number; target: CtxTarget } | null>(null);
  const [renamingId, setRenamingId] = useState<number | null>(null);
  const [draft, setDraft] = useState("");
  const [confirmDelete, setConfirmDelete] = useState<LoadoutDto | null>(null);
  const skipRename = useRef(false);

  function openLoadoutMenu(e: MouseEvent<HTMLElement>, loadoutId: number) {
    e.preventDefault();
    setCtx({ x: e.clientX, y: e.clientY, target: { kind: "loadout", loadoutId } });
  }

  function openGameMenu(e: MouseEvent<HTMLElement>, gameId: string) {
    e.preventDefault();
    setCtx({ x: e.clientX, y: e.clientY, target: { kind: "game", gameId } });
  }

  function startRename(l: LoadoutDto) {
    skipRename.current = false;
    setDraft(l.name);
    setRenamingId(l.id);
  }

  function commitRename() {
    if (skipRename.current) {
      skipRename.current = false;
      return;
    }
    skipRename.current = true;
    const id = renamingId;
    setRenamingId(null);
    if (id === null) return;
    const trimmed = draft.trim();
    const current = loadouts.find((l) => l.id === id)?.name;
    if (trimmed && trimmed !== current) onRenameLoadout(id, trimmed);
  }

  function cancelRename() {
    skipRename.current = true;
    setRenamingId(null);
  }

  function handleMenuSelect(item: PopoverMenuEntry) {
    if (!ctx) return;
    const { target } = ctx;
    if (target.kind === "game") {
      if (item.id === "create") onCreateLoadoutForGame(target.gameId);
      else if (item.id === "settings") onOpenGameSettings();
      return;
    }
    const l = loadouts.find((x) => x.id === target.loadoutId);
    if (!l) return;
    if (item.id === "activate") onPickLoadout(l.id);
    else if (item.id === "rename") startRename(l);
    else if (item.id.startsWith("channel-")) {
      const targetId =
        l.instanceId === activeInstanceId && activeLoadoutId != null
          ? activeLoadoutId
          : l.id;
      onSetLoadoutChannel(targetId, item.id.slice("channel-".length) as Channel);
    } else if (item.id === "fav") onToggleFavorite(l.id);
    else if (item.id === "delete") setConfirmDelete(l);
  }

  function ctxMenuItems(target: CtxTarget): PopoverMenuEntry[] {
    if (target.kind === "game") return GAME_MENU_ITEMS;
    const fav = isFavorite(target.loadoutId);
    return LOADOUT_MENU_ITEMS.map((it) =>
      it.id === "fav" ? { ...it, label: fav ? "Retirer des favoris" : "Ajouter aux favoris" } : it,
    );
  }

  return (
    <>
      <Accordion
        title="Catégories"
        count={(categoryCounts ?? []).length}
        collapsible={false}
        action={
          <>
            <IconButton
              name="more"
              className="sec-act"
              title="Configurer les catégories"
              aria-label="Configurer les catégories"
              onClick={onOpenCategorySettings}
            />
            <Switch
              className="sec-act-cat"
              checked={showEmptyCategories}
              onToggle={onToggleShowEmptyCategories}
              title="Afficher les catégories vides"
              aria-label="Afficher les catégories vides"
            />
          </>
        }
      >
        {(categoryCounts ?? []).length === 0 ? (
          <SideEmpty title="Aucune catégorie">Choisis-en une sur la fiche du mod</SideEmpty>
        ) : (
          (categoryCounts ?? []).map((c) => {
            const shown = presentCategory(c.name, categoryCatalog ?? []);
            const active = activeCategory === c.name;
            return (
              <button
                key={c.name}
                className={`tr explorer-line${active ? " sel" : ""}`}
                type="button"
                aria-pressed={active}
                title={active ? "Retirer le filtre catégorie" : `Filtrer : ${shown.label}`}
                onClick={() => onPickCategory(c.name)}
              >
                <RowMark>
                  <Ico name={shown.icon ?? "box"} />
                </RowMark>
                <span className="lbl">{shown.label}</span>
                <span className="tail">{c.count}</span>
              </button>
            );
          })
        )}
      </Accordion>

      <Accordion title="Favoris" count={favorites.length}>
        {favorites.map(({ loadout: l, short }) => (
          <LoadoutRow
            key={l.id}
            l={l}
            current={l.instanceId === activeInstanceId}
            short={short}
            fav={isFavorite(l.id)}
            renaming={renamingId === l.id}
            draft={draft}
            onDraftChange={setDraft}
            onCommitRename={commitRename}
            onCancelRename={cancelRename}
            onPick={() => onPickLoadout(l.id)}
            onToggleFav={() => onToggleFavorite(l.id)}
            onContextMenu={(e) => openLoadoutMenu(e, l.id)}
            running={l.id === runningLoadoutId}
          />
        ))}
      </Accordion>
      <Accordion
        title="Jeux"
        count={games.length}
        defaultOpen
        action={
          <IconButton
            name="plus"
            className="sec-act"
            title="Ajouter un jeu"
            onClick={onAddGame}
          />
        }
      >
        {games.length === 0 ? (
          <SideEmpty title="Aucun jeu détecté">Aucune installation trouvée sur cette machine</SideEmpty>
        ) : (
          games.map((g) => {
            const open = expanded.includes(g.id);
            const gameLoadouts = (loadouts ?? []).filter((l) => l.gameId === g.id);
            return (
              <div key={g.id}>
                <SideRow
                  className="explorer-line"
                  icon={<GameFolderMark open={open} />}
                  label={g.name}
                  expanded={open}
                  onClick={() => onToggleExpanded(g.id)}
                  onContextMenu={(e) => openGameMenu(e, g.id)}
                  trailing={
                    <span className="side-row-trailing">
                      <IconButton
                        name="plus"
                        title="Créer une instance"
                        onClick={() => onCreateLoadoutForGame(g.id)}
                      />
                    </span>
                  }
                />
                {open ? (
                  gameLoadouts.length === 0 ? (
                    <button
                      className="tr muted instance-empty explorer-line"
                      type="button"
                      onClick={() => onCreateLoadoutForGame(g.id)}
                    >
                      <RowMark />
                      Aucune instance — créer
                    </button>
                  ) : (
                    gameLoadouts.map((l) => (
                      <LoadoutRow
                        key={l.id}
                        l={l}
                        current={l.instanceId === activeInstanceId}
                        fav={isFavorite(l.id)}
                        renaming={renamingId === l.id}
                        draft={draft}
                        onDraftChange={setDraft}
                        onCommitRename={commitRename}
                        onCancelRename={cancelRename}
                        onPick={() => onPickLoadout(l.id)}
                        onToggleFav={() => onToggleFavorite(l.id)}
                        onContextMenu={(e) => openLoadoutMenu(e, l.id)}
                        running={l.id === runningLoadoutId}
                      />
                    ))
                  )
                ) : null}
              </div>
            );
          })
        )}
      </Accordion>

      {ctx ? (
        <PopoverMenu
          chrome="context"
          anchorPoint={{ x: ctx.x, y: ctx.y }}
          onClose={() => setCtx(null)}
        >
          <PopoverMenuEntries
            items={ctxMenuItems(ctx.target)}
            onSelect={(item) => {
              handleMenuSelect(item);
              setCtx(null);
            }}
          />
        </PopoverMenu>
      ) : null}

      {confirmDelete ? (
        <ConfirmDialog
          title="Supprimer l’instance"
          body={`« ${confirmDelete.name} » sera retirée de la liste. Ses mods et son historique restent en base — il n'y a juste pas encore d'écran pour la restaurer.`}
          confirmLabel="Supprimer"
          danger
          onConfirm={() => onDeleteLoadout(confirmDelete.id)}
          onClose={() => setConfirmDelete(null)}
        />
      ) : null}
    </>
  );
}
