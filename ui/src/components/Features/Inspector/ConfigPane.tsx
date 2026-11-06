import type { ModRow } from "../../../api";
import { Badge } from "../../ui";

/**
 * The right panel's Config tab: where a mod's own `.ini` and in-app settings
 * will eventually live, edited directly instead of leaving the shell.
 * Reading and writing real `.ini` files isn't wired to the Core yet — this
 * is the placeholder every other not-yet-wired screen in Réglages already
 * uses (`Badge tone="root"`), not a dead end.
 */
export function ConfigPane({ mod }: { mod: ModRow | null }) {
  return (
    <div className="rtitle">
      <div className="eyebrow">Config</div>
      {mod ? (
        <>
          <h2>{mod.name}</h2>
          <p className="muted">
            Accès direct au .ini et aux paramètres de ce mod — pas encore branché au Core.
          </p>
        </>
      ) : (
        <>
          <h2>Aucun mod sélectionné</h2>
          <p className="muted">Clique une ligne de la table.</p>
        </>
      )}
      <div style={{ marginTop: 12 }}>
        <Badge tone="root">Phase 3</Badge>
      </div>
    </div>
  );
}
