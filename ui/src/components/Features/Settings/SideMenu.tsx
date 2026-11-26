import { Accordion, SideBack, SideRow } from "../../ui";
import type { SettingsMenu } from "./SettingsPane";

/**
 * The Réglages entries, grouped the way an app-settings menu (Cursor,
 * VS Code…) groups them: everyday preferences first, the more specialised
 * ModLab-platform sections after. Same Accordion + SideRow shape as the
 * explorer / history sidebar — not a second nav system. Purely a nav model:
 * every id maps to a `SettingsPane` branch. Profil and Communauté are not
 * here: they're identity/social, not configuration — see `SidebarProfile`
 * and `CommunityMenu`.
 */
const SETTINGS_SECTIONS: readonly {
  title: string;
  entries: readonly (readonly [SettingsMenu, string, string])[];
}[] = [
  {
    title: "Préférences",
    entries: [
      ["general", "Général", "sliders"],
      ["account", "Compte", "user"],
      ["appearance", "Apparence", "palette"],
      ["browser", "Navigateur", "globe"],
      ["notifications", "Notifications", "alert"],
      ["designsystem", "Design system", "sparkle"],
    ],
  },
  {
    title: "Plateforme",
    entries: [
      ["games", "Jeux", "layers"],
      ["categories", "Catégorie", "box"],
      ["models", "Modèles", "box"],
      ["mcp", "MCP & Outils", "tool"],
      ["privacy", "Confidentialité", "shield"],
    ],
  },
];

/**
 * The settings side menu: a `SideBack` row, then Accordion sections of
 * SideRow entries — same tokens and spacing as the main sidebar body.
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
      {SETTINGS_SECTIONS.map((section) => (
        <Accordion key={section.title} title={section.title} count={section.entries.length}>
          {section.entries.map(([id, label, icon]) => (
            <SideRow
              key={id}
              icon={icon}
              label={label}
              current={active === id}
              onClick={() => onSelect(id)}
            />
          ))}
        </Accordion>
      ))}
    </div>
  );
}
