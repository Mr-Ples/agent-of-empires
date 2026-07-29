import { safeGetItem, safeSetItem } from "./safeStorage";

export type SidebarMode = "sessions" | "issues";

export const SIDEBAR_MODE_KEY = "aoe-sidebar-mode";

export function loadSidebarMode(): SidebarMode {
  return safeGetItem(SIDEBAR_MODE_KEY) === "issues" ? "issues" : "sessions";
}

export function saveSidebarMode(mode: SidebarMode): void {
  safeSetItem(SIDEBAR_MODE_KEY, mode);
}
