import { getLoadoutPlugins } from "../../api";
import { DirectoryList } from "../../components/Features/Directory/DirectoryList";

/**
 * Every `.esp`/`.esm`/`.esl` the enabled mods provide, resolved to whichever
 * mod wins when more than one provides the same file. Mod-position order,
 * not a validated LOOT load order, and no missing-master detection — that
 * still doesn't exist (see `docs`/ClickUp for the tracked follow-up).
 */
export default function PluginsPage({ loadoutId }: { loadoutId: number | null }) {
  return (
    <DirectoryList
      loadoutId={loadoutId}
      fetchEntries={getLoadoutPlugins}
      emptyTitle="Aucun plugin"
      emptyBody="Aucun .esp/.esm/.esl trouvé dans les mods activés de cette instance."
    />
  );
}
