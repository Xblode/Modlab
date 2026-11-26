import type { FormEvent, MouseEvent, ReactNode } from "react";
import { IconButton } from "../atoms/IconButton";

/**
 * Shared modal shell: backdrop, title, close, optional action row.
 * Callers keep their own body. Markup matches the dialogs that used to
 * repeat `modal-backdrop` / `modal` / `modal-head` / `modal-actions`.
 */
type SharedProps = {
  titleId: string;
  title: ReactNode;
  /** Small line above the title. Omit it and the title sits directly in the head. */
  eyebrow?: ReactNode;
  onClose: () => void;
  /** Accessible name of the close icon. Defaults to Fermer. */
  closeTitle?: string;
  /**
   * Backdrop press closes the dialog. Off for a flow that must not be
   * dismissed by a click outside (the FOMOD installer).
   */
  dismissOnBackdrop?: boolean;
  role?: "dialog" | "alertdialog";
  className?: string;
  children?: ReactNode;
  actions?: ReactNode;
};

type DialogProps = SharedProps &
  (
    | { as?: "div" }
    | { as: "form"; onSubmit: (event: FormEvent<HTMLFormElement>) => void }
  );

function stop(event: MouseEvent) {
  event.stopPropagation();
}

export function Dialog(props: DialogProps) {
  const {
    titleId,
    title,
    eyebrow,
    onClose,
    closeTitle = "Fermer",
    dismissOnBackdrop = true,
    role,
    className,
    children,
    actions,
  } = props;
  const panelClass = ["modal", className ?? ""].filter(Boolean).join(" ");
  const inner = (
    <>
      <div className="modal-head">
        {eyebrow ? (
          <div>
            <div className="eyebrow">{eyebrow}</div>
            <h2 id={titleId}>{title}</h2>
          </div>
        ) : (
          <h2 id={titleId}>{title}</h2>
        )}
        <IconButton name="x" title={closeTitle} onClick={onClose} />
      </div>
      {children}
      {actions != null ? <div className="modal-actions">{actions}</div> : null}
    </>
  );

  return (
    <div
      className="modal-backdrop"
      role="presentation"
      onMouseDown={dismissOnBackdrop ? onClose : undefined}
    >
      {props.as === "form" ? (
        <form className={panelClass} aria-labelledby={titleId} onSubmit={props.onSubmit} onMouseDown={stop}>
          {inner}
        </form>
      ) : (
        <div className={panelClass} role={role} aria-labelledby={titleId} onMouseDown={stop}>
          {inner}
        </div>
      )}
    </div>
  );
}
