import { useRef, useState } from "react";
import type { Channel } from "../../../api";
import { CHANNEL_ORDER, CHANNELS } from "../../../channel";
import { Ico } from "../../../Ico";
import { ChannelDot, PopoverMenu, PopoverMenuItem, tip } from "../../ui";
import type { LaunchNotice } from "../../../sessionNotice";
import "./sidebar-actions.css";

/**
 * The active instance, pinned at the top of the explorer. Three list rows,
 * each `var(--row)` tall and sharing a 24px leading column: the name (edited
 * in place, channel dot centered in that column),
 * Play, and Share. Play and Share have no fill until hover; Play stays
 * filled while the instance is running and its label becomes Arrêter.
 * Revision, save and mod counts live at the top of Mods. A
 * refused launch still shows under the three rows.
 *
 * Every mutation is a callback prop — this component only routes clicks up.
 */
export function ActiveInstanceCard({
  name,
  channel,
  running,
  onPlay,
  onShare,
  onRename,
  onSetChannel,
  notice,
  onDismissNotice,
}: {
  name: string;
  channel: Channel;
  running: boolean;
  onPlay: () => void;
  onShare: () => void;
  onRename: (name: string) => void;
  onSetChannel: (channel: Channel) => void;
  /** Why the last launch was refused, or what SKSE's journals said about
   * the last session — shown under the three rows until dismissed. */
  notice?: LaunchNotice | null;
  onDismissNotice?: () => void;
}) {
  const [renaming, setRenaming] = useState(false);
  const [draft, setDraft] = useState(name);
  const [chanOpen, setChanOpen] = useState(false);
  const chanBtnRef = useRef<HTMLButtonElement>(null);
  const skipRename = useRef(false);

  function startRename() {
    skipRename.current = false;
    setDraft(name);
    setRenaming(true);
  }
  function commitRename() {
    if (skipRename.current) {
      skipRename.current = false;
      return;
    }
    skipRename.current = true;
    setRenaming(false);
    const trimmed = draft.trim();
    if (trimmed && trimmed !== name) onRename(trimmed);
  }
  function cancelRename() {
    skipRename.current = true;
    setDraft(name);
    setRenaming(false);
  }

  return (
    <div className="side-instance">
      <div className="lo instance-actions">
        <div className="lo-top">
          <button
            className="lo-chan"
            type="button"
            ref={chanBtnRef}
            {...tip("Changer de canal")}
            aria-haspopup="dialog"
            aria-expanded={chanOpen}
            onClick={() => setChanOpen((v) => !v)}
          >
            <ChannelDot channel={channel} />
          </button>
          {renaming ? (
            <input
              className="tr-rename-input"
              autoFocus
              maxLength={100}
              value={draft}
              onChange={(e) => setDraft(e.target.value)}
              onFocus={(e) => e.currentTarget.select()}
              onKeyDown={(e) => {
                e.stopPropagation();
                if (e.key === "Enter") {
                  e.preventDefault();
                  commitRename();
                }
                if (e.key === "Escape") {
                  e.preventDefault();
                  cancelRename();
                }
              }}
              onBlur={commitRename}
            />
          ) : (
            <button
              className="lo-name"
              type="button"
              {...tip("Renommer l’instance")}
              onClick={startRename}
            >
              <span className="fade">{name}</span>
            </button>
          )}
          {chanOpen ? (
            <PopoverMenu
              chrome="context"
              title="Canal"
              anchorRef={chanBtnRef}
              onClose={() => setChanOpen(false)}
            >
              {CHANNEL_ORDER.map((ch) => (
                <PopoverMenuItem
                  key={ch}
                  icon={CHANNELS[ch].icon}
                  label={CHANNELS[ch].label}
                  sub={CHANNELS[ch].sub}
                  current={channel === ch}
                  onPick={() => {
                    setChanOpen(false);
                    onSetChannel(ch);
                  }}
                />
              ))}
            </PopoverMenu>
          ) : null}
        </div>
        <button
          className={running ? "play-btn is-active" : "play-btn"}
          type="button"
          aria-pressed={running}
          onClick={onPlay}
        >
          <span className="act-lead">
            <Ico name={running ? "x" : "play"} className={running ? undefined : "play-glyph"} />
          </span>
          <span className="act-label">{running ? "Arrêter" : "Jouer"}</span>
        </button>
        <button className="share-btn" type="button" onClick={onShare}>
          <span className="act-lead">
            <Ico name="share" />
          </span>
          <span className="act-label">Partager</span>
        </button>
        {notice ? (
          <div className={`notice ${notice.tone}`} role="status">
            <div className="nt">
              <span className={`hd ${notice.tone === "bad" ? "danger" : notice.tone === "warn" ? "warn" : ""}`} />
              <span className="ttl">{notice.title}</span>
              <button
                className="x"
                type="button"
                {...tip("Fermer le bilan")}
                onClick={onDismissNotice}
              >
                <Ico name="x" className="sm" />
              </button>
            </div>
            {notice.lines.map((line, i) => (
              <div className="nl" key={i}>
                {line}
              </div>
            ))}
          </div>
        ) : null}
      </div>
    </div>
  );
}
