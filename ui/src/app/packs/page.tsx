import { FolderList } from "../../components/Features/Minecraft/FolderList";
import { useMinecraftContent } from "../../hooks/useMinecraftContent";
import { Ico } from "../../Ico";

/**
 * Resource packs and shader packs of the detected Minecraft instance — real
 * directory listings from the Core, not a mock. What is *not* here: pack
 * metadata (`pack.mcmeta`, format version, compatibility) — that needs the
 * archives actually opened, which is separate work from listing them.
 */
export default function PacksPage() {
  const { content, error } = useMinecraftContent();

  if (error) {
    return (
      <div className="content">
        <div className="empty">
          <div>
            <Ico name="info" />
            <h3>Packs indisponibles</h3>
            <p>{error}</p>
          </div>
        </div>
      </div>
    );
  }

  return (
    <div className="content">
      <div className="dash dpad">
        <FolderList
          title="Resource packs"
          icon="image"
          names={content?.resourcePacks ?? null}
          emptyLabel="Aucun resource pack dans ce dossier."
        />
        <FolderList
          title="Shader packs"
          icon="sparkle"
          names={content?.shaderPacks ?? null}
          emptyLabel="Aucun shader pack dans ce dossier."
        />
      </div>
    </div>
  );
}
