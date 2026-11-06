import { useEffect, useState } from "react";
import { getSkyrimSaves, type SaveFile } from "../../api";
import { Ico } from "../../Ico";

function formatSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} o`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(0)} Ko`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} Mo`;
}

/**
 * Every `.ess` file in the detected Saves folder. What the Core still can't
 * do — say which loadout revision was active when a given save was
 * written — stays unsaid rather than guessed; this only lists what's on disk.
 */
export default function SavesPage() {
  const [saves, setSaves] = useState<SaveFile[] | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    getSkyrimSaves()
      .then((s) => {
        if (!cancelled) setSaves(s);
      })
      .catch((e: unknown) => {
        if (!cancelled) setError(e instanceof Error ? e.message : String(e));
      });
    return () => {
      cancelled = true;
    };
  }, []);

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

  if (saves === null) {
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

  if (saves.length === 0) {
    return (
      <div className="content">
        <div className="empty">
          <div>
            <Ico name="disc" />
            <h3>Aucune sauvegarde trouvée</h3>
            <p>
              Dossier « Saves » introuvable ou vide. Associer une sauvegarde à la
              révision qui tournait au moment où elle a été écrite n'existe pas encore
              — cette liste ne montre que ce qui est sur le disque.
            </p>
          </div>
        </div>
      </div>
    );
  }

  return (
    <div className="content" style={{ overflow: "auto" }}>
      <div className="grid" style={{ display: "block" }}>
        <div
          className="gh"
          style={{
            display: "grid",
            gridTemplateColumns: "1fr 100px 180px",
            position: "sticky",
            top: 0,
            zIndex: 2,
            background: "var(--c2)",
          }}
        >
          <div>Fichier</div>
          <div>Taille</div>
          <div>Modifié</div>
        </div>
        {saves.map((s) => (
          <div
            key={s.fileName}
            className="row"
            style={{ display: "grid", gridTemplateColumns: "1fr 100px 180px" }}
          >
            <div className="nm">
              <Ico name="disc" className="sm" />
              <span className="t">{s.fileName}</span>
            </div>
            <div className="num">{formatSize(s.sizeBytes)}</div>
            <div>
              {s.modifiedAt != null
                ? new Date(s.modifiedAt * 1000).toLocaleString("fr-FR")
                : "—"}
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}
