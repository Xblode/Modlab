import type { FormEvent } from "react";
import { Ico } from "../../../Ico";
import { IconButton, Input } from "../../ui";

/**
 * The bottom dock's search / command bar. The field itself is read-only on
 * purpose: focusing it opens the command palette, which is where typing
 * actually happens (Ctrl+K, §6.2).
 */
export function Composer({
  draft,
  modelLabel,
  onDraftChange,
  onFocus,
  onSubmit,
}: {
  draft: string;
  /** Human name of the active local model, as shown on the toolbar. */
  modelLabel: string | undefined;
  onDraftChange: (value: string) => void;
  onFocus: () => void;
  onSubmit: (e?: FormEvent) => void;
}) {
  return (
    <div className="composer-wrap">
      <form className="composer-box" onSubmit={onSubmit}>
        <div className="composer">
          <Ico name="search" />
          <IconButton name="term" title="Console" />
          <Input
            value={draft}
            onChange={(e) => onDraftChange(e.target.value)}
            onFocus={onFocus}
            placeholder="Rechercher ou commander… (Ctrl+K)"
            autoComplete="off"
            readOnly
          />
          <span className="kbd">Ctrl K</span>
        </div>
        <div className="composer-toolbar">
          <button className="cb-add" type="button" title="Ajouter">
            +
          </button>
          <button className="cb-select" type="button">
            {modelLabel}
            <Ico name="chev" />
          </button>
          <span className="spacer" />
          <button className="cb-claude" type="button" onClick={() => onSubmit()}>
            <Ico name="sparkle" />
            Claude
          </button>
        </div>
      </form>
    </div>
  );
}
