import { useEffect, useState, type ReactNode } from "react";
import {
  getGamePathSlots,
  listGameInstallEntries,
  type GameInstallEntry,
} from "../../api";
import { Crumbs } from "../../components/ui";
import { Ico } from "../../Ico";

/**
 * The active game's install folder — path slot `root`, the same folder
 * Réglages shows as the install directory. Not the merged Data view and
 * not another slot (Data, saves, mods/). Subfolders open in place; the
 * crumb trail never climbs above that root.
 */
export default function RepertoirePage({ gameId }: { gameId: string | null }) {
  const [rootLabel, setRootLabel] = useState<string | null>(null);
  const [rootPath, setRootPath] = useState<string | null>(null);
  const [locatedFor, setLocatedFor] = useState<string | null>(null);
  const [segments, setSegments] = useState<string[]>([]);
  const [entries, setEntries] = useState<GameInstallEntry[] | null>(null);
  const [listKey, setListKey] = useState<string | null>(null);
  const [missingPath, setMissingPath] = useState(false);
  const [unavailable, setUnavailable] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (gameId == null) {
      setRootPath(null);
      setRootLabel(null);
      setLocatedFor(null);
      setSegments([]);
      setEntries(null);
      setListKey(null);
      setMissingPath(false);
      setUnavailable(false);
      setError(null);
      return;
    }
    let cancelled = false;
    setRootPath(null);
    setRootLabel(null);
    setLocatedFor(null);
    setSegments([]);
    setEntries(null);
    setListKey(null);
    setMissingPath(false);
    setUnavailable(false);
    setError(null);
    getGamePathSlots(gameId)
      .then((slots) => {
        if (cancelled) return;
        const root = slots.find((slot) => slot.id === "root" && slot.kind === "dir");
        if (!root?.effective) {
          setMissingPath(true);
          setEntries([]);
          setLocatedFor(gameId);
          return;
        }
        setRootPath(root.effective);
        setRootLabel(root.label);
        setLocatedFor(gameId);
      })
      .catch((e: unknown) => {
        if (cancelled) return;
        setError(e instanceof Error ? e.message : String(e));
        setEntries([]);
        setLocatedFor(gameId);
      });
    return () => {
      cancelled = true;
    };
  }, [gameId]);

  useEffect(() => {
    if (gameId == null || locatedFor !== gameId || rootPath == null) return;
    let cancelled = false;
    const relativePath = segments.join("/");
    const key = `${gameId}\u0000${relativePath}`;
    listGameInstallEntries(gameId, relativePath)
      .then((listed) => {
        if (cancelled) return;
        if (listed == null) {
          setUnavailable(true);
          setError(null);
          setEntries([]);
          setListKey(key);
          return;
        }
        setUnavailable(false);
        setError(null);
        setEntries(listed);
        setListKey(key);
      })
      .catch((e: unknown) => {
        if (cancelled) return;
        setUnavailable(false);
        setError(e instanceof Error ? e.message : String(e));
        setEntries([]);
        setListKey(key);
      });
    return () => {
      cancelled = true;
    };
  }, [gameId, locatedFor, rootPath, segments]);

  if (gameId == null) {
    return (
      <EmptyState
        icon="folder"
        title="Aucun jeu"
        body="Aucun jeu actif dont afficher le dossier"
      />
    );
  }

  const synced = locatedFor === gameId;
  if (!synced) {
    return <EmptyState icon="info" title="Lecture en cours…" />;
  }

  if (missingPath) {
    return (
      <EmptyState
        icon="folder"
        title="Aucun dossier d'installation"
        body="Le chemin du jeu n'est pas renseigné"
      />
    );
  }

  if (rootPath == null) {
    return <EmptyState icon="alert" title="Lecture impossible" body={error ?? undefined} />;
  }

  const relativePath = segments.join("/");
  const requestKey = `${gameId}\u0000${relativePath}`;
  const current = listKey === requestKey && entries !== null;
  const rootName = rootLabel?.trim() ? rootLabel : "Dossier d'installation";
  const shownPath = displayPath(rootPath, segments);

  function goTo(depth: number) {
    if (depth < 0 || depth >= segments.length) return;
    setSegments(segments.slice(0, depth));
  }

  function openDir(name: string) {
    if (!isSingleSegment(name)) return;
    setSegments([...segments, name]);
  }

  return (
    <>
      <Crumbs
        path={installCrumbs(rootName, segments, goTo)}
        tail={
          <span className="mono" title={shownPath}>
            {shownPath}
          </span>
        }
      />
      {folderBody(current, entries, error, unavailable, openDir)}
    </>
  );
}

function folderBody(
  current: boolean,
  entries: GameInstallEntry[] | null,
  error: string | null,
  unavailable: boolean,
  openDir: (name: string) => void,
) {
  if (!current || entries === null) {
    return <EmptyState icon="info" title="Lecture en cours…" />;
  }
  if (error) {
    return <EmptyState icon="alert" title="Lecture impossible" body={error} />;
  }
  if (unavailable) {
    return (
      <EmptyState
        icon="folder"
        title="Contenu non lisible ici"
        body="Ouvre l'application pour voir ce dossier"
      />
    );
  }
  if (entries.length === 0) {
    return (
      <EmptyState icon="folder" title="Dossier vide" body="Ce dossier ne contient rien" />
    );
  }

  const rowStyle = {
    display: "grid" as const,
    gridTemplateColumns: "1fr 120px",
    width: "100%",
    textAlign: "left" as const,
  };

  return (
    <div className="content" style={{ overflow: "auto" }}>
      <div className="grid" style={{ display: "block" }}>
        <div
          className="gh"
          style={{
            display: "grid",
            gridTemplateColumns: "1fr 120px",
            position: "sticky",
            top: 0,
            zIndex: 2,
            background: "var(--c2)",
          }}
        >
          <div>Nom</div>
          <div>Type</div>
        </div>
        {entries.map((entry) => {
          const cells = (
            <>
              <div className="nm">
                <Ico name={entry.kind === "dir" ? "folder" : "file"} className="sm" />
                <span className="t">{entry.name}</span>
              </div>
              <div>{entry.kind === "dir" ? "Dossier" : "Fichier"}</div>
            </>
          );
          if (entry.kind === "dir" && isSingleSegment(entry.name)) {
            return (
              <button
                key={entry.name}
                type="button"
                className="row"
                style={rowStyle}
                onClick={() => openDir(entry.name)}
              >
                {cells}
              </button>
            );
          }
          return (
            <div key={entry.name} className="row" style={rowStyle}>
              {cells}
            </div>
          );
        })}
      </div>
    </div>
  );
}

/** A name read from one directory: no separator, no parent step. */
function isSingleSegment(name: string): boolean {
  return (
    name !== "" &&
    name !== "." &&
    name !== ".." &&
    !name.includes("/") &&
    !name.includes("\\") &&
    !name.includes("\0")
  );
}

function displayPath(root: string, segments: string[]): string {
  const sep = root.includes("\\") ? "\\" : "/";
  const base = root.replace(/[\\/]+$/, "");
  return segments.length === 0 ? base : [base, ...segments].join(sep);
}

function installCrumbs(
  rootLabel: string,
  segments: string[],
  onGo: (depth: number) => void,
): ReactNode[] {
  const here = (label: string) => <span aria-current="page">{label}</span>;
  const up = (label: string, depth: number) => (
    <button type="button" onClick={() => onGo(depth)}>
      {label}
    </button>
  );
  const path: ReactNode[] = [segments.length === 0 ? here(rootLabel) : up(rootLabel, 0)];
  segments.forEach((name, index) => {
    const depth = index + 1;
    path.push(depth === segments.length ? here(name) : up(name, depth));
  });
  return path;
}

function EmptyState({ icon, title, body }: { icon: string; title: string; body?: string }) {
  return (
    <div className="content">
      <div className="empty">
        <div>
          <Ico name={icon} />
          <h3>{title}</h3>
          {body ? <p>{body}</p> : null}
        </div>
      </div>
    </div>
  );
}
