import type { ComponentProps } from "react";
import { Ico } from "../../Ico";

export function FavoriteButton({
  className,
  on,
  onToggle,
  titleOn,
  titleOff,
  ...props
}: ComponentProps<"button"> & {
  on: boolean;
  onToggle: () => void;
  titleOn?: string;
  titleOff?: string;
}) {
  const classNames = ["fav-star", className ?? ""]
    .filter(Boolean)
    .join(" ");
  return (
    <button
      className={classNames + (on ? " on" : "")}
      type="button"
      title={on ? (titleOn ?? "Retirer des favoris") : (titleOff ?? "Ajouter aux favoris")}
      aria-pressed={on}
      onClick={(e) => {
        e.stopPropagation();
        onToggle();
      }}
      {...props}
    >
      <Ico name="star" className={on ? "fill" : ""} />
    </button>
  );
}
