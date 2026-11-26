import { useEffect, useRef, useState } from "react";
import { writeUiMemory } from "../uiMemory";
import { tabsFor, type TabId } from "../gameChrome";
import type { SideAct } from "../components/Features/Sidebar/Sidebar";
import type { SettingsMenu } from "../components/Features/Settings/SettingsPane";
import type { CommunityMenuId } from "../components/Features/Community/CommunityMenu";
import {
  isFixedRightTab,
  type RightPaneState,
} from "../components/Features/Inspector/RightPanel";
import type { Density } from "../density";
import type { LoadoutDto } from "../api";
import type { DetectedGame } from "../api";
import { expandGameKeepingOthers } from "./shellNavHelpers";

export type UseShellNavigationParams = {
  hasSnap: boolean;
  gameId: string | null;
  loadoutId: number | null;
  setGameId: (id: string | null) => void;
  setLoadoutId: (id: number | null) => void;
  tab: TabId;
  setTab: (tab: TabId) => void;
  expanded: string[];
  setExpanded: React.Dispatch<React.SetStateAction<string[]>>;
  pane: RightPaneState;
  llm: string;
  communityUpdate: boolean;
  showEmptyCategories: boolean;
  density: Density;
  loadouts: LoadoutDto[];
  activeLoadout: LoadoutDto | undefined;
  activeGame: DetectedGame | undefined;
  refresh: (id: number | null, opts?: { hydrateMetadata?: boolean }) => Promise<number | null>;
  setSelectedMod: (id: number | null) => void;
  setError: (msg: string | null) => void;
  setPaletteOpen: React.Dispatch<React.SetStateAction<boolean>>;
  setTxOpen: React.Dispatch<React.SetStateAction<boolean>>;
  saveActive: () => Promise<void>;
};

export function useShellNavigation(params: UseShellNavigationParams) {
  const {
    hasSnap,
    gameId,
    loadoutId,
    setGameId,
    setLoadoutId,
    tab,
    setTab,
    expanded,
    setExpanded,
    pane,
    llm,
    communityUpdate,
    showEmptyCategories,
    density,
    loadouts,
    activeLoadout,
    activeGame,
    refresh,
    setSelectedMod,
    setError,
    setPaletteOpen,
    setTxOpen,
    saveActive,
  } = params;

  const [act, setAct] = useState<SideAct>("explorer");
  const [prevAct, setPrevAct] = useState<SideAct>("explorer");
  const [menuOpen, setMenuOpen] = useState(false);
  const [settingsMenu, setSettingsMenu] = useState<SettingsMenu>("general");
  const [communityMenu, setCommunityMenu] = useState<CommunityMenuId>("patchWorkshop");
  /** Last branch opened in each instance, so the sidebar can come back to it. */
  const lastBranch = useRef(new Map<number, number>());

  function isSpecialAct(a: SideAct): boolean {
    return a === "settings" || a === "profile" || a === "community";
  }

  function handleAct(next: SideAct) {
    if (isSpecialAct(next) && !isSpecialAct(act)) {
      setPrevAct(act);
    }
    setAct(next);
  }

  useEffect(() => {
    if (!hasSnap) return;
    writeUiMemory({
      gameId,
      loadoutId,
      tab,
      expanded,
      rightOpen: pane.tabs.length > 0,
      // Web pages (`web:<id>`) are session-only; memory keeps fixed tabs only.
      rightTab: pane.active && isFixedRightTab(pane.active) ? pane.active : "info",
      rightTabs: pane.tabs.filter(isFixedRightTab),
      llm,
      communityUpdate,
      showEmptyCategories,
      density,
    });
  }, [hasSnap, gameId, loadoutId, tab, expanded, pane, llm, communityUpdate, showEmptyCategories, density]);

  useEffect(() => {
    function onKey(e: KeyboardEvent) {
      if (!(e.ctrlKey || e.metaKey) || e.altKey) return;
      const k = e.key.toLowerCase();
      if (k === "k" || (e.shiftKey && k === "p")) {
        e.preventDefault();
        setPaletteOpen((v) => !v);
      } else if (k === "j") {
        e.preventDefault();
        setTxOpen((v) => !v);
      } else if (k === "s") {
        e.preventDefault();
        void saveActive();
      }
    }
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [activeLoadout]);

  useEffect(() => {
    if (!activeLoadout) return;
    lastBranch.current.set(activeLoadout.instanceId, activeLoadout.id);
  }, [activeLoadout]);

  async function pickLoadout(id: number, nextTab?: TabId) {
    setLoadoutId(id);
    setSelectedMod(null);
    const owner = loadouts.find((l) => l.id === id);
    if (owner) {
      setGameId(owner.gameId);
      setExpanded((current) => expandGameKeepingOthers(current, owner.gameId));
    }
    if (nextTab) setTab(nextTab);
    try {
      await refresh(id, { hydrateMetadata: false });
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }

  function openInstance(representativeId: number, nextTab?: TabId) {
    const row = loadouts.find((l) => l.id === representativeId);
    if (!row) return;
    const branches = loadouts
      .filter((l) => l.instanceId === row.instanceId)
      .sort((a, b) => a.id - b.id);
    const remembered = lastBranch.current.get(row.instanceId);
    const target = branches.find((l) => l.id === remembered) ?? branches[0];
    if (!target) return;
    if (target.id === activeLoadout?.id) {
      if (nextTab) setTab(nextTab);
      return;
    }
    void pickLoadout(target.id, nextTab);
  }

  function pickGame(id: string) {
    setGameId(id);
    setTab("dashboard");
    setSelectedMod(null);
    setExpanded((current) => expandGameKeepingOthers(current, id));
    const first = loadouts.find((l) => l.gameId === id);
    if (first) void pickLoadout(first.id, "dashboard");
  }

  const tabs = tabsFor(activeGame?.capabilities);
  const currentTab: TabId =
    tab !== "web" && tabs.some((t) => t.id === tab) ? tab : "dashboard";

  return {
    act,
    setAct,
    prevAct,
    menuOpen,
    setMenuOpen,
    settingsMenu,
    setSettingsMenu,
    communityMenu,
    setCommunityMenu,
    handleAct,
    pickGame,
    pickLoadout,
    openInstance,
    tabs,
    currentTab,
    lastBranch,
  };
}