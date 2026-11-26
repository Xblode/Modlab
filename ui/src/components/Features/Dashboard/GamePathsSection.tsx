import { useEffect, useState } from "react";
import {
  clearGamePathOverride,
  getGamePathSlots,
  setGamePathOverride,
  type GameIniFile,
  type GamePathSlot,
  type GamePaths,
  type ManagedPath,
} from "../../../api";
import { SettingsRow } from "../../ui";
import { PathRow } from "./PathRow";

const EXTRA_SLOT_COPY: Record<string, string> = {
  launcher_exe:
    "Fichier lancé par Jouer (minecraft.exe CurseForge ou MinecraftLauncher.exe). Sans CurseForge, parcourez jusqu’à votre launcher.",
  exe: "Fichier lancé pour démarrer le jeu.",
  data_dir: "Dossier Data lu par le jeu.",
  usvfs_dll: "Bibliothèque USVFS de Mod Organizer 2.",
  saves_dir: "Dossier des sauvegardes.",
};

function extraSlotDescription(id: string, effective: string | null): string {
  if (id === "launcher_exe" && !effective) {
    return "Introuvable automatiquement. Parcourir pour choisir minecraft.exe (dossier Install CurseForge) ou MinecraftLauncher.exe — pas le Store Windows.";
  }
  return EXTRA_SLOT_COPY[id] ?? "Chemin déclaré par ce jeu, en plus des dossiers principaux.";
}

function managedDescription(path: ManagedPath): string {
  if (path.update?.via === "instancePathOverride" && path.update.slotId === "instance_root") {
    return "Dossier propre à cette instance. Un renommage ne le déplace pas.";
  }
  if (path.update?.via === "instancePathOverride") {
    return "Un autre dossier remplace celui de l'instance.";
  }
  if (path.update?.via === "downloadsDirOverride") {
    return "Dossier de ce jeu, sous une racine partagée.";
  }
  if (path.update === null) {
    if (path.id === "mods_dir") {
      return "Emplacement fixe des mods installés. Le Core n’accepte pas de le changer.";
    }
    if (path.id === "config_dir") {
      return "Dossier calculé. Il suit le jeu ou le fichier de préférences.";
    }
    return "Ce chemin est calculé. Il ne se modifie pas ici.";
  }
  return "Un autre dossier remplace la détection automatique.";
}

function iniDescription(file: GameIniFile): string {
  if (file.slotId) {
    return file.present
      ? "Fichier de préférences. Parcourir choisit un fichier .ini."
      : "Fichier de préférences introuvable. Parcourir choisit un fichier .ini.";
  }
  return file.present
    ? "Fichier du dossier de configuration."
    : "Absent de ce dossier.";
}

function ManagedRow({
  epoch,
  path,
  description,
  editHint,
  resetTitle,
  onSave,
  onReset,
  onError,
}: {
  epoch: string;
  path: ManagedPath;
  description?: string;
  editHint?: string;
  resetTitle?: string;
  onSave: (path: ManagedPath, value: string) => Promise<void>;
  onReset: (path: ManagedPath) => Promise<void>;
  onError: (message: string) => void;
}) {
  return (
    <PathRow
      epoch={epoch}
      label={path.label}
      description={description ?? managedDescription(path)}
      path={path.effective}
      canEdit={path.update !== null}
      pickKind={path.kind}
      startPath={path.editPath ?? path.effective}
      editValue={path.editPath ?? ""}
      editHint={editHint}
      canReset={path.origin === "override" && path.update !== null}
      resetTitle={resetTitle}
      onSave={(value) => onSave(path, value)}
      onReset={() => onReset(path)}
      onError={onError}
    />
  );
}

function IniRow({
  epoch,
  file,
  canReset,
  onSave,
  onReset,
  onError,
}: {
  epoch: string;
  file: GameIniFile;
  canReset: boolean;
  onSave: (slotId: string, value: string) => Promise<void>;
  onReset: (slotId: string) => Promise<void>;
  onError: (message: string) => void;
}) {
  const slotId = file.slotId;
  const editable = slotId !== null;
  return (
    <PathRow
      epoch={`${epoch}:${file.name}`}
      label={file.name}
      description={iniDescription(file)}
      path={file.path}
      nested
      canEdit={editable}
      pickKind="file"
      startPath={file.path}
      editValue={file.path ?? ""}
      canReset={canReset && editable}
      onSave={(value) => {
        if (!slotId) return Promise.resolve();
        return onSave(slotId, value);
      }}
      onReset={() => {
        if (!slotId) return Promise.resolve();
        return onReset(slotId);
      }}
      onError={onError}
    />
  );
}

type PathActions = {
  saveManaged: (path: ManagedPath, value: string) => Promise<void>;
  resetManaged: (path: ManagedPath) => Promise<void>;
  saveIni: (slotId: string, value: string) => Promise<void>;
  resetIni: (slotId: string) => Promise<void>;
  onError: (message: string) => void;
};

/**
 * Slot ids already drawn from `game_paths` (the managed folder, or the
 * override slot it writes). Réglages uses this to keep only the adapter
 * slots that list does not cover — exe, Data, USVFS, saves.
 */
export function slotIdsInGamePaths(paths: GamePaths): Set<string> {
  const ids = new Set<string>();
  for (const path of [paths.gameDir, paths.modsDir, paths.downloadsDir, paths.configDir, paths.referenceDir]) {
    if (!path) continue;
    ids.add(path.id);
    if (path.update?.via === "gamePathOverride") ids.add(path.update.slotId);
  }
  for (const file of paths.iniFiles) {
    if (file.slotId) ids.add(file.slotId);
  }
  return ids;
}

/** The folder and ini rows, without the Mon jeu heading. Réglages reuses this. */
export function GamePathRows({
  gameId,
  paths,
  saveManaged,
  resetManaged,
  saveIni,
  resetIni,
  onError,
}: { gameId: string; paths: GamePaths } & PathActions) {
  return (
    <>
      <div className="settings-group">
        <div className="settings-group-title">Dossiers</div>
        <ManagedRow
          epoch={gameId}
          path={paths.gameDir}
          onSave={saveManaged}
          onReset={resetManaged}
          onError={onError}
        />
        {paths.referenceDir ? (
          <ManagedRow
            epoch={`${gameId}:ref`}
            path={paths.referenceDir}
            description={
              paths.referenceDir.effective
                ? "Dossier Install / .minecraft du launcher (profils, versions). Les mods ModLab ne s’y installent pas."
                : "Launcher introuvable. Parcourir vers le dossier Install CurseForge (…\\curseforge\\minecraft\\Install) ou votre .minecraft."
            }
            onSave={saveManaged}
            onReset={resetManaged}
            onError={onError}
          />
        ) : null}
        <ManagedRow
          epoch={gameId}
          path={paths.modsDir}
          onSave={saveManaged}
          onReset={resetManaged}
          onError={onError}
        />
        <ManagedRow
          epoch={gameId}
          path={paths.downloadsDir}
          editHint="Racine partagée, pas le sous-dossier du jeu."
          resetTitle="Revenir au dossier par défaut"
          onSave={saveManaged}
          onReset={resetManaged}
          onError={onError}
        />
        <ManagedRow
          epoch={gameId}
          path={paths.configDir}
          onSave={saveManaged}
          onReset={resetManaged}
          onError={onError}
        />
      </div>
      {paths.iniFiles.length > 0 ? (
        <div className="settings-group">
          <div className="settings-group-title">Fichiers .ini</div>
          {paths.iniFiles.map((file) => (
            <IniRow
              key={`${file.slotId ?? ""}:${file.path ?? file.name}`}
              epoch={gameId}
              file={file}
              canReset={paths.configDir.origin === "override"}
              onSave={saveIni}
              onReset={resetIni}
              onError={onError}
            />
          ))}
        </div>
      ) : null}
    </>
  );
}

/**
 * Adapter path slots not already drawn by `game_paths` (e.g. Minecraft
 * `launcher_exe`, Skyrim `exe` / Data / USVFS). Same override commands as Réglages.
 */
function ExtraPathSlots({
  gameId,
  paths,
  onError,
}: {
  gameId: string;
  paths: GamePaths | null;
  onError: (message: string) => void;
}) {
  const [slots, setSlots] = useState<GamePathSlot[] | null>(null);
  const [slotsError, setSlotsError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    setSlots(null);
    setSlotsError(null);
    getGamePathSlots(gameId)
      .then((next) => {
        if (!cancelled) setSlots(next);
      })
      .catch((e: unknown) => {
        if (!cancelled) setSlotsError(e instanceof Error ? e.message : String(e));
      });
    return () => {
      cancelled = true;
    };
  }, [gameId]);

  const covered = paths ? slotIdsInGamePaths(paths) : new Set<string>();
  const extras = (slots ?? []).filter((slot) => !covered.has(slot.id));

  if (slotsError) {
    return (
      <div className="settings-group">
        <SettingsRow title="Autres chemins" desc={slotsError} />
      </div>
    );
  }
  if (extras.length === 0) return null;

  const groupTitle = extras.some((slot) => slot.id === "launcher_exe")
    ? "Launcher Minecraft"
    : "Autres chemins";

  return (
    <div className="settings-group">
      <div className="settings-group-title">{groupTitle}</div>
      {extras.map((slot) => (
        <PathRow
          key={slot.id}
          epoch={`${gameId}:${slot.id}:${slot.overridePath ?? ""}:${slot.effective ?? ""}`}
          label={slot.label}
          description={extraSlotDescription(slot.id, slot.effective)}
          path={slot.effective}
          canEdit
          pickKind={slot.kind}
          startPath={slot.effective}
          editValue={slot.effective ?? ""}
          canReset={Boolean(slot.overridePath)}
          onSave={async (value) => {
            setSlots(await setGamePathOverride(gameId, slot.id, value));
          }}
          onReset={async () => {
            setSlots(await clearGamePathOverride(gameId, slot.id));
          }}
          onError={onError}
        />
      ))}
    </div>
  );
}

/**
 * Chemins on Mon jeu: game folder, installed mods, downloads, configs/.ini,
 * plus uncovered adapter slots (launcher exe, etc.).
 * Edit only where `game_paths` / path slots say a command can write.
 * Every row is the field itself: title, short description, path input,
 * Parcourir. Parcourir opens the native file dialog.
 */
export function GamePathsSection({
  gameId,
  paths,
  error,
  saveManaged,
  resetManaged,
  saveIni,
  resetIni,
  onError,
}: {
  gameId: string;
  paths: GamePaths | null;
  error: string | null;
} & PathActions) {
  return (
    <>
      <div className="dh">Chemins</div>
      <div className="dash-paths">
        {error ? (
          <div className="settings-group">
            <SettingsRow title="Lecture impossible" desc={error} />
          </div>
        ) : paths === null ? (
          <div className="settings-group">
            <SettingsRow title="Chemins" desc="Lecture des chemins…" />
          </div>
        ) : (
          <>
            <GamePathRows
              gameId={gameId}
              paths={paths}
              saveManaged={saveManaged}
              resetManaged={resetManaged}
              saveIni={saveIni}
              resetIni={resetIni}
              onError={onError}
            />
            <ExtraPathSlots gameId={gameId} paths={paths} onError={onError} />
          </>
        )}
      </div>
    </>
  );
}
