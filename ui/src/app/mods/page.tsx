import type { GameCategory, ModConflicts, ModRow, ResolvedDependency } from "../../api";
import type { DownloadBytes } from "../../downloadProgress";
import type { BrowseSite } from "../../gameChrome";
import { ModTable } from "../../components/Features/Mods/ModTable";
import { Ico } from "../../Ico";
import { Button } from "../../components/ui";

/**
 * The mod list of the active loadout. `rows` is what the search left; an
 * *empty loadout* is a different thing from *a search that matched nothing*,
 * so only the first one gets the empty state.
 */
export default function ModsPage({
  rows,
  entryCount,
  selectedId,
  onSelect,
  onToggle,
  onReorder,
  gameId,
  conflicts,
  dependencies,
  onImportSource,
  onInstallArchived,
  installingIds,
  downloadProgress,
  catalog,
  otherLoadouts,
  onReassign,
  onRename,
  onDelete,
  onUninstall,
  onRevealFolder,
  onRevealArchive,
  onOpenPage,
  onChat,
  browseSites,
  onOpenSite,
  onOpenInstanceFolder,
  canOpenInstanceFolder,
}: {
  rows: ModRow[];
  entryCount: number;
  selectedId: number | null;
  onSelect: (id: number) => void;
  onToggle: (row: ModRow) => void;
  onReorder: (row: ModRow, to: number) => Promise<void>;
  /** Active game. The table remembers its display sort for this id. */
  gameId: string | null;
  onInstallArchived: (row: ModRow) => void;
  /** Mods whose install is in flight. Not a database status. */
  installingIds: ReadonlySet<number>;
  /** Live byte counts for rows still downloading, keyed by mod id. */
  downloadProgress?: ReadonlyMap<number, DownloadBytes>;
  catalog: GameCategory[];
  otherLoadouts: { id: number; name: string }[];
  onReassign: (row: ModRow, toLoadoutId: number) => void;
  conflicts: Map<number, ModConflicts> | null;
  dependencies: Map<number, ResolvedDependency[]> | null;
  /** Same import action as the dashboard header — whichever source the
   * active game's adapter declares (MO2, l'instance Minecraft détectée…). */
  onImportSource: () => void;
  onRename: (row: ModRow, name: string) => void;
  onDelete: (row: ModRow) => void;
  onUninstall: (row: ModRow) => void;
  onRevealFolder: (row: ModRow) => void;
  onRevealArchive: (row: ModRow) => void;
  onOpenPage: (url: string) => void;
  onChat: (row: ModRow) => void;
  browseSites: readonly BrowseSite[];
  onOpenSite: (url: string) => void;
  onOpenInstanceFolder: () => void;
  canOpenInstanceFolder: boolean;
}) {
  if (entryCount === 0) {
    return (
      <div className="content">
        <div className="empty">
          <div>
            <Ico name="box" />
            <h3>Aucun mod</h3>
            <p>Importe une instance, ouvre un site de mods, ou parcours le dossier de l’instance.</p>
            <div className="empty-actions" role="group" aria-label="Actions">
              <Button variant="action" type="button" onClick={onImportSource}>
                <Ico name="download" className="sm" />
                Importer…
              </Button>
              {browseSites.map((site) => (
                <Button
                  key={site.id}
                  variant="action"
                  type="button"
                  title={`Ouvrir ${site.label}`}
                  onClick={() => onOpenSite(site.url)}
                >
                  {site.label}
                </Button>
              ))}
              {canOpenInstanceFolder ? (
                <Button
                  variant="action"
                  type="button"
                  title="Ouvrir le dossier de l’instance"
                  onClick={onOpenInstanceFolder}
                >
                  <Ico name="folder" className="sm" />
                  Ouvrir le dossier
                </Button>
              ) : null}
            </div>
          </div>
        </div>
      </div>
    );
  }
  return (
    <ModTable
      rows={rows}
      selectedId={selectedId}
      onSelect={onSelect}
      onToggle={onToggle}
      onReorder={onReorder}
      // A search hides rows: an index in a partial list is not a place in the real order.
      canReorder={rows.length === entryCount}
      gameId={gameId}
      onInstallArchived={onInstallArchived}
      installingIds={installingIds}
      downloadProgress={downloadProgress}
      catalog={catalog}
      otherLoadouts={otherLoadouts}
      onReassign={onReassign}
      conflicts={conflicts}
      dependencies={dependencies}
      onRename={onRename}
      onDelete={onDelete}
      onUninstall={onUninstall}
      onRevealFolder={onRevealFolder}
      onRevealArchive={onRevealArchive}
      onOpenPage={onOpenPage}
      onChat={onChat}
    />
  );
}
