import type { ReactNode } from "react";

/**
 * One label and description, with an optional control pinned to the right.
 * Shared by Réglages and the game-path rows.
 */
export function SettingsRow({
  title,
  desc,
  control,
  indent = false,
}: {
  title: ReactNode;
  desc?: ReactNode;
  control?: ReactNode;
  /** Ini files sit in under their config folder. */
  indent?: boolean;
}) {
  return (
    <div className={indent ? "settings-row indent" : "settings-row"}>
      <div className="settings-row-text">
        <div className="settings-row-title">{title}</div>
        {desc ? <div className="settings-row-desc">{desc}</div> : null}
      </div>
      {control ? <div className="settings-row-control">{control}</div> : null}
    </div>
  );
}
