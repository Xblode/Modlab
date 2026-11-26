import { type RefObject } from "react";
import { Ico } from "../../../Ico";
import type { BrowseSite } from "../../../gameChrome";
import {
  FavoriteButton,
  PopoverMenu,
  PopoverMenuItem,
  PopoverMenuSep,
} from "../../ui";

export type BrowseSiteMenuMode = "add-mod" | "open-site";

type Props = {
  mode: BrowseSiteMenuMode;
  sites: readonly BrowseSite[];
  favoriteIds: readonly string[];
  onToggleFavorite: (siteId: string) => void;
  onOpenSite: (url: string) => void;
  onClose: () => void;
  /** Local file import — only shown in `add-mod` mode when provided. */
  onImport?: () => void;
  anchorRef?: RefObject<HTMLElement | null>;
  anchorPoint?: { x: number; y: number };
};

/**
 * Intermediate step before opening the embedded Web tab: pick a mod site
 * (favourites first) or, for « Ajouter un mod », import a local source.
 */
export function BrowseSiteMenu({
  mode,
  sites,
  favoriteIds,
  onToggleFavorite,
  onOpenSite,
  onClose,
  onImport,
  anchorRef,
  anchorPoint,
}: Props) {
  const title = mode === "add-mod" ? "Ajouter un mod" : "Ouvrir un site";
  const favSet = new Set(favoriteIds);

  return (
    <PopoverMenu title={title} anchorRef={anchorRef} anchorPoint={anchorPoint} onClose={onClose}>
      {sites.length === 0 ? (
        <div className="popover-menu-empty">Aucun site pour ce jeu</div>
      ) : (
        sites.map((site) => {
          const fav = favSet.has(site.id);
          return (
            <div key={site.id} className="browse-site-row" role="none">
              <button
                type="button"
                role="menuitem"
                className="browse-site-row-pick"
                title={`Ouvrir ${site.label}`}
                onClick={() => {
                  onOpenSite(site.url);
                  onClose();
                }}
              >
                <span className="popover-menu-item-lead" aria-hidden="true">
                  <Ico name="globe" />
                </span>
                <span className="popover-menu-item-label">{site.label}</span>
              </button>
              <FavoriteButton on={fav} onToggle={() => onToggleFavorite(site.id)} />
            </div>
          );
        })
      )}
      {mode === "add-mod" && onImport ? (
        <>
          {sites.length > 0 ? <PopoverMenuSep /> : null}
          <PopoverMenuItem
            icon="download"
            label="Importer…"
            sub="Fichier ou dossier (Manuel)"
            onPick={() => {
              onImport();
              onClose();
            }}
          />
        </>
      ) : null}
    </PopoverMenu>
  );
}
