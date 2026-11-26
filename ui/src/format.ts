const monthYear = new Intl.DateTimeFormat("fr-FR", { month: "short", year: "numeric" });
const fullWhen = new Intl.DateTimeFormat("fr-FR", {
  day: "numeric",
  month: "long",
  year: "numeric",
  hour: "2-digit",
  minute: "2-digit",
});

function parsed(iso: string): Date | null {
  const date = new Date(iso);
  return Number.isNaN(date.getTime()) ? null : date;
}

/** `nov. 2026`. The day is not shown. */
export function formatMonthYear(iso: string): string | null {
  const date = parsed(iso);
  return date ? monthYear.format(date) : null;
}

/** Full local date, day and time included. */
export function formatDateTime(iso: string): string | null {
  const date = parsed(iso);
  return date ? fullWhen.format(date) : null;
}
