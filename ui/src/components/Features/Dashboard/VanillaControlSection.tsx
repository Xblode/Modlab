import { useEffect, useState } from "react";
import type { VanillaExtra, VanillaScan } from "../../../api";
import { useVanillaControl } from "../../../hooks/useVanillaControl";
import { Button, Dialog } from "../../ui";

/** Game configs the scan keeps apart. Never offered for removal. */
const KEPT_CONFIGS = new Set([
  "skyrim.ini",
  "skyrimprefs.ini",
  "skyrimcustom.ini",
  "controlmap_custom.txt",
]);

type CleanPlan = {
  extras: VanillaExtra[];
  documentsScanned: boolean;
};

function baseName(path: string): string {
  const parts = path.split(/[\\/]/).filter((part) => part.length > 0);
  return parts[parts.length - 1] ?? path;
}

/** Top-level Data names the scan already marked as non-vanilla. */
function cleanableExtras(scan: VanillaScan): VanillaExtra[] {
  return scan.extras.filter((extra) => !KEPT_CONFIGS.has(baseName(extra.path).toLowerCase()));
}

function extraSentence(count: number): string {
  if (count === 0) return "Tout est propre.";
  if (count === 1) return "1 élément hors vanilla détecté.";
  return `${count} éléments hors vanilla détectés.`;
}

/**
 * Summary of the vanilla scan plus a clean plan. The plan lists Data extras
 * only. Nothing here deletes or moves a file.
 */
export function VanillaControlSection({ gameId }: { gameId: string }) {
  const { state, refresh } = useVanillaControl(gameId);
  const [busy, setBusy] = useState(false);
  const [plan, setPlan] = useState<CleanPlan | null>(null);

  useEffect(() => {
    setPlan(null);
  }, [gameId]);

  const ready = state.status === "ready" ? state.scan : null;
  const extras = ready?.scanned ? cleanableExtras(ready) : [];
  const canClean = ready?.scanned === true && extras.length > 0 && !busy;

  let line1 = "Lecture du dossier…";
  let line2: string | null = null;
  let disabledTip = "Lecture du dossier";
  if (state.status === "error") {
    line1 = state.message;
    disabledTip = "Lecture impossible";
  } else if (ready && !ready.scanned) {
    line1 = "Aucun dossier Data n’est connu.";
    line2 = "Le contrôle n’a pas été lancé.";
    disabledTip = "Aucun dossier Data";
  } else if (ready?.scanned) {
    line1 = extraSentence(extras.length);
    if (extras.length > 0) {
      line2 = "Les configs du jeu restent en place.";
    } else if (!ready.documentsScanned) {
      line2 = "Le dossier des configs n’a pas été lu.";
    } else if (ready.unreadable.length > 0) {
      line2 = "Certains chemins n’ont pas pu être lus.";
    }
    if (extras.length === 0) disabledTip = "Rien à nettoyer";
  }
  if (busy) disabledTip = "Lecture du dossier";

  async function openPlan() {
    if (!canClean) return;
    setBusy(true);
    try {
      const view = await refresh();
      if (view.status !== "ready" || !view.scan.scanned) {
        setPlan(null);
        return;
      }
      const next = cleanableExtras(view.scan);
      setPlan(
        next.length > 0
          ? { extras: next, documentsScanned: view.scan.documentsScanned }
          : null,
      );
    } finally {
      setBusy(false);
    }
  }

  return (
    <>
      <div className="dh">Contrôle vanilla</div>
      <div className="vanilla-blurb">
        <p>{line1}</p>
        {line2 ? <p>{line2}</p> : null}
      </div>
      <span className="vanilla-actions" {...(canClean ? {} : { "data-tip": disabledTip })}>
        <Button disabled={!canClean} onClick={() => void openPlan()}>
          Nettoyer
        </Button>
      </span>
      {plan ? <VanillaCleanDialog plan={plan} onClose={() => setPlan(null)} /> : null}
    </>
  );
}

function VanillaCleanDialog({ plan, onClose }: { plan: CleanPlan; onClose: () => void }) {
  const count = plan.extras.length;
  const lead =
    count === 1
      ? "1 élément hors vanilla dans Data serait retiré."
      : `${count} éléments hors vanilla dans Data seraient retirés.`;

  return (
    <Dialog
      titleId="vanilla-clean-title"
      title="Plan de nettoyage"
      role="dialog"
      onClose={onClose}
      actions={
        <Button variant="sm" onClick={onClose}>
          Fermer
        </Button>
      }
    >
        <p className="modal-copy">{lead}</p>
        <ul className="vanilla-plan">
          {plan.extras.map((extra) => (
            <li key={extra.path}>
              <span className="vanilla-plan-path">{extra.path}</span>
              <span className="muted">{extra.kind === "dir" ? "dossier" : "fichier"}</span>
            </li>
          ))}
        </ul>
        <p className="modal-copy">Les configs du jeu restent en place : Skyrim.ini, Prefs et Custom.</p>
        {!plan.documentsScanned ? (
          <p className="modal-copy">Le dossier des configs n’a pas été lu.</p>
        ) : null}
        <p className="modal-copy">
          Aucun fichier n’est modifié. Le Core n’a pas encore de commande pour les déplacer vers la
          corbeille.
        </p>
    </Dialog>
  );
}
