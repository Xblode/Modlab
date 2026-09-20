import type { RefObject } from "react";
import { IconButton } from "../../ui";
import type { SideAct } from "./Sidebar";

const ACTIVITIES = [
  ["explorer", "Explorateur", "layers"],
  ["search", "Recherche", "search"],
  ["history", "Historique", "branch"],
  ["tools", "Outils", "tool"],
] as const;

/** The sidebar's icon rail: the menu button and the activities. Réglages
 * lives only in the profile row now (`SidebarProfile`), not here too. */
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
    </div>
  );
}
