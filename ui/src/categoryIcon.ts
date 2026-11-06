/**
 * A deterministic icon per category name — same name always gets the same
 * icon, with no per-category storage anywhere (a category is just a free
 * tag on `mod_`, not a first-class Core entity yet). Picked from the
 * existing icon set rather than adding new art, biased toward glyphs that
 * read reasonably as "kind of mod" (palette, wand, shield…) without
 * pretending to know what any given tag actually means.
 */
const CATEGORY_ICONS = [
  "palette",
  "wand",
  "shield",
  "zap",
  "flag",
  "image",
  "box",
  "layers",
  "tool",
  "map",
];

export function categoryIcon(category: string): string {
  let hash = 0;
  for (let i = 0; i < category.length; i++) {
    hash = (hash * 31 + category.charCodeAt(i)) >>> 0;
  }
  return CATEGORY_ICONS[hash % CATEGORY_ICONS.length];
}
