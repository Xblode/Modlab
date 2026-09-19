import type { DetectedGame } from "../../../api";
import { Ico } from "../../../Ico";
import { Crumbs, Panel } from "../../ui";

export type SettingsMenu = "general" | "notifications" | "models" | "games";

/**
 * Settings detail surface, shown in the central content zone while the side
 * toolbar is in settings mode. One pane per "Side Menu" entry; only what the
 * app actually knows is shown interacted with, everything else is an honest,
 * Core-labelled placeholder.
 */
export function SettingsPane({
  menu,
  games,
  llm,
  llmModels,
  onSelectModel,
}: {
  menu: SettingsMenu;
  games: DetectedGame[];
  llm: string;
  llmModels: readonly (readonly [string, string, string])[];
  onSelectModel: (id: string, name: string) => void;
}) {
  if (menu === "models") {
    return (
      <>
        <Crumbs path={["Réglages", "Modèles"]} />
        <div className="content">
          <div className="dash">
            <div className="dh">Modèle local actif</div>
            {llmModels.map(([id, name, sub]) => (
              <button
                key={id}
                className={`tr ${llm === id ? "sel" : ""}`}
                type="button"
                onClick={() => onSelectModel(id, name)}
                title={sub}
              >
                <Ico name={llm === id ? "check" : "box"} />
                <span className="lbl">
                  {name}
                  <div className="muted" style={{ fontSize: 11 }}>
                    {sub}
                  </div>
                </span>
              </button>
            ))}
            <Panel
              icon="info"
              title="Téléchargement et chargement à la demande"
              bodyClassName="why"
              style={{ marginTop: 22 }}
            >
              Pas encore branchés au Core : le modèle se choisira ici, le
              téléchargement et le chargement resteront visibles dans ce
              panneau.
            </Panel>
          </div>
        </div>
      </>
    );
  }
  if (menu === "games") {
    return (
      <>
        <Crumbs path={["Réglages", "Jeux"]} />
        <div className="content">
          <div className="dash">
            <div className="dh">Jeux détectés ({games.length})</div>
            {games.length === 0 ? (
              <div className="tr muted" style={{ fontStyle: "italic" }}>
                Aucun jeu détecté par les adaptateurs.
              </div>
            ) : (
              games.map((g) => (
                <div key={g.id} className="tr-row">
                  <span className="tr-label">
                    <Ico name="layers" />
                    <span className="lbl">{g.name}</span>
                    <span className="tail">
                      <span className="muted" style={{ fontSize: 10 }}>
                        {g.short}
                      </span>
                    </span>
                  </span>
                </div>
              ))
            )}
            <div className="dh">Ajouter un jeu</div>
            <div className="tr muted" style={{ fontStyle: "italic" }}>
              Requête Core : l’ajout manuel d’un dossier de jeu n’existe pas
              encore.
            </div>
          </div>
        </div>
      </>
    );
  }
  if (menu === "notifications") {
    return (
      <>
        <Crumbs path={["Réglages", "Notifications"]} />
        <div className="content">
          <div className="dash">
            <div className="dh">Notifications</div>
            <div className="tr muted" style={{ fontStyle: "italic" }}>
              Pas encore suivi par le Core : fins de job, mises à jour de mods
              et santé des instances arriveront ici.
            </div>
          </div>
        </div>
      </>
    );
  }
  return (
    <>
      <Crumbs path={["Réglages", "Général"]} />
      <div className="content">
        <div className="dash">
          <div className="dh">Général</div>
          <div className="tr muted" style={{ fontStyle: "italic" }}>
            Préférences générales : pas encore suivi par le Core.
          </div>
        </div>
      </div>
    </>
  );
}
