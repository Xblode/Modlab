/**
 * Embedded-browser homepage and omnibox helpers — UI-only, stored in
 * `uiMemory`. The Core never sees these preferences.
 */

export type WebHomepageKind = "google" | "bing" | "yandex" | "custom";

export type WebHomepage = {
  kind: WebHomepageKind;
  /** Used when `kind === "custom"`. May include `%s` for search substitution. */
  customUrl: string;
};

export const DEFAULT_WEB_HOMEPAGE: WebHomepage = {
  kind: "google",
  customUrl: "",
};

/** Sentinel shown as the new-tab screen inside `BrowserPane` (not loaded in WebView2). */
export const NEW_TAB_URL = "modlab://new-tab";

export const WEB_HOMEPAGE_OPTIONS: readonly {
  kind: WebHomepageKind;
  label: string;
}[] = [
  { kind: "google", label: "Google" },
  { kind: "bing", label: "Bing" },
  { kind: "yandex", label: "Yandex" },
  { kind: "custom", label: "Adresse personnalisée" },
];

export function isWebHomepageKind(value: unknown): value is WebHomepageKind {
  return value === "google" || value === "bing" || value === "yandex" || value === "custom";
}

export function sanitizeWebHomepage(value: unknown): WebHomepage {
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    return { ...DEFAULT_WEB_HOMEPAGE };
  }
  const record = value as Record<string, unknown>;
  const kind = isWebHomepageKind(record.kind) ? record.kind : DEFAULT_WEB_HOMEPAGE.kind;
  const customUrl =
    typeof record.customUrl === "string" ? record.customUrl.trim() : "";
  return { kind, customUrl };
}

export function isNewTabUrl(url: string | null | undefined): boolean {
  if (url == null || url === "") return true;
  return url === NEW_TAB_URL || url === "about:blank";
}

/** Real page for the engine (or custom address). Used by the new-tab “home” link. */
export function engineHomeUrl(hp: WebHomepage): string {
  switch (hp.kind) {
    case "google":
      return "https://www.google.com/";
    case "bing":
      return "https://www.bing.com/";
    case "yandex":
      return "https://yandex.com/";
    case "custom": {
      const raw = hp.customUrl.trim();
      if (!raw) return "https://www.google.com/";
      // Strip a trailing %s search template if present.
      const withoutQuery = raw.includes("%s") ? raw.split("%s")[0]!.replace(/[?&]$/, "") : raw;
      const base = withoutQuery.trim() || "https://www.google.com/";
      return /^https?:\/\//i.test(base) ? base : `https://${base}`;
    }
  }
}

export function searchUrlFor(hp: WebHomepage, query: string): string {
  const q = encodeURIComponent(query);
  switch (hp.kind) {
    case "google":
      return `https://www.google.com/search?q=${q}`;
    case "bing":
      return `https://www.bing.com/search?q=${q}`;
    case "yandex":
      return `https://yandex.com/search/?text=${q}`;
    case "custom": {
      const raw = hp.customUrl.trim();
      if (raw.includes("%s")) {
        const template = /^https?:\/\//i.test(raw) ? raw : `https://${raw}`;
        return template.split("%s").join(q);
      }
      return `https://www.google.com/search?q=${q}`;
    }
  }
}

/**
 * True when the omnibox text should navigate as an address rather than a
 * search query: scheme present, or a plausible host (dots / localhost / IP).
 */
export function looksLikeUrl(text: string): boolean {
  const t = text.trim();
  if (!t) return false;
  if (/^[a-z][a-z0-9+.-]*:/i.test(t)) return true;
  if (/^(localhost|(\d{1,3}\.){3}\d{1,3})(:\d+)?(\/|\?|#|$)/i.test(t)) return true;
  if (/^[a-z0-9]([a-z0-9-]*[a-z0-9])?(\.[a-z0-9]([a-z0-9-]*[a-z0-9])?)+([/:?#].*)?$/i.test(t)) {
    return true;
  }
  return false;
}

/** Turn omnibox input into a navigable URL (address or search). */
export function resolveOmnibox(text: string, hp: WebHomepage): string {
  const t = text.trim();
  if (!t) return NEW_TAB_URL;
  if (!looksLikeUrl(t)) return searchUrlFor(hp, t);
  if (/^[a-z][a-z0-9+.-]*:/i.test(t)) return t;
  return `https://${t}`;
}

/** Stable id for a custom favourite site derived from its URL. */
export function customSiteIdForUrl(url: string): string {
  let host = url.trim().toLowerCase();
  try {
    host = new URL(/^https?:\/\//i.test(url) ? url : `https://${url}`).hostname;
  } catch {
    /* keep raw */
  }
  const safe = host.replace(/[^a-z0-9.-]+/g, "-").replace(/^-+|-+$/g, "") || "site";
  return `custom:${safe}`;
}

export function labelForUrl(url: string): string {
  try {
    const parsed = new URL(/^https?:\/\//i.test(url) ? url : `https://${url}`);
    return parsed.hostname.replace(/^www\./i, "") || parsed.href;
  } catch {
    return url;
  }
}
