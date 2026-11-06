import { Fragment, useEffect, useState, type ReactNode } from "react";
import {
  clearDownloadsDirOverride,
  clearGamePathOverride,
  downloadsDirSetting,
  getGamePathSlots,
  setDownloadsDirOverride,
  setGamePathOverride,
  type DetectedGame,
  type DownloadsDirSetting,
  type GamePathSlot,
} from "../../../api";
import { Ico } from "../../../Ico";
import { Badge, IconButton, Input, Panel, Switch } from "../../ui";

export type SettingsMenu =
  | "general"
  | "appearance"
  | "notifications"
  | "games"
  | "models"
  | "mcp"
  | "privacy";

/** A titled group of rows — the maquette-agnostic building block every
 * settings page is made of. */
function SettingsGroup({ title, children }: { title?: string; children: ReactNode }) {
  return (
    <div className="settings-group">
      {title ? <div className="settings-group-title">{title}</div> : null}
      {children}
    </div>
  );
}

/** One label/description row with a control (or note) pinned to the right —
 * the shape every non-list settings page uses. */
function SettingsRow({
  title,
  desc,
  control,
}: {
  title: ReactNode;
  desc?: ReactNode;
  control?: ReactNode;
}) {
  return (
    <div className="settings-row">
      <div className="settings-row-text">
        <div className="settings-row-title">{title}</div>
        {desc ? <div className="settings-row-desc">{desc}</div> : null}
      </div>
      {control ? <div className="settings-row-control">{control}</div> : null}
    </div>
  );
}

/**
 * The folders/executables one game's adapter cares about (§3.1 — each
 * adapter decides its own list and how it finds them: Skyrim from its own
 * `.exe`, Minecraft from `%APPDATA%`), each editable in place. Rows are
 * returned flat (a `Fragment`, no wrapping element) so they stay direct
 * children of the surrounding `SettingsGroup` and inherit its row borders.
 */
function GamePathList({
  gameId,
  onUnwired,
}: {
  gameId: string;
  onUnwired: (label: string) => void;
}) {
  const [slots, setSlots] = useState<GamePathSlot[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [editing, setEditing] = useState<string | null>(null);
  const [draft, setDraft] = useState("");

  useEffect(() => {
    let cancelled = false;
    setSlots(null);
    setError(null);
    getGamePathSlots(gameId)
      .then((s) => {
        if (!cancelled) setSlots(s);
      })
      .catch((e: unknown) => {
        if (!cancelled) setError(e instanceof Error ? e.message : String(e));
      });
    return () => {
      cancelled = true;
    };
  }, [gameId]);

  async function save(slotId: string) {
    const value = draft.trim();
    if (!value) {
      setEditing(null);
      return;
    }
    try {
      setSlots(await setGamePathOverride(gameId, slotId, value));
      setEditing(null);
    } catch (e) {
      onUnwired(e instanceof Error ? e.message : String(e));
    }
  }

  async function reset(slotId: string) {
    try {
      setSlots(await clearGamePathOverride(gameId, slotId));
    } catch (e) {
      onUnwired(e instanceof Error ? e.message : String(e));
    }
  }

  if (error) {
    return <SettingsRow title="Chemins" desc={error} />;
  }
  if (!slots) {
    return <SettingsRow title="Chemins" desc="Détection en cours…" />;
  }

  return (
    <>
      {slots.map((slot) => (
        <div className="settings-row indent" key={slot.id}>
          <div className="settings-row-text">
            <div className="settings-row-title">{slot.label}</div>
            {editing === slot.id ? (
              <Input
                variant="field"
                autoFocus
                value={draft}
                onChange={(e) => setDraft(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === "Enter") void save(slot.id);
                  if (e.key === "Escape") setEditing(null);
                }}
                onBlur={() => void save(slot.id)}
                style={{ marginTop: 4, height: 28, fontFamily: "var(--font-mono)", fontSize: 11.5 }}
              />
            ) : (
              <div
                className="settings-row-desc"
                style={{ fontFamily: "var(--font-mono)", maxWidth: "none", wordBreak: "break-all" }}
              >
                {slot.effective ?? "Non détecté sur cette machine"}
              </div>
            )}
          </div>
          {editing === slot.id ? null : (
            <div className="settings-row-control">
              <Badge tone={slot.overridePath ? "root" : slot.detected ? "root" : "bad"}>
                {slot.overridePath ? "personnalisé" : slot.detected ? "détecté" : "introuvable"}
              </Badge>
              <IconButton
                name="edit"
                title="Modifier le chemin"
                onClick={() => {
                  setEditing(slot.id);
                  setDraft(slot.effective ?? "");
                }}
              />
              {slot.overridePath ? (
                <IconButton
                  name="refresh"
                  title="Revenir à la détection automatique"
                  onClick={() => void reset(slot.id)}
                />
              ) : null}
            </div>
          )}
        </div>
      ))}
    </>
  );
}

/**
 * The one global downloads-folder setting — same edit-in-place shape as a
 * `GamePathList` row (pencil → input → save/cancel, reset button once
 * overridden), just for a single path rather than a per-game list.
 */
function DownloadsDirRow({ onUnwired }: { onUnwired: (label: string) => void }) {
  const [setting, setSetting] = useState<DownloadsDirSetting | null>(null);
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState("");

  useEffect(() => {
    let cancelled = false;
    downloadsDirSetting()
      .then((s) => {
        if (!cancelled) setSetting(s);
      })
      .catch((e: unknown) => onUnwired(e instanceof Error ? e.message : String(e)));
    return () => {
      cancelled = true;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  async function save() {
    const value = draft.trim();
    if (!value) {
      setEditing(false);
      return;
    }
    try {
      setSetting(await setDownloadsDirOverride(value));
      setEditing(false);
    } catch (e) {
      onUnwired(e instanceof Error ? e.message : String(e));
    }
  }

  async function reset() {
    try {
      setSetting(await clearDownloadsDirOverride());
    } catch (e) {
      onUnwired(e instanceof Error ? e.message : String(e));
    }
  }

  return (
    <div className="settings-row">
      <div className="settings-row-text">
        <div className="settings-row-title">Dossier de téléchargement</div>
        {editing ? (
          <Input
            variant="field"
            autoFocus
            value={draft}
            onChange={(e) => setDraft(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") void save();
              if (e.key === "Escape") setEditing(false);
            }}
            onBlur={() => void save()}
            style={{ marginTop: 4, height: 28, fontFamily: "var(--font-mono)", fontSize: 11.5 }}
          />
        ) : (
          <div
            className="settings-row-desc"
            style={{ fontFamily: "var(--font-mono)", maxWidth: "none", wordBreak: "break-all" }}
          >
            {setting ? setting.effective : "Détection en cours…"} — les archives y sont rangées
            par jeu.
          </div>
        )}
      </div>
      {editing || !setting ? null : (
        <div className="settings-row-control">
          <Badge tone={setting.overridePath ? "root" : "root"}>
            {setting.overridePath ? "personnalisé" : "défaut"}
          </Badge>
          <IconButton
            name="edit"
            title="Modifier le dossier"
            onClick={() => {
              setEditing(true);
              setDraft(setting.effective);
            }}
          />
          {setting.overridePath ? (
            <IconButton
              name="refresh"
              title="Revenir au dossier par défaut"
              onClick={() => void reset()}
            />
          ) : null}
        </div>
      )}
    </div>
  );
}

function Page({ title, children }: { title: string; children: ReactNode }) {
  return (
    <div className="content">
      <div className="settings-page">
        <h1>{title}</h1>
        {children}
      </div>
    </div>
  );
}

/**
 * Settings detail surface, shown centered in the main content zone while the
 * sidebar is in Réglages mode — no tab strip, no breadcrumb toolbar above it,
 * same as a normal app settings screen. One page per `SideMenu` entry; only
 * what the app actually knows is shown interacted with, everything else is an
 * honest, Core-labelled placeholder rather than a fake control.
 */
export function SettingsPane({
  menu,
  games,
  llm,
  llmModels,
  dataRoot,
  onSelectModel,
  communityUpdate,
  onToggleCommunityUpdate,
  onUnwired,
}: {
  menu: SettingsMenu;
  games: DetectedGame[];
  llm: string;
  llmModels: readonly (readonly [string, string, string])[];
  dataRoot: string | undefined;
  onSelectModel: (id: string, name: string) => void;
  /** Whether the experimental "Profil / Communauté / Économie" preview
   * (`docs/deep-research-report.md`) is switched on — the actual toggle
   * lives in the `"general"` branch below. */
  communityUpdate: boolean;
  onToggleCommunityUpdate: () => void;
  onUnwired: (label: string) => void;
}) {
  if (menu === "appearance") {
    return (
      <Page title="Apparence">
        <SettingsGroup title="Thème">
          {(["Clair", "Sombre", "Auto"] as const).map((label) => (
            <button
              key={label}
              type="button"
              className="tr"
              onClick={() => onUnwired(`Thème : ${label}`)}
            >
              <Ico name={label === "Clair" ? "sparkle" : label === "Sombre" ? "disc" : "refresh"} />
              <span className="lbl">{label}</span>
            </button>
          ))}
        </SettingsGroup>
        <SettingsGroup title="Texte">
          <SettingsRow
            title="Taille de police"
            desc="Ajuster la taille du texte dans toute l'application."
            control={<Badge tone="root">pas branché</Badge>}
          />
        </SettingsGroup>
      </Page>
    );
  }

  if (menu === "models") {
    return (
      <Page title="Modèles">
        <SettingsGroup title="Modèle local actif">
          {llmModels.map(([id, name, sub]) => (
            <button
              key={id}
              className={`tr ${llm === id ? "sel" : ""}`}
              type="button"
              onClick={() => onSelectModel(id, name)}
              title={sub}
            >
              <Ico name={llm === id ? "check" : "box"} />
              <span className="lbl">
                {name}
                <div className="muted" style={{ fontSize: 11 }}>
                  {sub}
                </div>
              </span>
            </button>
          ))}
        </SettingsGroup>
        <Panel
          icon="info"
          title="Téléchargement et chargement à la demande"
          bodyClassName="why"
        >
          Pas encore branchés au Core : le modèle se choisira ici, le
          téléchargement et le chargement resteront visibles dans ce panneau.
        </Panel>
      </Page>
    );
  }

  if (menu === "games") {
    return (
      <Page title="Jeux">
        <SettingsGroup title={`Jeux détectés (${games.length})`}>
          {games.length === 0 ? (
            <SettingsRow
              title="Aucun jeu détecté"
              desc="Aucun adaptateur n'a trouvé d'installation sur cette machine."
            />
          ) : (
            games.map((g) => (
              <Fragment key={g.id}>
                <SettingsRow
                  title={
                    <span style={{ display: "flex", alignItems: "center", gap: 8 }}>
                      <Ico name="layers" />
                      {g.name}
                    </span>
                  }
                  control={<Badge tone="root">détecté</Badge>}
                />
                <GamePathList gameId={g.id} onUnwired={onUnwired} />
              </Fragment>
            ))
          )}
        </SettingsGroup>
        <SettingsGroup title="Ajouter un jeu">
          <SettingsRow
            title="Dossier de jeu manuel"
            desc="L'ajout manuel d'un dossier de jeu n'existe pas encore côté Core."
            control={
              <button type="button" className="btn ghost" onClick={() => onUnwired("Ajouter un jeu")}>
                Ajouter
              </button>
            }
          />
        </SettingsGroup>
      </Page>
    );
  }

  if (menu === "mcp") {
    return (
      <Page title="MCP & Outils">
        <SettingsGroup title="Serveur MCP">
          <SettingsRow
            title="Outils exposés à une IA compatible MCP"
            desc="Lecture, mutation réversible et transactionnel (inspect_game, install_mod, modlab_plan…) — voir docs/01-architecture-decidee.md §7."
            control={<Badge tone="root">Phase 3</Badge>}
          />
          <SettingsRow
            title="Permissions à 3 niveaux"
            desc="Lecture automatique, mutation réversible configurable, action dangereuse toujours confirmée."
            control={<Badge tone="root">Phase 3</Badge>}
          />
        </SettingsGroup>
      </Page>
    );
  }

  if (menu === "privacy") {
    return (
      <Page title="Confidentialité">
        <SettingsGroup title="Données locales">
          <SettingsRow
            title="Tout reste sur cette machine"
            desc="Aucune télémétrie, aucun envoi de données hors de F:\\ModLab, sauf les appels aux Providers (Nexus, CurseForge) que tu déclenches toi-même."
          />
        </SettingsGroup>
        <SettingsGroup title="Suppression">
          <SettingsRow
            title="Corbeille interne"
            desc="ModLab ne supprime jamais un fichier directement : chaque suppression passe par une corbeille interne et reste réversible."
            control={<Badge tone="root">toujours actif</Badge>}
          />
        </SettingsGroup>
      </Page>
    );
  }

  if (menu === "notifications") {
    return (
      <Page title="Notifications">
        <SettingsGroup>
          <SettingsRow
            title="Fins de job et santé des instances"
            desc="Pas encore suivi par le Core : fins de job, mises à jour de mods et santé des instances arriveront ici."
            control={<Badge tone="root">pas branché</Badge>}
          />
        </SettingsGroup>
      </Page>
    );
  }

  return (
    <Page title="Général">
      <SettingsGroup title="Confirmations">
        <SettingsRow
          title="Mutations réversibles"
          desc="Demander confirmation avant d'appliquer une mutation réversible (installer, activer/désactiver un mod, changer l'ordre de charge) — voir §7.2."
          control={<Switch checked disabled onToggle={() => onUnwired("Confirmation des mutations réversibles")} />}
        />
        <SettingsRow
          title="Actions dangereuses"
          desc="Suppression, écriture hors du périmètre ModLab : toujours confirmées, sans exception."
          control={<Badge tone="root">toujours actif</Badge>}
        />
      </SettingsGroup>
      <SettingsGroup title="Données">
        <SettingsRow
          title="Racine des données"
          desc="Le dossier où ModLab range archives, mods déployés et la base SQLite."
          control={<code className="muted" style={{ fontSize: 12 }}>{dataRoot ?? "—"}</code>}
        />
        <DownloadsDirRow onUnwired={onUnwired} />
      </SettingsGroup>
      <SettingsGroup title="Communauté">
        <SettingsRow
          title="Mises à jour communautaires"
          desc="Active les écrans expérimentaux Profil, Atelier de patchs et Boutique (voir docs/deep-research-report.md) : profil social, stats, titres, patchs partagés et cosmétiques. Aperçu visuel seulement — aucune économie ni synchronisation réelle pour l'instant."
          control={<Switch checked={communityUpdate} onToggle={onToggleCommunityUpdate} />}
        />
      </SettingsGroup>
    </Page>
  );
}
