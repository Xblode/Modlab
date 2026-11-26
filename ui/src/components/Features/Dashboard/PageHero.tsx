import { useRef, useState, type ReactNode, type RefObject } from "react";
import { Ico } from "../../../Ico";
import { PopoverMenu, PopoverMenuItem, PopoverMenuSep } from "../../ui";

/** One row in a fact's edit menu (former Pop item shape). */
export type FactMenuItem =
  | "-"
  | {
      label: string;
      sub?: string;
      icon?: string;
      /** Pastille inline (ex. « Installé ») — même chrome que PopoverMenuItem. */
      badge?: string;
      current?: boolean;
      onPick?: () => void;
    };

export type HeaderFactEdit =
  | { mode: "action"; label: string; onClick: () => void }
  | { mode: "menu"; label: string; items: FactMenuItem[] };

export type HeaderFact = {
  id: string;
  icon: string;
  label: string;
  value: ReactNode;
  title?: string;
  /** A read still in flight, or a value the page does not have. */
  muted?: boolean;
  /** The value cell edits this fact. The label stays as it is. */
  edit?: HeaderFactEdit;
};

type Props = {
  name: string;
  facts: readonly HeaderFact[];
  children?: ReactNode;
};

/**
 * Sheet header: the name, then a two-column fact grid. A fact with `edit`
 * makes its value cell a button — same type, a hover fill, a light radius.
 * The page supplies every fact; this component does not invent a stat.
 */
export function PageHero({ name, facts, children }: Props) {
  const buttonRefs = useRef(new Map<string, HTMLButtonElement>());
  const anchorRef = useRef<HTMLButtonElement | null>(null);
  const [openId, setOpenId] = useState<string | null>(null);
  const openFact = facts.find((fact) => fact.id === openId && fact.edit?.mode === "menu");
  anchorRef.current = openFact ? (buttonRefs.current.get(openFact.id) ?? null) : null;

  return (
    <header className="ghero">
      <h1>
        <span className="fade">{name}</span>
      </h1>
      {facts.length > 0 ? (
        <dl className="game-facts">
          {facts.map((fact) => (
            <div key={fact.id} className="game-fact" title={fact.title}>
              <dt>
                <Ico name={fact.icon} />
                <span>{fact.label}</span>
              </dt>
              <dd className={fact.muted ? "muted" : undefined}>
                {fact.edit ? (
                  <button
                    type="button"
                    className="game-fact-value"
                    ref={(node) => {
                      if (node) buttonRefs.current.set(fact.id, node);
                      else buttonRefs.current.delete(fact.id);
                    }}
                    aria-label={fact.edit.label}
                    aria-haspopup={fact.edit.mode === "menu" ? "dialog" : undefined}
                    aria-expanded={fact.edit.mode === "menu" ? openId === fact.id : undefined}
                    onClick={() => {
                      if (fact.edit?.mode === "action") {
                        setOpenId(null);
                        fact.edit.onClick();
                        return;
                      }
                      setOpenId((current) => (current === fact.id ? null : fact.id));
                    }}
                  >
                    {fact.value}
                  </button>
                ) : (
                  fact.value
                )}
              </dd>
            </div>
          ))}
        </dl>
      ) : null}
      {openFact?.edit?.mode === "menu" ? (
        <FactMenu
          title={openFact.edit.label}
          anchorRef={anchorRef}
          items={openFact.edit.items}
          onClose={() => setOpenId(null)}
        />
      ) : null}
      {children}
    </header>
  );
}

function FactMenu({
  title,
  anchorRef,
  items,
  onClose,
}: {
  title: string;
  anchorRef: RefObject<HTMLButtonElement | null>;
  items: FactMenuItem[];
  onClose: () => void;
}) {
  return (
    <PopoverMenu chrome="context" title={title} anchorRef={anchorRef} onClose={onClose}>
      {items.map((it, i) =>
        it === "-" ? (
          <PopoverMenuSep key={`sep-${i}`} />
        ) : (
          <PopoverMenuItem
            key={`${i}-${it.label}`}
            icon={it.icon}
            label={it.label}
            sub={it.sub}
            badge={it.badge}
            current={it.current}
            onPick={() => {
              onClose();
              it.onPick?.();
            }}
          />
        ),
      )}
    </PopoverMenu>
  );
}
