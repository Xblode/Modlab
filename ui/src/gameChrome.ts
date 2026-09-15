/**
 * Per-game *presentation chrome* — the purely visual constants the maquette
 * carries in its `games` array: hero art, gradient colours, glyph, the tab set
 * a game exposes, and the human label of its deployment backend.
 *
 * This is design data, not state: nothing here is a fact about the machine or
 * about a loadout. Every *fact* on screen still comes from the Core through
 * `api.ts`.
 *
 * It lives in the frontend only because the Core's `GameAdapter` contract does
 * not expose any of it yet. When it does, this table shrinks to the art/colour
 * tokens and the tab set moves to the adapter — the game shape (does it have
 * plugins? archives? worlds?) is adapter knowledge, not UI knowledge.
 */

export type TabId =
  | "dashboard"
  | "mods"
  | "plugins"
  | "archives"
  | "repertoire"
  | "saves"
  | "sante"
  | "packs"
  | "worlds"
  | "servers";

export type TabDef = { id: TabId; label: string; icon: string };

const SKYRIM_TABS: TabDef[] = [
  { id: "dashboard", label: "Tableau de bord", icon: "sliders" },
  { id: "mods", label: "Mods", icon: "layers" },
  { id: "plugins", label: "Plugins", icon: "file" },
  { id: "archives", label: "Archives", icon: "db" },
  { id: "repertoire", label: "Répertoire", icon: "folder" },
  { id: "saves", label: "Sauvegardes", icon: "disc" },
  { id: "sante", label: "Santé", icon: "alert" },
];

const MINECRAFT_TABS: TabDef[] = [
  { id: "dashboard", label: "Tableau de bord", icon: "sliders" },
  { id: "mods", label: "Mods", icon: "layers" },
  { id: "packs", label: "Packs & shaders", icon: "image" },
  { id: "worlds", label: "Mondes", icon: "map" },
  { id: "servers", label: "Serveurs", icon: "globe" },
  { id: "sante", label: "Santé", icon: "alert" },
];

const FALLBACK_TABS: TabDef[] = [
  { id: "dashboard", label: "Tableau de bord", icon: "sliders" },
  { id: "mods", label: "Mods", icon: "layers" },
  { id: "sante", label: "Santé", icon: "alert" },
];

export type GameChrome = {
  glyph: string;
  /** Hero gradient, matching the maquette's `--c1g` / `--c2g`. */
  c1: string;
  c2: string;
  /** Full-bleed hero art, served from `ui/public`. Undefined → glyph tile. */
  art?: string;
  /** Deployment backend label. Belongs to the adapter; hardcoded here for now. */
  backend: string;
  /**
   * Script-extender / loader name, when the Core actually drives one. Only set
   * where it is a certainty (ModLab launches `skse64_loader.exe` itself) — the
   * *version* is still unknown and rendered as a placeholder.
   */
  engine?: string;
  tabs: TabDef[];
};

const CHROME: Record<string, GameChrome> = {
  skyrimse: {
    glyph: "SK",
    c1: "#3b6ea5",
    c2: "#1a2f4d",
    art: "img/header_skyrim.jpg",
    backend: "USVFS",
    engine: "SKSE",
    tabs: SKYRIM_TABS,
  },
  minecraft: {
    glyph: "MC",
    c1: "#5c8f3a",
    c2: "#2b3f1a",
    backend: "dossier mods/",
    tabs: MINECRAFT_TABS,
  },
};

const UNKNOWN: GameChrome = {
  glyph: "??",
  c1: "#4a4844",
  c2: "#2a2928",
  backend: "—",
  tabs: FALLBACK_TABS,
};

export function chromeFor(gameId: string | null | undefined): GameChrome {
  if (!gameId) return UNKNOWN;
  return CHROME[gameId] ?? UNKNOWN;
}

/** Two-letter thumbnail initials, same rule as the maquette's instance cards. */
export function initials(name: string): string {
  return name
    .split(/\s+/)
    .slice(0, 2)
    .map((w) => w[0] ?? "")
    .join("")
    .toUpperCase();
}
