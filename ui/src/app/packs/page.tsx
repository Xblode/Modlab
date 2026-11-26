import { InstanceFolderTable } from "../../components/Features/Minecraft/InstanceFolderTable";
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
    <InstanceFolderTable
      sections={[
        {
          title: "Resource packs",
          icon: "image",
          names: content?.resourcePacks ?? null,
          emptyTitle: "Aucun resource pack",
          emptyHint: "Rien dans resourcepacks/",
        },
        {
          title: "Shader packs",
          icon: "sparkle",
          names: content?.shaderPacks ?? null,
          emptyTitle: "Aucun shader pack",
          emptyHint: "Rien dans shaderpacks/",
        },
      ]}
    />
  );
}
