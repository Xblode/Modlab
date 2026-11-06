import type { ModConflicts, ModRow } from "../../api";
import { ModTable } from "../../components/Features/Mods/ModTable";
import { Button } from "../../components/ui";
import { Ico } from "../../Ico";

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
  conflicts,
  onImportSource,
  onInstallArchived,
  categories,
  onSetCategory,
  otherLoadouts,
  onReassign,
  onRename,
  onDelete,
  onUninstall,
  onRevealFolder,
  onRevealArchive,
}: {
  rows: ModRow[];
  entryCount: number;
  selectedId: number | null;
  onSelect: (id: number) => void;
  onToggle: (row: ModRow) => void;
  onInstallArchived: (row: ModRow) => void;
  categories: string[];
  onSetCategory: (row: ModRow, category: string | null) => void;
  otherLoadouts: { id: number; name: string }[];
  onReassign: (row: ModRow, toLoadoutId: number) => void;
  conflicts: Map<number, ModConflicts> | null;
  /** Same import action as the dashboard header — whichever source the
   * active game's adapter declares (MO2, l'instance Minecraft détectée…). */
  onImportSource: () => void;
  onRename: (row: ModRow, name: string) => void;
  onDelete: (row: ModRow) => void;
  onUninstall: (row: ModRow) => void;
  onRevealFolder: (row: ModRow) => void;
  onRevealArchive: (row: ModRow) => void;
}) {
  if (entryCount === 0) {
    return (
      <div className="content">
        <div className="empty">
          <div>
            <Ico name="box" />
            <h3>Aucun mod dans ce loadout</h3>
            <p>Importe une instance existante pour la remplir.</p>
            <Button variant="sm" onClick={onImportSource} style={{ marginTop: 10 }}>
              <Ico name="download" className="sm" />
              Importer…
            </Button>
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
      onInstallArchived={onInstallArchived}
      categories={categories}
      onSetCategory={onSetCategory}
      otherLoadouts={otherLoadouts}
      onReassign={onReassign}
      conflicts={conflicts}
      onRename={onRename}
      onDelete={onDelete}
      onUninstall={onUninstall}
      onRevealFolder={onRevealFolder}
      onRevealArchive={onRevealArchive}
    />
  );
}
