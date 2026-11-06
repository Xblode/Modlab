import { useLayoutEffect, useRef, useState, type FormEvent } from "react";
import { browserGo, browserHide, browserNavigate, browserShow, isTauriShell } from "../../../api";
import { Ico } from "../../../Ico";
import { IconButton, Input } from "../../ui";

/**
 * The embedded browser tab.
 *
 * Nothing of the page is rendered by React: the page is a *native* child
 * WebView2, a sibling of the shell's own webview, which the OS paints over
 * this component's hole. So this component's whole job is to be a
 * well-measured empty rectangle — it reports where the hole is, and the Rust
 * side moves the real browser there.
 *
 * Stays mounted for the rest of the session once opened — the caller (see
 * `App.tsx`) renders it always, toggling `visible` rather than mounting and
 * unmounting on every tab switch. That's what makes "leave the tab, come
 * back" a no-op: a component that unmounted on every tab switch would redo
 * its "first open" work — reset scroll to top, and previously actually
 * re-navigate — each time, which reads as an unwanted refresh even though
 * nothing about the active game changed.
 *
 * Consequences worth knowing: the page cannot be inspected from here, it
 * always paints on top of any React element overlapping it (which is why the
 * address bar sits *above* the hole rather than floating over it), and in the
 * browser preview — where there is no Tauri and therefore no second webview —
 * this shows an explanation instead of pretending.
 */
export function BrowserPane({
  homeUrl,
  gameId,
  visible,
  openToken,
  onOpenDownloads,
}: {
  /** Where the browser lands each time it's explicitly (re)opened — the
   * active game's Nexus page at that moment. */
  homeUrl: string;
  /** The active game at the moment of that same explicit open — recorded
   * Rust-side so a download captured during this session files under this
   * game's own downloads subfolder. */
  gameId: string | null;
  /** Whether the browser tab is the one currently on screen. Toggling this
   * repositions/hides the native webview; it never re-navigates. */
  visible: boolean;
  /** Bumped by the caller each time the human explicitly clicks "open the
   * browser" (the title bar's globe button) — the one and only trigger for
   * navigating to `homeUrl`. Merely becoming `visible` again (switching back
   * to this tab) must not trigger it. */
  openToken: number;
  onOpenDownloads: () => void;
}) {
  const holeRef = useRef<HTMLDivElement>(null);
  const [address, setAddress] = useState(homeUrl);
  const [failure, setFailure] = useState<string | null>(null);
  const embedded = isTauriShell();

  // Always fresh, every render, no effect involved — what an explicit open
  // (see below) should navigate to right now, as opposed to whatever it was
  // when the pane last actually opened.
  const liveRef = useRef({ homeUrl, gameId });
  liveRef.current = { homeUrl, gameId };
  // What the currently displayed page was actually opened with — only
  // updated when `openToken` changes, i.e. on an explicit open. `place()`
  // below reads this, never `homeUrl`/`gameId` directly, so a game switch
  // elsewhere in the app can't leak into a running browse session.
  const sessionRef = useRef({ homeUrl, gameId });
  // Sentinel, not `openToken`'s own initial value: this must never equal a
  // real token on the very first effect run, or the pane's first-ever open
  // would skip its own navigate (see the check below).
  const lastTokenRef = useRef(-1);

  useLayoutEffect(() => {
    if (!embedded) return;

    if (openToken !== lastTokenRef.current) {
      lastTokenRef.current = openToken;
      sessionRef.current = liveRef.current;
      setAddress(sessionRef.current.homeUrl);
      void browserNavigate(sessionRef.current.homeUrl).catch(() => {
        // The `browserShow` call below already reports a visible failure if
        // the browser can't come up at all; a navigate failure on top of
        // that isn't worth a second error surface.
      });
    }

    if (!visible) {
      void browserHide();
      return;
    }
    const hole = holeRef.current;
    if (!hole) return;

    function place() {
      const r = hole!.getBoundingClientRect();
      browserShow(
        sessionRef.current.homeUrl,
        { x: r.left, y: r.top, width: r.width, height: r.height },
        sessionRef.current.gameId,
      ).catch((e: unknown) => {
        // Never swallow this: a browser that fails to appear must say so,
        // not leave an empty rectangle with no explanation.
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
  }, [embedded, visible, openToken]);

  function submit(e: FormEvent) {
    e.preventDefault();
    const url = address.trim();
    if (!url) return;
    const full = /^https?:\/\//i.test(url) ? url : `https://${url}`;
    setAddress(full);
    void browserNavigate(full);
  }

  return (
    <div className="browser">
      <div className="browser-bar">
        <IconButton name="arrowleft" title="Précédent" onClick={() => void browserGo(-1)} />
        <IconButton
          name="chevr"
          title="Suivant"
          onClick={() => void browserGo(1)}
        />
        <IconButton
          name="refresh"
          title="Recharger"
          onClick={() => void browserNavigate(address)}
        />
        <form className="browser-url" onSubmit={submit}>
          <Ico name="globe" className="sm" />
          <Input
            value={address}
            onChange={(e) => setAddress(e.target.value)}
            spellCheck={false}
            autoComplete="off"
            aria-label="Adresse"
          />
        </form>
        <button className="btn sm" type="button" onClick={onOpenDownloads}>
          <Ico name="download" className="sm" />
          Téléchargements
        </button>
      </div>

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
              <h3>Navigateur indisponible dans l’aperçu</h3>
              <p>
                La page est un vrai WebView2 natif, posé par-dessus la fenêtre —
                il n’existe que dans le shell Tauri. Lance l’application pour
                naviguer et capturer des téléchargements.
              </p>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
