import type { ModRow } from "../../api";

/** The Skyrim adapter already classified the row. No file-name rules here. */
export function looksLikeSkse(row: ModRow): boolean {
  return row.tool === "skse" || row.tool === "skse-vr";
}

/** What the Mods header shows for SKSE. Presence comes from the instance's
 * mod rows only — files left in the game folder do not count. */
export type SkseView = {
  value: string;
  title: string;
  muted: boolean;
  /** No matching row. The cell opens the official page. */
  absent?: boolean;
  /** The one row a click installs or toggles. */
  direct?: { row: ModRow; action: "toggle" | "install" };
  /** More than one archive. The cell opens a version menu. */
  choices?: readonly ModRow[];
  /** Row the menu marks, and whose version the cell prints. */
  currentId?: number;
};

export function describeSkse(entries: readonly ModRow[]): SkseView {
  const rows = entries.filter(looksLikeSkse);
  if (rows.length === 0) {
    return {
      value: "absent",
      title: "SKSE n’est pas dans cette instance. Ouvre la page officielle dans l’onglet Web.",
      muted: true,
      absent: true,
    };
  }

  const current = preferredSkse(rows);
  const version = versionLabel(current);
  const live = current.status === "installed" && current.enabled;

  if (rows.length > 1) {
    return {
      value: version ?? stateWord(current),
      title: "Plusieurs versions de SKSE. Cliquer pour en choisir une.",
      muted: !live,
      choices: rows,
      currentId: current.modId,
    };
  }

  if (current.status === "downloading") {
    return {
      value: "téléchargement…",
      title: "SKSE est en cours de téléchargement.",
      muted: true,
    };
  }

  if (current.status === "archived") {
    return {
      value: version ?? "à installer",
      title: version
        ? `${version} est dans les mods, pas encore installé.`
        : "SKSE est dans les mods, pas encore installé.",
      muted: true,
      direct: { row: current, action: "install" },
    };
  }

  return {
    value: version ?? (current.enabled ? "installé" : "inactif"),
    title: current.enabled
      ? "SKSE est activé sur cette instance. Cliquer pour le désactiver."
      : "SKSE est installé, inactif. Cliquer pour l’activer.",
    muted: !current.enabled,
    direct: { row: current, action: "toggle" },
    currentId: current.modId,
  };
}

export function skseChoiceLabel(row: ModRow): string {
  return versionLabel(row) ?? row.name;
}

export function skseChoiceSub(row: ModRow): string {
  if (row.status === "downloading") return "téléchargement…";
  if (row.status === "archived") return "à installer";
  return row.enabled ? "activé" : "installé, inactif";
}

/** Version the adapter attached to this row. Never parsed from the name. */
export function skseVersion(row: ModRow): string | null {
  if (!looksLikeSkse(row)) return null;
  const version = row.toolVersion?.trim();
  return version ? version : null;
}

function versionLabel(row: ModRow): string | null {
  const version = skseVersion(row);
  return version ? `v${version}` : null;
}

function stateWord(row: ModRow): string {
  if (row.status === "downloading") return "téléchargement…";
  if (row.status === "archived") return "à installer";
  return row.enabled ? "installé" : "inactif";
}

function preferredSkse(rows: readonly ModRow[]): ModRow {
  return (
    rows.find((row) => row.status === "installed" && row.enabled) ??
    rows.find((row) => row.status === "installed") ??
    rows[0]
  );
}
