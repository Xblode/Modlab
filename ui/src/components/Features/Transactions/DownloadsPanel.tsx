import type { DetectedGame, Download, LoadoutDto } from "../../../api";
import { Ico } from "../../../Ico";
import { IconButton } from "../../ui";
import type { BottomTab } from "./BottomTab";

function formatSize(bytes: number | null): string {
  if (bytes == null) return "—";
  if (bytes < 1024) return `${bytes} o`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(0)} Ko`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} Mo`;
}

type Group = {
  key: string;
  label: string;
  downloads: Download[];
};

/** Downloads grouped by loadout (game short name · instance name), newest
 * first within each group; unfiled captures (no active instance when they
 * landed) get their own "Non classés" group at the top. */
function groupByLoadout(
  downloads: Download[],
  loadouts: LoadoutDto[],
  games: DetectedGame[],
): Group[] {
  const unfiled: Download[] = [];
  const byLoadout = new Map<number, Download[]>();
  for (const d of downloads) {
    if (d.loadoutId == null) {
      unfiled.push(d);
      continue;
    }
    const list = byLoadout.get(d.loadoutId) ?? [];
    list.push(d);
    byLoadout.set(d.loadoutId, list);
  }

  const groups: Group[] = [];
  if (unfiled.length > 0) {
    groups.push({ key: "unfiled", label: "Non classés", downloads: unfiled });
  }
  for (const [loadoutId, list] of byLoadout) {
    const loadout = loadouts.find((l) => l.id === loadoutId);
    const game = games.find((g) => g.id === loadout?.gameId);
    const label = loadout
      ? `${game?.short ?? loadout.gameId} · ${loadout.name}`
      : `Instance #${loadoutId}`;
    groups.push({ key: String(loadoutId), label, downloads: list });
  }
  return groups;
}

/**
 * The bottom panel's "Téléchargements" tab: every capture the browser has
 * recorded, grouped by which instance it landed in. Clicking a grouped
 * download's instance name jumps there — the same "open this instance's
 * Mods table" action the sidebar and dashboard already use.
 */
export function DownloadsPanel({
  downloads,
  loadouts,
  games,
  onClose,
  onBottomTabChange,
  onOpenLoadout,
}: {
  downloads: Download[] | null;
  loadouts: LoadoutDto[];
  games: DetectedGame[];
  onClose: () => void;
  onBottomTabChange: (tab: BottomTab) => void;
  onOpenLoadout: (loadoutId: number) => void;
}) {
  const groups = downloads ? groupByLoadout(downloads, loadouts, games) : [];

  return (
    <div className="tpanel open" role="dialog" aria-label="Panneau du bas">
      <div className="tp-bar">
        <button
          className="tab"
          type="button"
          aria-selected="false"
          onClick={() => onBottomTabChange("transactions")}
        >
          <Ico name="history" className="sm" />
          <span>Transactions</span>
        </button>
        <button className="tab" type="button" aria-selected="true">
          <Ico name="download" className="sm" />
          <span>Téléchargements</span>
          <span className="cnt">{downloads?.length ?? 0}</span>
        </button>
        <span className="spacer" />
        <IconButton name="x" title="Fermer (Ctrl+J)" onClick={onClose} />
      </div>
      <div className="tp-body">
        {downloads && downloads.length === 0 ? (
          <div className="hint" style={{ border: 0 }}>
            Aucun téléchargement capturé pour l'instant — ouvre le navigateur
            intégré et clique un lien de téléchargement.
          </div>
        ) : null}
        {groups.map((group) => (
          <div key={group.key} className="tx">
            <div className="t">
              <Ico name="folder" className="sm" />
              {group.key === "unfiled" ? (
                group.label
              ) : (
                <button
                  type="button"
                  className="lo-name"
                  style={{ font: "inherit" }}
                  onClick={() => onOpenLoadout(Number(group.key))}
                  title="Ouvrir cette instance"
                >
                  {group.label}
                </button>
              )}
            </div>
            <div className="d">
              {group.downloads.map((d) => (
                <div key={d.id} style={{ display: "flex", gap: 8, alignItems: "center" }}>
                  <Ico name="box" className="sm" />
                  <span style={{ flex: 1 }}>{d.fileName}</span>
                  <span className="muted">{formatSize(d.sizeBytes)}</span>
                  <span className="muted">
                    {new Date(d.capturedAt).toLocaleString("fr-FR")}
                  </span>
                </div>
              ))}
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}
