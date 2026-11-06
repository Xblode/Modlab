import { useEffect, useState } from "react";
import {
  getSkyrimGraphicsSettings,
  setSkyrimGraphicsSettings,
  type GraphicsSettings,
} from "../../../api";
import { Button, IconButton, Input, Switch } from "../../ui";

const AA_LEVELS = [0, 2, 4, 8];

/** The dialog's own form state — every field resolved to a real value (see
 * `FALLBACK`), unlike `GraphicsSettings` itself where `null` means
 * "`SkyrimPrefs.ini` doesn't set this key". */
type GraphicsForm = {
  width: number;
  height: number;
  fullscreen: boolean;
  vsync: boolean;
  antialiasing: number;
  fxaa: boolean;
};

/** Skyrim's own hardcoded defaults — shown for a field `SkyrimPrefs.ini`
 * doesn't set yet, never persisted as a lie about what the file actually
 * contains. */
const FALLBACK: GraphicsForm = {
  width: 1920,
  height: 1080,
  fullscreen: true,
  vsync: true,
  antialiasing: 0,
  fxaa: false,
};

/**
 * The dashboard's "Graphismes" quick action: a handful of `SkyrimPrefs.ini`
 * `[Display]` keys, edited in place. Everything else in that file — every
 * other key, every other section — is untouched by this (see
 * `modlab_game_skyrim::graphics`'s doc on the Rust side).
 */
export function GraphicsSettingsDialog({ onClose }: { onClose: () => void }) {
  const [loaded, setLoaded] = useState<GraphicsSettings | null>(null);
  const [draft, setDraft] = useState<GraphicsForm>(FALLBACK);
  const [saving, setSaving] = useState(false);
  const [message, setMessage] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    getSkyrimGraphicsSettings()
      .then((s) => {
        if (cancelled) return;
        setLoaded(s);
        setDraft({
          width: s.width ?? FALLBACK.width,
          height: s.height ?? FALLBACK.height,
          fullscreen: s.fullscreen ?? FALLBACK.fullscreen,
          vsync: s.vsync ?? FALLBACK.vsync,
          antialiasing: s.antialiasing ?? FALLBACK.antialiasing,
          fxaa: s.fxaa ?? FALLBACK.fxaa,
        });
      })
      .catch((e: unknown) => setMessage(e instanceof Error ? e.message : String(e)));
    return () => {
      cancelled = true;
    };
  }, []);

  async function save() {
    setSaving(true);
    setMessage(null);
    try {
      await setSkyrimGraphicsSettings(draft);
      onClose();
    } catch (e) {
      setMessage(e instanceof Error ? e.message : String(e));
      setSaving(false);
    }
  }

  return (
    <div className="modal-backdrop" role="presentation" onMouseDown={onClose}>
      <div
        className="modal"
        role="dialog"
        aria-labelledby="graphics-dialog-title"
        onMouseDown={(e) => e.stopPropagation()}
      >
        <div className="modal-head">
          <div>
            <div className="eyebrow">Tableau de bord</div>
            <h2 id="graphics-dialog-title">Graphismes</h2>
          </div>
          <IconButton name="x" title="Fermer" onClick={onClose} />
        </div>

        {!loaded ? (
          <p className="modal-copy">Lecture de SkyrimPrefs.ini…</p>
        ) : (
          <>
            <p className="modal-copy">
              Modifie directement <code>SkyrimPrefs.ini</code> — les autres réglages du
              fichier ne sont pas touchés.
            </p>
            <div className="settings-row">
              <div className="settings-row-text">
                <div className="settings-row-title">Résolution</div>
              </div>
              <div className="settings-row-control" style={{ gap: 6 }}>
                <Input
                  variant="field"
                  type="number"
                  min={640}
                  style={{ width: 84 }}
                  value={draft.width}
                  onChange={(e) =>
                    setDraft((d) => ({ ...d, width: Number(e.target.value) || d.width }))
                  }
                />
                <span className="muted">×</span>
                <Input
                  variant="field"
                  type="number"
                  min={480}
                  style={{ width: 84 }}
                  value={draft.height}
                  onChange={(e) =>
                    setDraft((d) => ({ ...d, height: Number(e.target.value) || d.height }))
                  }
                />
              </div>
            </div>
            <div className="settings-row">
              <div className="settings-row-text">
                <div className="settings-row-title">Plein écran</div>
              </div>
              <div className="settings-row-control">
                <Switch
                  checked={draft.fullscreen}
                  onToggle={() => setDraft((d) => ({ ...d, fullscreen: !d.fullscreen }))}
                />
              </div>
            </div>
            <div className="settings-row">
              <div className="settings-row-text">
                <div className="settings-row-title">VSync</div>
              </div>
              <div className="settings-row-control">
                <Switch
                  checked={draft.vsync}
                  onToggle={() => setDraft((d) => ({ ...d, vsync: !d.vsync }))}
                />
              </div>
            </div>
            <div className="settings-row">
              <div className="settings-row-text">
                <div className="settings-row-title">Anticrénelage (MSAA)</div>
              </div>
              <div className="settings-row-control" style={{ gap: 4 }}>
                {AA_LEVELS.map((level) => (
                  <button
                    key={level}
                    type="button"
                    className={`btn sm ${draft.antialiasing === level ? "accent" : "ghost"}`}
                    onClick={() => setDraft((d) => ({ ...d, antialiasing: level }))}
                  >
                    {level === 0 ? "Off" : `${level}x`}
                  </button>
                ))}
              </div>
            </div>
            <div className="settings-row">
              <div className="settings-row-text">
                <div className="settings-row-title">FXAA</div>
              </div>
              <div className="settings-row-control">
                <Switch
                  checked={draft.fxaa}
                  onToggle={() => setDraft((d) => ({ ...d, fxaa: !d.fxaa }))}
                />
              </div>
            </div>
          </>
        )}

        {message ? <p className="modal-error">{message}</p> : null}
        <div className="modal-actions">
          <Button variant="sm-ghost" onClick={onClose}>
            Annuler
          </Button>
          <Button variant="sm" disabled={!loaded || saving} onClick={() => void save()}>
            {saving ? "Enregistrement…" : "Enregistrer"}
          </Button>
        </div>
      </div>
    </div>
  );
}
