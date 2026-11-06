import { useState, type MouseEvent } from "react";
import type { Channel, DetectedGame, LoadoutDto } from "../../../api";
import { categoryIcon } from "../../../categoryIcon";
import { Ico } from "../../../Ico";
import { Accordion, ChannelBadge, ConfirmDialog, FavoriteButton, IconButton } from "../../ui";
import { MenuPicker, type MenuItem } from "../MenuPicker/MenuPicker";

export type FavoriteEntry = { loadout: LoadoutDto; short: string };

type CtxTarget = { kind: "game"; gameId: string } | { kind: "loadout"; loadoutId: number };

const LOADOUT_MENU_ITEMS: MenuItem[] = [
  { id: "activate", label: "Activer", icon: "branch" },
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

const GAME_MENU_ITEMS: MenuItem[] = [
  { id: "create", label: "Créer une instance", icon: "plus" },
  { id: "settings", label: "Réglages du jeu…", icon: "gear" },
];

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
  onContextMenu: (e: MouseEvent) => void;
}) {
  return (
    <div className={`tr-row inst-row ${current ? "cur" : ""}`} onContextMenu={onContextMenu}>
      {renaming ? (
        <div className="tr-label" style={{ cursor: "default" }}>
          <Ico name="branch" />
          <input
            className="tr-rename-input"
            autoFocus
            maxLength={100}
            value={draft}
            onChange={(e) => onDraftChange(e.target.value)}
            onFocus={(e) => e.currentTarget.select()}
            onKeyDown={(e) => {
              e.stopPropagation();
              if (e.key === "Enter") onCommitRename();
              if (e.key === "Escape") onCancelRename();
            }}
            onBlur={onCommitRename}
          />
        </div>
      ) : (
        <button className="tr-label" type="button" onClick={onPick}>
          <Ico name="branch" />
          <span className="lbl">{l.name}</span>
          <span className="tail">
            {short ? (
              <span className="muted" style={{ fontSize: 10 }}>
                {short}
              </span>
            ) : null}
            <ChannelBadge channel={l.channel} className="xs">
              {l.channel}
            </ChannelBadge>
          </span>
        </button>
      )}
      <FavoriteButton on={fav} onToggle={onToggleFav} />
    </div>
  );
}

/**
 * The explorer's three sections: starred instances, the games tree with their
 * instances, and the categories. Everything listed here comes from the Core's
 * snapshot — the stars are the one exception, and they live in the browser.
 *
 * Right-clicking a game or an instance opens a `MenuPicker` context menu
 * (edit/delete-style actions); the mutations themselves are routed back up
 * through callback props, same rule as every click in this tree.
 */
export function ExplorerTree({
  favorites,
  games,
  loadouts,
  expanded,
  activeGameId,
  activeLoadoutId,
  categoryCounts,
  isFavorite,
  onToggleFavorite,
  onPickLoadout,
  onPickGame,
  onToggleExpanded,
  onAddGame,
  onRenameLoadout,
  onSetLoadoutChannel,
  onDeleteLoadout,
  onCreateLoadoutForGame,
  onOpenGameSettings,
  onPickCategory,
}: {
  favorites: FavoriteEntry[];
  games: DetectedGame[];
  loadouts: LoadoutDto[];
  expanded: string[];
  activeGameId: string | undefined;
  activeLoadoutId: number | undefined;
  /** Distinct categories in the active loadout, with how many mods carry
   * each — a free tag on `mod_`, not a first-class list a human can create
   * ahead of tagging a mod with it. */
  categoryCounts: { name: string; count: number }[];
  isFavorite: (loadoutId: number) => boolean;
  onToggleFavorite: (loadoutId: number) => void;
  onPickLoadout: (loadoutId: number) => void;
  onPickGame: (gameId: string) => void;
  onToggleExpanded: (gameId: string) => void;
  onAddGame: () => void;
  onRenameLoadout: (loadoutId: number, name: string) => void;
  onSetLoadoutChannel: (loadoutId: number, channel: Channel) => void;
  onDeleteLoadout: (loadoutId: number) => void;
  onCreateLoadoutForGame: (gameId: string) => void;
  onOpenGameSettings: () => void;
  /** Filters the Mods table down to this category and switches to it. */
  onPickCategory: (name: string) => void;
}) {
  const [ctx, setCtx] = useState<{ x: number; y: number; target: CtxTarget } | null>(null);
  const [renamingId, setRenamingId] = useState<number | null>(null);
  const [draft, setDraft] = useState("");
  const [confirmDelete, setConfirmDelete] = useState<LoadoutDto | null>(null);

  function openLoadoutMenu(e: MouseEvent, loadoutId: number) {
    e.preventDefault();
    setCtx({ x: e.clientX, y: e.clientY, target: { kind: "loadout", loadoutId } });
  }

  function openGameMenu(e: MouseEvent, gameId: string) {
    e.preventDefault();
    setCtx({ x: e.clientX, y: e.clientY, target: { kind: "game", gameId } });
  }

  function startRename(l: LoadoutDto) {
    setDraft(l.name);
    setRenamingId(l.id);
  }

  function commitRename() {
    const id = renamingId;
    setRenamingId(null);
    if (id === null) return;
    const trimmed = draft.trim();
    const current = loadouts.find((l) => l.id === id)?.name;
    if (trimmed && trimmed !== current) onRenameLoadout(id, trimmed);
  }

  function handleMenuSelect(item: MenuItem) {
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
      onSetLoadoutChannel(l.id, item.id.slice("channel-".length) as Channel);
    } else if (item.id === "fav") onToggleFavorite(l.id);
    else if (item.id === "delete") setConfirmDelete(l);
  }

  function ctxMenuItems(target: CtxTarget): MenuItem[] {
    if (target.kind === "game") return GAME_MENU_ITEMS;
    const fav = isFavorite(target.loadoutId);
    return LOADOUT_MENU_ITEMS.map((it) =>
      it.id === "fav" ? { ...it, label: fav ? "Retirer des favoris" : "Ajouter aux favoris" } : it,
    );
  }

  return (
    <>
      {favorites.length === 0 ? null : (
        <Accordion title="Favoris" count={favorites.length}>
          {favorites.map(({ loadout: l, short }) => (
            <LoadoutRow
              key={l.id}
              l={l}
              current={l.id === activeLoadoutId}
              short={short}
              fav={isFavorite(l.id)}
              renaming={renamingId === l.id}
              draft={draft}
              onDraftChange={setDraft}
              onCommitRename={commitRename}
              onCancelRename={() => setRenamingId(null)}
              onPick={() => onPickLoadout(l.id)}
              onToggleFav={() => onToggleFavorite(l.id)}
              onContextMenu={(e) => openLoadoutMenu(e, l.id)}
            />
          ))}
        </Accordion>
      )}
      <Accordion
        title="Jeux"
        count={games.length}
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
          <div className="tr muted" style={{ fontStyle: "italic" }}>
            Aucun jeu détecté
          </div>
        ) : (
          games.map((g) => {
            const open = expanded.includes(g.id);
            return (
              <div key={g.id}>
                <div
                  className={`tr-row ${g.id === activeGameId ? "sel" : ""}`}
                  onContextMenu={(e) => openGameMenu(e, g.id)}
                >
                  <button
                    className="tr-chev"
                    type="button"
                    title={open ? "Réduire" : "Déplier"}
                    onClick={(e) => {
                      e.stopPropagation();
                      onToggleExpanded(g.id);
                    }}
                  >
                    <Ico name={open ? "chev" : "chevr"} className="sm" />
                  </button>
                  <button
                    className="tr-label"
                    type="button"
                    onClick={() => onPickGame(g.id)}
                  >
                    <span className="lbl">{g.name}</span>
                  </button>
                </div>
                {open ? (
                  (() => {
                    const gameLoadouts = (loadouts ?? []).filter(
                      (l) => l.gameId === g.id,
                    );

                    return gameLoadouts.length === 0 ? (
                      <div
                        className="tr muted"
                        data-d="2"
                        style={{ fontStyle: "italic" }}
                      >
                        Instance sans branche — créer…
                      </div>
                    ) : (
                      gameLoadouts.map((l) => (
                        <LoadoutRow
                          key={l.id}
                          l={l}
                          current={l.id === activeLoadoutId}
                          fav={isFavorite(l.id)}
                          renaming={renamingId === l.id}
                          draft={draft}
                          onDraftChange={setDraft}
                          onCommitRename={commitRename}
                          onCancelRename={() => setRenamingId(null)}
                          onPick={() => onPickLoadout(l.id)}
                          onToggleFav={() => onToggleFavorite(l.id)}
                          onContextMenu={(e) => openLoadoutMenu(e, l.id)}
                        />
                      ))
                    );
                  })()
                ) : null}
              </div>
            );
          })
        )}
      </Accordion>

      <Accordion
        title="Catégories"
        count={(categoryCounts ?? []).length}
        defaultOpen={false}
      >
        {(categoryCounts ?? []).length === 0 ? (
          <div className="tr muted" style={{ fontStyle: "italic" }}>
            Aucune catégorie — tagge un mod depuis le tableau
          </div>
        ) : (
          (categoryCounts ?? []).map((c) => (
            <button
              key={c.name}
              className="tr"
              type="button"
              onClick={() => onPickCategory(c.name)}
            >
              <Ico name={categoryIcon(c.name)} />
              <span className="lbl">{c.name}</span>
              <span className="tail">{c.count}</span>
            </button>
          ))
        )}
      </Accordion>

      {ctx ? (
        <MenuPicker
          items={ctxMenuItems(ctx.target)}
          anchorPoint={{ x: ctx.x, y: ctx.y }}
          onClose={() => setCtx(null)}
          onItemSelect={handleMenuSelect}
        />
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