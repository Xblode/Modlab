import type { FormEvent } from "react";
import { Ico } from "../../../Ico";
import { Input } from "../../ui";

export type ChatMsg = { who: "me" | "ai"; text: string; source?: string };

/**
 * The right panel's Claude tab. A real client of the AI layer: every answer
 * says where it came from — the Core's rules engine, or the local LLM
 * rephrasing those same facts.
 */
export function ChatPanel({
  messages,
  llm,
  draft,
  context,
  onDraftChange,
  onSubmit,
}: {
  messages: ChatMsg[];
  /** Active local model id, shown as the source of an LLM-phrased answer. */
  llm: string;
  draft: string;
  /** What the question will be asked about — instance, then selected mod. */
  context: string;
  onDraftChange: (value: string) => void;
  onSubmit: (e?: FormEvent) => void;
}) {
  return (
    <div className="chat" style={{ height: "100%" }}>
      <div className="msgs">
        {messages.map((m, i) => (
          <div key={i} className={`msg ${m.who}`}>
            {m.who === "ai" ? (
              <div className="who">
                <Ico name="sparkle" className="sm" />
                {m.source === "local-llm"
                  ? `Local · ${llm}`
                  : m.source === "pending"
                    ? "Local · réflexion…"
                    : "Local · moteur de règles"}
              </div>
            ) : null}
            {m.text}
          </div>
        ))}
      </div>
      <form className="in" onSubmit={onSubmit}>
        <Input
          value={draft}
          onChange={(e) => onDraftChange(e.target.value)}
          placeholder="Demander à Claude… (@mod pour cibler)"
        />
        <div className="row2">
          <span>Contexte : {context}</span>
          <span className="spacer" />
          <span className="kbd">↵</span>
          <Ico name="send" className="sm" />
        </div>
      </form>
    </div>
  );
}
