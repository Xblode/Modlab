import { Ico } from "../../../Ico";

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
}: {
  title: string;
  icon: string;
  names: string[] | null;
  /** Shown when the folder exists but holds nothing ModLab can list. */
  emptyLabel: string;
}) {
  return (
    <>
      <div className="dh">
        {title}
        {names ? <span className="cnt"> ({names.length})</span> : null}
      </div>
      {names === null ? (
        <div className="tr muted" style={{ fontStyle: "italic" }}>
          Lecture du dossier…
        </div>
      ) : names.length === 0 ? (
        <div className="tr muted" style={{ fontStyle: "italic" }}>
          {emptyLabel}
        </div>
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
