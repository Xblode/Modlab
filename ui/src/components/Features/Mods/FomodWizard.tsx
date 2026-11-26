import { useEffect, useRef, useState, type MutableRefObject, type ReactNode } from "react";
import {
  fomodImage,
  getFomodModule,
  previewFomod,
  type FomodGroup,
  type FomodGroupType,
  type FomodModule,
  type FomodPlugin,
  type FomodStep,
} from "../../../api";
import { Button, Checkbox, Dialog, Radio } from "../../ui";

/**
 * Guided FOMOD installer. The questions, the flags and the file list are
 * settled in Rust (`modlab_game_skyrim::fomod`); this screen only renders
 * the latest view and sends back the checked plugin ids.
 */
export function FomodWizard({
  contentRoot,
  loadoutId,
  onInstall,
  onClose,
}: {
  contentRoot: string;
  loadoutId: number;
  onInstall: (picked: string[]) => Promise<void>;
  onClose: () => void;
}) {
  const [module, setModule] = useState<FomodModule | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [syncError, setSyncError] = useState<string | null>(null);
  const [stepId, setStepId] = useState<string | null>(null);
  const [phase, setPhase] = useState<"steps" | "summary">("steps");
  const [cursor, setCursor] = useState<string | null>(null);
  const [hoverId, setHoverId] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [installing, setInstalling] = useState(false);
  const [installError, setInstallError] = useState<string | null>(null);
  const pickedRef = useRef<string[]>([]);
  const ticket = useRef(0);
  const rowRefs = useRef(new Map<string, HTMLLabelElement>());
  const live = useRef({ module, phase, stepId, busy, installing, cursor, syncError });
  live.current = { module, phase, stepId, busy, installing, cursor, syncError };

  function applyView(view: FomodModule) {
    setModule(view);
    pickedRef.current = selectedIds(view);
    const visible = view.steps.filter((step) => step.visible);
    if (visible.length === 0) {
      setPhase("summary");
      setStepId(null);
      return;
    }
    setStepId((current) => (current && visible.some((step) => step.id === current) ? current : visible[0].id));
  }

  useEffect(() => {
    const mine = ++ticket.current;
    setLoadError(null);
    getFomodModule(contentRoot, loadoutId)
      .then((view) => {
        if (mine !== ticket.current) return;
        applyView(view);
      })
      .catch((e: unknown) => {
        if (mine === ticket.current) setLoadError(e instanceof Error ? e.message : String(e));
      });
    return () => {
      ticket.current += 1;
    };
    // The archive and the branch identify the wizard. A new pair starts over.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [contentRoot, loadoutId]);

  const visibleSteps = module?.steps.filter((step) => step.visible) ?? [];
  const onSummary = phase === "summary" || (module != null && visibleSteps.length === 0);
  const stepIndex = visibleSteps.findIndex((step) => step.id === stepId);
  const step = !onSummary && stepIndex >= 0 ? visibleSteps[stepIndex] : null;
  const stepInvalid = Boolean(step?.groups.some((group) => group.hint));
  const summaryInvalid = visibleSteps.some((item) => item.groups.some((group) => group.hint));

  useEffect(() => {
    if (onSummary || !stepId) return;
    const current = live.current.module?.steps.find((item) => item.id === stepId && item.visible);
    if (!current) return;
    const first = firstOption(current);
    setCursor(first);
    setHoverId(null);
    if (first) rowRefs.current.get(first)?.focus();
  }, [onSummary, stepId]);

  function reload(ids: string[]) {
    const mine = ++ticket.current;
    setBusy(true);
    setSyncError(null);
    previewFomod(contentRoot, loadoutId, ids)
      .then((view) => {
        if (mine !== ticket.current) return;
        applyView(view);
        setBusy(false);
      })
      .catch((e: unknown) => {
        if (mine !== ticket.current) return;
        setBusy(false);
        setSyncError(e instanceof Error ? e.message : String(e));
      });
  }

  function choose(group: FomodGroup, pluginId: string | null) {
    const plugin = pluginId ? group.plugins.find((item) => item.id === pluginId) : null;
    if (plugin?.locked || group.groupType === "select-all") return;
    if (pluginId === null && group.plugins.some((item) => item.selected && item.locked)) return;
    const ids = new Set(pickedRef.current);
    if (group.groupType === "select-exactly-one" || group.groupType === "select-at-most-one") {
      for (const item of group.plugins) ids.delete(item.id);
      if (pluginId) ids.add(pluginId);
    } else if (pluginId) {
      if (ids.has(pluginId)) ids.delete(pluginId);
      else ids.add(pluginId);
    }
    const next = [...ids];
    pickedRef.current = next;
    setModule((current) => (current ? paint(current, new Set(next)) : current));
    reload(next);
  }

  function activateCursor() {
    const current = live.current;
    if (!current.module || !current.cursor || current.phase !== "steps") return;
    const found = findOption(current.module, current.cursor);
    if (!found) return;
    choose(found.group, found.pluginId);
  }

  function moveCursor(delta: number) {
    const current = live.current;
    if (!current.module || current.phase !== "steps" || !current.stepId) return;
    const currentStep = current.module.steps.find((item) => item.id === current.stepId && item.visible);
    if (!currentStep) return;
    const options = optionIds(currentStep);
    if (options.length === 0) return;
    const at = Math.max(0, options.indexOf(current.cursor ?? options[0]));
    const next = options[Math.min(options.length - 1, Math.max(0, at + delta))];
    setCursor(next);
    setHoverId(null);
    rowRefs.current.get(next)?.focus();
  }

  function goForward() {
    const current = live.current;
    if (!current.module || current.busy || current.installing || current.syncError) return;
    const visible = current.module.steps.filter((item) => item.visible);
    if (current.phase === "summary" || visible.length === 0) {
      void finish();
      return;
    }
    const index = visible.findIndex((item) => item.id === current.stepId);
    const currentStep = visible[index];
    if (!currentStep || currentStep.groups.some((group) => group.hint)) return;
    if (index >= visible.length - 1) setPhase("summary");
    else setStepId(visible[index + 1].id);
  }

  function goBack() {
    const current = live.current;
    if (!current.module || current.installing) return;
    const visible = current.module.steps.filter((item) => item.visible);
    if (current.phase === "summary") {
      if (visible.length === 0) return;
      setPhase("steps");
      setStepId(visible[visible.length - 1].id);
      return;
    }
    const index = visible.findIndex((item) => item.id === current.stepId);
    if (index > 0) setStepId(visible[index - 1].id);
  }

  async function finish() {
    if (!module || busy || installing || syncError) return;
    const visible = module.steps.filter((item) => item.visible);
    const broken = visible.find((item) => item.groups.some((group) => group.hint));
    if (broken) {
      setPhase("steps");
      setStepId(broken.id);
      return;
    }
    setInstalling(true);
    setInstallError(null);
    try {
      await onInstall(pickedRef.current);
    } catch (e) {
      setInstallError(e instanceof Error ? e.message : String(e));
      setInstalling(false);
    }
  }

  const actions = useRef({ activateCursor, goForward, goBack });
  actions.current = { activateCursor, goForward, goBack };

  useEffect(() => {
    function onKey(event: KeyboardEvent) {
      if (event.altKey || event.ctrlKey || event.metaKey) return;
      const target = event.target;
      if (target instanceof HTMLButtonElement && (event.key === "Enter" || event.key === " ")) return;
      if (event.key === "ArrowDown" || event.key === "ArrowUp") {
        event.preventDefault();
        moveCursor(event.key === "ArrowDown" ? 1 : -1);
        return;
      }
      if (event.key === " " || event.code === "Space") {
        event.preventDefault();
        actions.current.activateCursor();
        return;
      }
      if (event.key === "Enter") {
        event.preventDefault();
        actions.current.goForward();
      }
    }
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
    // The handler reads the latest actions through the ref.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const shown = shownPlugin(step, hoverId, cursor);
  const headerPath = module?.image?.show ? module.image.path : null;
  const headerUrl = useArchiveImage(contentRoot, headerPath);
  const previewUrl = useArchiveImage(contentRoot, shown?.image ?? null);
  const showSteps = Boolean(module && !loadError);
  const canGoBack = showSteps && !installing && (onSummary ? visibleSteps.length > 0 : stepIndex > 0);

  return (
    <Dialog
      className="fomod-wizard"
      role="dialog"
      titleId="fomod-title"
      eyebrow="Installeur guidé"
      title={
        module ? (
          <span className="fomod-title">
            {headerUrl ? (
              <img
                src={headerUrl}
                alt=""
                style={module.image?.height ? { height: Math.min(module.image.height, 96) } : undefined}
              />
            ) : null}
            <span>{module.name}</span>
          </span>
        ) : (
          "Lecture de l’archive…"
        )
      }
      closeTitle="Annuler"
      dismissOnBackdrop={false}
      onClose={() => {
        if (!installing) onClose();
      }}
      actions={
        showSteps ? (
          <>
            <Button variant="sm-ghost" onClick={onClose} disabled={installing}>
              Annuler
            </Button>
            <span className="spacer" />
            {canGoBack ? (
              <Button variant="sm-ghost" onClick={goBack} disabled={installing}>
                Précédent
              </Button>
            ) : null}
            {onSummary ? (
              <Button
                variant="sm"
                className="accent"
                disabled={summaryInvalid || busy || installing || Boolean(syncError)}
                onClick={() => void finish()}
              >
                {installing ? "Installation…" : "Installer"}
              </Button>
            ) : (
              <Button
                variant="sm"
                className="accent"
                disabled={stepInvalid || busy || installing || Boolean(syncError)}
                onClick={goForward}
              >
                Suivant
              </Button>
            )}
          </>
        ) : undefined
      }
    >
      {loadError ? <p className="modal-error">{loadError}</p> : null}
      {!loadError && !module ? <p className="modal-copy">Lecture de ModuleConfig.xml…</p> : null}

      {module && !loadError ? (
        <>
          <p className="fomod-step-label">
            {onSummary || visibleSteps.length === 0
              ? "Récapitulatif"
              : `Étape ${stepIndex + 1} / ${visibleSteps.length} – ${step?.name ?? ""}`}
          </p>

          {onSummary ? (
            <Summary module={module} />
          ) : step ? (
            <div className="fomod-layout">
              <div className="fomod-list">
                {step.groups.length === 0 ? (
                  <p className="muted">Rien à choisir à cette étape.</p>
                ) : (
                  step.groups.map((group) => (
                    <GroupBlock
                      key={group.id}
                      group={group}
                      cursor={hoverId ?? cursor}
                      rowRefs={rowRefs}
                      onChoose={choose}
                      onHover={setHoverId}
                      onCursor={setCursor}
                    />
                  ))
                )}
              </div>
              <aside className="fomod-preview">
                {previewUrl ? (
                  <img src={previewUrl} alt="" />
                ) : (
                  <div className="fomod-preview-empty">Aucune image</div>
                )}
                <p className="fomod-preview-desc">
                  {shown?.description || (shown ? "Pas de description." : "Survole une option pour la lire.")}
                </p>
              </aside>
            </div>
          ) : null}

          {syncError ? <p className="modal-error">{syncError}</p> : null}
          {installError ? <p className="modal-error">{installError}</p> : null}
        </>
      ) : null}
    </Dialog>
  );
}

function GroupBlock({
  group,
  cursor,
  rowRefs,
  onChoose,
  onHover,
  onCursor,
}: {
  group: FomodGroup;
  cursor: string | null;
  rowRefs: MutableRefObject<Map<string, HTMLLabelElement>>;
  onChoose: (group: FomodGroup, pluginId: string | null) => void;
  onHover: (id: string | null) => void;
  onCursor: (id: string) => void;
}) {
  const noneId = `${group.id}::none`;
  const noneLocked = group.plugins.some((plugin) => plugin.selected && plugin.locked);
  const noneSelected = !group.plugins.some((plugin) => plugin.selected);
  const radio = group.groupType === "select-exactly-one" || group.groupType === "select-at-most-one";

  return (
    <div className="fomod-group">
      <div className="fomod-group-h">{group.name}</div>
      <div className="fomod-group-rule">{ruleLabel(group.groupType)}</div>
      {group.groupType === "select-at-most-one" ? (
        <OptionRow
          id={noneId}
          cursor={cursor === noneId}
          locked={noneLocked}
          unusable={false}
          tip={null}
          rowRefs={rowRefs}
          onHover={onHover}
          onCursor={onCursor}
        >
          <Radio
            name={group.id}
            checked={noneSelected}
            disabled={noneLocked}
            tabIndex={-1}
            onSelect={() => onChoose(group, null)}
          />
          <span className="fomod-plugin-body">
            <span className="fomod-plugin-name">
              <span className="fade muted">Aucun</span>
            </span>
          </span>
        </OptionRow>
      ) : null}
      {group.plugins.map((plugin) => (
        <OptionRow
          key={plugin.id}
          id={plugin.id}
          cursor={cursor === plugin.id}
          locked={plugin.locked}
          unusable={plugin.typeName === "not-usable"}
          tip={plugin.reason}
          rowRefs={rowRefs}
          onHover={onHover}
          onCursor={onCursor}
        >
          {radio ? (
            <Radio
              name={group.id}
              checked={plugin.selected}
              disabled={plugin.locked}
              tabIndex={-1}
              onSelect={() => onChoose(group, plugin.id)}
            />
          ) : (
            <Checkbox
              checked={plugin.selected}
              disabled={plugin.locked}
              tabIndex={-1}
              onToggle={() => onChoose(group, plugin.id)}
            />
          )}
          <span className="fomod-plugin-body">
            <span className="fomod-plugin-name">
              <span className="fade">{plugin.name}</span>
              <PluginMark plugin={plugin} />
            </span>
          </span>
        </OptionRow>
      ))}
      {group.hint ? <p className="fomod-invalid">{group.hint}</p> : null}
    </div>
  );
}

function OptionRow({
  id,
  cursor,
  locked,
  unusable,
  tip,
  rowRefs,
  onHover,
  onCursor,
  children,
}: {
  id: string;
  cursor: boolean;
  locked: boolean;
  unusable: boolean;
  tip: string | null;
  rowRefs: MutableRefObject<Map<string, HTMLLabelElement>>;
  onHover: (id: string | null) => void;
  onCursor: (id: string) => void;
  children: ReactNode;
}) {
  const className = [
    "fomod-plugin",
    cursor ? "cursor" : "",
    locked ? "is-locked" : "",
    unusable ? "is-unusable" : "",
  ]
    .filter(Boolean)
    .join(" ");
  return (
    <label
      ref={(el) => {
        if (el) rowRefs.current.set(id, el);
        else rowRefs.current.delete(id);
      }}
      className={className}
      tabIndex={cursor ? 0 : -1}
      data-tip={tip ?? undefined}
      aria-disabled={locked || undefined}
      onMouseEnter={() => onHover(id)}
      onMouseLeave={() => onHover(null)}
      onFocus={() => onCursor(id)}
      onKeyDown={(event) => {
        if (event.key === " " || event.key === "Enter" || event.key === "ArrowUp" || event.key === "ArrowDown") {
          event.preventDefault();
        }
      }}
    >
      {children}
    </label>
  );
}

function PluginMark({ plugin }: { plugin: FomodPlugin }) {
  if (plugin.typeName === "recommended") return <span className="tag">Recommandé</span>;
  if (plugin.typeName === "could-be-usable") return <span className="tag archived">À vérifier</span>;
  return null;
}

function Summary({ module }: { module: FomodModule }) {
  const visible = module.steps.filter((step) => step.visible);
  if (visible.length === 0) {
    return <p className="muted">Rien à choisir — les fichiers requis seront installés.</p>;
  }
  return (
    <div className="fomod-summary">
      {visible.map((step) => (
        <section key={step.id}>
          <h3>{step.name}</h3>
          {step.groups.map((group) => {
            const chosen = group.plugins.filter((plugin) => plugin.selected);
            return (
              <div key={group.id}>
                <div className="fomod-summary-name">{group.name}</div>
                <ul>
                  {chosen.length === 0 ? (
                    <li className="muted">Aucun</li>
                  ) : (
                    chosen.map((plugin) => <li key={plugin.id}>{plugin.name}</li>)
                  )}
                </ul>
              </div>
            );
          })}
        </section>
      ))}
    </div>
  );
}

function ruleLabel(type: FomodGroupType): string {
  switch (type) {
    case "select-exactly-one":
      return "Choisis-en un";
    case "select-at-most-one":
      return "Facultatif, un au plus";
    case "select-at-least-one":
      return "Au moins un";
    case "select-any":
      return "Autant que tu veux";
    case "select-all":
      return "Tout est inclus";
  }
}

function selectedIds(view: FomodModule): string[] {
  const ids: string[] = [];
  for (const step of view.steps) {
    if (!step.visible) continue;
    for (const group of step.groups) {
      for (const plugin of group.plugins) {
        if (plugin.selected) ids.push(plugin.id);
      }
    }
  }
  return ids;
}

function paint(view: FomodModule, ids: Set<string>): FomodModule {
  return {
    ...view,
    steps: view.steps.map((step) => ({
      ...step,
      groups: step.groups.map((group) => ({
        ...group,
        plugins: group.plugins.map((plugin) => ({ ...plugin, selected: ids.has(plugin.id) })),
      })),
    })),
  };
}

function optionIds(step: FomodStep): string[] {
  const ids: string[] = [];
  for (const group of step.groups) {
    if (group.groupType === "select-at-most-one") ids.push(`${group.id}::none`);
    for (const plugin of group.plugins) ids.push(plugin.id);
  }
  return ids;
}

function firstOption(step: FomodStep): string | null {
  return optionIds(step)[0] ?? null;
}

function findOption(view: FomodModule, id: string): { group: FomodGroup; pluginId: string | null } | null {
  for (const step of view.steps) {
    for (const group of step.groups) {
      if (id === `${group.id}::none`) return { group, pluginId: null };
      if (group.plugins.some((plugin) => plugin.id === id)) return { group, pluginId: id };
    }
  }
  return null;
}

function shownPlugin(step: FomodStep | null, hoverId: string | null, cursor: string | null): FomodPlugin | null {
  if (!step) return null;
  const prefer = [hoverId, cursor].filter((id): id is string => Boolean(id) && !id?.endsWith("::none"));
  for (const id of prefer) {
    for (const group of step.groups) {
      const plugin = group.plugins.find((item) => item.id === id);
      if (plugin) return plugin;
    }
  }
  for (const group of step.groups) {
    const selected = group.plugins.find((plugin) => plugin.selected);
    if (selected) return selected;
  }
  return step.groups[0]?.plugins[0] ?? null;
}

function useArchiveImage(contentRoot: string, path: string | null): string | null {
  const [url, setUrl] = useState<string | null>(null);
  const cache = useRef(new Map<string, string | null>());
  useEffect(() => {
    if (!path) {
      setUrl(null);
      return;
    }
    const hit = cache.current.get(path);
    if (hit !== undefined) {
      setUrl(hit);
      return;
    }
    let cancel = false;
    fomodImage(contentRoot, path)
      .then((value) => {
        cache.current.set(path, value);
        if (!cancel) setUrl(value);
      })
      .catch(() => {
        cache.current.set(path, null);
        if (!cancel) setUrl(null);
      });
    return () => {
      cancel = true;
    };
  }, [contentRoot, path]);
  return url;
}
