import { FolderList } from "../../components/Features/Minecraft/FolderList";
import { useMinecraftContent } from "../../hooks/useMinecraftContent";
import { Ico } from "../../Ico";

/**
 * The instance's worlds — one folder per world under `saves/`, listed as the
 * Core reads them. What is *not* here: anything inside `level.dat` (seed,
 * game mode, last played, which mods the world was created with). That file
 * is binary NBT and ModLab has no parser for it; showing an empty "last
 * played" would be a claim, not a blank.
 */
export default function WorldsPage() {
  const { content, error } = useMinecraftContent();

  if (error) {
    return (
      <div className="content">
        <div className="empty">
          <div>
            <Ico name="info" />
            <h3>Mondes indisponibles</h3>
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
          title="Mondes"
          icon="map"
          names={content?.worlds ?? null}
          emptyLabel="Aucun monde dans saves/."
        />
        <div className="dh">Détails d’un monde</div>
        {/* Not a `.tr` row: those fade long text out at the right edge,
            which is right for a mod name and wrong for a sentence. */}
        <p className="muted" style={{ margin: "0 2px", fontSize: 12.5, maxWidth: "62ch" }}>
          Graine, mode de jeu et dernière session vivent dans level.dat (NBT
          binaire) : pas de lecteur NBT dans ModLab pour l’instant.
        </p>
      </div>
    </div>
  );
}
