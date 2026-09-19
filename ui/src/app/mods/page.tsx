import type { ModConflicts, ModRow } from "../../api";
import { ModTable } from "../../components/Features/Mods/ModTable";
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
}: {
  rows: ModRow[];
  entryCount: number;
  selectedId: number | null;
  onSelect: (id: number) => void;
  onToggle: (row: ModRow) => void;
  conflicts: Map<number, ModConflicts> | null;
}) {
  if (entryCount === 0) {
    return (
      <div className="content">
        <div className="empty">
          <div>
            <Ico name="box" />
            <h3>Aucun mod dans ce loadout</h3>
            <p>
              Importe une instance MO2 via la CLI (`modlab mo2-import`) — l’UI
              lit le même Core.
            </p>
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
      conflicts={conflicts}
    />
  );
}
