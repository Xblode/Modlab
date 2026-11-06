import { useMinecraftContent } from "../../hooks/useMinecraftContent";
import { Ico } from "../../Ico";

/**
 * Servers. The Core can see whether `servers.dat` exists, and nothing more:
 * that file is binary NBT and ModLab has no parser for it. So this says
 * exactly that, rather than rendering an empty list a human would read as
 * "you have no servers saved".
 */
export default function ServersPage() {
  const { content, error } = useMinecraftContent();

  const body = error
    ? error
    : content === null
      ? "Lecture de l’instance…"
      : content.serversDatPresent
        ? "Un servers.dat existe dans cette instance, mais ModLab ne sait pas encore le lire : c’est du NBT binaire, et aucun lecteur NBT n’est branché. Afficher une liste vide laisserait croire que tu n’as aucun serveur enregistré."
        : "Aucun servers.dat dans cette instance — aucun serveur n’y a encore été enregistré par le jeu.";

  return (
    <div className="content">
      <div className="empty">
        <div>
          <Ico name="info" />
          <h3>Serveurs — lecture NBT pas encore branchée</h3>
          <p>{body}</p>
        </div>
      </div>
    </div>
  );
}
