import { useEffect, useMemo, useState, type FormEvent } from "react";
import type { McLoaderId } from "../../../api";
import { listMcGameVersions, listMcLoaderVersions } from "../../../api";
import { Button, Dialog, Input } from "../../ui";

const DEFAULT_INSTANCE_NAME = "Nouvelle instance";

const LOADERS: { id: McLoaderId; label: string }[] = [
  { id: "vanilla", label: "Vanilla (ne rien installer)" },
  { id: "fabric", label: "Fabric" },
  { id: "forge", label: "Forge" },
  { id: "neoforge", label: "NeoForge" },
  { id: "quilt", label: "Quilt" },
];

/** Next free « Nouvelle instance » / « Nouvelle instance (n) » among live names. */
export function nextUniqueInstanceName(
  existingNames: string[],
  base = DEFAULT_INSTANCE_NAME,
): string {
  const taken = new Set(
    existingNames.map((n) => n.trim().toLowerCase()).filter(Boolean),
  );
  if (!taken.has(base.toLowerCase())) return base;
  let n = 1;
  while (taken.has(`${base} (${n})`.toLowerCase())) n += 1;
  return `${base} (${n})`;
}

export type CreateInstancePayload = {
  name: string;
  /** Minecraft only — set on the new branch after create. */
  mc?: {
    gameVersion: string;
    loader: McLoaderId;
    /** Required when loader ≠ vanilla. */
    loaderVersion: string | null;
  };
};

/** A new, empty instance for the active game. The Core owns the name rules. */
export function CreateLoadoutDialog({
  gameName,
  gameId,
  existingInstanceNames,
  onClose,
  onCreate,
}: {
  gameName: string;
  gameId: string;
  /** Live instance names for this game — used for the default label and confirm check. */
  existingInstanceNames: string[];
  onClose: () => void;
  onCreate: (payload: CreateInstancePayload) => Promise<void>;
}) {
  const isMinecraft = gameId === "minecraft";
  const defaultName = useMemo(
    () => nextUniqueInstanceName(existingInstanceNames),
    [existingInstanceNames],
  );
  const [name, setName] = useState(defaultName);
  const [mcVersion, setMcVersion] = useState("");
  const [loader, setLoader] = useState<McLoaderId | "">("");
  const [loaderVersion, setLoaderVersion] = useState("");
  const [gameVersions, setGameVersions] = useState<string[]>([]);
  const [loaderVersions, setLoaderVersions] = useState<string[]>([]);
  const [loadingVersions, setLoadingVersions] = useState(false);
  const [loadingLoaderVersions, setLoadingLoaderVersions] = useState(false);
  const [saving, setSaving] = useState(false);
  const [message, setMessage] = useState<string | null>(null);

  useEffect(() => {
    setName(defaultName);
  }, [defaultName]);

  useEffect(() => {
    if (!isMinecraft) return;
    let cancelled = false;
    setLoadingVersions(true);
    void listMcGameVersions()
      .then((rows) => {
        if (!cancelled) setGameVersions(rows);
      })
      .catch((e: unknown) => {
        if (!cancelled) {
          setMessage(e instanceof Error ? e.message : String(e));
        }
      })
      .finally(() => {
        if (!cancelled) setLoadingVersions(false);
      });
    return () => {
      cancelled = true;
    };
  }, [isMinecraft]);

  // Changing Minecraft version resets loader + loader version.
  useEffect(() => {
    setLoader("");
    setLoaderVersion("");
    setLoaderVersions([]);
  }, [mcVersion]);

  useEffect(() => {
    if (!isMinecraft || !loader || loader === "vanilla" || !mcVersion) {
      setLoaderVersions([]);
      setLoaderVersion("");
      return;
    }
    let cancelled = false;
    setLoadingLoaderVersions(true);
    setLoaderVersion("");
    void listMcLoaderVersions(loader, mcVersion)
      .then((rows) => {
        if (!cancelled) setLoaderVersions(rows);
      })
      .catch((e: unknown) => {
        if (!cancelled) {
          setLoaderVersions([]);
          setMessage(e instanceof Error ? e.message : String(e));
        }
      })
      .finally(() => {
        if (!cancelled) setLoadingLoaderVersions(false);
      });
    return () => {
      cancelled = true;
    };
  }, [isMinecraft, loader, mcVersion]);

  const nameTaken = useMemo(() => {
    const needle = name.trim().toLowerCase();
    if (!needle) return false;
    return existingInstanceNames.some((n) => n.trim().toLowerCase() === needle);
  }, [name, existingInstanceNames]);

  const mcReady =
    !isMinecraft ||
    (Boolean(mcVersion) &&
      Boolean(loader) &&
      (loader === "vanilla" || Boolean(loaderVersion)));

  const canSubmit =
    Boolean(name.trim()) && !nameTaken && mcReady && !saving && !loadingVersions;

  async function submit(e: FormEvent) {
    e.preventDefault();
    if (!canSubmit) return;
    setSaving(true);
    setMessage(null);
    try {
      if (nameTaken) {
        throw new Error("Une instance porte déjà ce nom pour ce jeu.");
      }
      const payload: CreateInstancePayload = { name: name.trim() };
      if (isMinecraft && mcVersion && loader) {
        payload.mc = {
          gameVersion: mcVersion,
          loader,
          loaderVersion: loader === "vanilla" ? null : loaderVersion || null,
        };
      }
      await onCreate(payload);
    } catch (error) {
      setMessage(error instanceof Error ? error.message : String(error));
    } finally {
      setSaving(false);
    }
  }

  return (
    <Dialog
      as="form"
      titleId="create-loadout-title"
      eyebrow="Nouvelle instance"
      title={gameName}
      onClose={onClose}
      onSubmit={(e) => void submit(e)}
      actions={
        <>
          <Button variant="sm-ghost" onClick={onClose}>
            Annuler
          </Button>
          <Button variant="sm" type="submit" disabled={!canSubmit}>
            {saving ? "Création…" : "Créer l’instance"}
          </Button>
        </>
      }
    >
      <label className="modal-label" htmlFor="loadout-name">
        Nom de l’instance
      </label>
      <Input
        variant="field"
        id="loadout-name"
        autoFocus
        maxLength={100}
        placeholder={DEFAULT_INSTANCE_NAME}
        value={name}
        onChange={(e) => setName(e.target.value)}
      />
      {nameTaken ? (
        <p className="modal-error">Une instance porte déjà ce nom pour ce jeu.</p>
      ) : null}

      {isMinecraft ? (
        <>
          <label className="modal-label" htmlFor="mc-game-version" style={{ marginTop: 14 }}>
            Version Minecraft
          </label>
          <select
            id="mc-game-version"
            className="modal-input"
            value={mcVersion}
            disabled={loadingVersions}
            onChange={(e) => setMcVersion(e.target.value)}
          >
            <option value="">
              {loadingVersions ? "Chargement…" : "Choisir une version…"}
            </option>
            {gameVersions.map((ver) => (
              <option key={ver} value={ver}>
                {ver}
              </option>
            ))}
          </select>

          <label className="modal-label" htmlFor="mc-loader" style={{ marginTop: 14 }}>
            Loader
          </label>
          <select
            id="mc-loader"
            className="modal-input"
            value={loader}
            disabled={!mcVersion}
            onChange={(e) => setLoader(e.target.value as McLoaderId | "")}
          >
            <option value="">
              {mcVersion ? "Choisir un loader…" : "Choisissez d’abord Minecraft"}
            </option>
            {LOADERS.map((row) => (
              <option key={row.id} value={row.id}>
                {row.label}
              </option>
            ))}
          </select>

          {loader && loader !== "vanilla" ? (
            <>
              <label
                className="modal-label"
                htmlFor="mc-loader-version"
                style={{ marginTop: 14 }}
              >
                Version du loader
              </label>
              <select
                id="mc-loader-version"
                className="modal-input"
                value={loaderVersion}
                disabled={!loader || loadingLoaderVersions}
                onChange={(e) => setLoaderVersion(e.target.value)}
              >
                <option value="">
                  {loadingLoaderVersions
                    ? "Chargement…"
                    : loaderVersions.length === 0
                      ? "Aucune version pour cette Minecraft"
                      : "Choisir une version…"}
                </option>
                {loaderVersions.map((ver) => (
                  <option key={ver} value={ver}>
                    {ver}
                  </option>
                ))}
              </select>
              <p className="modal-copy">
                Un loader autre que Vanilla exige les fichiers Vanilla de cette version
                Minecraft (téléchargés à l’installation du loader).
              </p>
            </>
          ) : loader === "vanilla" ? (
            <p className="modal-copy">
              Vanilla : aucun loader à installer. Les fichiers du jeu seront préparés au
              premier lancement ou via « Installer Vanilla ».
            </p>
          ) : null}
        </>
      ) : (
        <p className="modal-copy">
          Une instance commence vide — pour récupérer une instance Mod Organizer 2 existante,
          utilise « Importer depuis MO2 ».
        </p>
      )}

      {message ? <p className="modal-error">{message}</p> : null}
    </Dialog>
  );
}
