import { Ico } from "../../../Ico";
import { IconButton } from "../../ui";
import type { SideAct } from "./Sidebar";

/**
 * The sidebar's persistent profile row — visible under every activity, not
 * just Réglages. The circle is a placeholder for a future profile picture
 * (a person glyph until one is set); clicking it opens the user's own
 * `"profile"` page (identity, not configuration — see `ProfilePage`'s own
 * doc). The gear opens Réglages, same as the rail's own gear icon used to.
 * The users icon (only once the "Communauté" preview is on) opens the
 * `"community"` act — Atelier de patchs / Boutique, also not configuration,
 * so it lives here rather than under Réglages too.
 */
export function SidebarProfile({
  act,
  onAct,
  dataRoot,
  communityEnabled,
}: {
  act: SideAct;
  onAct: (act: SideAct) => void;
  dataRoot: string | undefined;
  communityEnabled: boolean;
}) {
  return (
    <div className="side-profile">
      <button
        type="button"
        className="profile-avatar-btn"
        title="Ouvrir le profil"
        aria-pressed={act === "profile"}
        onClick={() => onAct("profile")}
      >
        <span className="profile-avatar" aria-hidden="true">
          <Ico name="user" />
        </span>
        <div className="profile-text">
          <span className="profile-name">Profil local</span>
          <span className="profile-sub muted">{dataRoot ?? "—"}</span>
        </div>
      </button>
      {communityEnabled ? (
        <IconButton
          name="users"
          title="Communauté"
          aria-pressed={act === "community"}
          onClick={() => onAct("community")}
        />
      ) : null}
      <IconButton
        name="gear"
        title="Réglages"
        aria-pressed={act === "settings"}
        onClick={() => onAct("settings")}
      />
    </div>
  );
}
