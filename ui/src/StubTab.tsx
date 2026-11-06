import { Ico } from "./Ico";
import type { TabId } from "./gameChrome";

/**
 * Honest placeholder for a maquette tab whose data has no Core API yet.
 * Every line here says *why* it is empty rather than faking a screen — the
 * same convention the rest of the shell follows.
 */
const COPY: Partial<Record<TabId, { title: string; body: string }>> = {
  plugins: {
    title: "Plugins — pas encore branché au Core",
    body: "L’ordre de chargement (ESM/ESP/ESL), les masters manquants et le tri LOOT vivent dans l’adaptateur Skyrim, qui ne les expose pas encore au Core. Rien n’est lu ici pour l’instant.",
  },
  archives: {
    title: "Archives — pas encore branché au Core",
    body: "Le Core ne lit pas encore les archives Bethesda (BSA/BA2) : ni inventaire, ni vérification d’intégrité, ni association archive → mod. L’onglet existe pour garder sa place dans la maquette.",
  },
  repertoire: {
    title: "Répertoire — pas encore branché au Core",
    body: "Le déploiement USVFS fonctionne, mais le Core n’expose pas encore l’arborescence fusionnée que le jeu voit réellement dans Data/. Afficher un faux arbre serait pire que rien.",
  },
  saves: {
    title: "Sauvegardes — pas encore branché au Core",
    body: "Aucune lecture des fichiers de sauvegarde : le Core ne sait pas encore associer une sauvegarde à la révision qui tournait au moment où elle a été écrite.",
  },
  sante: {
    title: "Santé — pas encore branché au Core",
    body: "Le moteur de règles de diagnostic n’existe pas encore. La détection de conflits de fichiers, elle, est réelle côté Core — elle remontera ici quand l’API sera exposée.",
  },
};

export function StubTab({ tab }: { tab: TabId }) {
  const copy = COPY[tab] ?? {
    title: "Onglet pas encore branché au Core",
    body: "Cette vue existe dans la maquette mais n’a pas encore d’API côté Core.",
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
