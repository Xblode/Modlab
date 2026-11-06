import { Crumbs, Panel } from "../../ui";

const PATCHES: readonly {
  mod: string;
  author: string;
  summary: string;
  diff: string;
}[] = [
  {
    mod: "Unofficial Skyrim SE Patch",
    author: "arthmoor",
    summary: "corrige un conflit de navmesh avec Whiterun Extended",
    diff: "- NavMesh [000812A3]\n+ NavMesh [000812A3] (edge trimmed, cell 12,-4)",
  },
  {
    mod: "Immersive Armors",
    author: "hothtrooper44",
    summary: "réaligne un poids d'armure après une mise à jour de CBBE",
    diff: "- KYWD Weight = 0.0\n+ KYWD Weight = 1.0",
  },
  {
    mod: "ELFX",
    author: "Anamorfus",
    summary: "supprime deux lumières dupliquées dans Dragonsreach",
    diff: "- LIGH DragonsreachHall01 (dup)\n- LIGH DragonsreachHall02 (dup)",
  },
  {
    mod: "SkyUI",
    author: "SkyUI Team",
    summary: "traduction FR manquante pour trois entrées MCM",
    diff: '- $ModName "SkyUI"\n+ $ModName "SkyUI (FR)"',
  },
];

/** Communauté → Atelier de patchs — a static preview of the shared-fix
 * workflow from `docs/deep-research-report.md`. Own page, own nav entry
 * (`CommunityMenu`): not a Réglages subsection. */
export function PatchWorkshopPage({ onUnwired }: { onUnwired: (label: string) => void }) {
  return (
    <>
      <Crumbs path={["Communauté", "Atelier de patchs"]} />
      <div className="settings-page">
        <Panel icon="branch" title="Aperçu, pas encore branché" bodyClassName="why">
          Prévisualisation de l'Atelier de patchs décrit dans le rapport
          communauté — des correctifs partagés, appliqués via la même
          transaction plan → approbation → snapshot → apply → vérification
          que le reste de ModLab. Rien ici n'écrit encore dans le Core.
        </Panel>
        <div className="settings-group">
          <div className="settings-group-title">Patchs disponibles ({PATCHES.length})</div>
          {PATCHES.map((p) => (
            <div className="patch-card" key={p.mod}>
              <div className="patch-card-head">
                <div>
                  <div className="settings-row-title">{p.mod}</div>
                  <div className="settings-row-desc">
                    par {p.author} · {p.summary}
                  </div>
                </div>
                <button
                  type="button"
                  className="btn ghost"
                  title="Pas encore branché au Core"
                  onClick={() => onUnwired(`Appliquer le patch « ${p.mod} »`)}
                >
                  Appliquer
                </button>
              </div>
              <code className="patch-diff">{p.diff}</code>
            </div>
          ))}
        </div>
      </div>
    </>
  );
}
