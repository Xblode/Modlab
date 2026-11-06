import { useEffect, useState } from "react";
import {
  CONFLICT_PRIORITY_CAVEAT,
  getModDependencies,
  type ModConflicts,
  type ModDependency,
  type ModRow,
} from "../../../api";
import { Ico } from "../../../Ico";
import { Accordion, Badge } from "../../ui";

/**
 * The right panel's Info tab: what the Core knows about the selected mod.
 *
 * `conflicts` is `undefined` for a mod the scan did not cover — a disabled
 * one, or one whose folder failed to read. The panel then stays quiet about
 * conflicts instead of claiming the mod has none.
 */
export function ModInspector({
  mod,
  conflicts,
  loadoutId,
}: {
  mod: ModRow | null;
  conflicts: ModConflicts | undefined;
  loadoutId: number | null;
}) {
  const [deps, setDeps] = useState<ModDependency[]>([]);

  useEffect(() => {
    if (!mod || loadoutId == null) {
      setDeps([]);
      return;
    }
    let cancelled = false;
    getModDependencies(loadoutId, mod.modId)
      .then((d) => {
        if (!cancelled) setDeps(d);
      })
      .catch(() => {
        if (!cancelled) setDeps([]);
      });
    return () => {
      cancelled = true;
    };
  }, [mod, loadoutId]);

  if (!mod) {
    return (
      <div className="rtitle">
        <div className="eyebrow">Panneau</div>
        <h2>Aucun mod sélectionné</h2>
        <p className="muted">Clique une ligne de la table.</p>
      </div>
    );
  }
  return (
    <>
      <div className="rtitle">
        <div className="eyebrow">Mod</div>
        <h2>{mod.name}</h2>
        <div className="line">
          <span className="mono">#{mod.modId}</span>
          <span>{mod.enabled ? "activé" : "désactivé"}</span>
        </div>
        <dl className="kv" style={{ marginTop: 12 }}>
          <dt>Chemin</dt>
          <dd>{mod.sourcePath}</dd>
          <dt>Priorité</dt>
          <dd>{mod.position}</dd>
          <dt>Version</dt>
          <dd title={mod.version ?? "Version non déclarée par la source"}>
            {mod.version ?? "—"}
          </dd>
          <dt>Version de jeu compatible</dt>
          <dd title="Pas encore de source pour cette info — aucun Provider (Nexus, CurseForge) branché">
            —
          </dd>
        </dl>
      </div>
      {deps.length > 0 ? (
        <Accordion variant="rsec" title="Dépendances" count={deps.length}>
          <div style={{ fontSize: 12, display: "flex", flexDirection: "column", gap: 6 }}>
            {deps.map((d) => (
              <div key={d.name} style={{ display: "flex", gap: 8, alignItems: "center" }}>
                <Badge
                  tone={d.satisfied ? "win" : "bad"}
                  title={
                    d.satisfied
                      ? `${d.name} est activé dans cette instance`
                      : `${d.name} n'est pas activé dans cette instance`
                  }
                >
                  <Ico name={d.satisfied ? "check" : "alert"} />
                  {d.name}
                </Badge>
                <span className="muted">
                  {d.satisfied ? "détecté" : "manquant"}
                </span>
              </div>
            ))}
          </div>
        </Accordion>
      ) : null}
      {conflicts ? (
        <Accordion
          variant="rsec"
          title="Conflits"
          count={conflicts.wins + conflicts.loses}
        >
          <div style={{ fontSize: 12 }}>
            {conflicts.wins > 0 ? (
              <div style={{ display: "flex", gap: 8, padding: "3px 0" }}>
                <Badge tone="win" title={CONFLICT_PRIORITY_CAVEAT}>
                  <Ico name="flag" />
                  {conflicts.wins}
                </Badge>
                <span>gagne sur des mods plus haut</span>
              </div>
            ) : null}
            {conflicts.loses > 0 ? (
              <div style={{ display: "flex", gap: 8, padding: "3px 0" }}>
                <Badge tone="lose" title={CONFLICT_PRIORITY_CAVEAT}>
                  <Ico name="flag" />
                  {conflicts.loses}
                </Badge>
                <span>perd face à des mods plus bas</span>
              </div>
            ) : null}
            {conflicts.wins === 0 && conflicts.loses === 0 ? (
              <span className="muted">Aucun conflit de fichier.</span>
            ) : null}
          </div>
        </Accordion>
      ) : null}
    </>
  );
}
