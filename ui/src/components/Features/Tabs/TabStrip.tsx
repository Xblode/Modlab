import type { TabDef, TabId } from "../../../gameChrome";
import { Ico } from "../../../Ico";

/**
 * The sheet's tab strip. The tabs themselves come from the active game's
 * adapter capabilities — this component never decides what a game can do, it
 * only renders the list it is given.
 */
export function TabStrip({
  tabs,
  currentTab,
  modCount,
  aiActive,
  onSelect,
  onOpenAi,
}: {
  tabs: TabDef[];
  currentTab: TabId;
  /** Shown as the Mods tab's counter. */
  modCount: number;
  aiActive: boolean;
  onSelect: (tab: TabId) => void;
  onOpenAi: () => void;
}) {
  return (
    <div className="tabs" role="tablist">
      <div className="tabs-l">
        {tabs.map((t) => (
          <button
            key={t.id}
            className="tab"
            type="button"
            role="tab"
            aria-selected={currentTab === t.id}
            onClick={() => onSelect(t.id)}
          >
            <Ico name={t.icon} className="sm" />
            <span>{t.label}</span>
            {t.id === "mods" ? <span className="cnt">{modCount}</span> : null}
          </button>
        ))}
      </div>
      <div className="tabs-r">
        <button
          className="ti claude"
          type="button"
          aria-pressed={aiActive}
          onClick={onOpenAi}
        >
          <Ico name="sparkle" />
          <span className="n">IA</span>
        </button>
      </div>
    </div>
  );
}
