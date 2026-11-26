import type { DetectedGame, LoadoutDto, ModRow } from "../../../api";
import { OFFICIAL_TOOL_PAGES } from "../../../api";
import { PageHero, type HeaderFact, type HeaderFactEdit } from "../Dashboard/PageHero";
import { Button, SideEmpty } from "../../ui";
import { Ico } from "../../../Ico";
import {
  describeSkse,
  skseChoiceLabel,
  skseChoiceSub,
  type SkseView,
} from "../../../app/dashboard/gameTools";

type Props = {
  game: DetectedGame;
  loadout: LoadoutDto | null;
  entries: readonly ModRow[];
  onToggleMod: (row: ModRow) => void | Promise<void>;
  onInstallArchived: (row: ModRow) => void | Promise<void>;
  /** Opens the address in the shell’s Web tab, not the system browser. */
  onOpenWeb: (url: string) => void;
  onUnwired: (label: string) => void;
  /** No instance yet: the empty state creates one, it does not invent a branch. */
  onCreateInstance: () => void;
  mcLoaderFacts?: HeaderFact[];
};

/**
 * Mods instance hero — mounted in `LayoutShell`'s `header` slot from `App.tsx`.
 * Status (channel), revision, and mod counts live in the sidebar and `ShellToolbar`.
 */
export function BranchHeader({
  game,
  loadout,
  entries,
  onToggleMod,
  onInstallArchived,
  onOpenWeb,
  onUnwired,
  onCreateInstance,
  mcLoaderFacts = [],
}: Props) {
  const facts: HeaderFact[] = loadout
    ? [
        ...skseFacts(game, entries, {
          onToggleMod,
          onInstallArchived,
          onOpenWeb,
          onUnwired,
        }),
        ...(game.id === "minecraft" ? mcLoaderFacts : []),
      ]
    : [];

  return (
    <div className="mods-head">
      {loadout ? (
        <PageHero name={loadout.instanceName} facts={facts} />
      ) : (
        <div className="ghero">
          <SideEmpty
            title="Aucune instance"
            action={
              <Button variant="sm" onClick={onCreateInstance}>
                <Ico name="plus" className="sm" />
                Créer une instance
              </Button>
            }
          >
            Ce jeu n’en a pas encore.
          </SideEmpty>
        </div>
      )}
    </div>
  );
}

function skseFacts(
  game: DetectedGame,
  entries: readonly ModRow[],
  actions: {
    onToggleMod: (row: ModRow) => void | Promise<void>;
    onInstallArchived: (row: ModRow) => void | Promise<void>;
    onOpenWeb: (url: string) => void;
    onUnwired: (label: string) => void;
  },
): HeaderFact[] {
  const capability = game.capabilities.scriptExtender;
  if (!capability || !/^skse\b/i.test(capability)) return [];
  const view = describeSkse(entries);
  return [
    {
      id: "skse",
      icon: "zap",
      label: skseLabel(capability, view, entries),
      value: view.value,
      title: view.title,
      muted: view.muted,
      edit: skseEdit(view, actions),
    },
  ];
}

function skseLabel(capability: string, view: SkseView, entries: readonly ModRow[]): string {
  const currentId = view.currentId;
  const row =
    (currentId != null ? entries.find((entry) => entry.modId === currentId) : undefined) ??
    view.direct?.row;
  if (row?.tool === "skse-vr") return "SKSE VR";
  return capability;
}

function skseEdit(
  view: SkseView,
  actions: {
    onToggleMod: (row: ModRow) => void | Promise<void>;
    onInstallArchived: (row: ModRow) => void | Promise<void>;
    onOpenWeb: (url: string) => void;
    onUnwired: (label: string) => void;
  },
): HeaderFactEdit | undefined {
  if (view.absent) {
    return {
      mode: "action",
      label: "Télécharger SKSE",
      onClick: () => openSksePage(actions.onOpenWeb, actions.onUnwired),
    };
  }
  if (view.direct?.action === "install") {
    const row = view.direct.row;
    return {
      mode: "action",
      label: "Installer SKSE",
      onClick: () => void actions.onInstallArchived(row),
    };
  }
  if (view.direct?.action === "toggle") {
    const row = view.direct.row;
    return {
      mode: "action",
      label: row.enabled ? "Désactiver SKSE" : "Activer SKSE",
      onClick: () => void actions.onToggleMod(row),
    };
  }
  if (!view.choices) return undefined;
  const choices = view.choices;
  return {
    mode: "menu",
    label: "Choisir la version de SKSE",
    items: choices.map((row) => ({
      label: skseChoiceLabel(row),
      sub: skseChoiceSub(row),
      icon: "zap",
      current: row.modId === view.currentId,
      onPick:
        row.status === "downloading"
          ? undefined
          : () => void pickSkse(row, choices, actions),
    })),
  };
}

/** Make `row` the instance's SKSE. An archive is installed. An installed
 * copy is enabled, and any other enabled SKSE on the instance is turned off. */
async function pickSkse(
  row: ModRow,
  rows: readonly ModRow[],
  actions: {
    onToggleMod: (row: ModRow) => void | Promise<void>;
    onInstallArchived: (row: ModRow) => void | Promise<void>;
  },
): Promise<void> {
  if (row.status === "archived") {
    await actions.onInstallArchived(row);
    return;
  }
  if (row.status !== "installed") return;
  const others = rows.filter(
    (other) => other.modId !== row.modId && other.status === "installed" && other.enabled,
  );
  for (const other of others) await actions.onToggleMod(other);
  if (!row.enabled) await actions.onToggleMod(row);
}

/** Silverlock, in the shell’s Web tab. The system browser is not used. */
function openSksePage(onOpenWeb: (url: string) => void, onUnwired: (label: string) => void): void {
  const url = OFFICIAL_TOOL_PAGES.find((page) => page.id === "skse")?.url;
  if (!url) {
    onUnwired("SKSE");
    return;
  }
  onOpenWeb(url);
}