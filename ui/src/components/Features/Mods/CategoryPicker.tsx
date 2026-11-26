import { useRef, useState } from "react";
import type { GameCategory } from "../../../api";
import { presentCategory } from "../../../categoryIcon";
import { Ico } from "../../../Ico";
import { PopoverMenu, PopoverMenuItem, PopoverMenuSep } from "../../ui";

/**
 * Category cell: icon and label, and a menu of the game's categories.
 * Same hover as the other editable metadata cells.
 */
export function CategoryPicker({
  stored,
  userSet,
  catalog,
  onPick,
}: {
  stored: string | null;
  userSet: boolean;
  catalog: readonly GameCategory[];
  onPick: (category: string | null) => void;
}) {
  const anchor = useRef<HTMLButtonElement>(null);
  const [open, setOpen] = useState(false);
  const shown = presentCategory(stored, catalog);
  const empty = !stored?.trim();
  const title = empty
    ? "Choisir une catégorie"
    : userSet
      ? `${shown.label} — choisie à la main`
      : shown.label;

  return (
    <>
      <button
        ref={anchor}
        type="button"
        className="cat-btn"
        title={title}
        onClick={(event) => {
          event.stopPropagation();
          setOpen((value) => !value);
        }}
      >
        {shown.icon ? <Ico name={shown.icon} className="sm" /> : null}
        <span className="lbl">{shown.label}</span>
      </button>
      {open ? (
        <PopoverMenu
          chrome="context"
          title="Catégorie"
          className="cat-menu"
          anchorRef={anchor}
          onClose={() => setOpen(false)}
        >
          {catalog.map((item) => (
            <PopoverMenuItem
              key={item.id}
              icon={item.icon}
              label={item.label}
              current={item.id === stored}
              onPick={() => {
                setOpen(false);
                onPick(item.id);
              }}
            />
          ))}
          <PopoverMenuSep />
          <PopoverMenuItem
            icon="x"
            label="Aucune"
            current={empty}
            onPick={() => {
              setOpen(false);
              onPick(null);
            }}
          />
        </PopoverMenu>
      ) : null}
    </>
  );
}
