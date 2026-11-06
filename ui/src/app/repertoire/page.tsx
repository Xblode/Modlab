import { getLoadoutDirectory } from "../../api";
import { DirectoryList } from "../../components/Features/Directory/DirectoryList";

/**
 * The logical merged view — every path the enabled mods provide, resolved
 * to whichever one wins, computed the same way the conflict scan is rather
 * than by asking a live USVFS session. Close to "what the game sees in
 * Data/", not a live read of it.
 */
export default function RepertoirePage({ loadoutId }: { loadoutId: number | null }) {
  return (
    <DirectoryList
      loadoutId={loadoutId}
      fetchEntries={getLoadoutDirectory}
      emptyTitle="Aucun fichier"
      emptyBody="Aucun mod activé dans cette instance, ou leurs dossiers sont illisibles."
    />
  );
}
