import { useState, type FormEvent } from "react";
import { Button, IconButton, Input } from "../../ui";

/**
 * Migration entry point for an existing Mod Organizer 2 instance. Path
 * inputs, not a native folder picker: no dialog plugin is wired into the
 * shell yet, and paths are what the Core's import actually needs.
 */
export function Mo2ImportDialog({
  gameName,
  onClose,
  onImport,
}: {
  gameName: string;
  onClose: () => void;
  onImport: (profile: string, modsDir: string, name: string) => Promise<number>;
}) {
  const [profile, setProfile] = useState("");
  const [modsDir, setModsDir] = useState("");
  const [name, setName] = useState("");
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState<string | null>(null);

  async function submit(e: FormEvent) {
    e.preventDefault();
    setBusy(true);
    setMessage(null);
    try {
      await onImport(profile.trim(), modsDir.trim(), name);
    } catch (error) {
      setMessage(error instanceof Error ? error.message : String(error));
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="modal-backdrop" role="presentation" onMouseDown={onClose}>
      <form
        className="modal"
        aria-labelledby="mo2-import-title"
        onSubmit={(e) => void submit(e)}
        onMouseDown={(e) => e.stopPropagation()}
      >
        <div className="modal-head">
          <div>
            <div className="eyebrow">Importer depuis MO2</div>
            <h2 id="mo2-import-title">{gameName}</h2>
          </div>
          <IconButton name="x" title="Fermer" onClick={onClose} />
        </div>
        <label className="modal-label" htmlFor="mo2-profile">
          Dossier de profil MO2
        </label>
        <Input
          variant="field"
          id="mo2-profile"
          autoFocus
          value={profile}
          onChange={(e) => setProfile(e.target.value)}
          placeholder={`…\\Mod Organazer\\profiles\\${"Default"}`}
        />
        <p className="modal-copy">
          Celui qui contient <span className="mono">modlist.txt</span>.
        </p>
        <label className="modal-label" htmlFor="mo2-mods">
          Dossier <span className="mono">mods</span> de MO2
        </label>
        <Input
          variant="field"
          id="mo2-mods"
          value={modsDir}
          onChange={(e) => setModsDir(e.target.value)}
          placeholder="…\Mod Organazer\mods"
        />
        <label className="modal-label" htmlFor="mo2-name">
          Nom de l’instance
        </label>
        <Input
          variant="field"
          id="mo2-name"
          maxLength={100}
          placeholder="Ex. Main 2026"
          value={name}
          onChange={(e) => setName(e.target.value)}
        />
        <p className="modal-copy">
          Rien n’est copié ni déplacé : les mods pointeront vers les dossiers de
          MO2, en place. Les DLC et contenus Creation Club listés dans le profil
          sont ignorés.
        </p>
        {message ? <p className="modal-error">{message}</p> : null}
        <div className="modal-actions">
          <Button variant="sm-ghost" onClick={onClose}>
            Annuler
          </Button>
          <Button
            variant="sm"
            type="submit"
            disabled={busy || !profile.trim() || !modsDir.trim() || !name.trim()}
          >
            {busy ? "Import…" : "Importer"}
          </Button>
        </div>
      </form>
    </div>
  );
}
