import { getLoadoutArchives } from "../../api";
import { DirectoryList } from "../../components/Features/Directory/DirectoryList";

/**
 * Every `.bsa`/`.ba2` the enabled mods provide. Which files exist and which
 * mod provides each — not an inventory of what's packed inside them, and no
 * integrity check: the Core doesn't parse Bethesda archives yet.
 */
export default function ArchivesPage({ loadoutId }: { loadoutId: number | null }) {
  return (
    <DirectoryList
      loadoutId={loadoutId}
      fetchEntries={getLoadoutArchives}
      emptyTitle="Aucune archive BSA/BA2"
      emptyBody="Aucun .bsa/.ba2 trouvé dans les mods activés de cette instance."
    />
  );
}
