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

  const [title, body] = error
    ? ["Lecture impossible", error]
    : content === null
      ? ["Lecture en cours…", null]
      : content.serversDatPresent
        ? ["Liste des serveurs indisponible", "servers.dat existe, mais ModLab ne sait pas encore le lire"]
        : ["Aucun serveur", "Pas de servers.dat dans cette instance"];

  return (
    <div className="content">
      <div className="empty">
        <div>
          <Ico name="info" />
          <h3>{title}</h3>
          {body ? <p>{body}</p> : null}
        </div>
      </div>
    </div>
  );
}
