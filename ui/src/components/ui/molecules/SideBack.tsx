import { Ico } from "../../../Ico";

/**
 * A "back" row for any sub-page hosted in the sidebar's body — Réglages
 * today, whatever else grows the same shape tomorrow. Deliberately generic:
 * it only knows a label and a click, never what it is going back from.
 */
export function SideBack({ label, onClick }: { label: string; onClick: () => void }) {
  return (
    <button type="button" className="side-back" onClick={onClick}>
      <Ico name="arrowleft" />
      <span className="lbl">{label}</span>
    </button>
  );
}
