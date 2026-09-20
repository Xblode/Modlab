import { Ico } from "../../../Ico";
import { IconButton } from "../../ui";
import type { SideAct } from "./Sidebar";

/**
 * The sidebar's persistent profile row — visible under every activity, not
 * just Réglages. The circle is a placeholder for a future profile picture
 * (a person glyph until one is set); the gear opens Réglages, same
 * destination as the rail's own gear icon.
 */
export function SidebarProfile({
  act,
  onAct,
  dataRoot,
}: {
  act: SideAct;
  onAct: (act: SideAct) => void;
  dataRoot: string | undefined;
}) {
  return (
    <div className="side-profile">
      <span className="profile-avatar" aria-hidden="true">
        <Ico name="user" />
      </span>
      <div className="profile-text">
        <span className="profile-name">Profil local</span>
        <span className="profile-sub muted">{dataRoot ?? "—"}</span>
      </div>
      <IconButton
        name="gear"
        title="Réglages"
        aria-pressed={act === "settings"}
        onClick={() => onAct("settings")}
      />
    </div>
  );
}
