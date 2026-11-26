import {
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
  type FormEvent,
  type KeyboardEvent,
} from "react";
import { Ico } from "../../../Ico";
import { Button, Chip, PopoverMenu, PopoverMenuItem, type ChipTone } from "../../ui";

export type ChatMsg = { who: "me" | "ai"; text: string; source?: string };

/** A tagged entity in the composer (from right-click → Chat). */
export type ChatTag = {
  id: string;
  kind: "mod";
  modId: number;
  label: string;
};

type TaskModeId = ChipTone;

const TASK_MODES: readonly { id: TaskModeId; label: string }[] = [
  { id: "plan", label: "Plan" },
  { id: "debug", label: "Debug" },
  { id: "multi", label: "Multi-tâches" },
];

/** Single-line field height — must match `.chat .in-editor .input` in shell.css. */
const COMPOSER_LINE_PX = 20;
/** Cap auto-grow so a long paste does not swallow the message list. */
const COMPOSER_MAX_PX = 120;
/** Soft-wrap past one line at the *compact* slot width (see mirror measure). */
const COMPOSER_WRAP_AT_PX = COMPOSER_LINE_PX + 1;

/**
 * Compact slot width for the single-row composer: form minus leading/trailing
 * chrome. Used by the offscreen mirror so open/close does not depend on the
 * full-width multi layout (which would un-wrap and flicker).
 */
function compactSlotWidth(form: HTMLFormElement): number {
  const leading = form.querySelector(".in-leading") as HTMLElement | null;
  const trailing = form.querySelector(".in-trailing") as HTMLElement | null;
  const cs = getComputedStyle(form);
  const padX = (parseFloat(cs.paddingLeft) || 0) + (parseFloat(cs.paddingRight) || 0);
  const gap = parseFloat(cs.columnGap || cs.gap) || 6;
  // Two gutters between three grid columns (or leading | field | trailing).
  const gutters = gap * 2;
  return Math.max(
    40,
    form.clientWidth - (leading?.offsetWidth ?? 0) - (trailing?.offsetWidth ?? 0) - padX - gutters,
  );
}

function ensureMirror(): HTMLTextAreaElement {
  let el = document.getElementById("chat-composer-mirror") as HTMLTextAreaElement | null;
  if (el) return el;
  el = document.createElement("textarea");
  el.id = "chat-composer-mirror";
  el.setAttribute("aria-hidden", "true");
  el.tabIndex = -1;
  el.style.cssText =
    "position:fixed;top:0;left:-9999px;height:0;min-height:0;max-height:none;overflow:hidden;" +
    "visibility:hidden;pointer-events:none;border:0;padding:0;margin:0;resize:none;" +
    "white-space:pre-wrap;overflow-wrap:break-word;box-sizing:border-box;";
  document.body.appendChild(el);
  return el;
}

/**
 * The right panel's Chat tab. A real client of the AI layer: every answer
 * says where it came from — the Core's rules engine, or the local LLM
 * rephrasing those same facts.
 *
 * Two composer states like Cursor compact glass:
 * 1. Empty / one line — single 36px row `[+] [chip?] [field] [Auto] [mic] [send]`
 * 2. Multi-line — field expands above; bottom toolbar stays
 *    `[+] [chip?] …… [Auto] [mic] [send]`.
 * The + opens a PopoverMenu (above) for Plan / Debug / Multi-tâches.
 */
export function ChatPanel({
  messages,
  llm,
  draft,
  tags = [],
  focusNonce = 0,
  running = false,
  gameName,
  branchLabel = "master",
  onDraftChange,
  onRemoveTag,
  onSubmit,
}: {
  messages: ChatMsg[];
  /** Active local model id, shown as the source of an LLM-phrased answer. */
  llm: string;
  draft: string;
  /** Mods (etc.) tagged into the composer via right-click → Chat. */
  tags?: readonly ChatTag[];
  /** Bumped by the parent after a tag insert so the field takes focus. */
  focusNonce?: number;
  /** Game session is live — shows a pill with `gameName`. */
  running?: boolean;
  /** Display name of the running game (e.g. Skyrim, Minecraft). */
  gameName?: string | null;
  /** Branch / loadout short name under the composer (Cursor “master”). */
  branchLabel?: string;
  onDraftChange: (value: string) => void;
  onRemoveTag?: (id: string) => void;
  onSubmit: (e?: FormEvent) => void;
}) {
  const showGame = running && Boolean(gameName?.trim());
  const formRef = useRef<HTMLFormElement>(null);
  const fieldRef = useRef<HTMLTextAreaElement>(null);
  const plusRef = useRef<HTMLSpanElement>(null);
  /** Pre-toggle height — CSS tweens height only (radius is fixed at --in-r). */
  const flipFromH = useRef<number | null>(null);
  const [multiLine, setMultiLine] = useState(false);
  const [plusOpen, setPlusOpen] = useState(false);
  const [taskMode, setTaskMode] = useState<TaskModeId | null>(null);
  const canSend = draft.trim().length > 0 || tags.length > 0;
  const activeMode = TASK_MODES.find((m) => m.id === taskMode) ?? null;

  useEffect(() => {
    return () => {
      document.getElementById("chat-composer-mirror")?.remove();
    };
  }, []);

  /**
   * Decide multi vs compact by measuring draft at the *single-row* slot width
   * (mirror), not at the live textarea width — which jumps wide when `is-multi`
   * and would otherwise un-wrap a single soft line and flicker.
   */
  useLayoutEffect(() => {
    const el = fieldRef.current;
    const form = formRef.current;
    if (!el || !form) return;

    const hasHardBreak = draft.includes("\n");
    let needsMulti = hasHardBreak;
    if (!needsMulti && draft.length > 0) {
      const probeW = compactSlotWidth(form);
      const mirror = ensureMirror();
      const cs = getComputedStyle(el);
      mirror.style.width = `${probeW}px`;
      mirror.style.font = cs.font;
      mirror.style.letterSpacing = cs.letterSpacing;
      mirror.style.lineHeight = cs.lineHeight;
      mirror.value = draft;
      mirror.style.height = "0px";
      needsMulti = mirror.scrollHeight > COMPOSER_WRAP_AT_PX;
    }

    if (needsMulti !== multiLine) {
      flipFromH.current = form.getBoundingClientRect().height;
    }

    setMultiLine(needsMulti);

    // Grow the visible field for the current layout width (after multi class applies next paint).
    el.style.height = "0px";
    const next = Math.min(Math.max(el.scrollHeight, COMPOSER_LINE_PX), COMPOSER_MAX_PX);
    el.style.height = `${next}px`;
  }, [draft, tags.length, taskMode, multiLine]);

  /** Compact↔multi: tween height only (radius stays --in-r / 18px). */
  useLayoutEffect(() => {
    const form = formRef.current;
    const fromH = flipFromH.current;
    if (!form || fromH == null) return;
    flipFromH.current = null;

    if (window.matchMedia("(prefers-reduced-motion: reduce)").matches) {
      form.style.height = "";
      return;
    }

    const toH = form.getBoundingClientRect().height;
    if (Math.abs(toH - fromH) < 1) {
      form.style.height = "";
      return;
    }

    // Commit start height without tween, then animate to end on the next frame.
    form.style.transition = "none";
    form.style.height = `${fromH}px`;
    void form.offsetHeight;
    form.style.transition = "";

    let cleared = false;
    const clear = () => {
      if (cleared) return;
      cleared = true;
      form.style.height = "";
    };
    const onEnd = (e: TransitionEvent) => {
      if (e.target !== form || e.propertyName !== "height") return;
      clear();
    };
    form.addEventListener("transitionend", onEnd);
    const raf = requestAnimationFrame(() => {
      form.style.height = `${toH}px`;
    });
    const safety = window.setTimeout(clear, 400);
    return () => {
      cancelAnimationFrame(raf);
      window.clearTimeout(safety);
      form.removeEventListener("transitionend", onEnd);
    };
  }, [multiLine]);

  useEffect(() => {
    if (focusNonce <= 0) return;
    const el = fieldRef.current;
    if (!el) return;
    el.focus();
    const end = el.value.length;
    el.setSelectionRange(end, end);
  }, [focusNonce]);

  const onComposerKeyDown = (e: KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      if (canSend) onSubmit();
    }
  };

  const selectMode = (id: TaskModeId) => {
    setTaskMode((prev) => (prev === id ? null : id));
    setPlusOpen(false);
  };

  return (
    <div className="chat" style={{ height: "100%" }}>
      <div className="msgs" role="log" aria-live="polite" aria-label="Messages du chat">
        {messages.map((m, i) => (
          <div key={i} className={`msg ${m.who}`}>
            {m.who === "ai" ? (
              <p className="who">
                <Ico name="sparkle" className="sm" aria-hidden />
                {m.source === "local-llm"
                  ? `Local · ${llm}`
                  : m.source === "pending"
                    ? "Local · réflexion…"
                    : "Local · moteur de règles"}
              </p>
            ) : null}
            <div className="msg-body">{m.text}</div>
          </div>
        ))}
      </div>

      <div className="chat-foot">
        <div className="chat-pills" aria-label="Statut">
          {showGame ? (
            <span className="chat-pill chat-pill-game" title="Jeu en cours">
              <Ico name="play" className="sm" aria-hidden />
              <span className="fade">{gameName}</span>
            </span>
          ) : null}
          <span className="chat-pill chat-pill-changes" aria-hidden="true">
            Modifications
            <span className="chat-delta-add">+128</span>
            <span className="chat-delta-del">−24</span>
          </span>
          <button type="button" className="chat-pill" tabIndex={-1} aria-hidden="true">
            Créer une révision
          </button>
        </div>

        <div className="in-wrap">
          <form
            ref={formRef}
            className={`in${multiLine ? " is-multi" : ""}`}
            onSubmit={onSubmit}
            aria-label="Composer un message"
            data-layout={multiLine ? "multi" : "compact"}
          >
            <div className="in-editor">
              <textarea
                ref={fieldRef}
                className="input ghost"
                value={draft}
                onChange={(e) => onDraftChange(e.target.value)}
                onKeyDown={onComposerKeyDown}
                placeholder="Demander à Chat…"
                aria-label="Votre message"
                rows={1}
              />
            </div>

            <div className="in-toolbar">
              <div className="in-leading">
                <span ref={plusRef} className="in-plus-anchor">
                  <Button
                    type="button"
                    variant="ghost-round"
                    className="in-plus"
                    aria-label="Ajouter du contexte"
                    aria-haspopup="menu"
                    aria-expanded={plusOpen}
                    onClick={() => setPlusOpen((v) => !v)}
                  >
                    <Ico name="plus" />
                  </Button>
                </span>
                {activeMode ? (
                  <Chip
                    tone={activeMode.id}
                    label={activeMode.label}
                    onDismiss={() => setTaskMode(null)}
                  />
                ) : null}
                {tags.length > 0 ? (
                  <div className="in-tags" aria-label="Éléments tagués">
                    {tags.map((tag) => (
                      <Chip
                        key={tag.id}
                        tone="mod"
                        icon="box"
                        label={`@${tag.label}`}
                        title={`Mod #${tag.modId}`}
                        onDismiss={onRemoveTag ? () => onRemoveTag(tag.id) : undefined}
                      />
                    ))}
                  </div>
                ) : null}
              </div>

              <div className="in-trailing">
                <button type="button" className="in-mode" tabIndex={-1} aria-hidden="true">
                  Auto
                  <Ico name="chev" className="sm" />
                </button>
                <Button
                  type="button"
                  variant="ghost-round"
                  className="in-mic"
                  aria-label="Dictée (bientôt)"
                  tabIndex={-1}
                >
                  <Ico name="mic" />
                </Button>
                <Button
                  type="submit"
                  variant="round"
                  className={`in-send${canSend ? " is-active" : " is-idle"}`}
                  aria-label={canSend ? "Envoyer" : "Dictée vocale (bientôt)"}
                  disabled={!canSend}
                >
                  <Ico name={canSend ? "up" : "wave"} />
                </Button>
              </div>
            </div>
          </form>

          {plusOpen ? (
            <PopoverMenu
              chrome="context"
              title="Mode"
              placement="above"
              anchorRef={plusRef}
              onClose={() => setPlusOpen(false)}
            >
              {TASK_MODES.map((mode) => (
                <PopoverMenuItem
                  key={mode.id}
                  icon="grip"
                  label={mode.label}
                  current={taskMode === mode.id}
                  onPick={() => selectMode(mode.id)}
                />
              ))}
            </PopoverMenu>
          ) : null}

          <div className="in-meta" aria-label="Contexte">
            <button type="button" className="in-meta-btn" tabIndex={-1} aria-hidden="true">
              <Ico name="branch" className="sm" />
              <span>{branchLabel}</span>
              <Ico name="chev" className="sm" />
            </button>
            <button type="button" className="in-meta-btn" tabIndex={-1} aria-hidden="true">
              <Ico name="monitor" className="sm" />
              <span>This PC</span>
              <Ico name="chev" className="sm" />
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
