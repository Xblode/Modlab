import type { SessionReport, SkseStatus } from "./api";

/** A short message pinned under the instance card: why a launch was
 * refused, or what SKSE's journals said about the session that just ended. */
export type LaunchNotice = {
  tone: "ok" | "warn" | "bad";
  title: string;
  lines: string[];
};

const plural = (n: number, one: string, many: string) => (n > 1 ? many : one);

/** The engine indicator's chip: what the sidebar shows for SKSE. Read from
 * the game folder (loader and the DLL of the game's version), never from
 * a mod's name. */
export function skseChip(status: SkseStatus | null): {
  sub: string;
  tone: "ok" | "warn" | "off";
  hint: string;
} {
  if (!status) {
    return { sub: "non vérifié", tone: "off", hint: "L’état de SKSE n’a pas pu être lu." };
  }
  switch (status.state) {
    case "ok":
      return { sub: status.gameVersion ?? "prêt", tone: "ok", hint: status.message };
    case "wrong_version":
      return { sub: "mauvaise version", tone: "warn", hint: status.message };
    case "unverifiable":
      return { sub: "non vérifiable", tone: "warn", hint: status.message };
    default:
      return { sub: "absent", tone: "off", hint: status.message };
  }
}

/** A launch the Core refused (its message is already French; the "Lancement
 * annulé." lead-in becomes the title). */
export function noticeFromRefusal(message: string): LaunchNotice {
  const lead = "Lancement annulé.";
  const body = message.startsWith(lead) ? message.slice(lead.length).trim() : message;
  return { tone: "bad", title: "Lancement annulé", lines: [body] };
}

/** The balance sheet of a finished session. */
export function noticeFromReport(r: SessionReport): LaunchNotice {
  const lines: string[] = [];
  if (r.error) lines.push(`Suivi de la partie : ${r.error}`);

  if (!r.logFound) {
    lines.push("skse64.log est introuvable : SKSE n’a rien écrit, il ne s’est probablement pas lancé.");
    return { tone: "bad", title: "SKSE : aucune trace", lines };
  }
  if (!r.logFresh) {
    lines.push(
      "skse64.log n’a pas été mis à jour par cette partie : SKSE ne s’est probablement pas lancé.",
    );
    return { tone: "bad", title: "SKSE : aucune trace de cette partie", lines };
  }
  if (!r.skseInitialized) {
    lines.push("Le journal de SKSE ne montre pas d’initialisation.");
    lines.push(...r.loaderNotes.map((n) => `Loader : ${n}`));
    return { tone: "bad", title: "SKSE n’a pas démarré", lines };
  }

  const n = r.loaded.length;
  lines.push(
    n === 0
      ? "Aucun plugin SKSE chargé."
      : `${n} ${plural(n, "plugin SKSE chargé", "plugins SKSE chargés")}.`,
  );
  const shown = r.rejected.slice(0, 5);
  for (const p of shown) lines.push(`Refusé : ${p.name} — ${p.reason}`);
  if (r.rejected.length > shown.length) {
    lines.push(`… et ${r.rejected.length - shown.length} autre(s) refusé(s).`);
  }
  if (r.notSeen.length > 0) {
    const names = r.notSeen.slice(0, 5).join(", ");
    const more = r.notSeen.length > 5 ? ` … (+${r.notSeen.length - 5})` : "";
    lines.push(`Jamais vus par SKSE : ${names}${more}`);
  }
  lines.push(...r.loaderNotes.map((x) => `Loader : ${x}`));

  const warn = r.rejected.length > 0 || r.notSeen.length > 0 || r.loaderNotes.length > 0 || !!r.error;
  return {
    tone: warn ? "warn" : "ok",
    title: `SKSE initialisé${r.skseVersion ? ` (${r.skseVersion})` : ""}`,
    lines,
  };
}

/** The one-line toast shown when a session ends. */
export function toastFromReport(r: SessionReport): string {
  if (!r.logFound || !r.logFresh || !r.skseInitialized) {
    return "Partie terminée — SKSE ne s’est pas initialisé (voir le bilan).";
  }
  const n = r.loaded.length;
  const refused = r.rejected.length;
  return (
    `Partie terminée — SKSE initialisé, ${n} ${plural(n, "plugin chargé", "plugins chargés")}` +
    (refused > 0 ? `, ${refused} ${plural(refused, "refusé", "refusés")}` : "") +
    "."
  );
}
