import { useState } from "react";
import { Button } from "../atoms/Button";
import { IconButton } from "../atoms/IconButton";

/**
 * A generic yes/no confirmation modal for an action worth a pause before it
 * fires — same `modal-backdrop`/`modal` markup as `CreateLoadoutDialog` and
 * `Mo2ImportDialog`, just without a form.
 */
export function ConfirmDialog({
  title,
  body,
  confirmLabel = "Confirmer",
  danger,
  onConfirm,
  onClose,
}: {
  title: string;
  body: string;
  confirmLabel?: string;
  /** Styles the confirm button as a destructive action (red). */
  danger?: boolean;
  onConfirm: () => void | Promise<void>;
  onClose: () => void;
}) {
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState<string | null>(null);

  async function confirm() {
    setBusy(true);
    setMessage(null);
    try {
      await onConfirm();
      onClose();
    } catch (error) {
      setMessage(error instanceof Error ? error.message : String(error));
      setBusy(false);
    }
  }

  return (
    <div className="modal-backdrop" role="presentation" onMouseDown={onClose}>
      <div
        className="modal"
        role="alertdialog"
        aria-labelledby="confirm-dialog-title"
        onMouseDown={(e) => e.stopPropagation()}
      >
        <div className="modal-head">
          <h2 id="confirm-dialog-title">{title}</h2>
          <IconButton name="x" title="Fermer" onClick={onClose} />
        </div>
        <p className="modal-copy">{body}</p>
        {message ? <p className="modal-error">{message}</p> : null}
        <div className="modal-actions">
          <Button variant="sm-ghost" onClick={onClose}>
            Annuler
          </Button>
          <Button
            variant="sm"
            className={danger ? "danger" : undefined}
            disabled={busy}
            onClick={() => void confirm()}
          >
            {busy ? "…" : confirmLabel}
          </Button>
        </div>
      </div>
    </div>
  );
}
