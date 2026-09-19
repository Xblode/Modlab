import type { Ref } from "react";

/**
 * The maquette's two artwork frames — the instance card's square `.thumb` and
 * the game hero's wide `.art` — are the same object: a `--c1g`/`--c2g`
 * gradient tile that shows an image when there is one and falls back to
 * initials or a glyph when there is not.
 *
 * The gradient variables are *not* set here: they belong to the surrounding
 * card or hero, which is where the maquette puts them.
 */
type AvatarProps =
  | {
      /** Wide 460/215 hero frame. Its height is measured by the caller. */
      variant: "art";
      label: string;
      image?: string;
      ref?: Ref<HTMLDivElement>;
    }
  | {
      /** 40px square tile of an instance card. */
      variant: "thumb";
      label: string;
      image?: string;
    };

export function Avatar(props: AvatarProps) {
  const { label, image } = props;
  const background = image ? { backgroundImage: `url('${image}')` } : undefined;

  if (props.variant === "art") {
    return (
      <div
        className={`art ${image ? "has-img" : ""}`}
        ref={props.ref}
        aria-hidden="true"
        style={background}
      >
        {image ? null : label}
      </div>
    );
  }

  return (
    <span className={`thumb${image ? " has-img" : ""}`} style={background}>
      {image ? null : label}
    </span>
  );
}
