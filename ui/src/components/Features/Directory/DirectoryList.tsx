import { useEffect, useState } from "react";
import type { DirectoryEntry } from "../../../api";
import { Ico } from "../../../Ico";

/**
 * A flat list of `DirectoryEntry` rows — the merged view (or a filtered
 * slice of it), shared by the Plugins/Archives/Répertoire tabs. Each one
 * only differs in which fetch it calls and what its empty state says.
 */
export function DirectoryList({
  loadoutId,
  fetchEntries,
  emptyTitle,
  emptyBody,
}: {
  loadoutId: number | null;
  fetchEntries: (loadoutId: number) => Promise<DirectoryEntry[]>;
  emptyTitle: string;
  emptyBody: string;
}) {
  const [entries, setEntries] = useState<DirectoryEntry[] | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (loadoutId == null) {
      setEntries([]);
      return;
    }
    let cancelled = false;
    setEntries(null);
    setError(null);
    fetchEntries(loadoutId)
      .then((rows) => {
        if (!cancelled) setEntries(rows);
      })
      .catch((e: unknown) => {
        if (!cancelled) setError(e instanceof Error ? e.message : String(e));
      });
    return () => {
      cancelled = true;
    };
  }, [loadoutId, fetchEntries]);

  if (error) {
    return (
      <div className="content">
        <div className="empty">
          <div>
            <Ico name="alert" />
            <h3>Lecture impossible</h3>
            <p>{error}</p>
          </div>
        </div>
      </div>
    );
  }

  if (entries === null) {
    return (
      <div className="content">
        <div className="empty">
          <div>
            <Ico name="info" />
            <h3>Lecture en cours…</h3>
          </div>
        </div>
      </div>
    );
  }

  if (entries.length === 0) {
    return (
      <div className="content">
        <div className="empty">
          <div>
            <Ico name="info" />
            <h3>{emptyTitle}</h3>
            <p>{emptyBody}</p>
          </div>
        </div>
      </div>
    );
  }

  return (
    <div className="content" style={{ overflow: "auto" }}>
      <div
        className="grid"
        style={{ display: "block" }}
      >
        <div
          className="gh"
          style={{
            display: "grid",
            gridTemplateColumns: "1fr 220px 90px",
            position: "sticky",
            top: 0,
            zIndex: 2,
            background: "var(--c2)",
          }}
        >
          <div>Chemin</div>
          <div>Mod (gagnant)</div>
          <div>Contributeurs</div>
        </div>
        {entries.map((e) => (
          <div
            key={e.relativePath}
            className="row"
            style={{ display: "grid", gridTemplateColumns: "1fr 220px 90px" }}
          >
            <div className="nm">
              <span className="t mono">{e.relativePath}</span>
            </div>
            <div>{e.modName}</div>
            <div className="num">
              {e.contributorCount > 1 ? (
                <span className="tag" title="Plus d'un mod fournit ce fichier — le dernier gagne">
                  {e.contributorCount}
                </span>
              ) : (
                "1"
              )}
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}
