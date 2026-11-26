import { useEffect, useMemo, useRef, useState } from "react";
import {
  deleteMod,
  inspectArchivedMod,
  installArchivedMod,
  installFomod,
  moveMods,
  reassignMod,
  renameMod,
  setModCategory,
  setModEnabled,
  uninstallMod,
  revealPath,
  type GameCategory,
  type LoadoutDto,
  type ModRow,
  type ShellSnapshot,
} from "../../api";
import { useConflicts } from "../../hooks/useConflicts";
import { useDependencies } from "../../hooks/useDependencies";
import type { TabId } from "../../gameChrome";

export type UseModsSpaceParams = {
  activeLoadout: LoadoutDto | undefined;
  entries: ModRow[];
  catalog: GameCategory[];
  otherLoadouts: LoadoutDto[];
  setSnap: React.Dispatch<React.SetStateAction<ShellSnapshot | null>>;
  setError: (msg: string | null) => void;
  setToast: (msg: string | null) => void;
  setVersionTick: React.Dispatch<React.SetStateAction<number>>;
  refreshTxs: () => void;
  setTab: (tab: TabId) => void;
  /** When true, the explorer lists every catalog category, including empty ones. */
  showEmptyCategories: boolean;
};

export function useModsSpace(params: UseModsSpaceParams) {
  const {
    activeLoadout,
    entries,
    catalog,
    otherLoadouts,
    setSnap,
    setError,
    setToast,
    setVersionTick,
    refreshTxs,
    setTab,
    showEmptyCategories,
  } = params;

  const [selectedMod, setSelectedMod] = useState<number | null>(null);
  const [query, setQuery] = useState("");
  /** Sidebar category filter (catalog id). Null = no category filter. */
  const [activeCategory, setActiveCategory] = useState<string | null>(null);
  const [fomodTarget, setFomodTarget] = useState<{
    modId: number;
    name: string;
    contentRoot: string;
  } | null>(null);
  const installingIdsRef = useRef(new Set<number>());
  const [installingIds, setInstallingIds] = useState<ReadonlySet<number>>(() => new Set());

  useEffect(() => {
    setActiveCategory(null);
  }, [activeLoadout?.id]);

  function markInstalling(modId: number, on: boolean) {
    const ids = installingIdsRef.current;
    if (on) ids.add(modId);
    else ids.delete(modId);
    setInstallingIds(new Set(ids));
  }

  const { conflictByMod } = useConflicts(activeLoadout, entries);
  const { dependencyByMod } = useDependencies(activeLoadout, entries);

  const filtered = useMemo(() => {
    const q = query.trim().toLowerCase();
    return entries.filter((e) => {
      if (activeCategory && e.category !== activeCategory) return false;
      if (!q) return true;
      const label = catalog.find((item) => item.id === e.category)?.label.toLowerCase() ?? "";
      return (
        e.name.toLowerCase().includes(q) ||
        e.category?.toLowerCase().includes(q) ||
        label.includes(q)
      );
    });
  }, [entries, query, catalog, activeCategory]);

  const categoryCounts = useMemo(() => {
    const counts = new Map<string, number>();
    for (const e of entries) {
      if (!e.category) continue;
      counts.set(e.category, (counts.get(e.category) ?? 0) + 1);
    }
    const labelOf = (id: string) => catalog.find((item) => item.id === id)?.label ?? id;
    const nonEmpty = Array.from(counts, ([name, count]) => ({ name, count })).sort((a, b) =>
      labelOf(a.name).localeCompare(labelOf(b.name), "fr"),
    );
    if (!showEmptyCategories) return nonEmpty;
    if (catalog.length === 0) return nonEmpty;
    return catalog
      .map((item) => ({ name: item.id, count: counts.get(item.id) ?? 0 }))
      .sort((a, b) => labelOf(a.name).localeCompare(labelOf(b.name), "fr"));
  }, [entries, catalog, showEmptyCategories]);

  function pickCategory(name: string) {
    setActiveCategory((prev) => (prev === name ? null : name));
    setTab("mods");
  }

  const selectedRow = entries.find((e) => e.modId === selectedMod) ?? null;
  const modConflict =
    selectedRow && conflictByMod ? conflictByMod.get(selectedRow.modId) : undefined;

  async function toggle(row: ModRow) {
    if (!activeLoadout) return;
    try {
      const next = await setModEnabled(activeLoadout.id, row.modId, !row.enabled);
      setSnap((prev) => (prev ? { ...prev, entries: next } : prev));
      setVersionTick((t) => t + 1);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }

  async function reorder(row: ModRow, to: number) {
    if (!activeLoadout) return;
    try {
      const next = await moveMods(activeLoadout.id, [row.modId], to);
      setSnap((prev) => (prev ? { ...prev, entries: next } : prev));
      setVersionTick((t) => t + 1);
      refreshTxs();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }

  async function installArchived(row: ModRow) {
    if (!activeLoadout) return;
    if (installingIdsRef.current.has(row.modId)) return;
    markInstalling(row.modId, true);
    let keepMarked = false;
    try {
      setToast(`Extraction de « ${row.name} »…`);
      const plan = await inspectArchivedMod(row.modId);
      if (plan.refusal) {
        setToast(plan.refusal);
        return;
      }
      if (plan.needsWizard) {
        keepMarked = true;
        setFomodTarget({ modId: row.modId, name: row.name, contentRoot: plan.contentRoot });
        return;
      }
      if (plan.shape === "unknown") {
        setToast(
          plan.refusal?.trim() ||
            `Contenu de « ${row.name} » non reconnu pour ce jeu. Rien n'a été installé.`,
        );
        return;
      }
      const rows = await installArchivedMod(row.modId, activeLoadout.id, plan.contentRoot);
      setSnap((s) => (s ? { ...s, entries: rows } : s));
      setToast(`« ${row.name} » installé — active-le quand tu veux.`);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      if (!keepMarked) markInstalling(row.modId, false);
    }
  }

  async function finishFomodInstall(picked: string[]) {
    if (!fomodTarget || !activeLoadout) return;
    const target = fomodTarget;
    const rows = await installFomod(target.modId, activeLoadout.id, target.contentRoot, picked);
    setSnap((s) => (s ? { ...s, entries: rows } : s));
    setToast(`« ${target.name} » installé — active-le quand tu veux.`);
    markInstalling(target.modId, false);
    setFomodTarget(null);
  }

  async function setCategoryFor(row: ModRow, category: string | null) {
    try {
      await setModCategory(row.modId, category);
      setSnap((s) =>
        s
          ? {
              ...s,
              entries: s.entries.map((e) =>
                e.modId === row.modId ? { ...e, category, categoryUserSet: true } : e,
              ),
            }
          : s,
      );
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }

  async function reassignModTo(row: ModRow, toLoadoutId: number) {
    if (!activeLoadout) return;
    const target = otherLoadouts.find((l) => l.id === toLoadoutId)?.name ?? "";
    try {
      const rows = await reassignMod(row.modId, activeLoadout.id, toLoadoutId);
      setSnap((s) => (s ? { ...s, entries: rows } : s));
      setToast(`« ${row.name} » déplacé vers « ${target} ».`);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }

  async function renameModRow(row: ModRow, name: string) {
    if (!activeLoadout) return;
    try {
      const rows = await renameMod(row.modId, activeLoadout.id, name);
      setSnap((s) => (s ? { ...s, entries: rows } : s));
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }

  async function deleteModRow(row: ModRow) {
    if (!activeLoadout) return;
    try {
      const rows = await deleteMod(row.modId, activeLoadout.id);
      setSnap((s) => (s ? { ...s, entries: rows } : s));
      setToast(`« ${row.name} » retiré de la liste.`);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }

  async function uninstallModRow(row: ModRow) {
    if (!activeLoadout) return;
    try {
      const rows = await uninstallMod(row.modId, activeLoadout.id);
      setSnap((s) => (s ? { ...s, entries: rows } : s));
      setToast(`« ${row.name} » désinstallé — revenu à l'état d'archive.`);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }

  function revealModFolder(row: ModRow) {
    const archive = row.archivePath?.trim();
    let path = row.sourcePath?.trim() ?? "";
    let select = false;
    if (!path) {
      if (archive) {
        path = archive;
        select = true;
      } else {
        setError("Aucun emplacement connu pour ce mod.");
        return;
      }
    } else if (row.status === "archived") {
      if (archive) {
        path = archive;
        select = true;
      } else {
        select = true;
      }
    }
    void revealPath(path, select).catch((e: unknown) =>
      setError(e instanceof Error ? e.message : String(e)),
    );
  }

  function revealModArchive(row: ModRow) {
    if (!row.archivePath) return;
    void revealPath(row.archivePath, true).catch((e: unknown) =>
      setError(e instanceof Error ? e.message : String(e)),
    );
  }

  return {
    selectedMod,
    setSelectedMod,
    query,
    setQuery,
    filtered,
    categoryCounts,
    activeCategory,
    pickCategory,
    selectedRow,
    modConflict,
    conflictByMod,
    dependencyByMod,
    toggle,
    reorder,
    installArchived,
    fomodTarget,
    setFomodTarget,
    finishFomodInstall,
    markInstalling,
    installingIds,
    setCategoryFor,
    reassignModTo,
    renameModRow,
    deleteModRow,
    uninstallModRow,
    revealModFolder,
    revealModArchive,
  };
}