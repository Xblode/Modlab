import { Fragment, type ReactNode } from "react";
import { Ico } from "../../../Ico";

/**
 * The maquette's `.tbar` breadcrumb strip. Every segment but the last is
 * emphasised and followed by a chevron; the last one is the current place.
 * `tail` is the right-aligned aside (the data root, today) — pass it, even
 * empty, on the surfaces that reserve the space for it.
 */
export function Crumbs({ path, tail }: { path: ReactNode[]; tail?: ReactNode }) {
  const last = path.length - 1;
  return (
    <div className="tbar">
      <div className="crumbs">
        <Ico name="box" />
        {path.map((item, i) =>
          i < last ? (
            <Fragment key={i}>
              <b>{item}</b>
              <Ico name="chevr" />
            </Fragment>
          ) : (
            <Fragment key={i}>{item}</Fragment>
          ),
        )}
      </div>
      {tail !== undefined ? (
        <>
          <span className="spacer" />
          <span className="muted" style={{ fontSize: 11.5 }}>
            {tail}
          </span>
        </>
      ) : null}
    </div>
  );
}
