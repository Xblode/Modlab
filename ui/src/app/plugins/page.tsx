import { useEffect, useState } from "react";
import { getLoadoutPlugins } from "../../api";
import type { PluginIssue, PluginRow, PluginsView } from "../../api";
import { Ico } from "../../Ico";

const COLUMNS = "44px 1fr 200px 150px 90px";

function issueText(i: PluginIssue, view: PluginsView): string {
  switch (i.kind) {
    case "missing_master":
      return `${i.plugin} : master manquant « ${i.detail} » — désactivé`;
    case "inactive_master":
      return `${i.plugin} : désactivé car son master « ${i.detail} » est désactivé`;
    case "unreadable":
      return `${i.plugin} : en-tête illisible — désactivé`;
    case "master_cycle":
      return `${i.plugin} : dépendance circulaire entre masters, aucun ordre valide`;
    case "master_after_non_master":
      return `${i.plugin} : master chargé après un plugin non-master`;
    case "full_limit":
      return `${i.plugin} : limite de ${view.fullLimit} plugins actifs dépassée — désactivé`;
    case "light_limit":
      return `${i.plugin} : limite de ${view.lightLimit} plugins ESL actifs dépassée — désactivé`;
    case "unencodable_name":
      return `${i.plugin} : le nom contient des caractères que le jeu ne sait pas lire — désactivé`;
  }
}

function kindBadges(p: PluginRow) {
  return (
    <>
      {p.official && (
        <span className="badge root" title="Plugin officiel : chargé en premier, toujours actif">
          Officiel
        </span>
      )}
      {p.light ? (
        <span className="badge plug" title="Plugin léger (ESL)">
          ESL
        </span>
      ) : p.master ? (
        <span className="badge plug" title="Plugin maître : chargé avant les autres">
          Maître
        </span>
      ) : null}
    </>
  );
}

function PluginLine({ index, p }: { index: number; p: PluginRow }) {
  return (
    <div
      className={p.active ? "row" : "row off"}
      style={{ display: "grid", gridTemplateColumns: COLUMNS }}
    >
      <div className="pri">{index}</div>
      <div className="nm">
        <span className="t mono">{p.fileName}</span>
        {p.missingMasters.map((m) => (
          <span key={m} className="badge bad" title="Ce master n'est fourni par aucun mod activé">
            <Ico name="alert" />
            manque {m}
          </span>
        ))}
      </div>
      <div>{p.official ? "Jeu de base" : (p.modName ?? "Dossier Data du jeu")}</div>
      <div style={{ display: "flex", gap: 4, alignItems: "center" }}>{kindBadges(p)}</div>
      <div>
        {p.active ? (
          <span title="Écrit avec « * » dans plugins.txt (les officiels sont actifs d'office)">
            Actif
          </span>
        ) : (
          <span className="badge upd" title="Ce plugin ne sera pas chargé — voir les alertes">
            Inactif
          </span>
        )}
      </div>
    </div>
  );
}

/**
 * The load order ModLab gives the game at launch: official plugins first,
 * then the enabled mods' plugins, masters before the plugins that need them.
 * Computed by the Skyrim adapter (the same code writes plugins.txt). No LOOT
 * sorting and no manual reordering yet — read-only.
 */
export default function PluginsPage({ loadoutId }: { loadoutId: number | null }) {
  const [view, setView] = useState<PluginsView | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (loadoutId == null) {
      setView(null);
      return;
    }
    let cancelled = false;
    setView(null);
    setError(null);
    getLoadoutPlugins(loadoutId)
      .then((v) => {
        if (!cancelled) setView(v);
      })
      .catch((e: unknown) => {
        if (!cancelled) setError(e instanceof Error ? e.message : String(e));
      });
    return () => {
      cancelled = true;
    };
  }, [loadoutId]);

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

  if (loadoutId == null || view === null) {
    return (
      <div className="content">
        <div className="empty">
          <div>
            <Ico name="info" />
            <h3>{loadoutId == null ? "Aucun plugin" : "Lecture en cours…"}</h3>
          </div>
        </div>
      </div>
    );
  }

  if (view.plugins.length === 0) {
    return (
      <div className="content">
        <div className="empty">
          <div>
            <Ico name="info" />
            <h3>Aucun plugin</h3>
            <p>Aucun .esp/.esm/.esl trouvé dans les mods activés de cette instance.</p>
          </div>
        </div>
      </div>
    );
  }

  return (
    <div className="content" style={{ overflow: "auto" }}>
      <div style={{ padding: "8px 12px", fontSize: 12, color: "var(--t2)" }}>
        <div>
          {view.activeFull} plugins actifs sur {view.fullLimit} · {view.activeLight} ESL sur{" "}
          {view.lightLimit} — ordre de chargement envoyé au jeu au lancement.
        </div>
        {view.issues.map((i, n) => (
          <div key={`${i.kind}-${i.plugin}-${n}`} style={{ color: "var(--warn)", marginTop: 4 }}>
            <Ico name="alert" /> {issueText(i, view)}
          </div>
        ))}
      </div>
      <div className="grid" style={{ display: "block" }}>
        <div
          className="gh"
          style={{
            display: "grid",
            gridTemplateColumns: COLUMNS,
            position: "sticky",
            top: 0,
            zIndex: 2,
            background: "var(--c2)",
          }}
        >
          <div>N°</div>
          <div>Plugin</div>
          <div>Mod</div>
          <div>Type</div>
          <div>État</div>
        </div>
        {view.plugins.map((p, n) => (
          <PluginLine key={p.fileName} index={n} p={p} />
        ))}
      </div>
    </div>
  );
}
