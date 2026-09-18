import { FormEvent, type ReactNode, useEffect, useMemo, useRef, useState } from "react";
import {
  getShellSnapshot,
  getLoadoutConflicts,
  createLoadout,
  mo2Import,
  launchSkse,
  setModEnabled,
  CONFLICT_PRIORITY_CAVEAT,
  type DetectedGame,
  type LoadoutConflicts,
  type LoadoutDto,
  type ModConflicts,
  type ModRow,
  type ShellSnapshot,
} from "./api";
import { Dashboard } from "./Dashboard";
import { useFavorites } from "./favorites";
import { tabsFor, type TabId } from "./gameChrome";
import { Ico } from "./Ico";
import { ModTable } from "./ModTable";
import { StubTab } from "./StubTab";


import { IconButton } from "./components/ui/IconButton";
import { Button } from "./components/ui/Button";
import { FavoriteButton } from "./components/ui/FavoriteButton";
import { MenuPicker, type MenuItem, type MenuVariant } from "./MenuPicker";

const LLM_MODELS = [
  ["qwen-1.5b", "Qwen2.5 1.5B", "défaut · CPU ok"],
  ["llama-3b", "Llama 3.2 3B", "meilleur raisonnement"],
  ["phi-mini", "Phi-3.5 mini", "compact"],
  ["qwen-8b", "Qwen3 8B", "si GPU ≥ 8 Go"],
] as const;

type SideAct = "explorer" | "search" | "history" | "tools" | "settings";
type RightTab = "info" | "claude";

type ChatMsg = { who: "me" | "ai"; text: string };

export type SettingsMenu = "general" | "notifications" | "models" | "games";

export default function App() {
  const [snap, setSnap] = useState<ShellSnapshot | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [act, setAct] = useState<SideAct>("explorer");
  const [settingsMenu, setSettingsMenu] = useState<SettingsMenu>("general");
  const [tab, setTab] = useState<TabId>("dashboard");
  const [gameId, setGameId] = useState<string | null>(null);
  const [loadoutId, setLoadoutId] = useState<number | null>(null);
  const [selectedMod, setSelectedMod] = useState<number | null>(null);
  const [expanded, setExpanded] = useState<string[]>([]);
  const [rightOpen, setRightOpen] = useState(false);
  const [rightTab, setRightTab] = useState<RightTab>("info");
  const [running, setRunning] = useState(false);
  const [toast, setToast] = useState<string | null>(null);
  const [llm, setLlm] = useState("qwen-1.5b");
  const [createOpen, setCreateOpen] = useState(false);
  const [mo2Open, setMo2Open] = useState(false);
  const [query, setQuery] = useState("");
  const [menuPickerOpen, setMenuPickerOpen] = useState(false);
  const [menuVariant, setMenuVariant] = useState<MenuVariant>("fichier");
  const menuButtonRef = useRef<HTMLButtonElement>(null);

  /**
   * Every menu entry lands here. Only the two entries the Core can already
   * serve are wired (new instance, MO2 import); the rest say so out loud
   * instead of pretending to work.
   */
  function handleMenuAction(item: MenuItem) {
    switch (item.id) {
      case "nouveau":
      case "nouveau-loadout":
        setCreateOpen(true);
        break;
      case "importer":
      case "importer-loadout":
        setMo2Open(true);
        break;
      default:
        setToast(`« ${item.label} » : pas encore branché au Core.`);
    }
  }
  /**
   * Conflict scan of the active loadout. `null` means *not scanned yet* — the
   * table must never render that as "no conflict".
   */
  const [conflicts, setConflicts] = useState<LoadoutConflicts | null>(null);
  const conflictsCache = useRef(new Map<string, LoadoutConflicts>());
  const { isFavorite, toggleFavorite, favoriteIds } = useFavorites();
  const [chat, setChat] = useState<ChatMsg[]>([
    {
      who: "ai",
      text: "Le moteur de règles diagnostique ; le LLM ne rédige qu’à partir des faits du Core. L’inférence llama-cpp n’est pas encore branchée dans ce shell — le panneau est là, le modèle se charge à la demande plus tard.",
    },
  ]);
  const [draft, setDraft] = useState("");

  useEffect(() => {
    let cancelled = false;
    getShellSnapshot()
      .then((s) => {
        if (cancelled) return;
        setSnap(s);
        setLoadoutId(s.selectedLoadoutId);
        const first = s.games[0]?.id ?? s.loadouts[0]?.gameId ?? null;
        setGameId(first);
        if (first) setExpanded([first]);
      })
      .catch((e: unknown) => {
        if (!cancelled) setError(e instanceof Error ? e.message : String(e));
      });
    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    if (toast == null) return;
    const t = window.setTimeout(() => setToast(null), 2800);
    return () => window.clearTimeout(t);
  }, [toast]);

  const games = snap?.games ?? [];
  const loadouts = snap?.loadouts ?? [];
  const activeGame: DetectedGame | undefined =
    games.find((g) => g.id === gameId) ?? games[0];
  const gameLoadouts = loadouts.filter(
    (l) => !activeGame || l.gameId === activeGame.id,
  );
  const activeLoadout: LoadoutDto | undefined =
    gameLoadouts.find((l) => l.id === loadoutId) ?? gameLoadouts[0];
  const entries = snap?.entries ?? [];
  const enabledCount = entries.filter((e) => e.enabled).length;
  const selectedRow = entries.find((e) => e.modId === selectedMod) ?? null;
  const tabs = tabsFor(activeGame?.capabilities);
  // A game switch can land on a tab the new game does not have.
  const currentTab: TabId = tabs.some((t) => t.id === tab) ? tab : "dashboard";
  const tabLabel = tabs.find((t) => t.id === currentTab)?.label ?? "—";

  // Ordered by the Core's own loadout order, not by when the star was clicked.
  const favorites = useMemo(
    () =>
      loadouts
        .filter((l) => favoriteIds.includes(l.id))
        .map((l) => ({
          loadout: l,
          short: games.find((x) => x.id === l.gameId)?.short ?? l.gameId,
        })),
    [favoriteIds, loadouts, games],
  );

  const filtered = useMemo(() => {
    const q = query.trim().toLowerCase();
    if (!q) return entries;
    return entries.filter((e) => e.name.toLowerCase().includes(q));
  }, [entries, query]);

  /**
   * The enabled set the current conflict scan describes. Enabling or
   * disabling a mod changes what conflicts exist, so this is what a scan is
   * keyed on — not the entries array identity.
   */
  const enabledKey = useMemo(
    () =>
      entries
        .filter((e) => e.enabled)
        .map((e) => e.modId)
        .join(","),
    [entries],
  );

  /**
   * Scanning a real loadout walks every enabled mod's folder, so this runs
   * once per (loadout, enabled set) instead of on every render: results are
   * memoized by that key, the call is debounced, and a key change cancels the
   * pending one. A stale or missing scan renders as *unknown* (no badge), never
   * as "no conflict".
   */
  useEffect(() => {
    if (!activeLoadout) {
      setConflicts(null);
      return;
    }
    const key = `${activeLoadout.id}|${enabledKey}`;
    const cached = conflictsCache.current.get(key);
    if (cached) {
      setConflicts(cached);
      return;
    }
    setConflicts(null);
    let cancelled = false;
    const timer = window.setTimeout(() => {
      getLoadoutConflicts(activeLoadout.id)
        .then((next) => {
          conflictsCache.current.set(key, next);
          if (!cancelled) setConflicts(next);
        })
        .catch(() => {
          /* stays null: an unscanned loadout is not an empty one */
        });
    }, 250);
    return () => {
      cancelled = true;
      window.clearTimeout(timer);
    };
  }, [activeLoadout, enabledKey]);

  const conflictByMod = useMemo(() => {
    if (!conflicts) return null;
    return new Map(conflicts.mods.map((m) => [m.modId, m]));
  }, [conflicts]);

  /**
   * The scan only covers enabled mods it could read. A disabled mod, or one
   * whose folder failed to read, is therefore absent from the map — and the
   * inspector stays quiet about it instead of claiming it conflicts with
   * nothing.
   */
  const modConflict: ModConflicts | undefined =
    selectedRow && conflictByMod ? conflictByMod.get(selectedRow.modId) : undefined;

  async function refresh(id: number | null) {
    const s = await getShellSnapshot(id);
    setSnap(s);
    setLoadoutId(s.selectedLoadoutId);
  }

  async function pickLoadout(id: number, nextTab?: TabId) {
    setLoadoutId(id);
    setSelectedMod(null);
    const owner = loadouts.find((l) => l.id === id);
    if (owner) {
      setGameId(owner.gameId);
      setExpanded((e) => (e.includes(owner.gameId) ? e : [...e, owner.gameId]));
    }
    if (nextTab) setTab(nextTab);
    try {
      await refresh(id);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }

  function pickGame(id: string) {
    setGameId(id);
    setTab("dashboard");
    setSelectedMod(null);
    setExpanded((e) => (e.includes(id) ? e : [...e, id]));
    const first = loadouts.find((l) => l.gameId === id);
    if (first) void pickLoadout(first.id, "dashboard");
  }

  function onFavorite(id: number) {
    const name = loadouts.find((l) => l.id === id)?.name ?? "";
    const now = toggleFavorite(id);
    setToast(
      now ? `« ${name} » ajoutée aux favoris` : `« ${name} » retirée des favoris`,
    );
  }

  function unwired(label: string) {
    setToast(`« ${label} » : pas encore branché au Core.`);
  }

  /**
   * Import an MO2 profile into a new loadout, then land on it. The Core
   * validates everything (folders exist, mods match) — the dialog only
   * surfaces its messages. Returns the count so the dialog can also show it
   * if a later refresh fails.
   */
  async function importMo2(
    profile: string,
    modsDir: string,
    name: string,
  ): Promise<number> {
    const result = await mo2Import(profile, modsDir, name);
    await refresh(result.loadout.id);
    setGameId(result.loadout.gameId);
    setExpanded((e) =>
      e.includes(result.loadout.gameId) ? e : [...e, result.loadout.gameId],
    );
    setTab("dashboard");
    setMo2Open(false);
    setToast(
      `${result.imported} mod(s) importé(s) dans « ${result.loadout.name} » — les mods pointent vers les dossiers MO2, en place.`,
    );
    return result.imported;
  }

  async function toggle(row: ModRow) {
    if (!activeLoadout) return;
    try {
      const next = await setModEnabled(activeLoadout.id, row.modId, !row.enabled);
      setSnap((prev) => (prev ? { ...prev, entries: next } : prev));
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }

  async function createInstance(name: string) {
    if (!activeGame) return;
    const created = await createLoadout(activeGame.id, name);
    const next = await getShellSnapshot(created.id);
    // The browser preview deliberately has a static fixture. Keep a newly
    // created preview instance visible there while the real Tauri shell gets
    // its authoritative list straight back from the Core.
    setSnap(
      next.loadouts.some((loadout) => loadout.id === created.id)
        ? next
        : { ...next, loadouts: [...next.loadouts, created], selectedLoadoutId: created.id },
    );
    setLoadoutId(created.id);
    setGameId(created.gameId);
    setTab("dashboard");
    setToast(`Instance « ${created.name} » créée.`);
    setCreateOpen(false);
  }

  async function play() {
    if (!activeLoadout) return;
    if (activeGame?.id !== "skyrimse") {
      setToast("Le lancement n’est câblé que pour Skyrim SE / SKSE pour l’instant.");
      return;
    }
    setRunning(true);
    setToast("Lancement SKSE via USVFS…");
    try {
      await launchSkse(activeLoadout.id);
      setToast("Session terminée.");
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setRunning(false);
    }
  }

  function sendChat(e?: FormEvent) {
    e?.preventDefault();
    const text = draft.trim();
    if (!text) return;
    setDraft("");
    setChat((c) => [
      ...c,
      { who: "me", text },
      {
        who: "ai",
        text: `Reçu (modèle ${llm}, file d’attente). Aucune inférence locale n’est lancée tant qu’un jeu tourne — et le binding n’est pas encore branché sur ce shell.`,
      },
    ]);
    setRightOpen(true);
    setRightTab("claude");
  }

  function openMod(id: number) {
    setSelectedMod(id);
    setRightOpen(true);
    setRightTab("info");
  }

  if (error && !snap) {
    return (
      <div className="empty">
        <h3>Impossible de joindre le Core</h3>
        <p>{error}</p>
      </div>
    );
  }

  return (
    <div className={`app ${rightOpen ? "" : "no-right"} ${running ? "running" : ""}`}>
      <aside className="side">
        <div className="side-tools">
          <IconButton
            name="menu"
            title="Menu"
            ref={menuButtonRef}
            aria-pressed={menuPickerOpen}
            onClick={() => setMenuPickerOpen((v) => !v)}
          />
          <div className="hact">
            {(
              [
                ["explorer", "Explorateur", "layers"],
                ["search", "Recherche", "search"],
                ["history", "Historique", "branch"],
                ["tools", "Outils", "tool"],
              ] as const
            ).map(([id, title, icon]) => (
              <IconButton key={id} name={icon} title={title} aria-pressed={act === id} onClick={() => setAct(id)} />
            ))}
          </div>
          <span className="spacer" />
          <IconButton name="globe" title="Nexus Mods" />
          <IconButton
            name="gear"
            title="Réglages"
            aria-pressed={act === "settings"}
            onClick={() => setAct("settings")}
          />
        </div>

        {menuPickerOpen ? (
          <MenuPicker
            anchorRef={menuButtonRef}
            variant={menuVariant}
            onClose={() => setMenuPickerOpen(false)}
            onVariantChange={setMenuVariant}
            onItemSelect={handleMenuAction}
          />
        ) : null}

        {act !== "settings" && act === "explorer" && activeLoadout ? (
          <div className="side-instance">
            <div className="lo">
              <div className="lo-top">
                <button className="lo-name" type="button">
                  <span className="fade">{activeLoadout.name}</span>
                </button>
                <span className="chan stable">stable</span>
              </div>
              <div className="lo-row">
                <button className="vsel" type="button">
                  r.—<span className="d ok">✓</span>
                  <span className="hd ok" />
                  <span className="spacer" />
                  <Ico name="chev" />
                </button>
                <button className="btn save-btn" type="button" disabled>
                  <Ico name="db" className="sm" />À jour
                </button>
              </div>
              <button
                className={`play-btn ${running ? "stop" : ""}`}
                type="button"
                onClick={() => void play()}
              >
                <Ico name={running ? "x" : "play"} />
                {running ? "Arrêter" : "Jouer"}
              </button>
              <button
                className="share-btn"
                type="button"
                onClick={() => setToast("Partage d’instance : pas encore branché.")}
              >
                <Ico name="share" />
                Partager
              </button>
              <div className="st">
                <span>
                  <b>{enabledCount}</b> mods
                </span>
                <span>
                  <b>{entries.length}</b> dans le loadout
                </span>
              </div>
            </div>
          </div>
        ) : null}

        <div className="side-body">
          {act === "explorer" ? (
            <>
              <Sec title="Favoris" count={favorites.length}>
                {favorites.length === 0 ? (
                  <div className="tr muted" style={{ fontStyle: "italic" }}>
                    Aucun favori — clique l’étoile d’une instance
                  </div>
                ) : (
                  favorites.map(({ loadout: l, short }) => (
                    <div
                      key={l.id}
                      className={`tr-row inst-row ${l.id === activeLoadout?.id ? "cur" : ""}`}
                    >
                      <button
                        className="tr-label"
                        type="button"
                        onClick={() => void pickLoadout(l.id, "mods")}
                      >
                        <Ico name="branch" />
                        <span className="lbl">{l.name}</span>
                        <span className="tail">
                          <span className="muted" style={{ fontSize: 10 }}>
                            {short}
                          </span>
                        </span>
                      </button>
                      <FavoriteButton on onToggle={() => onFavorite(l.id)} />
                    </div>
                  ))
                )}
              </Sec>
              <Sec
                title="Jeux"
                count={games.length}
                action={
                  <IconButton
                    name="plus"
                    className="sec-act"
                    title="Ajouter un jeu"
                    onClick={() => unwired("Ajouter un jeu")}
                  />
                }
              >
                {games.length === 0 ? (
                  <div className="tr muted" style={{ fontStyle: "italic" }}>
                    Aucun jeu détecté
                  </div>
                ) : (
                  games.map((g) => {
                    const open = expanded.includes(g.id);
                    return (
                      <div key={g.id}>
                        <div className={`tr-row ${g.id === activeGame?.id ? "sel" : ""}`}>
                          <button
                            className="tr-chev"
                            type="button"
                            title={open ? "Réduire" : "Déplier"}
                            onClick={(e) => {
                              e.stopPropagation();
                              setExpanded((v) =>
                                v.includes(g.id)
                                  ? v.filter((x) => x !== g.id)
                                  : [...v, g.id],
                              );
                            }}
                          >
                            <Ico name={open ? "chev" : "chevr"} className="sm" />
                          </button>
                          <button
                            className="tr-label"
                            type="button"
                            onClick={() => pickGame(g.id)}
                          >
                            <span className="lbl">{g.name}</span>
                          </button>
                        </div>
                        {open
                          ? loadouts
                              .filter((l) => l.gameId === g.id)
                              .map((l) => (
                                <div
                                  key={l.id}
                                  className={`tr-row inst-row ${l.id === activeLoadout?.id ? "cur" : ""}`}
                                >
                                  <button
                                    className="tr-label"
                                    type="button"
                                    onClick={() => void pickLoadout(l.id, "mods")}
                                  >
                                    <Ico name="branch" />
                                    <span className="lbl">{l.name}</span>
                                    <span className="tail">
                                      <span className="mono">r.—</span>
                                    </span>
                                  </button>
                                  <FavoriteButton
                                    on={isFavorite(l.id)}
                                    onToggle={() => onFavorite(l.id)}
                                  />
                                </div>
                              ))
                          : null}
                      </div>
                    );
                  })
                )}
              </Sec>
              <Sec title="Catégories" count={1}>
                <button className="tr" type="button">
                  <Ico name="folder" />
                  <span className="lbl">Mods</span>
                  <span className="tail">{entries.length}</span>
                </button>
              </Sec>
            </>
          ) : act === "search" ? (
            <div style={{ padding: "8px 8px 6px" }}>
              <div
                className="composer"
                style={{
                  height: 28,
                  padding: "0 10px",
                  borderRadius: 6,
                  fontSize: 12,
                  boxShadow: "none",
                  background: "var(--c2)",
                }}
              >
                <Ico name="search" className="sm" />
                <input
                  value={query}
                  onChange={(e) => setQuery(e.target.value)}
                  placeholder="Filtrer les mods…"
                  style={{ background: "none", border: 0, color: "var(--t1)", outline: "none", flex: 1 }}
                />
              </div>
            </div>
          ) : act === "history" ? (
            <Sec title="Révisions" count={0}>
              <div className="tr muted" style={{ fontStyle: "italic" }}>
                Pas encore de révisions immuables (cycle snapshot reporté).
              </div>
            </Sec>
          ) : act === "tools" ? (
            <Sec title="Outils" count={0}>
              <div className="tr muted" style={{ fontStyle: "italic" }}>
                Pas de registre d’outils dans le Core pour l’instant.
              </div>
            </Sec>
          ) : null}
          {act === "settings" ? (
            <Sec title="Menu" count={4}>
              {(
                [
                  ["general", "Général", "sliders"],
                  ["notifications", "Notifications", "alert"],
                  ["models", "Modèles", "box"],
                  ["games", "Jeux", "layers"],
                ] as const
              ).map(([id, label, icon]) => (
                <button
                  key={id}
                  className={`tr ${settingsMenu === id ? "sel" : ""}`}
                  type="button"
                  onClick={() => setSettingsMenu(id)}
                >
                  <Ico name={icon} />
                  <span className="lbl">{label}</span>
                </button>
              ))}
            </Sec>
          ) : null}
        </div>
        {act === "settings" ? (
          <div className="side-profile">
            <div className="lo">
              <div className="lo-top">
                <span className="lo-name">
                  <span className="fade">Profils info</span>
                </span>
                <span className="chan stable">local</span>
              </div>
              <div className="lo-row muted" style={{ fontSize: 11.5, padding: "0 2px" }}>
                Racine : {snap?.dataRoot ?? "—"}
              </div>
            </div>
          </div>
        ) : null}
        <div className="side-foot">
          {running ? (
            <span className="fi">
              <Ico name="play" />
              Skyrim en cours
            </span>
          ) : (
            <>
              <span className="fi fade" title="Contexte">
                <Ico name="box" />
                {activeGame?.short ?? "—"}
                {activeLoadout ? ` · ${activeLoadout.name}` : ""}
              </span>
              <span className="spacer" />
              <span className="fi" title="Backend de déploiement">
                <Ico name="layers" />
                {activeGame?.capabilities.deploymentLabel ?? "—"}
              </span>
            </>
          )}
        </div>
      </aside>

      <div className="main">
        <div className="sheet">
          <div className="tabs" role="tablist">
            <div className="tabs-l">
              {tabs.map((t) => (
                <button
                  key={t.id}
                  className="tab"
                  type="button"
                  role="tab"
                  aria-selected={currentTab === t.id}
                  onClick={() => setTab(t.id)}
                >
                  <Ico name={t.icon} className="sm" />
                  <span>{t.label}</span>
                  {t.id === "mods" ? <span className="cnt">{entries.length}</span> : null}
                </button>
              ))}
            </div>
            <div className="tabs-r">
              <button
                className="ti claude"
                type="button"
                aria-pressed={rightOpen && rightTab === "claude"}
                onClick={() => {
                  setRightOpen(true);
                  setRightTab("claude");
                }}
              >
                <Ico name="sparkle" />
                <span className="n">IA</span>
              </button>
            </div>
          </div>

          {currentTab === "dashboard" && activeGame ? (
            act === "settings" ? (
              <SettingsPane
                menu={settingsMenu}
                games={games}
                llm={llm}
                llmModels={LLM_MODELS}
                onSelectModel={(id, name) => {
                  setLlm(id);
                  setToast(`Modèle local sélectionné : ${name}`);
                }}
              />
            ) : (
              <Dashboard
                game={activeGame}
                loadouts={gameLoadouts}
                activeLoadoutId={activeLoadout?.id ?? null}
                modCount={activeLoadout?.modCount ?? entries.length}
                isFavorite={isFavorite}
                onToggleFavorite={onFavorite}
                onPickLoadout={(id) => void pickLoadout(id, "dashboard")}
                onCreateLoadout={() => setCreateOpen(true)}
                onImportSource={() => setMo2Open(true)}
                onGoToTab={(t) => setTab(t)}
                onUnwired={unwired}
              />
            )
          ) : (
            <>
              <div className="tbar">
                <div className="crumbs">
                  <Ico name="box" />
                  <b>{activeGame?.short ?? "—"}</b>
                  <Ico name="chevr" />
                  <b>{activeLoadout?.name ?? "—"}</b>
                  <Ico name="chevr" />
                  {tabLabel}
                </div>
                <span className="spacer" />
                <span className="muted" style={{ fontSize: 11.5 }}>
                  {snap?.dataRoot}
                </span>
              </div>
              {currentTab === "mods" ? (
                entries.length === 0 ? (
                  <div className="content">
                    <div className="empty">
                      <div>
                        <Ico name="box" />
                        <h3>Aucun mod dans ce loadout</h3>
                        <p>
                          Importe une instance MO2 via la CLI (`modlab mo2-import`) — l’UI
                          lit le même Core.
                        </p>
                      </div>
                    </div>
                  </div>
                ) : (
                  <ModTable
                    rows={filtered}
                    selectedId={selectedMod}
                    onSelect={openMod}
                    onToggle={(row) => void toggle(row)}
                    conflicts={conflictByMod}
                  />
                )
              ) : (
                <StubTab tab={currentTab} />
              )}
            </>
          )}
        </div>


        {rightOpen ? (
          <aside className="right">
            <div className="rhead">
              <button
                className="tab"
                type="button"
                aria-selected={rightTab === "info"}
                onClick={() => setRightTab("info")}
              >
                Info
              </button>
              <button
                className="tab"
                type="button"
                aria-selected={rightTab === "claude"}
                onClick={() => setRightTab("claude")}
              >
                Claude
              </button>
              <span className="spacer" />
              <IconButton name="x" title="Fermer" onClick={() => setRightOpen(false)} />
            </div>
            <div className="rbody">
              {rightTab === "info" ? (
                selectedRow ? (
                  <>
                    <div className="rtitle">
                      <div className="eyebrow">Mod</div>
                      <h2>{selectedRow.name}</h2>
                      <div className="line">
                        <span className="mono">#{selectedRow.modId}</span>
                        <span>{selectedRow.enabled ? "activé" : "désactivé"}</span>
                      </div>
                      <dl className="kv" style={{ marginTop: 12 }}>
                        <dt>Chemin</dt>
                        <dd>{selectedRow.sourcePath}</dd>
                        <dt>Priorité</dt>
                        <dd>{selectedRow.position}</dd>
                      </dl>
                    </div>
                    {modConflict ? (
                      <Sec
                        variant="rsec"
                        title="Conflits"
                        count={modConflict.wins + modConflict.loses}
                      >
                        <div style={{ fontSize: 12 }}>
                          {modConflict.wins > 0 ? (
                            <div style={{ display: "flex", gap: 8, padding: "3px 0" }}>
                              <span
                                className="badge win"
                                title={CONFLICT_PRIORITY_CAVEAT}
                              >
                                <Ico name="flag" />
                                {modConflict.wins}
                              </span>
                              <span>gagne sur des mods plus haut</span>
                            </div>
                          ) : null}
                          {modConflict.loses > 0 ? (
                            <div style={{ display: "flex", gap: 8, padding: "3px 0" }}>
                              <span
                                className="badge lose"
                                title={CONFLICT_PRIORITY_CAVEAT}
                              >
                                <Ico name="flag" />
                                {modConflict.loses}
                              </span>
                              <span>perd face à des mods plus bas</span>
                            </div>
                          ) : null}
                          {modConflict.wins === 0 && modConflict.loses === 0 ? (
                            <span className="muted">Aucun conflit de fichier.</span>
                          ) : null}
                        </div>
                      </Sec>
                    ) : null}
                  </>
                ) : (
                  <div className="rtitle">
                    <div className="eyebrow">Panneau</div>
                    <h2>Aucun mod sélectionné</h2>
                    <p className="muted">Clique une ligne de la table.</p>
                  </div>
                )
              ) : (
                <div className="chat" style={{ height: "100%" }}>
                  <div className="msgs">
                    {chat.map((m, i) => (
                      <div key={i} className={`msg ${m.who}`}>
                        {m.who === "ai" ? (
                          <div className="who">
                            <Ico name="sparkle" className="sm" />
                            Local · {llm}
                          </div>
                        ) : null}
                        {m.text}
                      </div>
                    ))}
                  </div>
                  <form className="in" onSubmit={sendChat}>
                    <input
                      value={draft}
                      onChange={(e) => setDraft(e.target.value)}
                      placeholder="Demander à Claude… (@mod pour cibler)"
                    />
                    <div className="row2">
                      <span>
                        Contexte : {activeLoadout?.name ?? "—"}
                        {selectedRow ? ` · ${selectedRow.name}` : ""}
                      </span>
                      <span className="spacer" />
                      <span className="kbd">↵</span>
                      <Ico name="send" className="sm" />
                    </div>
                  </form>
                </div>
              )}
            </div>
          </aside>
        ) : null}

        <div className="dock">
          <div className="composer-wrap">
            <form className="composer-box" onSubmit={sendChat}>
              <div className="composer">
                <Ico name="search" />
                <IconButton name="term" title="Console" />
                <input
                  value={draft}
                  onChange={(e) => setDraft(e.target.value)}
                  placeholder="Rechercher, commander, demander à Claude…"
                  autoComplete="off"
                />
                <span className="kbd">Ctrl K</span>
              </div>
              <div className="composer-toolbar">
                <button className="cb-add" type="button" title="Ajouter">
                  +
                </button>
                <button className="cb-select" type="button">
                  {LLM_MODELS.find((m) => m[0] === llm)?.[1]}
                  <Ico name="chev" />
                </button>
                <span className="spacer" />
                <button className="cb-claude" type="button" onClick={() => sendChat()}>
                  <Ico name="sparkle" />
                  Claude
                </button>
              </div>
            </form>
          </div>
        </div>
      </div>

      {toast ? (
        <div className="toast show">
          <Ico name="check" className="sm" />
          <span>{toast}</span>
        </div>
      ) : null}
      {error && snap ? (
        <div className="toast show" style={{ bottom: 80 }}>
          <Ico name="alert" className="sm" />
          <span>{error}</span>
        </div>
      ) : null}
      {createOpen && activeGame ? (
        <CreateLoadoutDialog
          gameName={activeGame.name}
          onClose={() => setCreateOpen(false)}
          onCreate={createInstance}
        />
      ) : null}
      {mo2Open && activeGame ? (
        <Mo2ImportDialog
          gameName={activeGame.name}
          onClose={() => setMo2Open(false)}
          onImport={importMo2}
        />
      ) : null}
    </div>
  );
}

/**
 * Migration entry point for an existing Mod Organizer 2 instance. Path
 * inputs, not a native folder picker: no dialog plugin is wired into the
 * shell yet, and paths are what the Core's import actually needs.
 */
function Mo2ImportDialog({
  gameName,
  onClose,
  onImport,
}: {
  gameName: string;
  onClose: () => void;
  onImport: (profile: string, modsDir: string, name: string) => Promise<number>;
}) {
  const [profile, setProfile] = useState("");
  const [modsDir, setModsDir] = useState("");
  const [name, setName] = useState("");
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState<string | null>(null);

  async function submit(e: FormEvent) {
    e.preventDefault();
    setBusy(true);
    setMessage(null);
    try {
      await onImport(profile.trim(), modsDir.trim(), name);
    } catch (error) {
      setMessage(error instanceof Error ? error.message : String(error));
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="modal-backdrop" role="presentation" onMouseDown={onClose}>
      <form
        className="modal"
        aria-labelledby="mo2-import-title"
        onSubmit={(e) => void submit(e)}
        onMouseDown={(e) => e.stopPropagation()}
      >
        <div className="modal-head">
          <div>
            <div className="eyebrow">Importer depuis MO2</div>
            <h2 id="mo2-import-title">{gameName}</h2>
          </div>
          <IconButton name="x" title="Fermer" onClick={onClose} />
        </div>
        <label className="modal-label" htmlFor="mo2-profile">
          Dossier de profil MO2
        </label>
        <input
          id="mo2-profile"
          className="modal-input"
          autoFocus
          value={profile}
          onChange={(e) => setProfile(e.target.value)}
          placeholder={`…\\Mod Organazer\\profiles\\${"Default"}`}
        />
        <p className="modal-copy">
          Celui qui contient <span className="mono">modlist.txt</span>.
        </p>
        <label className="modal-label" htmlFor="mo2-mods">
          Dossier <span className="mono">mods</span> de MO2
        </label>
        <input
          id="mo2-mods"
          className="modal-input"
          value={modsDir}
          onChange={(e) => setModsDir(e.target.value)}
          placeholder="…\Mod Organazer\mods"
        />
        <label className="modal-label" htmlFor="mo2-name">
          Nom de l’instance
        </label>
        <input
          id="mo2-name"
          className="modal-input"
          maxLength={100}
          placeholder="Ex. Main 2026"
          value={name}
          onChange={(e) => setName(e.target.value)}
        />
        <p className="modal-copy">
          Rien n’est copié ni déplacé : les mods pointeront vers les dossiers de
          MO2, en place. Les DLC et contenus Creation Club listés dans le profil
          sont ignorés.
        </p>
        {message ? <p className="modal-error">{message}</p> : null}
        <div className="modal-actions">
          <Button variant="sm-ghost" onClick={onClose}>
            Annuler
          </Button>
          <Button
            variant="sm"
            type="submit"
            disabled={busy || !profile.trim() || !modsDir.trim() || !name.trim()}
          >
            {busy ? "Import…" : "Importer"}
          </Button>
        </div>
      </form>
    </div>
  );
}

function CreateLoadoutDialog({
  gameName,
  onClose,
  onCreate,
}: {
  gameName: string;
  onClose: () => void;
  onCreate: (name: string) => Promise<void>;
}) {
  const [name, setName] = useState("");
  const [saving, setSaving] = useState(false);
  const [message, setMessage] = useState<string | null>(null);

  async function submit(e: FormEvent) {
    e.preventDefault();
    setSaving(true);
    setMessage(null);
    try {
      await onCreate(name);
    } catch (error) {
      setMessage(error instanceof Error ? error.message : String(error));
    } finally {
      setSaving(false);
    }
  }

  return (
    <div className="modal-backdrop" role="presentation" onMouseDown={onClose}>
      <form className="modal" aria-labelledby="create-loadout-title" onSubmit={(e) => void submit(e)} onMouseDown={(e) => e.stopPropagation()}>
        <div className="modal-head">
          <div><div className="eyebrow">Nouvelle instance</div><h2 id="create-loadout-title">{gameName}</h2></div>
          <IconButton name="x" title="Fermer" onClick={onClose} />
        </div>
        <label className="modal-label" htmlFor="loadout-name">Nom de l’instance</label>
        <input id="loadout-name" className="modal-input" autoFocus maxLength={100} placeholder="Ex. Labo météo" value={name} onChange={(e) => setName(e.target.value)} />
        <p className="modal-copy">Une instance commence vide — pour récupérer une instance Mod Organizer 2 existante, utilise « Importer depuis MO2 ».</p>
        {message ? <p className="modal-error">{message}</p> : null}
        <div className="modal-actions">
          <Button variant="sm-ghost" onClick={onClose}>Annuler</Button>
          <Button variant="sm" type="submit" disabled={saving || !name.trim()}>{saving ? "Création…" : "Créer l’instance"}</Button>
        </div>
      </form>
    </div>
  );
}

function Sec({
  title,
  count,
  action,
  variant,
  children,
}: {
  title: string;
  count: number;
  /** Optional affordance for the section, pinned to the right of the title row. */
  action?: ReactNode;
  /** `rsec` is the right-hand inspector's compact variant (maquette's `.rsec`). */
  variant?: "rsec";
  children: ReactNode;
}) {
  const [open, setOpen] = useState(true);
  return (
    <div className={`sec ${variant ?? ""} ${open ? "" : "closed"}`}>
      <div className="sec-h">
        <button
          className="sec-t"
          type="button"
          aria-expanded={open}
          onClick={() => setOpen((v) => !v)}
        >
          <Ico name="chev" />
          <span className="u">{title}</span>
          <span className="cnt">({count})</span>
        </button>
        {action}
      </div>
      <div className="sec-b">{children}</div>
    </div>
  );
}

/**
 * Settings detail surface, shown in the central content zone while the side
 * toolbar is in settings mode. One pane per "Side Menu" entry; only what the
 * app actually knows is shown interacted with, everything else is an honest,
 * Core-labelled placeholder.
 */
function SettingsPane({
  menu,
  games,
  llm,
  llmModels,
  onSelectModel,
}: {
  menu: SettingsMenu;
  games: DetectedGame[];
  llm: string;
  llmModels: readonly (readonly [string, string, string])[];
  onSelectModel: (id: string, name: string) => void;
}) {
  if (menu === "models") {
    return (
      <>
        <div className="tbar">
          <div className="crumbs">
            <Ico name="box" />
            <b>Réglages</b>
            <Ico name="chevr" />
            Modèles
          </div>
        </div>
        <div className="content">
          <div className="dash">
            <div className="dh">Modèle local actif</div>
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
            <div className="box" style={{ marginTop: 22 }}>
              <div className="bh">
                <Ico name="info" className="sm" />
                Téléchargement et chargement à la demande
              </div>
              <div className="why">
                Pas encore branchés au Core : le modèle se choisira ici, le
                téléchargement et le chargement resteront visibles dans ce
                panneau.
              </div>
            </div>
          </div>
        </div>
      </>
    );
  }
  if (menu === "games") {
    return (
      <>
        <div className="tbar">
          <div className="crumbs">
            <Ico name="box" />
            <b>Réglages</b>
            <Ico name="chevr" />
            Jeux
          </div>
        </div>
        <div className="content">
          <div className="dash">
            <div className="dh">Jeux détectés ({games.length})</div>
            {games.length === 0 ? (
              <div className="tr muted" style={{ fontStyle: "italic" }}>
                Aucun jeu détecté par les adaptateurs.
              </div>
            ) : (
              games.map((g) => (
                <div key={g.id} className="tr-row">
                  <span className="tr-label">
                    <Ico name="layers" />
                    <span className="lbl">{g.name}</span>
                    <span className="tail">
                      <span className="muted" style={{ fontSize: 10 }}>
                        {g.short}
                      </span>
                    </span>
                  </span>
                </div>
              ))
            )}
            <div className="dh">Ajouter un jeu</div>
            <div className="tr muted" style={{ fontStyle: "italic" }}>
              Requête Core : l’ajout manuel d’un dossier de jeu n’existe pas
              encore.
            </div>
          </div>
        </div>
      </>
    );
  }
  if (menu === "notifications") {
    return (
      <>
        <div className="tbar">
          <div className="crumbs">
            <Ico name="box" />
            <b>Réglages</b>
            <Ico name="chevr" />
            Notifications
          </div>
        </div>
        <div className="content">
          <div className="dash">
            <div className="dh">Notifications</div>
            <div className="tr muted" style={{ fontStyle: "italic" }}>
              Pas encore suivi par le Core : fins de job, mises à jour de mods
              et santé des instances arriveront ici.
            </div>
          </div>
        </div>
      </>
    );
  }
  return (
    <>
      <div className="tbar">
        <div className="crumbs">
          <Ico name="box" />
          <b>Réglages</b>
          <Ico name="chevr" />
          Général
        </div>
      </div>
      <div className="content">
        <div className="dash">
          <div className="dh">Général</div>
          <div className="tr muted" style={{ fontStyle: "italic" }}>
            Préférences générales : pas encore suivi par le Core.
          </div>
        </div>
      </div>
    </>
  );
}
