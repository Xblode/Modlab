import { KIND_LABELS, type Transaction } from "../../../api";
import { Ico } from "../../../Ico";
import { Button, IconButton } from "../../ui";

/**
 * The bottom panel (Ctrl+J): the Core's transaction journal, newest first.
 * `txs === null` means "not loaded", which is why an empty journal and an
 * unloaded one do not print the same thing.
 */
export function TransactionsPanel({
  txs,
  canSnapshot,
  onSnapshot,
  onRollback,
  onClose,
}: {
  txs: Transaction[] | null;
  canSnapshot: boolean;
  onSnapshot: () => void;
  onRollback: () => void;
  onClose: () => void;
}) {
  return (
    <div className="tpanel open" role="dialog" aria-label="Panneau du bas">
      <div className="tp-bar">
        <button className="tab" type="button" aria-selected="true">
          <Ico name="history" className="sm" />
          <span>Transactions</span>
          <span className="cnt">{txs?.length ?? 0}</span>
        </button>
        <span className="spacer" />
        <Button variant="sm-ghost" disabled={!canSnapshot} onClick={onSnapshot}>
          <Ico name="shield" className="sm" />
          Instantané
        </Button>
        <IconButton name="x" title="Fermer (Ctrl+J)" onClick={onClose} />
      </div>
      <div className="tp-body">
        {txs && txs.length === 0 ? (
          <div className="hint" style={{ border: 0 }}>
            Aucune mutation journalisée pour l'instant — importe une
            instance ou prends un instantané de l'instance active.
          </div>
        ) : null}
        {(txs ?? []).map((t) => (
          <div key={t.id} className="tx">
            <div className="t">
              <Ico
                name={
                  t.kind === "loadout.snapshot"
                    ? "shield"
                    : t.kind === "loadout.rollback"
                      ? "refresh"
                      : "db"
                }
                className="sm"
              />
              {KIND_LABELS[t.kind] ?? t.kind}
              <span className="muted" style={{ fontWeight: 400 }}>
                {new Date(t.occurredAt).toLocaleString("fr-FR")}
              </span>
            </div>
            <div className="d">
              {t.steps.map((s, i) => (
                <div key={i}>{s}</div>
              ))}
            </div>
            {t.kind === "loadout.snapshot" ? (
              <div className="acts">
                <Button variant="sm-ghost" onClick={onRollback}>
                  <Ico name="refresh" className="sm" />
                  Restaurer
                </Button>
              </div>
            ) : null}
          </div>
        ))}
      </div>
    </div>
  );
}
