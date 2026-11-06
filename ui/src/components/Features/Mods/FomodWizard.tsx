import { useEffect, useMemo, useState } from "react";
import { getFomodModule, type FomodGroup, type FomodGroupType, type FomodModule } from "../../../api";
import { Ico } from "../../../Ico";
import { Button, Checkbox, IconButton, Radio } from "../../ui";

/**
 * Whether `count` selected plugins satisfies a group — mirrors
 * `modlab_game_skyrim::fomod::GroupType::accepts` exactly, so the wizard
 * never lets the human submit something the Core would refuse anyway.
 */
function accepts(type: FomodGroupType, count: number): boolean {
  switch (type) {
    case "select-exactly-one":
      return count === 1;
    case "select-at-most-one":
      return count <= 1;
    case "select-at-least-one":
      return count >= 1;
    case "select-any":
    case "select-all":
      return true;
  }
}

/**
 * The FOMOD guided installer: one step at a time, same shape as MO2's own
 * — `installStep`s in order, each asking one or more grouped questions.
 * `select-all` groups have no real question (every plugin in them is always
 * included) and render as a plain list, not a control.
 *
 * What this does *not* implement — deliberately, not by oversight — is
 * documented on the Rust side (`modlab_game_skyrim::fomod`): step/plugin
 * visibility conditions, `moduleDependencies`, `conditionalFileInstalls`.
 * Every step here always shows.
 */
export function FomodWizard({
  contentRoot,
  onInstall,
  onClose,
}: {
  contentRoot: string;
  onInstall: (picked: string[]) => Promise<void>;
  onClose: () => void;
}) {
  const [module, setModule] = useState<FomodModule | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [stepIdx, setStepIdx] = useState(0);
  const [picked, setPicked] = useState<Set<string>>(new Set());
  const [installing, setInstalling] = useState(false);
  const [installError, setInstallError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    getFomodModule(contentRoot)
      .then((m) => {
        if (cancelled) return;
        setModule(m);
        // `select-all` groups have no real question — pre-included so the
        // per-step validity check doesn't ask the human to confirm a
        // choice they were never offered.
        const preselected = new Set<string>();
        for (const step of m.steps) {
          for (const group of step.groups) {
            if (group.groupType === "select-all") {
              for (const p of group.plugins) preselected.add(p.id);
            }
          }
        }
        setPicked(preselected);
      })
      .catch((e: unknown) => {
        if (!cancelled) setLoadError(e instanceof Error ? e.message : String(e));
      });
    return () => {
      cancelled = true;
    };
  }, [contentRoot]);

  const totalSteps = module?.steps.length ?? 0;
  // No steps at all is a valid FOMOD (nothing but `requiredInstallFiles`) —
  // it still needs a working "Installer" button, not a dead end.
  const step = module && totalSteps > 0 ? module.steps[stepIdx] : null;
  const isLastStep = totalSteps === 0 || stepIdx === totalSteps - 1;

  const stepValid = useMemo(() => {
    if (!step) return true;
    return step.groups.every((g) => {
      const count = g.plugins.filter((p) => picked.has(p.id)).length;
      return accepts(g.groupType, count);
    });
  }, [step, picked]);

  function pickRadio(group: FomodGroup, pluginId: string | null) {
    setPicked((prev) => {
      const next = new Set(prev);
      for (const p of group.plugins) next.delete(p.id);
      if (pluginId) next.add(pluginId);
      return next;
    });
  }

  function toggleCheckbox(pluginId: string) {
    setPicked((prev) => {
      const next = new Set(prev);
      if (next.has(pluginId)) next.delete(pluginId);
      else next.add(pluginId);
      return next;
    });
  }

  async function finish() {
    setInstalling(true);
    setInstallError(null);
    try {
      await onInstall([...picked]);
    } catch (e) {
      setInstallError(e instanceof Error ? e.message : String(e));
      setInstalling(false);
    }
  }

  return (
    <div className="modal-backdrop" role="presentation">
      <div
        className="modal fomod-wizard"
        role="dialog"
        aria-labelledby="fomod-title"
        onMouseDown={(e) => e.stopPropagation()}
      >
        <div className="modal-head">
          <div>
            <div className="eyebrow">Installeur guidé</div>
            <h2 id="fomod-title">{module?.name ?? "Lecture de l’archive…"}</h2>
          </div>
          <IconButton name="x" title="Annuler" onClick={onClose} />
        </div>

        {loadError ? <p className="modal-error">{loadError}</p> : null}
        {!loadError && !module ? (
          <p className="modal-copy">Lecture de ModuleConfig.xml…</p>
        ) : null}

        {module && !loadError ? (
          <>
            {step ? (
              <div className="fomod-steps">
                {module.steps.map((s, i) => (
                  <span
                    key={s.id}
                    className={`fomod-dot ${i === stepIdx ? "cur" : i < stepIdx ? "done" : ""}`}
                  />
                ))}
                <span className="muted" style={{ fontSize: 11.5 }}>
                  Étape {stepIdx + 1}/{totalSteps} · {step.name}
                </span>
              </div>
            ) : null}

            <div className="fomod-body">
              {!step ? (
                <p className="muted" style={{ fontSize: 12.5 }}>
                  Rien à choisir — l’archive n’installe que ses fichiers requis.
                </p>
              ) : step.groups.length === 0 ? (
                <p className="muted" style={{ fontSize: 12.5 }}>
                  Rien à choisir à cette étape.
                </p>
              ) : (
                step.groups.map((group) => (
                  <div className="fomod-group" key={group.id}>
                    <div className="fomod-group-h">{group.name}</div>
                    {group.groupType === "select-all"
                      ? group.plugins.map((p) => (
                          <div className="fomod-plugin" key={p.id}>
                            <Ico name="check" className="sm fomod-included" />
                            <span className="fomod-plugin-body">
                              <span className="fomod-plugin-name">{p.name}</span>
                              {p.description ? (
                                <span className="fomod-plugin-desc">{p.description}</span>
                              ) : null}
                            </span>
                          </div>
                        ))
                      : group.groupType === "select-exactly-one" ||
                          group.groupType === "select-at-most-one"
                        ? [
                            group.groupType === "select-at-most-one" ? (
                              <label className="fomod-plugin" key={`${group.id}-none`}>
                                <Radio
                                  name={group.id}
                                  checked={!group.plugins.some((p) => picked.has(p.id))}
                                  onSelect={() => pickRadio(group, null)}
                                />
                                <span className="fomod-plugin-body">
                                  <span className="fomod-plugin-name muted">Aucune</span>
                                </span>
                              </label>
                            ) : null,
                            ...group.plugins.map((p) => (
                              <label className="fomod-plugin" key={p.id}>
                                <Radio
                                  name={group.id}
                                  checked={picked.has(p.id)}
                                  onSelect={() => pickRadio(group, p.id)}
                                />
                                <span className="fomod-plugin-body">
                                  <span className="fomod-plugin-name">{p.name}</span>
                                  {p.description ? (
                                    <span className="fomod-plugin-desc">{p.description}</span>
                                  ) : null}
                                </span>
                              </label>
                            )),
                          ]
                        : group.plugins.map((p) => (
                            <label className="fomod-plugin" key={p.id}>
                              <Checkbox checked={picked.has(p.id)} onToggle={() => toggleCheckbox(p.id)} />
                              <span className="fomod-plugin-body">
                                <span className="fomod-plugin-name">{p.name}</span>
                                {p.description ? (
                                  <span className="fomod-plugin-desc">{p.description}</span>
                                ) : null}
                              </span>
                            </label>
                          ))}
                  </div>
                ))
              )}
            </div>

            {installError ? <p className="modal-error">{installError}</p> : null}

            <div className="modal-actions">
              <Button variant="sm-ghost" onClick={onClose} disabled={installing}>
                Annuler
              </Button>
              <span className="spacer" />
              {stepIdx > 0 ? (
                <Button variant="sm-ghost" onClick={() => setStepIdx((i) => i - 1)} disabled={installing}>
                  Précédent
                </Button>
              ) : null}
              {isLastStep ? (
                <Button
                  variant="sm"
                  className="accent"
                  disabled={!stepValid || installing}
                  onClick={() => void finish()}
                >
                  {installing ? "Installation…" : "Installer"}
                </Button>
              ) : (
                <Button
                  variant="sm"
                  className="accent"
                  disabled={!stepValid}
                  onClick={() => setStepIdx((i) => i + 1)}
                >
                  Suivant
                </Button>
              )}
            </div>
          </>
        ) : null}
      </div>
    </div>
  );
}
