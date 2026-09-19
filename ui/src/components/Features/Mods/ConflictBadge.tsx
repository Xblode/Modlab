import { CONFLICT_PRIORITY_CAVEAT, type ModConflicts } from "../../../api";
import { Ico } from "../../../Ico";
import { Badge } from "../../ui";

/**
 * The maquette's `badgeConflict()`: one badge per mod, and the *shape* of the
 * badge is the message — it only wins, it only loses, or both, in which case
 * the two counts are shown side by side. A mod in no conflict gets no badge
 * at all rather than a "0".
 */
export function ConflictBadge({ conflicts }: { conflicts: ModConflicts | undefined }) {
  if (!conflicts) return null;
  const { wins, loses } = conflicts;
  if (wins > 0 && loses > 0) {
    return (
      <Badge
        tone="both"
        title={`Écrase ${wins} fichier(s), écrasé sur ${loses} — ${CONFLICT_PRIORITY_CAVEAT}`}
      >
        <Ico name="zap" />
        {wins}/{loses}
      </Badge>
    );
  }
  if (wins > 0) {
    return (
      <Badge tone="win" title={`Écrase ${wins} fichier(s) — ${CONFLICT_PRIORITY_CAVEAT}`}>
        <Ico name="flag" />
        {wins}
      </Badge>
    );
  }
  if (loses > 0) {
    return (
      <Badge tone="lose" title={`${loses} fichier(s) écrasés — ${CONFLICT_PRIORITY_CAVEAT}`}>
        <Ico name="flag" />
        {loses}
      </Badge>
    );
  }
  return null;
}
