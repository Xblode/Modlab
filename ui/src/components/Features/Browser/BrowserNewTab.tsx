import { useState, type FormEvent } from "react";
import type { BrowseSite } from "../../../gameChrome";
import { Ico } from "../../../Ico";
import { Button, Input } from "../../ui";
import {
  engineHomeUrl,
  resolveOmnibox,
  type WebHomepage,
} from "../../../webHomepage";

/**
 * Chrome-style new-tab surface for the embedded browser: search field,
 * shortcut tiles for the active game’s favourite sites, and a link to the
 * configured homepage. Dark ModLab shell styles — not a redesign.
 */
export function BrowserNewTab({
  homepage,
  favorites,
  onNavigate,
}: {
  homepage: WebHomepage;
  favorites: readonly BrowseSite[];
  onNavigate: (url: string) => void;
}) {
  const [query, setQuery] = useState("");

  function submit(e: FormEvent) {
    e.preventDefault();
    const next = resolveOmnibox(query, homepage);
    onNavigate(next);
  }

  return (
    <div className="browser-newtab">
      <div className="browser-newtab-inner">
        <div className="browser-newtab-brand">
          <Ico name="globe" />
          <span>Web</span>
        </div>
        <form className="browser-newtab-search" onSubmit={submit}>
          <Ico name="search" className="sm" />
          <Input
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder="Rechercher ou saisir une adresse"
            spellCheck={false}
            autoComplete="off"
            autoFocus
            aria-label="Recherche"
          />
        </form>
        {favorites.length > 0 ? (
          <div className="browser-newtab-shortcuts" role="list">
            {favorites.map((site) => (
              <button
                key={site.id}
                type="button"
                className="browser-newtab-tile"
                role="listitem"
                title={site.url}
                onClick={() => onNavigate(site.url)}
              >
                <span className="browser-newtab-tile-ico" aria-hidden="true">
                  <Ico name="globe" />
                </span>
                <span className="browser-newtab-tile-label">{site.label}</span>
              </button>
            ))}
          </div>
        ) : null}
        <div className="browser-newtab-home">
          <Button
            variant="ghost"
            onClick={() => onNavigate(engineHomeUrl(homepage))}
          >
            Page d’accueil
          </Button>
        </div>
      </div>
    </div>
  );
}
