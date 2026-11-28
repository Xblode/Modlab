import { useEffect, useRef, useState } from "react";
import type { Channel, LoadoutDto, LoadoutVersion } from "../../../api";
import type { ToolbarPrimaryAction } from "../../../app/useMcLoaderBranch";
import { CHANNEL_NAME, revisionLabel } from "../../../channel";
import type { BrowseSite } from "../../../gameChrome";
import { Ico } from "../../../Ico";
import {
  Button,
  ChannelDot,
  IconButton,
  PopoverMenu,
  PopoverMenuItem,
  Select,
  tip,
} from "../../ui";
import { BrowseSiteMenu } from "../Browser/BrowseSiteMenu";
import "./shell-toolbar.css";

/**
 * Shell toolbar under the page hero (`LayoutShell`'s `header` slot) — branch
 * picker (or primary-action slot), revision sync, add mod, and branch metadata.
 * Mounted from `App.tsx` on the Mods tab only.
 *
 * Primary-action slot (`.tbar-primary-slot`): same footprint as the branch
 * `Select` (`--ctl-sm`). Default = `Select` + `PopoverMenu`; when a loader must
 * be installed it swaps to a bordered `Button` « Installer … », then returns.
 *
 * Compaction (see `shell-toolbar.css` / .bframe/DESIGN.md): at narrow container widths,
 * labels hide (`.btn-label`, `.select-label`, `.tbar-unit`) while controls keep
 * their type; tips carry the missing text. Borders stay on primary actions only.
 * The install CTA in the primary slot keeps its label (not icon-only).
 */

const VISIBLE_REVISIONS = 12;
const RESTORE_ONLY_LATEST =
  "Le Core ne restaure que le dernier instantané. Les révisions plus anciennes restent dans le journal.";

type BranchTab = "branches" | "revisions";

type Props = {
  activeId: number | null;
  name: string | null;
  channel: Channel | null;
  /** Snapshot count of the active branch. Null when there is no branch. */
  revision: number | null;
  version: LoadoutVersion | null;
  branches: readonly LoadoutDto[];
  enabledCount: number;
  totalCount: number;
  onPickBranch: (id: number) => void;
  onCreateBranch: () => void;
  onSnapshot: () => Promise<void>;
  onRollback: () => Promise<void>;
  onNotice: (message: string) => void;
  /** Sites for the « Ajouter un mod » picker (favourites already ordered). */
  browseSites: readonly BrowseSite[];
  favoriteSiteIds: readonly string[];
  onToggleSiteFavorite: (siteId: string) => void;
  onOpenSite: (url: string) => void;
  /** Same import flow as the empty Mods state « Importer… ». */
  onImport: () => void;
  onOpenInstanceFolder: () => void;
  /**
   * When set, replaces the branch select (same slot / height). Used for
   * « Installer NeoForge … » and future primary CTAs.
   */
  primaryAction?: ToolbarPrimaryAction | null;
  /** Minecraft only: show « Tester le lancement » next to Ajouter. */
  showTestLaunch?: boolean;
  /** Smoke test in progress (disable the button). */
  testLaunchBusy?: boolean;
  onTestLaunch?: () => void;
};

function isDirty(version: LoadoutVersion | null): boolean {
  return version !== null && (version.dirtyPlus > 0 || version.dirtyMinus > 0);
}

function dirtyText(version: LoadoutVersion): string {
  const plus = version.dirtyPlus > 0 ? `+${version.dirtyPlus}` : "";
  const minus = version.dirtyMinus > 0 ? `−${version.dirtyMinus}` : "";
  if (plus && minus) return `${plus} ${minus}`;
  return plus || minus;
}

function branchStat(count: number): string {
  if (count <= 0) return "Aucune branche";
  if (count === 1) return "1 branche";
  return `${count} branches`;
}

function revisionStat(count: number): string {
  if (count <= 0) return "Aucune révision";
  if (count === 1) return "1 révision";
  return `${count} révisions`;
}

type RevRow = {
  key: string;
  label: string;
  sub: string;
  icon: string;
  current: boolean;
  onPick: () => void;
};

function revisionRows(
  channel: Channel,
  revision: number,
  version: LoadoutVersion | null,
  onRollback: () => Promise<void>,
  onNotice: (message: string) => void,
): RevRow[] {
  const dirty = isDirty(version);
  if (revision <= 0) {
    return [
      {
        key: "empty",
        label: revisionLabel(channel, 0),
        sub: "aucun instantané",
        icon: "branch",
        current: !dirty,
        onPick: () => {},
      },
    ];
  }

  const items: RevRow[] = [];
  if (dirty && version) {
    items.push({
      key: "dirty",
      label: "Modifications",
      sub: dirtyText(version),
      icon: "branch",
      current: true,
      onPick: () => {},
    });
  }

  const shown = Math.min(revision, VISIBLE_REVISIONS);
  for (let offset = 0; offset < shown; offset += 1) {
    const rev = revision - offset;
    const tip = rev === revision;
    const current = !dirty && tip;
    items.push({
      key: `r-${rev}`,
      label: revisionLabel(channel, rev),
      sub: current
        ? "révision active"
        : tip && dirty
          ? "annuler les modifications"
          : "instantané passé",
      icon: tip ? "branch" : "history",
      current,
      onPick: () => {
        if (current) return;
        if (tip && dirty) {
          void onRollback();
          return;
        }
        onNotice(RESTORE_ONLY_LATEST);
      },
    });
  }

  const hidden = revision - shown;
  if (hidden > 0) {
    items.push({
      key: "older",
      label: hidden === 1 ? "1 révision plus ancienne" : `${hidden} révisions plus anciennes`,
      sub: "dans le journal",
      icon: "history",
      current: false,
      onPick: () => onNotice(RESTORE_ONLY_LATEST),
    });
  }
  return items;
}

function matchesQuery(query: string, ...parts: Array<string | undefined>): boolean {
  const q = query.trim().toLowerCase();
  if (!q) return true;
  return parts.some((p) => p?.toLowerCase().includes(q));
}

export function ShellToolbar({
  activeId,
  name,
  channel,
  revision,
  version,
  branches,
  enabledCount,
  totalCount,
  onPickBranch,
  onCreateBranch,
  onSnapshot,
  onRollback,
  onNotice,
  browseSites,
  favoriteSiteIds,
  onToggleSiteFavorite,
  onOpenSite,
  onImport,
  onOpenInstanceFolder,
  primaryAction = null,
  showTestLaunch = false,
  testLaunchBusy = false,
  onTestLaunch,
}: Props) {
  const btnRef = useRef<HTMLButtonElement>(null);
  const addModRef = useRef<HTMLSpanElement>(null);
  const [open, setOpen] = useState(false);
  const [addModOpen, setAddModOpen] = useState(false);
  const [tab, setTab] = useState<BranchTab>("branches");
  const [query, setQuery] = useState("");
  const dirty = isDirty(version);
  const code = channel !== null && revision !== null ? revisionLabel(channel, revision) : null;
  const showInstallCta = primaryAction?.kind === "install-loader";

  useEffect(() => {
    setOpen(false);
    setAddModOpen(false);
    setQuery("");
    setTab("branches");
  }, [activeId]);

  useEffect(() => {
    if (showInstallCta) setOpen(false);
  }, [showInstallCta]);

  const saveTitle =
    version && channel
      ? `Enregistrer l’état actif/inactif comme ${revisionLabel(channel, version.revision + 1)}`
      : "Révision pas encore chargée";
  const discardTitle =
    version && channel
      ? `Annuler les modifications et revenir à ${revisionLabel(channel, version.revision)}`
      : "Révision pas encore chargée";

  const branchRows = branches.filter((branch) => {
    const rev = branch.id === activeId && revision !== null ? revision : branch.revision;
    const branchCode = revisionLabel(branch.channel, rev);
    return matchesQuery(query, branch.name, CHANNEL_NAME[branch.channel], branchCode);
  });

  const revRows =
    channel !== null && revision !== null
      ? revisionRows(channel, revision, version, onRollback, onNotice).filter((row) =>
          matchesQuery(query, row.label, row.sub),
        )
      : [];

  const branchTip =
    name && code ? `${name} · ${code}` : name ? name : code ? code : "Aucune branche";

  return (
    <div className="tbar shell-toolbar" role="toolbar" aria-label="Outils de l'instance">
      <div className="repo-lead">
        <div className="tbar-primary-slot">
          {showInstallCta && primaryAction ? (
            <Button
              variant="action"
              className="tbar-primary-action"
              title={primaryAction.label}
              aria-label={primaryAction.label}
              disabled={primaryAction.installing}
              onClick={primaryAction.onInstall}
            >
              <Ico name="download" className="sm" />
              <span className="btn-label">{primaryAction.label}</span>
            </Button>
          ) : (
            <Select
              ref={btnRef}
              icon="branch"
              label={name ?? "Aucune branche"}
              meta={code ?? undefined}
              aria-haspopup="dialog"
              aria-expanded={open}
              title={branchTip}
              aria-label={
                code ? `Branche ${name ?? "sans nom"}, révision ${code}` : "Aucune branche"
              }
              onClick={() => setOpen((value) => !value)}
            />
          )}
        </div>
        {activeId !== null ? (
          <IconButton
            name="folder"
            title="Ouvrir le dossier de l’instance"
            aria-label="Ouvrir le dossier de l’instance"
            onClick={onOpenInstanceFolder}
          />
        ) : null}
        <IconButton
          name="plus"
          title="Créer une branche"
          aria-label="Créer une branche"
          onClick={onCreateBranch}
        />
        <span className="repo-stat" {...tip(branchStat(branches.length))}>
          <Ico name="branch" />
          <span className="tbar-stat-wide">{branchStat(branches.length)}</span>
          <span className="tbar-stat-narrow" aria-hidden="true">
            {branches.length}
          </span>
        </span>
        {revision !== null ? (
          <span className="repo-stat" {...tip(revisionStat(revision))}>
            <Ico name="history" />
            <span className="tbar-stat-wide">{revisionStat(revision)}</span>
            <span className="tbar-stat-narrow" aria-hidden="true">
              {revision}
            </span>
          </span>
        ) : null}
      </div>

      <div className="repo-actions">
        {activeId !== null ? (
          <>
            {!version ? (
              <span className="repo-sync pending" {...tip("Lecture de la révision")}>
                …
              </span>
            ) : dirty ? (
              <span className="repo-sync dirty" {...tip("Modifications non enregistrées")}>
                {dirtyText(version)}
              </span>
            ) : (
              <span className="repo-sync ok" {...tip(code ? `À jour avec ${code}` : "À jour")}>
                <Ico name="check" />
              </span>
            )}
            <span className="repo-counts" aria-label="Métadonnées de la branche">
              {channel ? (
                <span
                  className="repo-count-status"
                  {...tip(`Statut : ${CHANNEL_NAME[channel]}`)}
                >
                  <ChannelDot channel={channel} />
                  <span className="tbar-unit">{CHANNEL_NAME[channel]}</span>
                </span>
              ) : null}
              <span {...tip("Nombre total de mods")}>
                <b>{totalCount}</b>
                <span className="tbar-unit"> mods</span>
              </span>
              <span {...tip("Mods actifs")}>
                <b>{enabledCount}</b>
                <span className="tbar-unit"> actifs</span>
              </span>
            </span>
            {dirty ? (
              <>
                <Button variant="sm-ghost" title={saveTitle} onClick={() => void onSnapshot()}>
                  <Ico name="db" className="sm" />
                  <span className="btn-label">Enregistrer</span>
                </Button>
                <Button variant="sm-ghost" title={discardTitle} onClick={() => void onRollback()}>
                  <Ico name="history" className="sm" />
                  <span className="btn-label">Annuler</span>
                </Button>
              </>
            ) : null}
          </>
        ) : null}
        {showTestLaunch ? (
          <Button
            variant="sm-ghost"
            title="Tester le lancement"
            disabled={testLaunchBusy}
            aria-busy={testLaunchBusy}
            onClick={() => onTestLaunch?.()}
          >
            <Ico name="zap" className="sm" />
            <span className="btn-label">
              {testLaunchBusy ? "Test…" : "Tester le lancement"}
            </span>
          </Button>
        ) : null}
        <span ref={addModRef} className="shell-add-mod-anchor">
          <Button
            variant="action"
            title="Ajouter un mod"
            aria-expanded={addModOpen}
            onClick={() => setAddModOpen((v) => !v)}
          >
            <Ico name="plus" className="sm" />
            <span className="btn-label">Ajouter un mod</span>
          </Button>
        </span>
      </div>

      {addModOpen ? (
        <BrowseSiteMenu
          mode="add-mod"
          sites={browseSites}
          favoriteIds={favoriteSiteIds}
          onToggleFavorite={onToggleSiteFavorite}
          onOpenSite={onOpenSite}
          onImport={onImport}
          anchorRef={addModRef}
          onClose={() => setAddModOpen(false)}
        />
      ) : null}

      {open && !showInstallCta ? (
        <PopoverMenu
          className="branch-menu"
          title="Changer de branche"
          anchorRef={btnRef}
          onClose={() => {
            setOpen(false);
            setQuery("");
          }}
          utility={{
            kind: "search",
            value: query,
            onChange: setQuery,
            placeholder: "Trouver une branche…",
          }}
          tabs={[
            { id: "branches", label: "Branches" },
            { id: "revisions", label: "Révisions" },
          ]}
          activeTab={tab}
          onTabChange={(id) => setTab(id as BranchTab)}
        >
          {tab === "branches" ? (
            branchRows.length === 0 ? (
              <div className="popover-menu-empty">Aucune branche</div>
            ) : (
              branchRows.map((branch) => {
                const rev =
                  branch.id === activeId && revision !== null ? revision : branch.revision;
                const branchCode = revisionLabel(branch.channel, rev);
                const active = branch.id === activeId;
                return (
                  <PopoverMenuItem
                    key={branch.id}
                    icon="branch"
                    label={branch.name}
                    sub={`${CHANNEL_NAME[branch.channel]} · ${branchCode}`}
                    current={active}
                    badge={active ? "active" : undefined}
                    onPick={() => {
                      if (branch.id !== activeId) onPickBranch(branch.id);
                      setOpen(false);
                      setQuery("");
                    }}
                  />
                );
              })
            )
          ) : revRows.length === 0 ? (
            <div className="popover-menu-empty">Aucune révision</div>
          ) : (
            revRows.map((row) => (
              <PopoverMenuItem
                key={row.key}
                icon={row.icon}
                label={row.label}
                sub={row.sub}
                current={row.current}
                onPick={() => {
                  row.onPick();
                  setOpen(false);
                  setQuery("");
                }}
              />
            ))
          )}
        </PopoverMenu>
      ) : null}
    </div>
  );
}
