import type { DetectedGame, LoadoutDto } from "../../../api";
import { Ico } from "../../../Ico";
import { Accordion, FavoriteButton, IconButton } from "../../ui";

export type FavoriteEntry = { loadout: LoadoutDto; short: string };

/**
 * The explorer's three sections: starred instances, the games tree with their
 * instances, and the categories. Everything listed here comes from the Core's
 * snapshot — the stars are the one exception, and they live in the browser.
 */
export function ExplorerTree({
  favorites,
  games,
  loadouts,
  expanded,
  activeGameId,
  activeLoadoutId,
  entryCount,
  isFavorite,
  onToggleFavorite,
  onPickLoadout,
  onPickGame,
  onToggleExpanded,
  onAddGame,
}: {
  favorites: FavoriteEntry[];
  games: DetectedGame[];
  loadouts: LoadoutDto[];
  expanded: string[];
  activeGameId: string | undefined;
  activeLoadoutId: number | undefined;
  entryCount: number;
  isFavorite: (loadoutId: number) => boolean;
  onToggleFavorite: (loadoutId: number) => void;
  onPickLoadout: (loadoutId: number) => void;
  onPickGame: (gameId: string) => void;
  onToggleExpanded: (gameId: string) => void;
  onAddGame: () => void;
}) {
  return (
    <>
      <Accordion title="Favoris" count={favorites.length}>
        {favorites.length === 0 ? (
          <div className="tr muted" style={{ fontStyle: "italic" }}>
            Aucun favori — clique l’étoile d’une instance
          </div>
        ) : (
          favorites.map(({ loadout: l, short }) => (
            <div
              key={l.id}
              className={`tr-row inst-row ${l.id === activeLoadoutId ? "cur" : ""}`}
            >
              <button
                className="tr-label"
                type="button"
                onClick={() => onPickLoadout(l.id)}
              >
                <Ico name="branch" />
                <span className="lbl">{l.name}</span>
                <span className="tail">
                  <span className="muted" style={{ fontSize: 10 }}>
                    {short}
                  </span>
                </span>
              </button>
              <FavoriteButton on onToggle={() => onToggleFavorite(l.id)} />
            </div>
          ))
        )}
      </Accordion>
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
                <div className={`tr-row ${g.id === activeGameId ? "sel" : ""}`}>
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
                {open
                  ? loadouts
                      .filter((l) => l.gameId === g.id)
                      .map((l) => (
                        <div
                          key={l.id}
                          className={`tr-row inst-row ${l.id === activeLoadoutId ? "cur" : ""}`}
                        >
                          <button
                            className="tr-label"
                            type="button"
                            onClick={() => onPickLoadout(l.id)}
                          >
                            <Ico name="branch" />
                            <span className="lbl">{l.name}</span>
                            <span className="tail">
                              <span className="mono">r.—</span>
                            </span>
                          </button>
                          <FavoriteButton
                            on={isFavorite(l.id)}
                            onToggle={() => onToggleFavorite(l.id)}
                          />
                        </div>
                      ))
                  : null}
              </div>
            );
          })
        )}
      </Accordion>
      <Accordion title="Catégories" count={1}>
        <button className="tr" type="button">
          <Ico name="folder" />
          <span className="lbl">Mods</span>
          <span className="tail">{entryCount}</span>
        </button>
      </Accordion>
    </>
  );
}
