import { Ico } from "../../../Ico";
import { SideBack } from "../../ui";
import type { SettingsMenu } from "./SettingsPane";

/**
 * The Réglages entries, grouped the way an app-settings menu (Cursor,
 * VS Code…) groups them: everyday preferences first, the more specialised
 * ModLab-platform sections after. Purely a nav model — every id maps to a
 * `SettingsPane` branch, nothing here decides what a section contains.
 */
const SETTINGS_GROUPS: readonly (readonly (readonly [SettingsMenu, string, string])[])[] = [
  [
    ["general", "Général", "sliders"],
    ["profile", "Profil", "user"],
    ["appearance", "Apparence", "palette"],
    ["notifications", "Notifications", "alert"],
  ],
  [
    ["games", "Jeux", "layers"],
    ["models", "Modèles", "box"],
    ["mcp", "MCP & Outils", "tool"],
    ["privacy", "Confidentialité", "shield"],
  ],
] as const;

/**
 * The settings side menu: a `SideBack` row, then the grouped nav list —
 * replaces the old accordion-based flat list with something that reads like
 * a real settings screen instead of another collapsible sidebar section.
 */
export function SideMenu({
  active,
  onSelect,
  onBack,
}: {
  active: SettingsMenu;
  onSelect: (menu: SettingsMenu) => void;
  onBack: () => void;
}) {
  return (
    <div className="side-menu">
      <SideBack label="Réglages" onClick={onBack} />
      <div className="side-menu-body">
        {SETTINGS_GROUPS.map((group, gi) => (
          <div className="side-menu-group" key={gi}>
            {group.map(([id, label, icon]) => (
              <button
                key={id}
                type="button"
                className={`tr ${active === id ? "sel" : ""}`}
                onClick={() => onSelect(id)}
              >
                <Ico name={icon} />
                <span className="lbl">{label}</span>
              </button>
            ))}
          </div>
        ))}
      </div>
    </div>
  );
}
