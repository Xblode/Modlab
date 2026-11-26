/**
 * Icon for a legacy free-text tag that is not one of the game's category
 * ids. A known id uses the icon the adapter sent (`presentCategory`).
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

export type CategoryRef = { id: string; label: string; icon: string };

/** Icon and French label for a stored category id. Empty stays a dash. */
export function presentCategory(
  stored: string | null | undefined,
  catalog: readonly CategoryRef[],
): { icon: string | null; label: string } {
  const value = stored?.trim() ?? "";
  if (!value) return { icon: null, label: "—" };
  const known = catalog.find((item) => item.id === value);
  if (known) return { icon: known.icon, label: known.label };
  return { icon: categoryIcon(value), label: value };
}
