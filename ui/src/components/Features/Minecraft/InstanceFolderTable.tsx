import { Ico } from "../../../Ico";

/** Same family as Archives / ModTable headers — name, status, minimal actions. */
const COLS = "28px minmax(200px, 1fr) 128px 48px";

export type InstanceFolderSection = {
  title: string;
  icon: string;
  names: string[] | null;
  emptyTitle: string;
  emptyHint: string;
};

/**
 * Directory names from a Minecraft instance folder (`resourcepacks/`,
 * `shaderpacks/`, …) in a scrollable grid. Only what the Core lists — no
 * pack.mcmeta, no “selected in options.txt” yet.
 */
export function InstanceFolderTable({ sections }: { sections: InstanceFolderSection[] }) {
  const loading = sections.some((s) => s.names === null);
  const allEmpty =
    !loading && sections.every((s) => s.names !== null && s.names.length === 0);

  if (allEmpty) {
    const first = sections[0];
    return (
      <div className="content">
        <div className="empty">
          <div>
            <Ico name="image" />
            <h3>{first?.emptyTitle ?? "Rien à afficher"}</h3>
            <p>{first?.emptyHint ?? ""}</p>
          </div>
        </div>
      </div>
    );
  }

  return (
    <div className="content" style={{ overflow: "auto" }}>
      <div className="grid pack-folder-grid" style={{ display: "block", minWidth: 520 }}>
        <div
          className="gh"
          style={{
            display: "grid",
            gridTemplateColumns: COLS,
            position: "sticky",
            top: 0,
            zIndex: 2,
            background: "var(--c2)",
          }}
        >
          <div />
          <div>Nom</div>
          <div>Statut</div>
          <div className="end"> </div>
        </div>
        {sections.map((section) => (
          <SectionBlock key={section.title} section={section} loading={loading} />
        ))}
      </div>
    </div>
  );
}

function SectionBlock({ section, loading }: { section: InstanceFolderSection; loading: boolean }) {
  const names = section.names;
  return (
    <>
      <div
        className="sep"
        style={{
          display: "grid",
          gridTemplateColumns: COLS,
        }}
      >
        <div style={{ gridColumn: "1 / -1" }}>
          <Ico name={section.icon} />
          {section.title}
          {names ? <span className="cnt"> ({names.length})</span> : null}
        </div>
      </div>
      {names === null ? (
        <div
          className="row muted"
          style={{
            display: "grid",
            gridTemplateColumns: COLS,
          }}
        >
          <div />
          <div className="nm" style={{ gridColumn: "2 / -1" }}>
            <span className="t">Lecture du dossier…</span>
          </div>
        </div>
      ) : names.length === 0 ? (
        <div
          className="row"
          style={{
            display: "grid",
            gridTemplateColumns: COLS,
          }}
        >
          <div />
          <div className="nm" style={{ gridColumn: "2 / -1" }}>
            <span className="t">{section.emptyHint}</span>
          </div>
        </div>
      ) : (
        names.map((name) => (
          <div
            key={`${section.title}-${name}`}
            className="row"
            style={{
              display: "grid",
              gridTemplateColumns: COLS,
            }}
          >
            <div className="num">
              <Ico name={section.icon} />
            </div>
            <div className="nm">
              <span className="t">{name}</span>
            </div>
            <div className="src">{loading ? "…" : "Sur disque"}</div>
            <div className="end" />
          </div>
        ))
      )}
    </>
  );
}