import type { ReactNode } from "react";
import { sidebarSettings, setSidebarSettings } from "@/api";
import { DEFAULT_SIDEBAR } from "@/lib/sidebar";
import { SidebarSettingsContext } from "@/store/sidebarSettingsContext";
import { useSettingsResource } from "@/store/useSettingsResource";

// Loads the Sidebar settings once and provides them to the whole app, so the always-rendered
// sidebar restyles its projection live (footer button, empty-section hiding) and the Settings
// panel edits the same record. Mounted at the app root alongside AppearanceProvider — both hold
// settings a surface outside the Settings overlay must read.
export function SidebarSettingsProvider({ children }: { children: ReactNode }) {
  const { value, update } = useSettingsResource(
    sidebarSettings,
    setSidebarSettings,
    DEFAULT_SIDEBAR,
  );
  return (
    <SidebarSettingsContext value={{ sidebar: value, setSidebar: update }}>
      {children}
    </SidebarSettingsContext>
  );
}
