import type { FormEvent } from "react";
import type { ModConflicts, ModRow } from "../../../api";
import { IconButton, Panel } from "../../ui";
import { ChatPanel, type ChatMsg } from "../Chat/ChatPanel";
import { ConfigPane } from "./ConfigPane";
import { ModInspector } from "./ModInspector";

export type RightTab = "info" | "config" | "claude";

/**
 * The floating right panel: the selected mod's inspector, or the local AI
 * chat. Which one is showing is shell state, so the caller owns it.
 */
export function RightPanel({
  tab,
  onTab,
  onClose,
  mod,
  conflicts,
  loadoutId,
  chat,
  llm,
  draft,
  chatContext,
  onDraftChange,
  onSendChat,
}: {
  tab: RightTab;
  onTab: (tab: RightTab) => void;
  onClose: () => void;
  mod: ModRow | null;
  conflicts: ModConflicts | undefined;
  loadoutId: number | null;
  chat: ChatMsg[];
  llm: string;
  draft: string;
  chatContext: string;
  onDraftChange: (value: string) => void;
  onSendChat: (e?: FormEvent) => void;
}) {
  return (
    <Panel
      variant="right"
      header={
        <>
          <button
            className="tab"
            type="button"
            aria-selected={tab === "info"}
            onClick={() => onTab("info")}
          >
            Info
          </button>
          <button
            className="tab"
            type="button"
            aria-selected={tab === "config"}
            onClick={() => onTab("config")}
          >
            Config
          </button>
          <button
            className="tab"
            type="button"
            aria-selected={tab === "claude"}
            onClick={() => onTab("claude")}
          >
            Claude
          </button>
          <span className="spacer" />
          <IconButton name="x" title="Fermer" onClick={onClose} />
        </>
      }
    >
      {tab === "info" ? (
        <ModInspector mod={mod} conflicts={conflicts} loadoutId={loadoutId} />
      ) : tab === "config" ? (
        <ConfigPane mod={mod} />
      ) : (
        <ChatPanel
          messages={chat}
          llm={llm}
          draft={draft}
          context={chatContext}
          onDraftChange={onDraftChange}
          onSubmit={onSendChat}
        />
      )}
    </Panel>
  );
}
