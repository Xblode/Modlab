import { useEffect, useRef, useState, type KeyboardEvent } from "react";
import { pickPath, type PathKind } from "../../../api";
import { Ico } from "../../../Ico";
import { Button, IconButton, Input, SettingsRow } from "../../ui";

function failure(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

const LOCKED = "Ce chemin ne se modifie pas.";

/**
 * One path row, on Mon jeu and in Réglages: title and short description on
 * the left, the path field and Parcourir on the right. The field stays
 * visible. A path the Core will not write (`canEdit` false) uses the same
 * field, read-only, and Parcourir stays in place but disabled.
 * Enter or blur saves only when the trimmed text changed.
 */
export function PathRow({
  epoch,
  label,
  description,
  path,
  nested = false,
  canEdit,
  pickKind,
  startPath,
  editValue,
  editHint,
  canReset,
  resetTitle = "Revenir à la détection automatique",
  onSave,
  onReset,
  onError,
}: {
  epoch: string;
  label: string;
  description: string;
  /** Effective path. Shown when the field is read-only. */
  path: string | null;
  nested?: boolean;
  canEdit: boolean;
  pickKind: PathKind;
  startPath: string | null;
  /** Value loaded into the field when `canEdit`. Downloads use the shared root. */
  editValue: string;
  editHint?: string;
  canReset: boolean;
  resetTitle?: string;
  onSave: (value: string) => Promise<void>;
  onReset: () => Promise<void>;
  onError: (message: string) => void;
}) {
  const baseline = canEdit ? editValue : (path ?? "");
  const [draft, setDraft] = useState(baseline);
  const skipSave = useRef(false);
  const saving = useRef(false);
  const browsing = useRef(false);
  const savedByBrowse = useRef<string | null>(null);

  useEffect(() => {
    setDraft(canEdit ? editValue : (path ?? ""));
  }, [canEdit, epoch, editValue, path]);

  async function commit() {
    if (!canEdit) return;
    if (browsing.current) return;
    if (skipSave.current) {
      skipSave.current = false;
      return;
    }
    if (saving.current) return;
    const value = draft.trim();
    if (savedByBrowse.current === value) {
      savedByBrowse.current = null;
      return;
    }
    savedByBrowse.current = null;
    if (!value || value === editValue.trim()) {
      setDraft(editValue);
      return;
    }
    saving.current = true;
    try {
      await onSave(value);
    } catch (error) {
      onError(failure(error));
    } finally {
      saving.current = false;
    }
  }

  async function browse() {
    if (!canEdit) return;
    browsing.current = true;
    try {
      const picked = await pickPath(pickKind, startPath);
      if (picked === null) return;
      const value = picked.trim();
      if (!value || value === editValue.trim()) {
        setDraft(editValue);
        return;
      }
      setDraft(value);
      saving.current = true;
      try {
        await onSave(value);
        savedByBrowse.current = value;
      } catch (error) {
        onError(failure(error));
      } finally {
        saving.current = false;
      }
    } catch (error) {
      onError(failure(error));
    } finally {
      browsing.current = false;
    }
  }

  function revert(event: KeyboardEvent<HTMLInputElement>) {
    skipSave.current = true;
    setDraft(editValue);
    event.currentTarget.blur();
  }

  const browseTitle = !canEdit
    ? LOCKED
    : pickKind === "file"
      ? "Choisir un fichier"
      : "Choisir un dossier";

  return (
    <SettingsRow
      indent={nested}
      title={label}
      desc={
        editHint ? (
          <>
            {description} {editHint}
          </>
        ) : (
          description
        )
      }
      control={
        <>
          <Input
            className="dash-path-edit"
            variant="field"
            readOnly={!canEdit}
            value={draft}
            aria-label={label}
            title={canEdit ? undefined : LOCKED}
            onChange={(event) => setDraft(event.target.value)}
            onKeyDown={(event) => {
              if (!canEdit) return;
              if (event.key === "Enter") {
                event.preventDefault();
                void commit();
              }
              if (event.key === "Escape") {
                event.preventDefault();
                revert(event);
              }
            }}
            onBlur={() => {
              void commit();
            }}
          />
          <Button
            variant="sm-ghost"
            className="dash-browse"
            title={browseTitle}
            disabled={!canEdit}
            onMouseDown={(event) => event.preventDefault()}
            onClick={() => {
              void browse();
            }}
          >
            <Ico name="folder" />
            Parcourir
          </Button>
          {canReset ? (
            <IconButton
              name="refresh"
              title={resetTitle}
              onMouseDown={(event) => event.preventDefault()}
              onClick={() => {
                void onReset().catch((error: unknown) => onError(failure(error)));
              }}
            />
          ) : null}
        </>
      }
    />
  );
}
