import { useCallback, useState } from "react";
import { loadSidebarMode, saveSidebarMode, type SidebarMode } from "../lib/sidebarMode";

export function useSidebarMode(): readonly [SidebarMode, (mode: SidebarMode) => void] {
  const [mode, setMode] = useState<SidebarMode>(loadSidebarMode);

  const update = useCallback((next: SidebarMode) => {
    setMode(next);
    saveSidebarMode(next);
  }, []);

  return [mode, update] as const;
}
