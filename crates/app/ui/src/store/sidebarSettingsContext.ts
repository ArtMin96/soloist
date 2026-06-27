import { createContext, useContext } from "react";
import { DEFAULT_SIDEBAR } from "@/lib/sidebar";
import type { Sidebar } from "@/domain";

// The live Sidebar settings read model: the document plus its auto-saving setter. Read by the
// always-rendered sidebar (which gates its footer button and empty-section hiding on it) and by
// the Sidebar settings panel, so it travels by context. The default is the documented defaults
// so a component rendered without the provider (a focused test) still works.
export interface SidebarSettingsState {
  sidebar: Sidebar;
  setSidebar: (next: Sidebar) => void;
}

const DEFAULT_STATE: SidebarSettingsState = {
  sidebar: DEFAULT_SIDEBAR,
  setSidebar: () => {},
};

export const SidebarSettingsContext = createContext<SidebarSettingsState>(DEFAULT_STATE);

/** The current Sidebar settings and the auto-saving setter. */
export function useSidebarSettings(): SidebarSettingsState {
  return useContext(SidebarSettingsContext);
}
