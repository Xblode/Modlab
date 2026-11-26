import type { LoadoutDto } from "./api";

/**
 * One row per instance, for the sidebar and anywhere that counts instances.
 *
 * `id` stays the initial branch (lowest id) so a star stored against that
 * loadout still matches after the instance/branch split. `name` is the
 * instance name, not the branch name. The branch list itself is the raw
 * loadouts filtered by `instanceId`.
 */
export function instanceRows(loadouts: readonly LoadoutDto[]): LoadoutDto[] {
  const best = new Map<number, LoadoutDto>();
  for (const loadout of loadouts) {
    const prev = best.get(loadout.instanceId);
    if (!prev || loadout.id < prev.id) best.set(loadout.instanceId, loadout);
  }
  return [...best.values()]
    .sort((a, b) => a.id - b.id)
    .map((loadout) => ({ ...loadout, name: loadout.instanceName }));
}
