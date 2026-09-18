import { useMemo, useRef, useState } from "react";
import { GameDashboardHeader, type HeaderAction } from "./components/modlab/games/GameDashboardHeader";
import type { CSSProperties } from "react";
import type { DetectedGame, LoadoutDto } from "./api";
import { chromeFor, initials, type TabId } from "./gameChrome";
import { FavoriteButton } from "./components/ui/FavoriteButton";
import { Ico } from "./Ico";
import { Pop } from "./Pop";

/** Placeholder for anything the Core does not track yet. */
const NOT_TRACKED = "—";
const NOT_TRACKED_TITLE = "Pas encore suivi par le Core";

/**
 * Import entry points, mapped from the adapter's `importSource` id — the same
 * capability-id-to-label pattern as the tab vocabulary in `gameChrome.ts`.
 * A game without a declared source gets no import button at all.
 */
const IMPORT_SOURCES: Record<string, { label: string; icon: string }> = {
  mo2: { label: "Importer depuis MO2", icon: "db" },
};

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
  onCreateLoadout: () => void;
  onImportSource: () => void;
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
  onCreateLoadout,
  onImportSource,
  onGoToTab,
  onUnwired,
}: Props) {
  const chrome = chromeFor(game.id);
  const [sort, setSort] = useState<InstSort>("core");
  const [sortOpen, setSortOpen] = useState(false);
  const sortRef = useRef<HTMLButtonElement>(null);
  const hasSante = game.capabilities.sections.includes("health");
  const openHealth = () => hasSante
    ? onGoToTab("sante")
    : onUnwired("Aucune vérification pour ce jeu");
  const headerActions: HeaderAction[] = [
    { id: "folder", label: "Ouvrir le dossier du jeu", icon: "folder", onClick: () => onUnwired("Ouvrir le dossier du jeu") },
    { id: "updates", label: "Vérifier les mises à jour", icon: "up", onClick: () => onUnwired("Vérifier les mises à jour") },
    { id: "cleanup", label: "Nettoyer les téléchargements", icon: "history", onClick: () => onUnwired("Nettoyer les téléchargements") },
  ];
  if (game.capabilities.loadOrderTool) {
    const tool = game.capabilities.loadOrderTool;
    headerActions.push({
      id: tool.id,
      label: `Trier avec ${tool.name}`,
      icon: "wand",
      onClick: () => onUnwired(`Trier avec ${tool.name}`),
    });
  }
  const importAction = game.capabilities.importSource
    ? IMPORT_SOURCES[game.capabilities.importSource]
    : undefined;
  if (importAction) {
    headerActions.push({
      id: "import",
      label: importAction.label,
      icon: importAction.icon,
      onClick: onImportSource,
    });
  }

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
          <GameDashboardHeader
            name={game.name}
            artwork={chrome}
            metadata={[
              { id: "version", title: NOT_TRACKED_TITLE, content: <>Version {NOT_TRACKED}</> },
              { id: "backend", content: game.capabilities.deploymentLabel },
              { id: "disk", title: NOT_TRACKED_TITLE, content: <><b>{NOT_TRACKED}</b> sur le disque</> },
              { id: "mods", content: <><b>{modCount || NOT_TRACKED}</b> mods</> },
              { id: "instances", content: <><b>{loadouts.length}</b> instance{loadouts.length > 1 ? "s" : ""}</> },
            ]}
            stats={[
              { id: "playtime", label: "Temps de jeu", value: NOT_TRACKED, icon: "history", title: NOT_TRACKED_TITLE },
              { id: "session", label: "Dernière session", value: NOT_TRACKED, icon: "play", title: NOT_TRACKED_TITLE },
              { id: "crashes", label: "Crashs · 30 j", value: NOT_TRACKED, icon: "info", title: `${NOT_TRACKED_TITLE} — aucun journal de crash`, onClick: openHealth },
              { id: "health", label: "Santé", value: "Non vérifié", icon: "info", title: "Aucune vérification de santé n’a encore tourné", onClick: openHealth },
            ]}
            actions={headerActions}
          />

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
                    <FavoriteButton on={fav} onToggle={() => onToggleFavorite(l.id)} />
                  </div>
                );
              })}
              <div
                className="inst-card add"
                role="button"
                tabIndex={0}
                onClick={onCreateLoadout}
                onKeyDown={(e) => {
                  if (e.key === "Enter" || e.key === " ") {
                    e.preventDefault();
                    onCreateLoadout();
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
              {game.capabilities.scriptExtender ? (
                <div className="dsel">
                  <span className="dsel-lbl">
                    Version de {game.capabilities.scriptExtender}
                  </span>
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
