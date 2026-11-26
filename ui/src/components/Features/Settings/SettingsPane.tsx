import { Fragment, useEffect, useState, type ReactNode } from "react";
import {
  clearDownloadsDirOverride,
  clearGamePathOverride,
  downloadsDirSetting,
  getGamePathSlots,
  msaAccountStatus,
  msaBeginLogin,
  msaCancelLogin,
  msaDisconnect,
  msaPollLogin,
  onMsaLoginFinished,
  openExternalUrl,
  setDownloadsDirOverride,
  setGamePathOverride,
  type DetectedGame,
  type DownloadsDirSetting,
  type GameCategory,
  type GamePathSlot,
  type MsaAccountStatus,
  type MsaLoginStart,
} from "../../../api";
import { Ico } from "../../../Ico";
import { useGamePaths } from "../../../hooks/useGamePaths";
import { Badge, Button, Dialog, Input, Panel, SettingsRow, Switch } from "../../ui";
import { GamePathRows, slotIdsInGamePaths } from "../Dashboard/GamePathsSection";
import { PathRow } from "../Dashboard/PathRow";
import type { Density } from "../../../density";
import {
  readUiMemory,
  rememberWebHomepage,
  webHomepageSetting,
  writeUiMemory,
} from "../../../uiMemory";
import {
  WEB_HOMEPAGE_OPTIONS,
  type WebHomepage,
  type WebHomepageKind,
} from "../../../webHomepage";
import { DesignSystemPage } from "./DesignSystemPage";

export type SettingsMenu =
  | "general"
  | "account"
  | "appearance"
  | "browser"
  | "notifications"
  | "designsystem"
  | "games"
  | "categories"
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

/** Adapter slots that `game_paths` does not already draw. */
const EXTRA_SLOT_COPY: Record<string, string> = {
  launcher_exe:
    "Fichier lancé par Jouer (minecraft.exe CurseForge ou MinecraftLauncher.exe). Sans CurseForge, parcourez jusqu’à votre launcher.",
  exe: "Exécutable du jeu.",
  data_dir: "Dossier Data lu par le jeu.",
  usvfs_dll: "Bibliothèque USVFS de Mod Organizer 2.",
  saves_dir: "Dossier des sauvegardes.",
};

function extraSlotDescription(id: string): string {
  return EXTRA_SLOT_COPY[id] ?? "Chemin déclaré par ce jeu, en plus des dossiers principaux.";
}

/**
 * Same path rows as Mon jeu (`getGamePaths`), plus any adapter slot that
 * list does not cover (exe, Data, USVFS, saves).
 */
function GamePathList({
  gameId,
  loadoutId,
  onUnwired,
}: {
  gameId: string;
  loadoutId: number | null;
  onUnwired: (label: string) => void;
}) {
  const { paths, error, saveManaged, resetManaged, saveIni, resetIni } = useGamePaths(
    gameId,
    loadoutId,
  );
  const [slots, setSlots] = useState<GamePathSlot[] | null>(null);
  const [slotsError, setSlotsError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    setSlots(null);
    setSlotsError(null);
    getGamePathSlots(gameId)
      .then((next) => {
        if (!cancelled) setSlots(next);
      })
      .catch((e: unknown) => {
        if (!cancelled) setSlotsError(e instanceof Error ? e.message : String(e));
      });
    return () => {
      cancelled = true;
    };
  }, [gameId]);

  if (!paths && !error) {
    return <SettingsRow title="Chemins" desc="Lecture des chemins…" />;
  }

  const covered = paths ? slotIdsInGamePaths(paths) : new Set<string>();
  const extras = (slots ?? []).filter((slot) => !covered.has(slot.id));

  return (
    <div className="dash-paths">
      {error ? (
        <div className="settings-group">
          <SettingsRow title="Lecture impossible" desc={error} />
        </div>
      ) : null}
      {paths ? (
        <GamePathRows
          gameId={gameId}
          paths={paths}
          saveManaged={saveManaged}
          resetManaged={resetManaged}
          saveIni={saveIni}
          resetIni={resetIni}
          onError={onUnwired}
        />
      ) : null}
      {slotsError ? (
        <div className="settings-group">
          <SettingsRow title="Autres chemins" desc={slotsError} />
        </div>
      ) : null}
      {extras.length > 0 ? (
        <div className="settings-group">
          <div className="settings-group-title">Autres chemins</div>
          {extras.map((slot) => (
            <PathRow
              key={slot.id}
              epoch={`${gameId}:${slot.id}:${slot.overridePath ?? ""}:${slot.effective ?? ""}`}
              label={slot.label}
              description={extraSlotDescription(slot.id)}
              path={slot.effective}
              canEdit
              pickKind={slot.kind}
              startPath={slot.effective}
              editValue={slot.effective ?? ""}
              canReset={Boolean(slot.overridePath)}
              onSave={async (value) => {
                setSlots(await setGamePathOverride(gameId, slot.id, value));
              }}
              onReset={async () => {
                setSlots(await clearGamePathOverride(gameId, slot.id));
              }}
              onError={onUnwired}
            />
          ))}
        </div>
      ) : null}
    </div>
  );
}

/**
 * The shared downloads root. Same field as the other path rows: the path
 * stays visible, Enter or blur writes the override, Parcourir picks a folder.
 */
function DownloadsDirRow({ onUnwired }: { onUnwired: (label: string) => void }) {
  const [setting, setSetting] = useState<DownloadsDirSetting | null>(null);
  const [pending, setPending] = useState(true);

  useEffect(() => {
    let cancelled = false;
    downloadsDirSetting()
      .then((next) => {
        if (!cancelled) setSetting(next);
      })
      .catch((e: unknown) => onUnwired(e instanceof Error ? e.message : String(e)))
      .finally(() => {
        if (!cancelled) setPending(false);
      });
    return () => {
      cancelled = true;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  if (pending || !setting) {
    return (
      <SettingsRow
        title="Dossier de téléchargement"
        desc={pending ? "Détection en cours…" : "Les archives y sont rangées par jeu."}
      />
    );
  }

  return (
    <PathRow
      epoch={`${setting.effective}:${setting.overridePath ?? ""}`}
      label="Dossier de téléchargement"
      description="Les archives y sont rangées par jeu."
      path={setting.effective}
      canEdit
      pickKind="dir"
      startPath={setting.effective}
      editValue={setting.effective}
      canReset={setting.overridePath !== null}
      resetTitle="Revenir au dossier par défaut"
      onSave={async (value) => {
        setSetting(await setDownloadsDirOverride(value));
      }}
      onReset={async () => {
        setSetting(await clearDownloadsDirOverride());
      }}
      onError={onUnwired}
    />
  );
}

/**
 * Minecraft identity: local offline pseudo + optional Microsoft (auth-code + PKCE).
 * Jouer uses MSA online when connected; otherwise the offline pseudo.
 */
function AccountSettingsPage() {
  const [player, setPlayer] = useState(() => readUiMemory().minecraftPlayer);
  const [msa, setMsa] = useState<MsaAccountStatus | null>(null);
  const [busy, setBusy] = useState(false);
  const [login, setLogin] = useState<MsaLoginStart | null>(null);
  const [loginError, setLoginError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    msaAccountStatus()
      .then((s) => {
        if (!cancelled) setMsa(s);
      })
      .catch((e: unknown) => {
        if (!cancelled) {
          setMsa({
            connected: false,
            username: null,
            uuid: null,
            clientConfigured: false,
            clientId: null,
            clientIdSource: null,
            clientIdOverride: null,
            mode: "error",
            error: e instanceof Error ? e.message : String(e),
          });
        }
      });
    return () => {
      cancelled = true;
    };
  }, []);

  // Lives for the whole Compte page so closing the dialog mid-exchange still
  // refreshes the badge when the loopback thread finishes.
  useEffect(() => {
    let cancelled = false;
    let unlisten: (() => void) | undefined;
    void onMsaLoginFinished((ev) => {
      if (cancelled) return;
      setLogin(null);
      if (ev.status === "connected") {
        setLoginError(null);
      } else if (ev.error) {
        setLoginError(ev.error);
      }
      void msaAccountStatus()
        .then((next) => {
          if (!cancelled) setMsa(next);
        })
        .catch((e: unknown) => {
          if (!cancelled) {
            setLoginError(e instanceof Error ? e.message : String(e));
          }
        });
    }).then((off) => {
      if (cancelled) off();
      else unlisten = off;
    });
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, []);

  useEffect(() => {
    if (!login) return;
    let cancelled = false;
    let timer: number | undefined;
    const intervalMs = Math.max(1, login.interval) * 1000;

    async function applyFinished(status: string, error: string | null) {
      if (cancelled) return;
      setLogin(null);
      if (status === "connected") {
        setLoginError(null);
      } else {
        setLoginError(error ?? "Connexion Microsoft interrompue.");
      }
      try {
        const next = await msaAccountStatus();
        if (!cancelled) setMsa(next);
      } catch (e: unknown) {
        if (!cancelled) {
          setLoginError(e instanceof Error ? e.message : String(e));
        }
      }
    }

    async function tick() {
      if (cancelled) return;
      try {
        const poll = await msaPollLogin();
        if (cancelled) return;
        if (poll.status === "pending" || poll.status === "slow_down") {
          const wait = poll.status === "slow_down" ? intervalMs + 2000 : intervalMs;
          timer = window.setTimeout(() => {
            void tick();
          }, wait);
          return;
        }
        await applyFinished(poll.status, poll.error);
      } catch (e: unknown) {
        if (cancelled) return;
        setLogin(null);
        setLoginError(e instanceof Error ? e.message : String(e));
        try {
          const next = await msaAccountStatus();
          if (!cancelled) setMsa(next);
        } catch {
          /* status refresh best-effort */
        }
      }
    }

    // First poll immediately — don't wait a full interval after the redirect.
    timer = window.setTimeout(() => {
      void tick();
    }, 0);
    return () => {
      cancelled = true;
      if (timer != null) window.clearTimeout(timer);
    };
  }, [login]);

  function savePlayer(next: string) {
    const trimmed = next.trim() || "Player";
    setPlayer(trimmed);
    writeUiMemory({ minecraftPlayer: trimmed });
  }

  async function connect() {
    setBusy(true);
    setLoginError(null);
    try {
      const start = await msaBeginLogin();
      setLogin(start);
      try {
        await openExternalUrl(start.authUrl);
      } catch {
        /* human can open the URL from the dialog */
      }
    } catch (e: unknown) {
      // Real MSA errors — never route through onUnwired (« pas encore branché au Core »).
      setLoginError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  }

  async function cancelLogin() {
    setLogin(null);
    try {
      await msaCancelLogin();
    } catch {
      /* ignore */
    }
    // Exchange may have finished (and saved) after Annuler — refresh badge.
    try {
      setMsa(await msaAccountStatus());
    } catch {
      /* ignore */
    }
  }

  async function disconnect() {
    setBusy(true);
    setLoginError(null);
    try {
      const next = await msaDisconnect();
      setMsa(next);
    } catch (e: unknown) {
      setLoginError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  }

  const modeBadge =
    msa?.mode === "online" ? (
      <Badge tone="win">en ligne</Badge>
    ) : msa?.mode === "error" ? (
      <Badge tone="bad">erreur</Badge>
    ) : (
      <Badge tone="root">hors-ligne</Badge>
    );

  return (
    <Page title="Compte">
      <SettingsGroup title="Minecraft — hors-ligne">
        <SettingsRow
          title="Pseudo local"
          desc="Utilisé pour Jouer quand aucun compte Microsoft n’est connecté (UUID dérivé du pseudo)."
          control={
            <Input
              variant="field"
              value={player}
              onChange={(e) => setPlayer(e.target.value)}
              onBlur={() => savePlayer(player)}
              onKeyDown={(e) => {
                if (e.key === "Enter") {
                  (e.target as HTMLInputElement).blur();
                }
              }}
              aria-label="Pseudo Minecraft hors-ligne"
              style={{ width: 160 }}
            />
          }
        />
        <SettingsRow
          title="Mode Jouer"
          desc={
            msa?.connected
              ? `Session Microsoft active (${msa.username ?? "—"}) — Jouer utilise l’auth en ligne.`
              : "Sans compte Microsoft, Jouer reste en solo hors-ligne avec le pseudo local."
          }
          control={modeBadge}
        />
      </SettingsGroup>
      <SettingsGroup title="Microsoft">
        <SettingsRow
          title="Compte Microsoft"
          desc={
            msa?.connected
              ? `Connecté en tant que ${msa.username ?? "—"}. Serveurs officiels et skin premium.`
              : "Connexion via le navigateur (OAuth + PKCE). Requis pour les serveurs officiels ; le solo hors-ligne reste possible sans."
          }
          control={
            msa?.connected ? (
              <Button type="button" variant="ghost" disabled={busy} onClick={() => void disconnect()}>
                Déconnecter
              </Button>
            ) : (
              <Button type="button" disabled={busy} onClick={() => void connect()}>
                {busy ? "…" : "Connecter…"}
              </Button>
            )
          }
        />
        <SettingsRow
          title="État"
          desc={
            loginError ??
            msa?.error ??
            (msa?.connected
              ? "Jetons chiffrés côté application (DPAPI) — absents du Core."
              : "Aucun compte Microsoft lié.")
          }
          control={
            msa?.connected ? (
              <Badge tone="win">{msa.username ?? "connecté"}</Badge>
            ) : (
              <Badge tone="root">aucun compte</Badge>
            )
          }
        />
      </SettingsGroup>
      {login ? (
        <Dialog
          titleId="msa-pkce-login"
          title="Connexion Microsoft"
          eyebrow="Minecraft Java"
          onClose={() => void cancelLogin()}
          dismissOnBackdrop={false}
          actions={
            <>
              <Button type="button" variant="ghost" onClick={() => void cancelLogin()}>
                Annuler
              </Button>
              <Button type="button" onClick={() => void openExternalUrl(login.authUrl)}>
                Rouvrir la page
              </Button>
            </>
          }
        >
          <p style={{ margin: "0 0 12px", maxWidth: 420 }}>
            Une page Microsoft s’ouvre dans le navigateur. Connectez-vous avec votre compte. La page
            « Connexion réussie » n’apparaît qu’après l’enregistrement de la session — gardez cette
            fenêtre ouverte jusqu’à la confirmation ici.
          </p>
          <p style={{ margin: 0, opacity: 0.75, fontSize: 13 }}>{login.message}</p>
        </Dialog>
      ) : null}
    </Page>
  );
}

function BrowserSettingsPage() {
  const [homepage, setHomepage] = useState<WebHomepage>(() => webHomepageSetting());

  function pick(kind: WebHomepageKind) {
    const next: WebHomepage = { ...homepage, kind };
    setHomepage(next);
    rememberWebHomepage(next);
  }

  function setCustomUrl(customUrl: string) {
    const next: WebHomepage = { kind: "custom", customUrl };
    setHomepage(next);
    rememberWebHomepage(next);
  }

  return (
    <Page title="Navigateur">
      <SettingsGroup title="Page d’accueil">
        {WEB_HOMEPAGE_OPTIONS.map(({ kind, label }) => (
          <button
            key={kind}
            type="button"
            className={`tr ${homepage.kind === kind ? "sel" : ""}`}
            aria-pressed={homepage.kind === kind}
            onClick={() => pick(kind)}
          >
            <Ico name={homepage.kind === kind ? "check" : "globe"} />
            <span className="lbl">{label}</span>
          </button>
        ))}
        {homepage.kind === "custom" ? (
          <SettingsRow
            title="Adresse"
            desc="URL libre. Pour un moteur, utilise %s à la place de la requête."
            control={
              <Input
                value={homepage.customUrl}
                onChange={(e) => setCustomUrl(e.target.value)}
                placeholder="https://…"
                spellCheck={false}
                autoComplete="off"
                aria-label="Adresse de la page d’accueil"
              />
            }
          />
        ) : null}
      </SettingsGroup>
    </Page>
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
  activeGame,
  categoryCatalog,
  llm,
  llmModels,
  dataRoot,
  onSelectModel,
  communityUpdate,
  onToggleCommunityUpdate,
  density,
  onDensityChange,
  onUnwired,
  loadoutId,
}: {
  menu: SettingsMenu;
  games: DetectedGame[];
  /** Game whose category list is shown on the Catégorie page — the shell's
   * active game, not a separate picker. */
  activeGame: DetectedGame | undefined;
  /** Fixed list from `game_categories` for `activeGame`. */
  categoryCatalog: GameCategory[];
  llm: string;
  llmModels: readonly (readonly [string, string, string])[];
  dataRoot: string | undefined;
  onSelectModel: (id: string, name: string) => void;
  /** Whether the experimental "Profil / Communauté / Économie" preview
   * (`docs/deep-research-report.md`) is switched on — the actual toggle
   * lives in the `"general"` branch below. */
  communityUpdate: boolean;
  onToggleCommunityUpdate: () => void;
  /** Row and control size, remembered with the rest of the shell's memory. */
  density: Density;
  onDensityChange: (density: Density) => void;
  onUnwired: (label: string) => void;
  loadoutId: number | null;
}) {
  if (menu === "designsystem") {
    return <DesignSystemPage />;
  }

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
        <SettingsGroup title="Densité">
          {(
            [
              ["compact", "Compact"],
              ["comfortable", "Confortable"],
            ] as const
          ).map(([id, label]) => (
            <button
              key={id}
              type="button"
              className={`tr ${density === id ? "sel" : ""}`}
              aria-pressed={density === id}
              onClick={() => onDensityChange(id)}
            >
              <Ico name={density === id ? "check" : "box"} />
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

  if (menu === "browser") {
    return <BrowserSettingsPage />;
  }

  if (menu === "account") {
    return <AccountSettingsPage />;
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
                <div className="muted" style={{ fontSize: 12 }}>
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

  if (menu === "categories") {
    return (
      <Page title="Catégorie">
        {!activeGame ? (
          <SettingsGroup>
            <SettingsRow
              title="Aucun jeu actif"
              desc="Choisis une instance dans l’Explorateur pour voir la liste des catégories de ce jeu."
            />
          </SettingsGroup>
        ) : (
          <>
            <SettingsGroup title={`${activeGame.name} — ${categoryCatalog.length} catégorie${categoryCatalog.length === 1 ? "" : "s"}`}>
              {categoryCatalog.length === 0 ? (
                <SettingsRow
                  title="Liste vide"
                  desc="L’adaptateur de ce jeu n’a pas encore publié de catégories, ou le Core n’a pas répondu."
                />
              ) : (
                categoryCatalog.map((c) => (
                  <SettingsRow
                    key={c.id}
                    title={
                      <span style={{ display: "flex", alignItems: "center", gap: 8 }}>
                        <Ico name={c.icon || "box"} />
                        {c.label}
                      </span>
                    }
                    desc={`Identifiant : ${c.id}`}
                  />
                ))
              )}
            </SettingsGroup>
            <Panel
              icon="info"
              title="Liste fixe par jeu"
              bodyClassName="why"
            >
              Ces catégories viennent de l’adaptateur du jeu (commande Core{" "}
              <code>game_categories</code>). Tu les assignes aux mods depuis la
              fiche ou la colonne Catégorie. Le Core ne propose pas encore de les
              créer, renommer ou masquer ici — lecture seule pour l’instant.
            </Panel>
          </>
        )}
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
              desc="Aucune installation trouvée sur cette machine"
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
                <GamePathList gameId={g.id} loadoutId={loadoutId} onUnwired={onUnwired} />
              </Fragment>
            ))
          )}
        </SettingsGroup>
        <SettingsGroup title="Ajouter un jeu">
          <SettingsRow
            title="Dossier de jeu manuel"
            desc="L'ajout manuel d'un dossier de jeu n'existe pas encore côté Core."
            control={
              <Button variant="ghost" onClick={() => onUnwired("Ajouter un jeu")}>
                Ajouter
              </Button>
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
