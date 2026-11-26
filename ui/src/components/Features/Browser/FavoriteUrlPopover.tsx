import { useMemo, useState, type RefObject } from "react";
import type { DetectedGame } from "../../../api";
import { Button, Checkbox, PopoverMenu } from "../../ui";

/**
 * Pick which game(s) should receive the current browser URL as a favourite
 * browse site (same list as « Ajouter un mod »).
 */
export function FavoriteUrlPopover({
  url,
  games,
  activeGameId,
  anchorRef,
  onConfirm,
  onClose,
}: {
  url: string;
  games: readonly DetectedGame[];
  activeGameId: string | null;
  anchorRef: RefObject<HTMLElement | null>;
  onConfirm: (gameIds: string[]) => void;
  onClose: () => void;
}) {
  const initial = useMemo(() => {
    const set = new Set<string>();
    if (activeGameId) set.add(activeGameId);
    else if (games[0]) set.add(games[0].id);
    return set;
  }, [activeGameId, games]);
  const [selected, setSelected] = useState<Set<string>>(initial);

  function toggle(id: string) {
    setSelected((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  }

  const canConfirm = selected.size > 0;

  return (
    <PopoverMenu
      title="Ajouter aux favoris"
      anchorRef={anchorRef}
      onClose={onClose}
      footer={
        <Button
          variant="action"
          disabled={!canConfirm}
          onClick={() => {
            if (!canConfirm) return;
            onConfirm([...selected]);
            onClose();
          }}
        >
          Confirmer
        </Button>
      }
    >
      <div className="browser-fav-url muted" title={url}>
        {url}
      </div>
      {games.length === 0 ? (
        <div className="popover-menu-empty">Aucun jeu détecté</div>
      ) : (
        games.map((game) => {
          const on = selected.has(game.id);
          return (
            <button
              key={game.id}
              type="button"
              role="menuitemcheckbox"
              aria-checked={on}
              className="popover-menu-item browser-fav-game"
              onClick={() => toggle(game.id)}
            >
              <span className="popover-menu-item-lead" aria-hidden="true">
                <Checkbox checked={on} onToggle={() => toggle(game.id)} stopPropagation />
              </span>
              <span className="popover-menu-item-label">{game.name}</span>
            </button>
          );
        })
      )}
    </PopoverMenu>
  );
}
