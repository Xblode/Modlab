import type { SessionReport } from "./api";

/** A short message pinned under the instance card: why a launch was
 * refused, or what SKSE's journals said about the session that just ended. */
export type LaunchNotice = {
  tone: "ok" | "warn" | "bad";
  title: string;
  lines: string[];
};

const plural = (n: number, one: string, many: string) => (n > 1 ? many : one);

/** A launch the Core refused (its message is already French; the "Lancement
 * annulé." lead-in becomes the title). */
export function noticeFromRefusal(message: string): LaunchNotice {
  const lead = "Lancement annulé.";
  const body = message.startsWith(lead) ? message.slice(lead.length).trim() : message;
  return { tone: "bad", title: "Lancement annulé", lines: [body] };
}

/** Informational notice under Jouer (vanilla fallback / offline notes). */
export function noticeFromInfo(message: string): LaunchNotice {
  return {
    tone: "warn",
    title: "Minecraft",
    lines: [message],
  };
}

/** The balance sheet of a finished session. */
export function noticeFromReport(r: SessionReport): LaunchNotice {
  if (r.sessionKind === "minecraft-smoke") {
    const lines: string[] = [];
    if (r.error) lines.push(r.error);
    for (const n of r.loaderNotes) lines.push(n);
    const ok = !r.error;
    return {
      tone: ok ? "ok" : "bad",
      title: ok ? "Test de lancement OK" : "Test de lancement échoué",
      lines: lines.length > 0 ? lines : [ok ? "Menu atteint." : "Échec."],
    };
  }
  if (r.sessionKind === "minecraft") {
    const lines: string[] = [];
    if (r.error) lines.push(r.error);
    for (const n of r.loaderNotes) lines.push(`Java : ${n}`);
    return {
      tone: r.error ? "warn" : "ok",
      title: r.error ? "Minecraft : session interrompue" : "Partie Minecraft terminée",
      lines,
    };
  }
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
  if (r.sessionKind === "minecraft-smoke") {
    if (r.error) return r.error;
    const note = r.loaderNotes.find((n) => n.startsWith("Test OK") || n.startsWith("Test "));
    return note ?? "Test de lancement terminé.";
  }
  if (r.sessionKind === "minecraft") {
    if (!r.error) return "Partie Minecraft terminée.";
    const tail = r.loaderNotes.length > 0 ? ` (${r.loaderNotes[r.loaderNotes.length - 1]})` : "";
    return `Partie terminée — ${r.error}${tail}`;
  }
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
