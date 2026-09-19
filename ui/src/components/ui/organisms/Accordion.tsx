import { useState, type ReactNode } from "react";
import { Ico } from "../../../Ico";

/**
 * The maquette's `.sec` — a titled, collapsible section with a count. Open by
 * default, exactly like the shell's sidebar sections; the open state is local
 * to the section, as it is in the maquette.
 */
export function Accordion({
  title,
  count,
  action,
  variant,
  children,
}: {
  title: string;
  count: number;
  /** Optional affordance for the section, pinned to the right of the title row. */
  action?: ReactNode;
  /** `rsec` is the right-hand inspector's compact variant (maquette's `.rsec`). */
  variant?: "rsec";
  children: ReactNode;
}) {
  const [open, setOpen] = useState(true);
  return (
    <div className={`sec ${variant ?? ""} ${open ? "" : "closed"}`}>
      <div className="sec-h">
        <button
          className="sec-t"
          type="button"
          aria-expanded={open}
          onClick={() => setOpen((v) => !v)}
        >
          <Ico name="chev" />
          <span className="u">{title}</span>
          <span className="cnt">({count})</span>
        </button>
        {action}
      </div>
      <div className="sec-b">{children}</div>
    </div>
  );
}
