import { useState } from "react";
import { Button } from "../atoms/Button";
import { Dialog } from "./Dialog";

/**
 * A generic yes/no confirmation modal for an action worth a pause before it
 * fires. The shell is `Dialog`; this only owns the confirm/cancel actions.
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
    <Dialog
      titleId="confirm-dialog-title"
      title={title}
      role="alertdialog"
      onClose={onClose}
      actions={
        <>
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
        </>
      }
    >
      <p className="modal-copy">{body}</p>
      {message ? <p className="modal-error">{message}</p> : null}
    </Dialog>
  );
}
