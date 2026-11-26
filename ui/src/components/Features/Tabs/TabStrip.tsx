import { splitGameTab, type TabDef, type TabId } from "../../../gameChrome";
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
  onSelect,
}: {
  tabs: TabDef[];
  currentTab: TabId;
  /** Shown as the Mods tab's counter. */
  modCount: number;
  onSelect: (tab: TabId) => void;
}) {
  const { group, game } = splitGameTab(tabs);
  return (
    <div className="tabs" role="tablist">
      <div className="tabs-l">
        {group.map((t) => (
          <TabButton
            key={t.id}
            tab={t}
            currentTab={currentTab}
            modCount={modCount}
            onSelect={onSelect}
          />
        ))}
        {game && group.length > 0 ? <span className="tab-sep" aria-hidden="true" /> : null}
        {game ? (
          <TabButton
            tab={game}
            currentTab={currentTab}
            modCount={modCount}
            onSelect={onSelect}
          />
        ) : null}
      </div>
    </div>
  );
}

function TabButton({
  tab,
  currentTab,
  modCount,
  onSelect,
}: {
  tab: TabDef;
  currentTab: TabId;
  modCount: number;
  onSelect: (tab: TabId) => void;
}) {
  return (
    <button
      className="tab"
      type="button"
      role="tab"
      aria-selected={currentTab === tab.id}
      onClick={() => onSelect(tab.id)}
    >
      <Ico name={tab.icon} className="sm" />
      <span>{tab.label}</span>
      {tab.id === "mods" ? <span className="cnt">{modCount}</span> : null}
    </button>
  );
}
