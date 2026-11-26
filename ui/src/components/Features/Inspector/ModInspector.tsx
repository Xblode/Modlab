import {
  CONFLICT_PRIORITY_CAVEAT,
  type DependencyKind,
  type GameCategory,
  type ModConflicts,
  type ModRow,
  type ResolvedDependency,
} from "../../../api";
import { CategoryPicker } from "../Mods/CategoryPicker";
import {
  dependencyKindLabel,
  formatDependencyLine,
} from "../Mods/DependencyBadge";
import { formatBytes, formatExactBytes } from "../../../formatBytes";
import { formatDateTime } from "../../../format";
import { Ico } from "../../../Ico";
import { modSourceDisplay } from "../../../modSource";
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
  dependencies,
  onOpenPage,
  catalog,
  onSetCategory,
}: {
  mod: ModRow | null;
  conflicts: ModConflicts | undefined;
  /** `undefined` while the branch has not been scanned. */
  dependencies: ResolvedDependency[] | undefined;
  /** Opens `pageUrl` in the embedded Web tab. */
  onOpenPage: (url: string) => void;
  catalog: GameCategory[];
  onSetCategory: (category: string | null) => void;
}) {
  if (!mod) {
    return (
      <div className="rtitle">
        <div className="eyebrow">Panneau</div>
        <h2>Aucun mod sélectionné</h2>
        <p className="muted">Clique une ligne de la table</p>
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
          <span>
            {mod.status === "downloading"
              ? "téléchargement…"
              : mod.enabled
                ? "activé"
                : "désactivé"}
          </span>
        </div>
        <dl className="kv" style={{ marginTop: 12 }}>
          <dt>Chemin</dt>
          <dd>{mod.sourcePath}</dd>
          <dt>Priorité</dt>
          <dd>{mod.position}</dd>
          <dt>Source</dt>
          <dd>
            <SourceLine mod={mod} onOpenPage={onOpenPage} />
          </dd>
          <dt>Version</dt>
          <dd title={mod.version ?? "Version non déclarée par la source"}>
            {mod.version ?? "—"}
          </dd>
          <dt>Taille</dt>
          <dd
            className="num"
            title={mod.sizeBytes == null ? "Taille inconnue" : formatExactBytes(mod.sizeBytes)}
          >
            {mod.sizeBytes == null ? "—" : formatBytes(mod.sizeBytes)}
          </dd>
          <dt>Téléchargé le</dt>
          <dd>{downloadedFull(mod)}</dd>
          <dt>Installé le</dt>
          <dd>{installedFull(mod)}</dd>
          <dt>Versions du jeu</dt>
          <dd>{joined(mod.gameVersions)}</dd>
          <dt>Catégorie</dt>
          <dd>
            <CategoryPicker
              stored={mod.category}
              userSet={mod.categoryUserSet}
              catalog={catalog}
              onPick={onSetCategory}
            />
          </dd>
          {mod.author?.trim() ? (
            <>
              <dt>Auteur</dt>
              <dd>{mod.author}</dd>
            </>
          ) : null}
          {mod.website?.trim() ? (
            <>
              <dt>Site</dt>
              <dd>
                <button type="button" title={mod.website} onClick={() => onOpenPage(mod.website!)}>
                  {mod.website}
                </button>
              </dd>
            </>
          ) : null}
          {mod.categoryRaw?.trim() ? (
            <>
              <dt>Catégorie brute</dt>
              <dd>{mod.categoryRaw}</dd>
            </>
          ) : null}
          {mod.categoryNexusId?.trim() ? (
            <>
              <dt>Id Nexus</dt>
              <dd className="mono">{mod.categoryNexusId}</dd>
            </>
          ) : null}
          <dt>Dépendances</dt>
          <dd>
            <DependenciesTable declared={mod.dependencies} resolved={dependencies} />
          </dd>
        </dl>
      </div>
      {conflicts ? (
        <Accordion
          variant="rsec"
          title="Conflits"
          count={conflicts.wins + conflicts.loses}
        >
          <div style={{ fontSize: 12 }}>
            {conflicts.wins > 0 ? (
              <div style={{ display: "flex", gap: 8, padding: "4px 0" }}>
                <Badge tone="win" title={CONFLICT_PRIORITY_CAVEAT}>
                  <Ico name="flag" />
                  {conflicts.wins}
                </Badge>
                <span>gagne sur des mods plus haut</span>
              </div>
            ) : null}
            {conflicts.loses > 0 ? (
              <div style={{ display: "flex", gap: 8, padding: "4px 0" }}>
                <Badge tone="lose" title={CONFLICT_PRIORITY_CAVEAT}>
                  <Ico name="flag" />
                  {conflicts.loses}
                </Badge>
                <span>perd face à des mods plus bas</span>
              </div>
            ) : null}
            {conflicts.wins === 0 && conflicts.loses === 0 ? (
              <span className="muted">Aucun conflit de fichier</span>
            ) : null}
          </div>
        </Accordion>
      ) : null}
    </>
  );
}

function downloadedFull(mod: ModRow): string {
  if (!mod.downloadedAt) return "—";
  return formatDateTime(mod.downloadedAt) ?? "—";
}

function installedFull(mod: ModRow): string {
  if (mod.status !== "installed" || !mod.installedAt) return "—";
  return formatDateTime(mod.installedAt) ?? "—";
}

function joined(values: string[]): string {
  const shown = values.map((value) => value.trim()).filter((value) => value.length > 0);
  return shown.length > 0 ? shown.join(", ") : "—";
}

const DEP_SECTIONS: DependencyKind[] = [
  "required",
  "optional",
  "incompatible",
  "discouraged",
  "usedBy",
];

function DependenciesTable({
  declared,
  resolved,
}: {
  declared: string[];
  resolved: ResolvedDependency[] | undefined;
}) {
  if (!resolved || resolved.length === 0) {
    return <>{formatDependencyLine(declared, resolved)}</>;
  }
  return (
    <div className="dep-table">
      {DEP_SECTIONS.map((kind) => {
        const rows = resolved.filter((dep) => (dep.kind ?? "required") === kind);
        if (rows.length === 0) return null;
        return (
          <div key={kind} className="dep-table-sec">
            <div className="dep-table-h">{sectionTitle(kind)}</div>
            <table>
              <tbody>
                {rows.map((dep) => (
                  <tr key={`${kind}:${dep.name}:${dep.providerModId ?? ""}`}>
                    <td>
                      {dep.name}
                      {dep.providerName && dep.providerName !== dep.name ? (
                        <span className="muted"> ({dep.providerName})</span>
                      ) : null}
                    </td>
                    <td className="dep-table-role">{dependencyKindLabel(kind)}</td>
                    <td className="dep-table-state" title={stateTitle(dep)}>
                      <DepStateIcon dep={dep} />
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        );
      })}
    </div>
  );
}

function sectionTitle(kind: DependencyKind): string {
  switch (kind) {
    case "required":
      return "Requises";
    case "optional":
      return "Optionnelles";
    case "incompatible":
      return "Incompatibles";
    case "discouraged":
      return "Déconseillées";
    case "usedBy":
      return "Utilisé par";
  }
}

function stateTitle(dep: ResolvedDependency): string {
  switch (dep.state) {
    case "active":
      return "active";
    case "disabled":
      return "inactive";
    case "missing":
      return "manquante";
  }
}

function DepStateIcon({ dep }: { dep: ResolvedDependency }) {
  if (dep.state === "active") {
    return <Ico name="check" />;
  }
  if (dep.state === "disabled") {
    return <Ico name="x" className="muted" />;
  }
  return <Ico name="x" />;
}

/** Host name, linked only when a page was stored. No page means plain text. */
function SourceLine({
  mod,
  onOpenPage,
}: {
  mod: ModRow;
  onOpenPage: (url: string) => void;
}) {
  const source = modSourceDisplay(mod);
  const page = mod.pageUrl?.trim() ?? "";
  if (!page) return source.label;
  return (
    <button type="button" title={page} onClick={() => onOpenPage(page)}>
      {source.label}
    </button>
  );
}
