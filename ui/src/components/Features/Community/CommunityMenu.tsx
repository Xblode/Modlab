import { Ico } from "../../../Ico";
import { SideBack } from "../../ui";

export type CommunityMenuId = "patchWorkshop" | "marketplace";

const ENTRIES: readonly (readonly [CommunityMenuId, string, string])[] = [
  ["patchWorkshop", "Atelier de patchs", "branch"],
  ["marketplace", "Boutique", "box"],
];

/**
 * Communauté's own side nav — same `SideBack` + grouped-list shape as
 * Réglages' `SideMenu`, but a separate, smaller tree: this isn't a
 * configuration surface, it's the "Communauté" preview from
 * `docs/deep-research-report.md` (Atelier de patchs, Boutique), reached from
 * `SidebarProfile`'s own icon, not from Réglages.
 */
export function CommunityMenu({
  active,
  onSelect,
  onBack,
}: {
  active: CommunityMenuId;
  onSelect: (menu: CommunityMenuId) => void;
  onBack: () => void;
}) {
  return (
    <div className="side-menu">
      <SideBack label="Communauté" onClick={onBack} />
      <div className="side-menu-body">
        <div className="side-menu-group">
          {ENTRIES.map(([id, label, icon]) => (
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
      </div>
    </div>
  );
}
