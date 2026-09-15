import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import type { CSSProperties } from "react";
import type { DetectedGame, LoadoutDto } from "./api";
import { chromeFor, initials, type TabId } from "./gameChrome";
import { Ico } from "./Ico";
import { Pop } from "./Pop";

/** Placeholder for anything the Core does not track yet. */
const NOT_TRACKED = "—";
const NOT_TRACKED_TITLE = "Pas encore suivi par le Core";

type InstSort = "core" | "name";

const SORT_LABELS: Record<InstSort, string> = {
  core: "Ordre du Core",
  name: "Nom A→Z",
};

type Props = {
  game: DetectedGame;
  loadouts: LoadoutDto[];
  activeLoadoutId: number | null;
  modCount: number;
  isFavorite: (loadoutId: number) => boolean;
  onToggleFavorite: (loadoutId: number) => void;
  onPickLoadout: (loadoutId: number) => void;
  onGoToTab: (tab: TabId) => void;
  onUnwired: (label: string) => void;
};

export function Dashboard({
  game,
  loadouts,
  activeLoadoutId,
  modCount,
  isFavorite,
  onToggleFavorite,
  onPickLoadout,
  onGoToTab,
  onUnwired,
}: Props) {
  const chrome = chromeFor(game.id);
  const [sort, setSort] = useState<InstSort>("core");
  const [sortOpen, setSortOpen] = useState(false);
  const sortRef = useRef<HTMLButtonElement>(null);
  const artRef = useRef<HTMLDivElement>(null);
  const infoRef = useRef<HTMLDivElement>(null);
  const hasSante = chrome.tabs.some((t) => t.id === "sante");
  const hasPlugins = chrome.tabs.some((t) => t.id === "plugins");

  // Same rule as the maquette's syncHeroArt(): the hero art matches the
  // height of the text column beside it, keeping the 460/215 Steam ratio.
  const syncHeroArt = useCallback(() => {
    const art = artRef.current;
    const info = infoRef.current;
    if (!art || !info) return;
    // Only the wide Steam-header variant is resized; the glyph tile stays 92px.
    // React reuses this node across games, so clear any size left by a previous one.
    if (!art.classList.contains("has-img")) {
      art.style.removeProperty("height");
      art.style.removeProperty("width");
      return;
    }
    const h = info.getBoundingClientRect().height;
    if (!h) return;
    art.style.height = `${h}px`;
    art.style.width = `${Math.max(140, Math.min(340, (h * 460) / 215))}px`;
  }, []);

  useLayoutEffect(syncHeroArt);
  useEffect(() => {
    window.addEventListener("resize", syncHeroArt);
    return () => window.removeEventListener("resize", syncHeroArt);
  }, [syncHeroArt]);

  const sorted = useMemo(() => {
    const rows = [...loadouts];
    rows.sort((a, b) => {
      const fa = isFavorite(a.id);
      const fb = isFavorite(b.id);
      if (fa !== fb) return fa ? -1 : 1;
      if (sort === "name") return a.name.localeCompare(b.name, "fr");
      return a.id - b.id;
    });
    return rows;
  }, [loadouts, sort, isFavorite]);

  const heroStyle = {
    "--c1g": chrome.c1,
    "--c2g": chrome.c2,
  } as CSSProperties;

  return (
    <>
      <div className="tbar">
        <div className="crumbs">
          <Ico name="box" />
          <b>{game.short}</b>
          <Ico name="chevr" />
          Tableau de bord
        </div>
      </div>
      <div className="content">
        <div className="dash">
          <header className={`ghero ${chrome.art ? "has-art" : ""}`} style={heroStyle}>
            {chrome.art ? (
              <div
                className="ghero-bg"
                style={{ backgroundImage: `url('${chrome.art}')` }}
              />
            ) : null}
            <div className="ghero-top">
              <div
                className={`art ${chrome.art ? "has-img" : ""}`}
                ref={artRef}
                style={
                  chrome.art ? { backgroundImage: `url('${chrome.art}')` } : undefined
                }
              >
                {chrome.art ? null : chrome.glyph}
              </div>
              <div className="ginfo" ref={infoRef}>
                <h1>
                  <span className="fade">{game.name}</span>
                </h1>
                <div className="meta">
                  <span title={NOT_TRACKED_TITLE}>Version {NOT_TRACKED}</span>
                  <span>{chrome.backend}</span>
                  <span title={NOT_TRACKED_TITLE}>
                    <b>{NOT_TRACKED}</b> sur le disque
                  </span>
                  <span>
                    <b>{modCount || NOT_TRACKED}</b> mods
                  </span>
                  <span>
                    <b>{loadouts.length}</b> instance{loadouts.length > 1 ? "s" : ""}
                  </span>
                </div>
                <div className="stats">
                  <div className="stat" title={NOT_TRACKED_TITLE}>
                    <span className="k">Temps de jeu</span>
                    <span className="v">
                      <Ico name="history" />
                      {NOT_TRACKED}
                    </span>
                  </div>
                  <div className="stat" title={NOT_TRACKED_TITLE}>
                    <span className="k">Dernière session</span>
                    <span className="v">
                      <Ico name="play" />
                      {NOT_TRACKED}
                    </span>
                  </div>
                  <button
                    className="stat"
                    type="button"
                    title={`${NOT_TRACKED_TITLE} — aucun journal de crash`}
                    onClick={() =>
                      hasSante
                        ? onGoToTab("sante")
                        : onUnwired("Aucune vérification pour ce jeu")
                    }
                  >
                    <span className="k">Crashs · 30 j</span>
                    <span className="v">
                      <Ico name="info" />
                      {NOT_TRACKED}
                    </span>
                  </button>
                  <button
                    className="stat"
                    type="button"
                    title="Aucune vérification de santé n’a encore tourné"
                    onClick={() =>
                      hasSante
                        ? onGoToTab("sante")
                        : onUnwired("Aucune vérification pour ce jeu")
                    }
                  >
                    <span className="k">Santé</span>
                    <span className="v">
                      <Ico name="info" />
                      Non vérifié
                    </span>
                  </button>
                </div>
              </div>
            </div>
            <div className="qa">
              <button
                className="btn sm ghost"
                type="button"
                onClick={() => onUnwired("Ouvrir le dossier du jeu")}
              >
                <Ico name="folder" className="sm" />
                Ouvrir le dossier du jeu
              </button>
              <button
                className="btn sm ghost"
                type="button"
                onClick={() => onUnwired("Vérifier les mises à jour")}
              >
                <Ico name="up" className="sm" />
                Vérifier les mises à jour
              </button>
              <button
                className="btn sm ghost"
                type="button"
                onClick={() => onUnwired("Nettoyer les téléchargements")}
              >
                <Ico name="history" className="sm" />
                Nettoyer les téléchargements
              </button>
              {hasPlugins ? (
                <button
                  className="btn sm ghost"
                  type="button"
                  onClick={() => onUnwired("Trier avec LOOT")}
                >
                  <Ico name="wand" className="sm" />
                  Trier avec LOOT
                </button>
              ) : null}
            </div>
          </header>

          <div className="dpad">
            <div className="dh row2">
              Instances <span className="cnt">{loadouts.length}</span>
              <button
                className="sortb"
                type="button"
                ref={sortRef}
                title="Trier les instances"
                onClick={() => setSortOpen((v) => !v)}
              >
                <Ico name="sliders" />
                {SORT_LABELS[sort]}
                <Ico name="chev" />
              </button>
              {sortOpen ? (
                <Pop
                  anchorRef={sortRef}
                  onClose={() => setSortOpen(false)}
                  items={[
                    { label: "Favoris toujours en tête", sub: "puis tri par :", icon: "star" },
                    "-",
                    {
                      label: SORT_LABELS.core,
                      icon: sort === "core" ? "check" : "db",
                      current: sort === "core",
                      onPick: () => setSort("core"),
                    },
                    {
                      label: SORT_LABELS.name,
                      icon: sort === "name" ? "check" : "note",
                      current: sort === "name",
                      onPick: () => setSort("name"),
                    },
                  ]}
                />
              ) : null}
            </div>

            <div className="inst-grid">
              {sorted.map((l) => {
                const cur = l.id === activeLoadoutId;
                const fav = isFavorite(l.id);
                return (
                  <div
                    key={l.id}
                    className={`inst-card ${cur ? "cur" : ""}`}
                    role="button"
                    tabIndex={0}
                    style={heroStyle}
                    onClick={() => onPickLoadout(l.id)}
                    onKeyDown={(e) => {
                      if (e.key === "Enter" || e.key === " ") {
                        e.preventDefault();
                        onPickLoadout(l.id);
                      }
                    }}
                  >
                    <span className="thumb">{initials(l.name)}</span>
                    <span className="ic-body">
                      <span className="ic-top">
                        <span className="t fade">{l.name}</span>
                      </span>
                      <span className="ic-meta fade">
                        <span className="mono" title="Pas encore de révisions immuables">
                          r.{NOT_TRACKED}
                        </span>
                        <span>{l.modCount} mods</span>
                        {cur ? <span className="on">● active</span> : null}
                      </span>
                    </span>
                    <button
                      className={`fav-star ${fav ? "on" : ""}`}
                      type="button"
                      title={fav ? "Retirer des favoris" : "Ajouter aux favoris"}
                      onClick={(e) => {
                        e.stopPropagation();
                        onToggleFavorite(l.id);
                      }}
                    >
                      <Ico name="star" className={fav ? "fill" : ""} />
                    </button>
                  </div>
                );
              })}
              <div
                className="inst-card add"
                role="button"
                tabIndex={0}
                onClick={() => onUnwired("Nouvelle instance")}
                onKeyDown={(e) => {
                  if (e.key === "Enter" || e.key === " ") {
                    e.preventDefault();
                    onUnwired("Nouvelle instance");
                  }
                }}
              >
                <Ico name="box" className="sm" />
                <span>Nouvelle instance</span>
              </div>
            </div>

            <div className="dsel-row">
              <div className="dsel">
                <span className="dsel-lbl">Version du jeu</span>
                <button className="dsel-btn" type="button" disabled title={NOT_TRACKED_TITLE}>
                  {NOT_TRACKED}
                  <span className="spacer" />
                  <Ico name="chev" className="sm" />
                </button>
              </div>
              {chrome.engine ? (
                <div className="dsel">
                  <span className="dsel-lbl">Version de {chrome.engine}</span>
                  <button
                    className="dsel-btn"
                    type="button"
                    disabled
                    title={NOT_TRACKED_TITLE}
                  >
                    {NOT_TRACKED}
                    <span className="spacer" />
                    <Ico name="chev" className="sm" />
                  </button>
                </div>
              ) : null}
            </div>

            <div className="dh">Outils</div>
            <div className="tools-row">
              <span className="muted" style={{ fontSize: 12, gridColumn: "1 / -1" }}>
                Le Core n’a pas encore de registre d’outils (LOOT, xEdit, Nemesis…).
              </span>
            </div>

            <div className="box" style={{ marginTop: 22 }}>
              <div className="bh">
                <Ico name="info" className="sm" />
                Ce que le Core ne suit pas encore
              </div>
              <div className="why">
                Temps de jeu, dernière session, crashs et santé n’ont <b>aucune source</b>{" "}
                dans le Core aujourd’hui : il n’existe ni journal de sessions ni métriques.
                Les tuiles gardent leur place mais affichent « — » plutôt qu’un chiffre
                inventé. Même chose pour la révision d’une instance — le cycle snapshot
                n’est pas encore là — et pour le registre d’outils. Les favoris, eux, sont
                stockés <b>dans ce navigateur uniquement</b> : le Core n’a pas encore de
                champ « favori ».
              </div>
            </div>
          </div>
        </div>
      </div>
    </>
  );
}
