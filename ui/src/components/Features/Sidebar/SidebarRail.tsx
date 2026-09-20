import type { RefObject } from "react";
import { IconButton } from "../../ui";
import type { SideAct } from "./Sidebar";

const ACTIVITIES = [
  ["explorer", "Explorateur", "layers"],
  ["search", "Recherche", "search"],
  ["history", "Historique", "branch"],
  ["tools", "Outils", "tool"],
] as const;

/** The sidebar's icon rail: the menu button, the activities, the settings. */
export function SidebarRail({
  act,
  onAct,
  menuButtonRef,
  menuOpen,
  onToggleMenu,
}: {
  act: SideAct;
  onAct: (act: SideAct) => void;
  menuButtonRef: RefObject<HTMLButtonElement | null>;
  menuOpen: boolean;
  onToggleMenu: () => void;
}) {
  return (
    <div className="side-tools">
      <IconButton
        name="menu"
        title="Menu"
        ref={menuButtonRef}
        aria-pressed={menuOpen}
        onClick={onToggleMenu}
      />
      <div className="hact">
        {ACTIVITIES.map(([id, title, icon]) => (
          <IconButton key={id} name={icon} title={title} aria-pressed={act === id} onClick={() => onAct(id)} />
        ))}
      </div>
      <span className="spacer" />
      <IconButton name="globe" title="Nexus Mods" />
      <IconButton
        name="gear"
        title="Réglages"
        aria-pressed={act === "settings"}
        onClick={() => onAct("settings")}
      />
    </div>
  );
}
