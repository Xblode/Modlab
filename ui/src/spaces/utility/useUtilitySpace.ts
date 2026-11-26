import { type FormEvent, useEffect, useRef, useState } from "react";
import { aiAsk, browserHide, refreshModMetadata, type LoadoutDto } from "../../api";
import {
  dropRightTab,
  isWebTab,
  webPageIdFromTab,
  webTabId,
  withRightTabs,
  type RightPaneState,
  type RightTab,
  type WebPageTab,
} from "../../components/Features/Inspector/RightPanel";
import { type ChatMsg, type ChatTag } from "../../components/Features/Chat/ChatPanel";
import { initialRightPane } from "../../shell/shellNavHelpers";
import type { UiMemory } from "../../uiMemory";
import { isNewTabUrl, NEW_TAB_URL } from "../../webHomepage";

function modChatTag(mod: { modId: number; name: string }): ChatTag {
  return {
    id: `mod:${mod.modId}`,
    kind: "mod",
    modId: mod.modId,
    label: mod.name,
  };
}

/** Prefix the draft with @tags so the Core sees which mods were mentioned. */
function questionWithTags(draft: string, tags: readonly ChatTag[]): string {
  const body = draft.trim();
  if (tags.length === 0) return body;
  const mention = tags.map((t) => `@${t.label}`).join(" ");
  return body ? `${mention} ${body}` : mention;
}

function hostTitle(url: string): string {
  if (isNewTabUrl(url)) return "Nouvel onglet";
  try {
    return new URL(url).hostname.replace(/^www\./i, "") || "Web";
  } catch {
    return "Web";
  }
}

function newWebPage(url = NEW_TAB_URL): WebPageTab {
  return {
    id: `p-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 7)}`,
    url,
    title: hostTitle(url),
  };
}

export type UseUtilitySpaceParams = {
  initialMemory: UiMemory;
  activeLoadout: LoadoutDto | undefined;
  selectedMod: number | null;
  setSnap: React.Dispatch<React.SetStateAction<import("../../api").ShellSnapshot | null>>;
  setToast: (msg: string | null) => void;
  setSelectedMod: (id: number | null) => void;
};

export function useUtilitySpace(params: UseUtilitySpaceParams) {
  const { initialMemory, activeLoadout, selectedMod, setSnap, setToast, setSelectedMod } = params;

  const [pane, setPane] = useState<RightPaneState>(() => initialRightPane(initialMemory));
  const paneRef = useRef(pane);
  paneRef.current = pane;

  const [webPages, setWebPages] = useState<WebPageTab[]>([]);
  const webPagesRef = useRef(webPages);
  webPagesRef.current = webPages;
  const lastWebPageIdRef = useRef<string | null>(null);

  const [llm, setLlm] = useState(initialMemory.llm);
  const [chat, setChat] = useState<ChatMsg[]>([
    {
      who: "ai",
      text: "Je réponds d'abord par le moteur de règles du Core — conflits, mods actifs, dossiers illisibles. Avec un modèle GGUF déposé dans <racine données>/models, la même réponse est reformulée par le LLM local, sans y ajouter un seul fait.",
    },
  ]);
  const [draft, setDraft] = useState("");
  const [chatTags, setChatTags] = useState<ChatTag[]>([]);
  const [chatFocusNonce, setChatFocusNonce] = useState(0);

  let activeWebPage: WebPageTab | null = null;
  if (pane.active && isWebTab(pane.active)) {
    const id = webPageIdFromTab(pane.active);
    activeWebPage = webPages.find((p) => p.id === id) ?? null;
  }
  if (activeWebPage) lastWebPageIdRef.current = activeWebPage.id;

  /** Page shown in BrowserPane — keeps the last Web page mounted while Chat/etc. is focused. */
  const displayWebPage: WebPageTab | null =
    activeWebPage ??
    webPages.find((p) => p.id === lastWebPageIdRef.current) ??
    webPages[0] ??
    null;

  function openWeb(url?: string) {
    const landing = url?.trim() ? url.trim() : NEW_TAB_URL;
    const page = newWebPage(landing);
    setWebPages((prev) => [...prev, page]);
    const tab = webTabId(page.id);
    setPane((prev) => ({ tabs: withRightTabs(prev.tabs, [tab]), active: tab }));
  }

  function updateWebPage(pageId: string, url: string, title?: string) {
    setWebPages((prev) =>
      prev.map((p) =>
        p.id === pageId ? { ...p, url, title: title ?? hostTitle(url) } : p,
      ),
    );
  }

  function closeRight(id: RightTab) {
    if (isWebTab(id)) {
      const pageId = webPageIdFromTab(id);
      setWebPages((prev) => prev.filter((p) => p.id !== pageId));
      const remaining = webPagesRef.current.filter((p) => p.id !== pageId);
      if (remaining.length === 0) void browserHide();
    }
    setPane((prev) => dropRightTab(prev, id));
  }

  function closeRightPane() {
    if (webPagesRef.current.length > 0) void browserHide();
    setWebPages([]);
    setPane({ tabs: [], active: null });
  }

  function toggleRight(id: RightTab) {
    const current = paneRef.current;
    if (current.active === id) {
      closeRight(id);
      return;
    }
    setPane((prev) => ({ tabs: withRightTabs(prev.tabs, [id]), active: id }));
  }

  function openMod(id: number) {
    setSelectedMod(id);
    setPane((prev) => ({
      tabs: withRightTabs(prev.tabs, ["info"]),
      active: "info",
    }));
  }

  /** Open / focus the Chat tab without toggling it closed. */
  function openChat() {
    setPane((prev) => ({ tabs: withRightTabs(prev.tabs, ["claude"]), active: "claude" }));
  }

  /** Open / focus Console without toggling it closed (used when Jouer starts). */
  function openConsole() {
    setPane((prev) => ({ tabs: withRightTabs(prev.tabs, ["console"]), active: "console" }));
  }

  /**
   * Right-click → Chat on a mod: open the Chat tab, tag the mod in the
   * composer, select the row, and focus the field.
   */
  function tagModInChat(mod: { modId: number; name: string }) {
    const tag = modChatTag(mod);
    setSelectedMod(mod.modId);
    setChatTags((prev) => (prev.some((t) => t.id === tag.id) ? prev : [...prev, tag]));
    openChat();
    setChatFocusNonce((n) => n + 1);
  }

  function removeChatTag(id: string) {
    setChatTags((prev) => prev.filter((t) => t.id !== id));
  }

  async function sendChat(e?: FormEvent) {
    e?.preventDefault();
    const text = questionWithTags(draft, chatTags);
    if (!text) return;
    if (!activeLoadout) {
      setToast("Sélectionne une instance pour donner du contexte à l'IA.");
      return;
    }
    setDraft("");
    setChatTags([]);
    openChat();
    setChat((c) => [
      ...c,
      { who: "me", text },
      { who: "ai", text: "…", source: "pending" },
    ]);
    try {
      const ans = await aiAsk(text, activeLoadout.id);
      setChat((c) => {
        const next = [...c];
        next[next.length - 1] = { who: "ai", text: ans.text, source: ans.source };
        return next;
      });
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      setChat((c) => {
        const next = [...c];
        next[next.length - 1] = { who: "ai", text: `⚠ ${msg}`, source: "error" };
        return next;
      });
    }
  }

  useEffect(() => {
    const loadout = activeLoadout?.id;
    if (selectedMod == null || loadout == null || pane.active !== "info") return;
    let cancelled = false;
    refreshModMetadata(loadout, selectedMod)
      .then((rows) => {
        if (cancelled || !rows) return;
        setSnap((s) => (s ? { ...s, entries: rows } : s));
      })
      .catch(() => {});
    return () => {
      cancelled = true;
    };
  }, [selectedMod, activeLoadout?.id, pane.active, setSnap]);

  return {
    pane,
    setPane,
    paneRef,
    webPages,
    activeWebPage,
    displayWebPage,
    updateWebPage,
    llm,
    setLlm,
    chat,
    draft,
    setDraft,
    chatTags,
    chatFocusNonce,
    openWeb,
    closeRight,
    closeRightPane,
    toggleRight,
    openChat,
    openConsole,
    openMod,
    tagModInChat,
    removeChatTag,
    sendChat,
  };
}
