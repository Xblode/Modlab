import { useEffect, useState } from "react";
import { getModConfigFiles, type ModConfigFile, type ModRow } from "../../../api";
import { Accordion } from "../../ui";

/**
 * Config section at the bottom of the Info tab. Lists the config files that
 * sit in the selected mod's own folder and, when the file is small text,
 * shows the body. Read-only: nothing here writes to the mod folder or to
 * the game directory.
 */
export function ConfigPane({
  mod,
  loadoutId,
}: {
  mod: ModRow | null;
  loadoutId: number | null;
}) {
  const modId = mod?.modId ?? null;
  const sourcePath = mod?.sourcePath ?? "";
  const [files, setFiles] = useState<ModConfigFile[] | null>(null);
  const [failed, setFailed] = useState(false);

  useEffect(() => {
    if (modId == null || loadoutId == null) {
      setFiles([]);
      setFailed(false);
      return;
    }
    let cancelled = false;
    setFiles(null);
    setFailed(false);
    getModConfigFiles(loadoutId, modId)
      .then((list) => {
        if (!cancelled) setFiles(list);
      })
      .catch(() => {
        if (!cancelled) {
          setFiles([]);
          setFailed(true);
        }
      });
    return () => {
      cancelled = true;
    };
  }, [modId, sourcePath, loadoutId]);

  if (!mod) return null;

  const count = files?.length ?? 0;
  return (
    <Accordion variant="rsec" title="Config" count={count}>
      {failed ? (
        <span className="muted">Lecture impossible</span>
      ) : files == null ? (
        <span className="muted">Lecture…</span>
      ) : files.length === 0 ? (
        <span className="muted">Aucun fichier de config</span>
      ) : (
        <div className="cfg-list">
          {files.map((file) => (
            <div key={file.relativePath} className="cfg-file">
              <div>{file.name}</div>
              <div className="mono">{file.relativePath}</div>
              {file.text != null ? (
                <pre className="cfg-preview" title="Lecture seule">
                  {file.text}
                </pre>
              ) : (
                <p className="muted">Aperçu non affiché : fichier trop gros ou non textuel.</p>
              )}
            </div>
          ))}
        </div>
      )}
    </Accordion>
  );
}
