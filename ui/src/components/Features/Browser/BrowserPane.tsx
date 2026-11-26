import { useEffect, useLayoutEffect, useRef, useState, type FormEvent } from "react";
import {
  browserGo,
  browserHide,
  browserNavigate,
  browserShow,
  isTauriShell,
  onBrowserNavigated,
  onDownloadStarted,
  type DetectedGame,
} from "../../../api";
import type { BrowseSite } from "../../../gameChrome";
import { Ico } from "../../../Ico";
import { IconButton, Input, PixelLoader, tip } from "../../ui";
import {
  addBrowseUrlFavorite,
  allBrowseSitesFor,
  browseSiteFavoritesFor,
  webHomepageSetting,
} from "../../../uiMemory";
import {
  isNewTabUrl,
  NEW_TAB_URL,
  resolveOmnibox,
  type WebHomepage,
} from "../../../webHomepage";
import { BrowserNewTab } from "./BrowserNewTab";
import { FavoriteUrlPopover } from "./FavoriteUrlPopover";

/** True when two browse URLs are the same page (ignore trailing slash / query noise). */
function sameBrowseUrl(a: string, b: string): boolean {
  if (a === b) return true;
  try {
    const left = new URL(a);
    const right = new URL(b);
    const strip = (u: URL) => {
      const path = u.pathname.replace(/\/+$/, "") || "/";
      return `${u.protocol}//${u.host}${path}`;
    };
    return strip(left) === strip(right);
  } catch {
    return false;
  }
}

/** Origin favicon for a bookmark; falls back to a generic globe on error. */
function BookmarkIcon({ url }: { url: string }) {
  const [failed, setFailed] = useState(false);
  let href: string | null = null;
  try {
    href = `${new URL(url).origin}/favicon.ico`;
  } catch {
    href = null;
  }
  if (!href || failed) {
    return <Ico name="globe" className="sm" />;
  }
  return (
    <img
      src={href}
      alt=""
      width={14}
      height={14}
      decoding="async"
      onError={() => setFailed(true)}
    />
  );
}

/**
 * The embedded browser body (omnibox + page). Page tabs live in the drawer
 * strip (`RightPanel` / `.rhead-tabs`); this pane only shows the active page.
 *
 * Nothing of the page is rendered by React: the page is a *native* child
 * WebView2, a sibling of the shell's own webview, which the OS paints over
 * this component's hole. So this component's whole job is to be a
 * well-measured empty rectangle — it reports where the hole is, and the Rust
 * side moves the real browser there.
 *
 * Stays mounted while any Web page tab is open. The caller toggles `visible`
 * when another drawer tab is on screen. Closing the last Web tab unmounts it;
 * the caller hides the native webview first.
 *
 * Multiple logical pages share the single native WebView: switching a page
 * passes a new `url` / `pageId` (history is not isolated). A `modlab://new-tab`
 * page shows a React new-tab screen and hides the native webview.
 */
export function BrowserPane({
  pageId,
  url,
  onNavigate,
  gameId,
  games,
  visible,
  favoriteSiteIds,
  onFavoriteChange,
}: {
  /** Active Web page id (drawer tab `web:<pageId>`). */
  pageId: string;
  /** Current URL for that page (may be the new-tab sentinel). */
  url: string;
  /** Called when the user or the native webview changes the URL/title. */
  onNavigate: (url: string, title: string) => void;
  gameId: string | null;
  games: readonly DetectedGame[];
  visible: boolean;
  /**
   * Favourite browse-site ids from the shell (`browseSiteFavorites`). When
   * this changes (toolbar star, omnibox add), the bookmarks row refreshes.
   */
  favoriteSiteIds?: readonly string[];
  /** Called after starring a URL so the shell can refresh favourite lists. */
  onFavoriteChange?: () => void;
}) {
  const holeRef = useRef<HTMLDivElement>(null);
  const starRef = useRef<HTMLButtonElement>(null);
  const [address, setAddress] = useState(() => (isNewTabUrl(url) ? "" : url));
  const [failure, setFailure] = useState<string | null>(null);
  const [favOpen, setFavOpen] = useState(false);
  const [homepage, setHomepage] = useState<WebHomepage>(() => webHomepageSetting());
  const [favoriteSites, setFavoriteSites] = useState<BrowseSite[]>([]);
  const [page, setPage] = useState(url);
  const [loading, setLoading] = useState(false);
  const embedded = isTauriShell();
  const onNewTab = isNewTabUrl(page);
  const editingRef = useRef(false);
  const liveRef = useRef({ gameId });
  liveRef.current = { gameId };
  const sessionRef = useRef({ gameId });
  /** Latest page URL — place()/restore read this so resize never re-navigates. */
  const pageRef = useRef(page);
  pageRef.current = page;
  /** Logical tabs already pointed at a real URL (avoid reloading on every paint). */
  const navigatedPageIdRef = useRef<string | null>(null);
  const onNavigateRef = useRef(onNavigate);
  onNavigateRef.current = onNavigate;

  function hostTitle(next: string): string {
    if (isNewTabUrl(next)) return "Nouvel onglet";
    try {
      return new URL(next).hostname.replace(/^www\./i, "") || "Web";
    } catch {
      return "Web";
    }
  }

  function refreshFavorites() {
    const ids = browseSiteFavoritesFor(gameId);
    const all = allBrowseSitesFor(gameId);
    const byId = new Map(all.map((s) => [s.id, s]));
    setFavoriteSites(ids.map((id) => byId.get(id)).filter((s): s is BrowseSite => !!s));
  }

  useEffect(() => {
    setHomepage(webHomepageSetting());
    refreshFavorites();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [gameId, pageId, favoriteSiteIds]);

  // Navigate only on logical Web-tab switch (`pageId`). Download start/progress/
  // captured/refresh must never remount this pane or re-navigate for sync.
  useLayoutEffect(() => {
    sessionRef.current = liveRef.current;
    setPage(url);
    pageRef.current = url;
    setAddress(isNewTabUrl(url) ? "" : url);
    setFailure(null);
    setLoading(false);
    if (embedded && !isNewTabUrl(url) && navigatedPageIdRef.current !== pageId) {
      navigatedPageIdRef.current = pageId;
      void browserNavigate(url).catch(() => {});
    } else if (isNewTabUrl(url)) {
      navigatedPageIdRef.current = null;
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps -- url applied on page switch only
  }, [pageId]);

  // Keep the hole URL in sync when the active page navigates (same tab).
  // Never call browserNavigate here — that was reloading the WebView on every
  // address-bar sync / focus-driven parent re-render.
  useEffect(() => {
    setPage(url);
    pageRef.current = url;
    if (!editingRef.current) setAddress(isNewTabUrl(url) ? "" : url);
  }, [url]);

  // Place/hide the native WebView2 hole. ResizeObserver only moves the rect —
  // never navigates. `pageId` here repositions when the logical tab changes;
  // download events must not change `pageId` or unmount this component.
  useLayoutEffect(() => {
    if (!embedded) return;

    if (!visible || onNewTab) {
      void browserHide();
      return;
    }

    const hole = holeRef.current;
    if (!hole) return;

    function place() {
      const r = hole!.getBoundingClientRect();
      // URL only used on the very first show (parked about:blank); after that
      // Rust keeps the live page. Passing pageRef avoids depending on `page`.
      browserShow(
        pageRef.current,
        { x: r.left, y: r.top, width: r.width, height: r.height },
        sessionRef.current.gameId,
      ).catch((e: unknown) => {
        setFailure(e instanceof Error ? e.message : String(e));
      });
    }

    place();
    const observer = new ResizeObserver(place);
    observer.observe(hole);
    window.addEventListener("resize", place);
    return () => {
      observer.disconnect();
      window.removeEventListener("resize", place);
    };
  }, [embedded, visible, onNewTab, pageId]);

  useEffect(() => {
    if (!embedded) return;
    let offNav: (() => void) | undefined;
    let offDl: (() => void) | undefined;
    let gone = false;
    void onBrowserNavigated(({ url: next, loading: busy }) => {
      if (next === "about:blank" || isNewTabUrl(next)) return;
      setPage(next);
      setLoading(busy);
      onNavigateRef.current(next, hostTitle(next));
      if (!editingRef.current) setAddress(next);
    }).then((unlisten) => {
      if (gone) unlisten();
      else offNav = unlisten;
    });
    // Voluntary restore after begin: leave the CDN blank and return to the
    // mod listing pageUrl. Not a remount — same WebView2, explicit navigate.
    void onDownloadStarted((event) => {
      const pageUrl = event.pageUrl?.trim();
      if (!pageUrl || isNewTabUrl(pageUrl)) return;
      if (sameBrowseUrl(pageRef.current, pageUrl)) return;
      setPage(pageUrl);
      pageRef.current = pageUrl;
      setAddress(pageUrl);
      onNavigateRef.current(pageUrl, hostTitle(pageUrl));
      void browserNavigate(pageUrl).catch(() => {});
    }).then((unlisten) => {
      if (gone) unlisten();
      else offDl = unlisten;
    });
    return () => {
      gone = true;
      offNav?.();
      offDl?.();
    };
  }, [embedded]);

  function goTo(nextUrl: string) {
    const next = nextUrl.trim() || NEW_TAB_URL;
    setPage(next);
    setAddress(isNewTabUrl(next) ? "" : next);
    onNavigate(next, hostTitle(next));
    if (!isNewTabUrl(next) && embedded) {
      navigatedPageIdRef.current = pageId;
      void browserNavigate(next).catch(() => {});
    } else if (isNewTabUrl(next) && embedded) {
      void browserHide();
    }
  }

  function submit(e: FormEvent<HTMLFormElement>) {
    e.preventDefault();
    const raw = address.trim();
    if (!raw && onNewTab) return;
    const full = resolveOmnibox(raw || page, homepage);
    (e.currentTarget.querySelector("input") as HTMLInputElement | null)?.blur();
    goTo(full);
  }

  const secure = /^https:\/\//i.test(page) && !onNewTab;
  const canFavorite = !onNewTab && !!page && /^https?:\/\//i.test(page);

  return (
    <div className="browser">
      <div className="browser-bar">
        <div className="browser-nav">
          <IconButton
            name="chevl"
            title="Précédent"
            onClick={() => {
              if (!onNewTab) void browserGo(-1);
            }}
          />
          <IconButton
            name="chevr"
            title="Suivant"
            onClick={() => {
              if (!onNewTab) void browserGo(1);
            }}
          />
          <IconButton
            name="refresh"
            title="Recharger"
            onClick={() => {
              if (onNewTab) return;
              void browserNavigate(page);
            }}
          />
        </div>
        <form className="browser-url" onSubmit={submit}>
          {loading && !onNewTab ? (
            <PixelLoader title="Chargement…" />
          ) : (
            <span
              title={
                onNewTab
                  ? "Nouvel onglet"
                  : secure
                    ? "Connexion sécurisée"
                    : "Connexion non chiffrée"
              }
            >
              <Ico name={onNewTab ? "search" : secure ? "lock" : "globe"} className="sm" />
            </span>
          )}
          <Input
            value={address}
            onChange={(e) => setAddress(e.target.value)}
            onFocus={(e) => {
              editingRef.current = true;
              e.currentTarget.select();
            }}
            onBlur={() => {
              editingRef.current = false;
            }}
            onKeyDown={(e) => {
              if (e.key === "Escape") {
                setAddress(onNewTab ? "" : page);
                e.currentTarget.blur();
              }
            }}
            placeholder="Rechercher ou saisir une adresse"
            spellCheck={false}
            autoComplete="off"
            aria-label="Adresse"
          />
          <IconButton
            ref={starRef}
            name="star"
            size="sm"
            title="Ajouter aux favoris"
            disabled={!canFavorite}
            onClick={() => {
              if (!canFavorite) return;
              setFavOpen(true);
            }}
          />
        </form>
      </div>

      {favoriteSites.length > 0 ? (
        <div className="browser-bookmarks" role="toolbar" aria-label="Favoris">
          {favoriteSites.map((site) => (
            <button
              key={site.id}
              type="button"
              className="browser-bookmark"
              {...tip(site.label || site.url)}
              onClick={() => goTo(site.url)}
            >
              <BookmarkIcon url={site.url} />
            </button>
          ))}
        </div>
      ) : null}

      {onNewTab ? (
        <BrowserNewTab homepage={homepage} favorites={favoriteSites} onNavigate={goTo} />
      ) : (
        <div className="browser-hole" ref={holeRef}>
          {embedded && failure ? (
            <div className="empty">
              <div>
                <Ico name="alert" />
                <h3>Le navigateur n’a pas pu s’ouvrir</h3>
                <p>{failure}</p>
              </div>
            </div>
          ) : null}
          {embedded ? null : (
            <div className="empty">
              <div>
                <Ico name="globe" />
                <h3>Navigateur indisponible</h3>
                <p>Lance l’application ModLab pour naviguer</p>
              </div>
            </div>
          )}
        </div>
      )}

      {favOpen ? (
        <FavoriteUrlPopover
          url={page}
          games={games}
          activeGameId={gameId}
          anchorRef={starRef}
          onClose={() => setFavOpen(false)}
          onConfirm={(gameIds) => {
            addBrowseUrlFavorite(gameIds, page);
            refreshFavorites();
            onFavoriteChange?.();
          }}
        />
      ) : null}
    </div>
  );
}
