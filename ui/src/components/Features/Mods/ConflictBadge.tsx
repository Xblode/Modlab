import { CONFLICT_PRIORITY_CAVEAT, type ModConflicts } from "../../../api";
import { Ico } from "../../../Ico";
import { Badge } from "../../ui";

/**
 * The maquette's `badgeConflict()`: one badge per mod, and the *shape* of the
 * badge is the message — it only wins, it only loses, or both. Counts and
 * wording live in the tooltip; the cell shows the icon only. A mod in no
 * conflict gets no badge at all rather than a "0".
 */
export function ConflictBadge({ conflicts }: { conflicts: ModConflicts | undefined }) {
  if (!conflicts) return null;
  const { wins, loses } = conflicts;
  if (wins > 0 && loses > 0) {
    const tip = `Écrase ${wins} fichier(s), écrasé sur ${loses} — ${CONFLICT_PRIORITY_CAVEAT}`;
    return (
      <Badge tone="both" data-tip={tip} aria-label={tip}>
        <Ico name="zap" />
      </Badge>
    );
  }
  if (wins > 0) {
    const tip = `Écrase ${wins} fichier(s) — ${CONFLICT_PRIORITY_CAVEAT}`;
    return (
      <Badge tone="win" data-tip={tip} aria-label={tip}>
        <Ico name="flag" />
      </Badge>
    );
  }
  if (loses > 0) {
    const tip = `${loses} fichier(s) écrasés — ${CONFLICT_PRIORITY_CAVEAT}`;
    return (
      <Badge tone="lose" data-tip={tip} aria-label={tip}>
        <Ico name="flag" />
      </Badge>
    );
  }
  return null;
}
