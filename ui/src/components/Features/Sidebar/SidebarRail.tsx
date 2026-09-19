import { IconButton } from "../../ui";
import type { SideAct } from "./Sidebar";

const ACTIVITIES = [
  ["explorer", "Explorateur", "layers"],
  ["search", "Recherche", "search"],
  ["history", "Historique", "branch"],
  ["tools", "Outils", "tool"],
] as const;

/** The sidebar's icon rail: the activities and the settings. The application
 * menu is not here any more — it lives in the window's title bar. */
export function SidebarRail({
  act,
  onAct,
}: {
  act: SideAct;
  onAct: (act: SideAct) => void;
}) {
  return (
    <div className="side-tools">
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
