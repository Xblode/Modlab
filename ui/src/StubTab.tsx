import { Ico } from "./Ico";
import type { TabId } from "./gameChrome";

/**
 * Honest placeholder for a maquette tab whose data has no Core API yet.
 * Every line here says *why* it is empty rather than faking a screen — the
 * same convention the rest of the shell follows.
 */
const COPY: Partial<Record<TabId, { title: string; body: string }>> = {
  plugins: {
    title: "Plugins indisponibles",
    body: "Le Core n’expose pas encore l’ordre de chargement",
  },
  archives: {
    title: "Archives indisponibles",
    body: "Le Core ne lit pas encore les archives BSA et BA2",
  },
  repertoire: {
    title: "Répertoire indisponible",
    body: "Le Core n’expose pas encore l’arborescence vue par le jeu",
  },
  saves: {
    title: "Sauvegardes indisponibles",
    body: "Le Core ne lit pas encore les fichiers de sauvegarde",
  },
  sante: {
    title: "Santé indisponible",
    body: "Le moteur de diagnostic n’existe pas encore",
  },
  dashboard: {
    title: "Aucun jeu détecté",
    body: "ModLab n’a trouvé aucune installation de jeu",
  },
};

export function StubTab({ tab }: { tab: TabId }) {
  const copy = COPY[tab] ?? {
    title: "Vue indisponible",
    body: "Le Core n’expose pas encore cette vue",
  };
  return (
    <div className="content">
      <div className="empty">
        <div>
          <Ico name="info" />
          <h3>{copy.title}</h3>
          <p>{copy.body}</p>
        </div>
      </div>
    </div>
  );
}
