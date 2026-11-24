import { Ico } from "../../../Ico";
import { SideEmpty } from "../../ui";

/**
 * One titled list of entry names read straight from an instance folder
 * (`resourcepacks/`, `shaderpacks/`, `saves/`). Deliberately dumb: the Core
 * lists the names, this renders them. It never claims more than a directory
 * listing gives — no sizes, no versions, no "compatible with your modlist",
 * because none of that is read yet.
 */
export function FolderList({
  title,
  icon,
  names,
  emptyLabel,
  emptyHint,
}: {
  title: string;
  icon: string;
  names: string[] | null;
  /** Shown when the folder exists but holds nothing ModLab can list. */
  emptyLabel: string;
  /** One short phrase under the title. */
  emptyHint: string;
}) {
  return (
    <>
      <div className="dh">
        {title}
        {names ? <span className="cnt"> ({names.length})</span> : null}
      </div>
      {names === null ? (
        <div className="tr muted">
          Lecture du dossier…
        </div>
      ) : names.length === 0 ? (
        <SideEmpty title={emptyLabel}>{emptyHint}</SideEmpty>
      ) : (
        names.map((name) => (
          <div className="tr-row" key={name}>
            <span className="tr-label">
              <Ico name={icon} />
              <span className="lbl">{name}</span>
            </span>
          </div>
        ))
      )}
    </>
  );
}
