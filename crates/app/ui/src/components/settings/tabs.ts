// The Settings window's tabs — one ordered source the rail iterates and the overlay routes on.
// Every tab is listed (faithful to the eight-tab source); which ones have a built panel vs a
// placeholder is decided by the overlay, not duplicated here.

export type SettingsTabId =
  | "appearance"
  | "sidebar"
  | "hotkeys"
  | "agents"
  | "tools"
  | "templates"
  | "integrations"
  | "notifications"
  | "account";

export interface SettingsTab {
  id: SettingsTabId;
  label: string;
}

export const SETTINGS_TABS: SettingsTab[] = [
  { id: "appearance", label: "Appearance" },
  { id: "sidebar", label: "Sidebar" },
  { id: "hotkeys", label: "Hotkeys" },
  { id: "agents", label: "Agents" },
  { id: "tools", label: "Tools" },
  { id: "templates", label: "Templates" },
  { id: "integrations", label: "Integrations" },
  { id: "notifications", label: "Notifications" },
  { id: "account", label: "Account" },
];

// The tabs whose contents were never shown in the source and remain undefined pending an owner
// decision — rendered as an explicit "to be defined" stub, never invented.
export const UNDEFINED_TABS: ReadonlySet<SettingsTabId> = new Set(["account"]);

// The DOM id of a tab's rail button — shared so the tabpanel can label itself by the active tab.
export function settingsTabButtonId(id: SettingsTabId): string {
  return `settings-tab-${id}`;
}
