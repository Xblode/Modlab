import { forwardRef, type ButtonHTMLAttributes } from "react";
import { Ico } from "../../../Ico";
import { tip } from "../molecules/Tooltip";

/**
 * Primary bordered select trigger. Visual reference = branch picker in
 * `ShellToolbar` (icon + label + optional meta + chevron, height `--ctl-sm`,
 * action border). Not a native `<select>`.
 *
 * Full pattern: this trigger + `PopoverMenu` for the panel (search / tabs /
 * items). Domain menus (branches, revisions, …) stay in Features; this atom
 * only owns the chrome of the closed control.
 *
 * `forwardRef` so a `PopoverMenu` can anchor to the real button node.
 */
export type SelectProps = Omit<ButtonHTMLAttributes<HTMLButtonElement>, "children"> & {
  /** Leading glyph (`Ico` name). Omit for label-only. */
  icon?: string;
  /** Primary label (fades when long). */
  label: string;
  /** Optional trailing meta (e.g. revision code `r.12`). */
  meta?: string;
  /** Tooltip via the shared `data-tip` layer. */
  title?: string;
  /** Only when a real key handler exists in the code. */
  shortcut?: string;
};

export const Select = forwardRef<HTMLButtonElement, SelectProps>(function Select(
  {
    className,
    icon,
    label,
    meta,
    title,
    shortcut,
    "aria-label": ariaLabel,
    type = "button",
    ...props
  },
  ref,
) {
  const classNames = ["select", className ?? ""].filter(Boolean).join(" ");
  return (
    <button
      ref={ref}
      type={type}
      className={classNames}
      {...(title ? tip(title, shortcut) : {})}
      {...(ariaLabel ? { "aria-label": ariaLabel } : {})}
      {...props}
    >
      {icon ? <Ico name={icon} /> : null}
      <span className="select-label fade">{label}</span>
      {meta ? <span className="select-meta">{meta}</span> : null}
      <Ico name="chev" className="caret" />
    </button>
  );
});
