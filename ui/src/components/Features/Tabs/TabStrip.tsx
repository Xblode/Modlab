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
  webOpen,
  onSelect,
}: {
  tabs: TabDef[];
  currentTab: TabId;
  /** Shown as the Mods tab's counter. */
  modCount: number;
  /** The embedded browser is a shell tab, not a game capability: it is
   * appended after the adapter's own tabs and only once opened. */
  webOpen: boolean;
  onSelect: (tab: TabId) => void;
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
        {webOpen ? (
          <button
            className="tab"
            type="button"
            role="tab"
            aria-selected={currentTab === "web"}
            onClick={() => onSelect("web")}
          >
            <Ico name="globe" className="sm" />
            <span>Navigateur</span>
          </button>
        ) : null}
      </div>
    </div>
  );
}
