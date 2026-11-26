import { useState, type FormEvent } from "react";
import { Button, Dialog, Input } from "../../ui";

/** A new branch of the instance on screen. Not a new instance. */
export function CreateBranchDialog({
  instanceName,
  onClose,
  onCreate,
}: {
  instanceName: string;
  onClose: () => void;
  onCreate: (name: string) => Promise<void>;
}) {
  const [name, setName] = useState("");
  const [saving, setSaving] = useState(false);
  const [message, setMessage] = useState<string | null>(null);

  async function submit(e: FormEvent) {
    e.preventDefault();
    setSaving(true);
    setMessage(null);
    try {
      await onCreate(name);
    } catch (error) {
      setMessage(error instanceof Error ? error.message : String(error));
    } finally {
      setSaving(false);
    }
  }

  return (
    <Dialog
      as="form"
      titleId="create-branch-title"
      eyebrow="Nouvelle branche"
      title={instanceName}
      onClose={onClose}
      onSubmit={(e) => void submit(e)}
      actions={
        <>
          <Button variant="sm-ghost" onClick={onClose}>
            Annuler
          </Button>
          <Button variant="sm" type="submit" disabled={saving || !name.trim()}>
            {saving ? "Création…" : "Créer la branche"}
          </Button>
        </>
      }
    >
      <label className="modal-label" htmlFor="branch-name">
        Nom de la branche
      </label>
      <Input
        variant="field"
        id="branch-name"
        autoFocus
        maxLength={100}
        placeholder="Ex. essai météo"
        value={name}
        onChange={(e) => setName(e.target.value)}
      />
      <p className="modal-copy">
        La branche part de l’état actuel de cette instance. Elle n’apparaît pas dans les autres
        instances.
      </p>
      {message ? <p className="modal-error">{message}</p> : null}
    </Dialog>
  );
}
