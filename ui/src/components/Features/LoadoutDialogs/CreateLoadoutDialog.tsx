import { useState, type FormEvent } from "react";
import { Button, IconButton, Input } from "../../ui";

/** A new, empty instance for the active game. The Core owns the name rules. */
export function CreateLoadoutDialog({
  gameName,
  onClose,
  onCreate,
}: {
  gameName: string;
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
    <div className="modal-backdrop" role="presentation" onMouseDown={onClose}>
      <form className="modal" aria-labelledby="create-loadout-title" onSubmit={(e) => void submit(e)} onMouseDown={(e) => e.stopPropagation()}>
        <div className="modal-head">
          <div><div className="eyebrow">Nouvelle instance</div><h2 id="create-loadout-title">{gameName}</h2></div>
          <IconButton name="x" title="Fermer" onClick={onClose} />
        </div>
        <label className="modal-label" htmlFor="loadout-name">Nom de l’instance</label>
        <Input variant="field" id="loadout-name" autoFocus maxLength={100} placeholder="Ex. Labo météo" value={name} onChange={(e) => setName(e.target.value)} />
        <p className="modal-copy">Une instance commence vide — pour récupérer une instance Mod Organizer 2 existante, utilise « Importer depuis MO2 ».</p>
        {message ? <p className="modal-error">{message}</p> : null}
        <div className="modal-actions">
          <Button variant="sm-ghost" onClick={onClose}>Annuler</Button>
          <Button variant="sm" type="submit" disabled={saving || !name.trim()}>{saving ? "Création…" : "Créer l’instance"}</Button>
        </div>
      </form>
    </div>
  );
}
