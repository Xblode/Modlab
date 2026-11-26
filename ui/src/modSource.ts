import type { ModRow } from "./api";

/** Fields the Source column / inspector line need. */
export type ModSourceFields = Pick<ModRow, "sourceHost" | "pageUrl">;

/**
 * Display name for a stored `sourceHost` token. `manual` is not a platform —
 * the table shows « Manuel » for that case instead.
 */
export function sourceHostLabel(host: string | null | undefined): string | null {
  switch ((host ?? "").trim().toLowerCase()) {
    case "nexus":
      return "Nexus";
    case "curseforge":
      return "CurseForge";
    case "modrinth":
      return "Modrinth";
    case "silverlock":
      return "Silverlock";
    case "other":
      return "Autre";
    case "unknown":
      return "Inconnu";
    default:
      return null;
  }
}

function hostFromPageUrl(pageUrl: string): string | null {
  try {
    const host = new URL(pageUrl).hostname.replace(/^www\./, "").toLowerCase();
    if (host === "nexusmods.com" || host.endsWith(".nexusmods.com")) return "Nexus";
    if (host === "curseforge.com" || host.endsWith(".curseforge.com")) return "CurseForge";
    if (host === "modrinth.com" || host.endsWith(".modrinth.com")) return "Modrinth";
    if (host.includes("silverlock")) return "Silverlock";
    return null;
  } catch {
    return null;
  }
}

export type ModSourceDisplay = {
  /** Short label shown in the Source column. */
  label: string;
  /** Tooltip: page URL, or « Ajouté manuellement ». */
  title: string;
  /** `globe` for a platform, `folder` for a manual disk add. */
  icon: "globe" | "folder";
  /** True when this row is a hand-added / disk import, not a platform capture. */
  manual: boolean;
};

/**
 * Source column rule: a platform host (or a page URL that names one) shows
 * the platform name; otherwise « Manuel » (added from disk, no platform page).
 */
export function modSourceDisplay(row: ModSourceFields): ModSourceDisplay {
  const host = row.sourceHost?.trim() ?? "";
  const page = row.pageUrl?.trim() ?? "";
  const manualToken = host.toLowerCase() === "manual";

  if (!manualToken) {
    const fromHost = sourceHostLabel(host);
    const fromPage = page ? hostFromPageUrl(page) : null;
    const label = fromHost ?? fromPage;
    if (label) {
      return {
        label,
        title: page || label,
        icon: "globe",
        manual: false,
      };
    }
    if (page) {
      return {
        label: "Page",
        title: page,
        icon: "globe",
        manual: false,
      };
    }
  }

  return {
    label: "Manuel",
    title: "Ajouté manuellement",
    icon: "folder",
    manual: true,
  };
}
