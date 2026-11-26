import {
  useEffect,
  useRef,
  useState,
  type FormEvent,
  type ReactNode,
  type RefObject,
} from "react";
import {
  KIND_LABELS,
  runPowerShell,
  type GameCategory,
  type GameLogChunk,
  type ModConflicts,
  type ModRow,
  type ResolvedDependency,
  type Transaction,
} from "../../../api";
import { Ico } from "../../../Ico";
import { SideDrawer } from "../../../shell/utilityColumnChrome";
import { IconButton, PopoverMenu, PopoverMenuItem } from "../../ui";
import { ChatPanel, type ChatMsg, type ChatTag } from "../Chat/ChatPanel";
import { ConfigPane } from "./ConfigPane";
import { ModInspector } from "./ModInspector";

/** Fixed utility tools in the right strip (not Web pages). */
export const FIXED_RIGHT_TABS = [
  { id: "info", label: "Info" },
  { id: "claude", label: "Chat" },
  { id: "console", label: "Console" },
  { id: "search", label: "Recherche" },
] as const;

export type FixedRightTab = (typeof FIXED_RIGHT_TABS)[number]["id"];

/** One Web page in the same strip as Chat / Console — id is the page key. */
export type WebTabId = `web:${string}`;

export type RightTab = FixedRightTab | WebTabId;

/** @deprecated Alias kept for older imports; prefer FIXED_RIGHT_TABS. */
export const RIGHT_TABS = FIXED_RIGHT_TABS;

/** Which tabs are in the strip, and which one is on screen. Empty means the column is closed. */
export type RightPaneState = {
  tabs: RightTab[];
  active: RightTab | null;
};

/** Metadata for a Web page shown as a drawer tab. */
export type WebPageTab = {
  id: string;
  url: string;
  title: string;
};

const WEB_PREFIX = "web:";

export function isWebTab(value: unknown): value is WebTabId {
  return typeof value === "string" && value.startsWith(WEB_PREFIX) && value.length > WEB_PREFIX.length;
}

export function webTabId(pageId: string): WebTabId {
  return `${WEB_PREFIX}${pageId}`;
}

export function webPageIdFromTab(tab: WebTabId): string {
  return tab.slice(WEB_PREFIX.length);
}

export function isFixedRightTab(value: unknown): value is FixedRightTab {
  return FIXED_RIGHT_TABS.some((item) => item.id === value);
}

export function isRightTab(value: unknown): value is RightTab {
  return isFixedRightTab(value) || isWebTab(value);
}

/**
 * A stored tab id, or `null` when it is not one we still open.
 * Older sessions kept a separate Config tab; that content now lives in Info,
 * so `config` folds into `info`. Legacy singular `"web"` is dropped — Web
 * pages are session-only `web:<pageId>` tabs and are not restored from memory.
 */
export function canonicalRightTab(value: unknown): RightTab | null {
  if (value === "config") return "info";
  if (value === "web") return null;
  if (isWebTab(value)) return value;
  return isFixedRightTab(value) ? value : null;
}

/** Fixed tabs only — used when reading/writing `uiMemory` (Web pages are not persisted). */
export function sanitizePersistedRightTabs(value: unknown): FixedRightTab[] {
  if (!Array.isArray(value)) return [];
  const tabs: FixedRightTab[] = [];
  for (const item of value) {
    const id = canonicalRightTab(item);
    if (id && isFixedRightTab(id) && !tabs.includes(id)) tabs.push(id);
  }
  return tabs;
}

/** Known ids only (fixed + live Web pages), first occurrence kept, in open order. */
export function sanitizeRightTabs(value: unknown): RightTab[] {
  if (!Array.isArray(value)) return [];
  const tabs: RightTab[] = [];
  for (const item of value) {
    const id = canonicalRightTab(item);
    if (id && !tabs.includes(id)) tabs.push(id);
  }
  return tabs;
}

/** Appends tabs that are not open yet. Tabs already in the strip stay put. */
export function withRightTabs(open: readonly RightTab[], add: readonly RightTab[]): RightTab[] {
  const tabs = sanitizeRightTabs(open);
  for (const id of add) {
    if (!tabs.includes(id)) tabs.push(id);
  }
  return tabs;
}

/**
 * Drops one tab. Closing the active tab focuses the one that was on its right,
 * or the one on its left when it was last. An empty strip closes the column.
 */
export function dropRightTab(state: RightPaneState, id: RightTab): RightPaneState {
  const tabs = state.tabs.filter((tab) => tab !== id);
  if (tabs.length === 0) return { tabs: [], active: null };
  if (state.active && state.active !== id && tabs.includes(state.active)) {
    return { tabs, active: state.active };
  }
  const index = state.tabs.indexOf(id);
  const neighbor = state.tabs[index + 1] ?? state.tabs[index - 1];
  const active = neighbor && tabs.includes(neighbor) ? neighbor : tabs[0];
  return { tabs, active };
}

const TAB_LABEL = new Map(FIXED_RIGHT_TABS.map((item) => [item.id, item.label]));

function labelForTab(id: RightTab, webPages: readonly WebPageTab[]): string {
  if (isWebTab(id)) {
    const page = webPages.find((p) => webTabId(p.id) === id);
    return page?.title || "Web";
  }
  return TAB_LABEL.get(id) ?? id;
}

/**
 * Contenu + bandeau d’onglets de la colonne utilitaire shell (`LayoutShell` slot `right`).
 * Le cadre `aside.utility-col` est `SideDrawer` colonne via `shell/utilityColumnChrome`.
 * The right column: one strip of open tabs (Info, Chat, Console, Web pages,
 * Recherche). Web pages sit in the same strip as Chat — there is no nested
 * tab bar inside the browser. The column is closed when the strip is empty.
 * The header cross closes every tab at once; closing the last tab does the
 * same. Browser stays mounted while any Web page tab is open.
 */
export function RightPanel({
  className = "utility-col",
  tabs,
  active,
  onFocus,
  onCloseTab,
  onClosePane,
  mod,
  conflicts,
  dependencies,
  loadoutId,
  chat,
  llm,
  draft,
  chatTags = [],
  chatFocusNonce = 0,
  onDraftChange,
  onRemoveChatTag,
  onSendChat,
  browser,
  webPages = [],
  transactions,
  gameLog = [],
  onOpenPage,
  catalog,
  onSetCategory,
  onOpenWeb,
  onOpenConsole,
  onOpenChat,
  running = false,
  gameName,
}: {
  /** Must include `utility-col` so the shell resizes this column. */
  className?: string;
  tabs: readonly RightTab[];
  active: RightTab | null;
  onFocus: (tab: RightTab) => void;
  onCloseTab: (tab: RightTab) => void;
  /** Empties the strip. The caller hides the column; width stays remembered. */
  onClosePane: () => void;
  mod: ModRow | null;
  conflicts: ModConflicts | undefined;
  /** `undefined` while the branch has not been scanned. */
  dependencies: ResolvedDependency[] | undefined;
  loadoutId: number | null;
  chat: ChatMsg[];
  llm: string;
  draft: string;
  /** Composer tags from right-click → Chat (mods). */
  chatTags?: readonly ChatTag[];
  chatFocusNonce?: number;
  onDraftChange: (value: string) => void;
  onRemoveChatTag?: (id: string) => void;
  onSendChat: (e?: FormEvent) => void;
  /** Mounted while any Web page tab is open. `null` until then. */
  browser: ReactNode;
  /** Live Web pages — titles for the strip; ids match `web:<id>` tabs. */
  webPages?: readonly WebPageTab[];
  transactions: Transaction[] | null;
  /** Live Minecraft JVM log chunks for Console → Jeu. */
  gameLog?: readonly GameLogChunk[];
  /** Opens a mod page in a new Web tab. */
  onOpenPage: (url: string) => void;
  catalog: GameCategory[];
  onSetCategory: (category: string | null) => void;
  /** Drawer « + » → new Web page / Console / Chat. */
  onOpenWeb?: () => void;
  onOpenConsole?: () => void;
  onOpenChat?: () => void;
  /** Live game session — Chat shows a game-name pill; Console prefers Jeu. */
  running?: boolean;
  gameName?: string | null;
}) {
  const open = sanitizeRightTabs(tabs);
  const current: RightTab | null = active && open.includes(active) ? active : (open[0] ?? null);
  const tabsRef = useRef<HTMLDivElement>(null);
  const addRef = useRef<HTMLButtonElement>(null);
  const [addOpen, setAddOpen] = useState(false);
  const hasWeb = open.some(isWebTab);
  const webActive = current != null && isWebTab(current);

  useEffect(() => {
    const selected = tabsRef.current?.querySelector<HTMLElement>('[aria-selected="true"]');
    selected?.scrollIntoView({ inline: "nearest", block: "nearest" });
  }, [current]);

  if (current == null) return null;

  return (
    <SideDrawer
      layout="column"
      className={className}
      label="Panneau"
      header={
        <>
          <div className="rhead-tabs" role="tablist" aria-label="Onglets du panneau" ref={tabsRef}>
            {open.map((id) => {
              const label = labelForTab(id, webPages);
              const selected = current === id;
              return (
                <div
                  key={id}
                  className="tab"
                  role="tab"
                  aria-selected={selected}
                  tabIndex={selected ? 0 : -1}
                  onClick={() => onFocus(id)}
                  onKeyDown={(event) => {
                    if (event.key === "Enter" || event.key === " ") {
                      event.preventDefault();
                      onFocus(id);
                    }
                  }}
                >
                  <span>{label}</span>
                  <button
                    type="button"
                    className="x"
                    aria-label={`Fermer ${label}`}
                    onClick={(event) => {
                      event.stopPropagation();
                      onCloseTab(id);
                    }}
                  >
                    <Ico name="x" className="sm" />
                  </button>
                </div>
              );
            })}
            {onOpenWeb || onOpenConsole || onOpenChat ? (
              <IconButton
                ref={addRef}
                name="plus"
                size="sm"
                title="Nouvel onglet"
                className="rhead-add"
                onClick={() => setAddOpen((v) => !v)}
              />
            ) : null}
          </div>
          <IconButton
            name="x"
            size="sm"
            title="Fermer le panneau"
            className="rhead-close"
            onClick={onClosePane}
          />
          {addOpen ? (
            <PopoverMenu
              chrome="context"
              title="Ouvrir"
              anchorRef={addRef}
              onClose={() => setAddOpen(false)}
            >
              {onOpenWeb ? (
                <PopoverMenuItem
                  icon="globe"
                  label="Web"
                  onPick={() => {
                    onOpenWeb();
                    setAddOpen(false);
                  }}
                />
              ) : null}
              {onOpenConsole ? (
                <PopoverMenuItem
                  icon="term"
                  label="Console"
                  onPick={() => {
                    onOpenConsole();
                    setAddOpen(false);
                  }}
                />
              ) : null}
              {onOpenChat ? (
                <PopoverMenuItem
                  icon="sparkle"
                  label="Chat"
                  onPick={() => {
                    onOpenChat();
                    setAddOpen(false);
                  }}
                />
              ) : null}
            </PopoverMenu>
          ) : null}
        </>
      }
    >
      {open.includes("info") ? (
        <div className="drawer-tab" role="tabpanel" hidden={current !== "info"}>
          <ModInspector
            mod={mod}
            conflicts={conflicts}
            dependencies={dependencies}
            onOpenPage={onOpenPage}
            catalog={catalog}
            onSetCategory={onSetCategory}
          />
          <ConfigPane
            key={mod ? `${loadoutId ?? "x"}-${mod.modId}-${mod.sourcePath}` : "none"}
            mod={mod}
            loadoutId={loadoutId}
          />
        </div>
      ) : null}
      {open.includes("claude") ? (
        <div className="drawer-tab is-fill" role="tabpanel" hidden={current !== "claude"}>
          <ChatPanel
            messages={chat}
            llm={llm}
            draft={draft}
            tags={chatTags}
            focusNonce={chatFocusNonce}
            running={running}
            gameName={gameName}
            onDraftChange={onDraftChange}
            onRemoveTag={onRemoveChatTag}
            onSubmit={onSendChat}
          />
        </div>
      ) : null}
      {open.includes("console") ? (
        <div className="drawer-tab is-fill" role="tabpanel" hidden={current !== "console"}>
          <ConsolePanel txs={transactions} gameLog={gameLog} running={running} />
        </div>
      ) : null}
      {hasWeb ? (
        <div className="drawer-tab is-fill" role="tabpanel" hidden={!webActive}>
          {browser ?? <ShellTab eyebrow="Web" title="Web" text="Le navigateur s’ouvrira ici." />}
        </div>
      ) : null}
      {open.includes("search") ? (
        <div className="drawer-tab" role="tabpanel" hidden={current !== "search"}>
          <ShellTab eyebrow="Recherche" title="Recherche" text="La recherche s’ouvrira ici." />
        </div>
      ) : null}
    </SideDrawer>
  );
}

/** Short shell for a drawer tab whose tool is not in this panel yet. */
function ShellTab({ eyebrow, title, text }: { eyebrow: string; title: string; text: string }) {
  return (
    <div className="rtitle">
      <div className="eyebrow">{eyebrow}</div>
      <h2>{title}</h2>
      <p className="muted">{text}</p>
    </div>
  );
}

function formatConsoleTime(iso: string): string {
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) return iso;
  return date.toLocaleTimeString("fr-FR", {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  });
}

type ConsoleMode = "journal" | "game" | "powershell";

type PsLine = { kind: "in" | "out" | "err" | "meta"; text: string };

/** Console drawer: Journal (txs) + Jeu (game log) + PowerShell (UI REPL). */
function ConsolePanel({
  txs,
  gameLog,
  running,
}: {
  txs: Transaction[] | null;
  gameLog: readonly GameLogChunk[];
  running: boolean;
}) {
  const [mode, setMode] = useState<ConsoleMode>(running ? "game" : "journal");
  const logEndRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (running) setMode("game");
  }, [running]);

  useEffect(() => {
    if (mode !== "game") return;
    logEndRef.current?.scrollIntoView({ block: "end" });
  }, [mode, gameLog.length]);

  return (
    <div className="console-panel">
      <div className="console-modes" role="tablist" aria-label="Mode console">
        <button
          type="button"
          role="tab"
          aria-selected={mode === "journal"}
          className={mode === "journal" ? "is-on" : undefined}
          onClick={() => setMode("journal")}
        >
          Journal
        </button>
        <button
          type="button"
          role="tab"
          aria-selected={mode === "game"}
          className={mode === "game" ? "is-on" : undefined}
          onClick={() => setMode("game")}
        >
          Jeu
          {running ? <span className="console-live" aria-hidden /> : null}
        </button>
        <button
          type="button"
          role="tab"
          aria-selected={mode === "powershell"}
          className={mode === "powershell" ? "is-on" : undefined}
          onClick={() => setMode("powershell")}
        >
          PowerShell
        </button>
      </div>
      {mode === "journal" ? (
        <ConsoleJournal txs={txs} />
      ) : mode === "game" ? (
        <ConsoleGameLog chunks={gameLog} logEndRef={logEndRef} running={running} />
      ) : (
        <ConsolePowerShell />
      )}
    </div>
  );
}

/** The existing `.console` log surface, filled with the Core's journal. */
function ConsoleJournal({ txs }: { txs: Transaction[] | null }) {
  if (txs == null) {
    return <div className="console">Chargement du journal…</div>;
  }
  if (txs.length === 0) {
    return (
      <div className="console">
        Journal vide.
        <span className="muted"> Importe une instance ou prends un instantané.</span>
      </div>
    );
  }
  return (
    <div className="console">
      {txs.map((tx) => (
        <div key={tx.id} className="utility-console-entry">
          <div>
            <span className="ts">{formatConsoleTime(tx.occurredAt)}</span>{" "}
            <span className="lv">INFO</span> {KIND_LABELS[tx.kind] ?? tx.kind}
          </div>
          {tx.steps.map((step, index) => (
            <div key={index} className="utility-console-step">
              {step}
            </div>
          ))}
        </div>
      ))}
    </div>
  );
}

function ConsoleGameLog({
  chunks,
  logEndRef,
  running,
}: {
  chunks: readonly GameLogChunk[];
  logEndRef: RefObject<HTMLDivElement | null>;
  running: boolean;
}) {
  if (chunks.length === 0) {
    return (
      <div className="console">
        {running ? (
          <>
            Session en cours.
            <span className="muted"> La sortie Java apparaîtra ici.</span>
          </>
        ) : (
          <>
            Aucun log de session.
            <span className="muted"> Lancez Jouer pour voir stdout/stderr ici.</span>
          </>
        )}
      </div>
    );
  }
  return (
    <div className="console console-game">
      {chunks.map((chunk, index) => (
        <span
          key={index}
          className={chunk.stream === "stderr" ? "console-game-err" : "console-game-out"}
        >
          {chunk.text}
        </span>
      ))}
      <div ref={logEndRef} />
    </div>
  );
}

/** Minimal PowerShell REPL: one-shot `powershell -Command` via Tauri (UI only). */
function ConsolePowerShell() {
  const [lines, setLines] = useState<PsLine[]>([
    {
      kind: "meta",
      text: "PowerShell (session courte). Entrez une commande ci-dessous.",
    },
  ]);
  const [input, setInput] = useState("");
  const [cwd, setCwd] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const endRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    endRef.current?.scrollIntoView({ block: "end" });
  }, [lines.length, busy]);

  async function onSubmit(e: FormEvent) {
    e.preventDefault();
    const command = input.trim();
    if (!command || busy) return;
    setInput("");
    setBusy(true);
    setLines((prev) => [...prev, { kind: "in", text: command }]);
    try {
      const result = await runPowerShell(command, cwd);
      setCwd(result.cwd);
      const next: PsLine[] = [];
      if (result.stdout) next.push({ kind: "out", text: result.stdout });
      if (result.stderr) next.push({ kind: "err", text: result.stderr });
      if (result.exitCode !== 0) {
        next.push({ kind: "meta", text: `code ${result.exitCode}` });
      }
      if (next.length > 0) setLines((prev) => [...prev, ...next]);
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      setLines((prev) => [...prev, { kind: "err", text: message }]);
    } finally {
      setBusy(false);
      inputRef.current?.focus();
    }
  }

  return (
    <div className="console-ps">
      <div className="console console-ps-log" onClick={() => inputRef.current?.focus()}>
        {lines.map((line, index) => (
          <div
            key={index}
            className={
              line.kind === "in"
                ? "console-ps-in"
                : line.kind === "err"
                  ? "console-ps-err"
                  : line.kind === "meta"
                    ? "console-ps-meta"
                    : "console-ps-out"
            }
          >
            {line.kind === "in" ? <span className="console-ps-prompt">PS&gt; </span> : null}
            {line.text}
          </div>
        ))}
        {busy ? <div className="console-ps-meta">…</div> : null}
        <div ref={endRef} />
      </div>
      <form className="console-ps-form" onSubmit={onSubmit}>
        <span className="console-ps-prompt" aria-hidden>
          PS&gt;
        </span>
        <input
          ref={inputRef}
          type="text"
          value={input}
          disabled={busy}
          spellCheck={false}
          autoComplete="off"
          autoCorrect="off"
          placeholder="Get-ChildItem"
          aria-label="Commande PowerShell"
          onChange={(e) => setInput(e.target.value)}
        />
      </form>
    </div>
  );
}
