import { useRef, useState } from "react";
import type { Channel, LoadoutVersion } from "../../../api";
import { CHANNEL_LETTER } from "../../../channel";
import { Ico } from "../../../Ico";
import { Pop, type PopItem } from "../../../Pop";

const CHANNEL_LABEL: Record<Channel, string> = { stable: "Stable", beta: "Beta", alpha: "Alpha" };
const CHANNEL_SUB: Record<Channel, string> = {
  stable: "jouable, ne bouge plus",
  beta: "en test, peut casser",
  alpha: "expérimental, jetable",
};
const CHANNEL_ICON: Record<Channel, string> = { stable: "shield", beta: "zap", alpha: "tri" };

/** One "Engine & Indicators" chip — an engine/loader mod (SKSE today,
 * Forge/Fabric tomorrow) or another notable capability, detected or not. */
export type EngineCap = {
  id: string;
  label: string;
  sub: string;
  detected: boolean;
  icon: string;
};

/**
 * The active instance, pinned at the top of the explorer: what it is
 * (renameable, its channel), whether it is up to date (its revision, real
 * ones now — see `useLoadoutVersion`), and the things one does with it —
 * play, share, save a new revision, discard, rename, switch channel.
 *
 * Every mutation here is a callback prop, same rule the rest of the sidebar
 * follows (`Sidebar`'s own doc comment: "this component only routes clicks
 * back up") — including reads: `version` and `onRefreshVersion` come from
 * `Sidebar`'s own `useLoadoutVersion`, shared with `SidebarFooter`, rather
 * than each fetching its own copy of the same Core call.
 */
export function ActiveInstanceCard({
  name,
  channel,
  running,
  enabledCount,
  totalCount,
  version,
  onRefreshVersion,
  onPlay,
  onShare,
  onRename,
  onSetChannel,
  onSnapshot,
  onRollback,
  onOpenHistory,
  caps,
}: {
  name: string;
  channel: Channel;
  running: boolean;
  enabledCount: number;
  totalCount: number;
  version: LoadoutVersion | null;
  onRefreshVersion: () => void;
  onPlay: () => void;
  onShare: () => void;
  onRename: (name: string) => void;
  onSetChannel: (channel: Channel) => void;
  onSnapshot: () => Promise<void>;
  onRollback: () => Promise<void>;
  onOpenHistory: () => void;
  /** "Engine & Indicators" — empty for a game with no known engine mod to
   * watch for yet. */
  caps?: EngineCap[];
}) {
  const [renaming, setRenaming] = useState(false);
  const [draft, setDraft] = useState(name);
  const [chanOpen, setChanOpen] = useState(false);
  const [verOpen, setVerOpen] = useState(false);
  const chanBtnRef = useRef<HTMLButtonElement>(null);
  const verBtnRef = useRef<HTMLButtonElement>(null);

  const dirty = version ? version.dirtyPlus > 0 || version.dirtyMinus > 0 : false;

  function startRename() {
    setDraft(name);
    setRenaming(true);
  }
  function commitRename() {
    setRenaming(false);
    const trimmed = draft.trim();
    if (trimmed && trimmed !== name) onRename(trimmed);
  }

  async function handleSave() {
    if (!dirty) return;
    await onSnapshot();
    onRefreshVersion();
  }

  const chanItems: PopItem[] = (["stable", "beta", "alpha"] as const).map((ch) => ({
    label: CHANNEL_LABEL[ch],
    sub: CHANNEL_SUB[ch],
    icon: CHANNEL_ICON[ch],
    current: channel === ch,
    onPick: () => onSetChannel(ch),
  }));

  const verItems: PopItem[] = version
    ? [
        ...(dirty
          ? ([
              {
                label: "Modifications non enregistrées",
                sub: `${version.dirtyPlus} activé(s) · ${version.dirtyMinus} désactivé(s) depuis ${CHANNEL_LETTER[channel]}.${version.revision}`,
                icon: "alert",
              },
              {
                label: "Annuler les modifications",
                sub: `revenir à ${CHANNEL_LETTER[channel]}.${version.revision}`,
                icon: "history",
                onPick: () => {
                  void onRollback().then(onRefreshVersion);
                },
              },
              "-",
            ] as PopItem[])
          : []),
        {
          label: "Voir l’historique complet",
          icon: "db",
          onPick: onOpenHistory,
        },
      ]
    : [];

  return (
    <div className="side-instance">
      <div className="lo">
        <div className="lo-top">
          {renaming ? (
            <input
              className="tr-rename-input"
              style={{ fontSize: 14, fontWeight: 600, height: 22 }}
              autoFocus
              maxLength={100}
              value={draft}
              onChange={(e) => setDraft(e.target.value)}
              onFocus={(e) => e.currentTarget.select()}
              onKeyDown={(e) => {
                e.stopPropagation();
                if (e.key === "Enter") commitRename();
                if (e.key === "Escape") setRenaming(false);
              }}
              onBlur={commitRename}
            />
          ) : (
            <button
              className="lo-name"
              type="button"
              title="Renommer l’instance"
              onClick={startRename}
            >
              <span className="fade">{name}</span>
            </button>
          )}
          <button
            className={`chan ${channel}`}
            type="button"
            ref={chanBtnRef}
            title="Canal de la branche"
            onClick={() => setChanOpen((v) => !v)}
          >
            {channel}
            <Ico name="chev" className="sm" />
          </button>
          {chanOpen ? (
            <Pop anchorRef={chanBtnRef} items={chanItems} onClose={() => setChanOpen(false)} />
          ) : null}
        </div>
        <div className="lo-row">
          <button
            className="vsel"
            type="button"
            ref={verBtnRef}
            title="Révision de la branche"
            onClick={() => setVerOpen((v) => !v)}
          >
            {CHANNEL_LETTER[channel]}.{version?.revision ?? "—"}
            {version ? (
              dirty ? (
                <span className="d">
                  {version.dirtyPlus ? `+${version.dirtyPlus}` : ""}
                  {version.dirtyPlus && version.dirtyMinus ? " " : ""}
                  {version.dirtyMinus ? `−${version.dirtyMinus}` : ""}
                </span>
              ) : (
                <span className="d ok">✓</span>
              )
            ) : null}
            <span className="spacer" />
            <Ico name="chev" />
          </button>
          {verOpen && version ? (
            <Pop anchorRef={verBtnRef} items={verItems} onClose={() => setVerOpen(false)} />
          ) : null}
          <button
            className={`btn save-btn ${dirty ? "accent" : ""}`}
            type="button"
            disabled={!dirty}
            title={
              dirty
                ? `Enregistrer l’état actif/inactif comme ${CHANNEL_LETTER[channel]}.${(version?.revision ?? 0) + 1}`
                : `Aucune modification depuis ${CHANNEL_LETTER[channel]}.${version?.revision ?? 0}`
            }
            onClick={() => void handleSave()}
          >
            <Ico name="db" className="sm" />
            {dirty ? "Enregistrer" : "À jour"}
          </button>
        </div>
        <button
          className={`play-btn ${running ? "stop" : ""}`}
          type="button"
          onClick={onPlay}
        >
          <Ico name={running ? "x" : "play"} />
          {running ? "Arrêter" : "Jouer"}
        </button>
        <button className="share-btn" type="button" onClick={onShare}>
          <Ico name="share" />
          Partager
        </button>
        <div className="st">
          <span>
            <b>{enabledCount}</b> mods
          </span>
          <span>
            <b>{totalCount}</b> dans le loadout
          </span>
        </div>
        {caps && caps.length > 0 ? (
          <div className="caps">
            {caps.map((c) => (
              <span
                key={c.id}
                className={`cap ${c.detected ? "on" : ""}`}
                title={`${c.label} · ${c.sub}`}
              >
                <Ico name={c.icon} />
                <span className="row">
                  <span className="dot" />
                  <span className="lbl">{c.label}</span>
                </span>
                <span className="ver">{c.sub}</span>
              </span>
            ))}
          </div>
        ) : null}
      </div>
    </div>
  );
}
