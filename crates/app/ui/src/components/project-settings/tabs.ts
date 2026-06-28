// The per-project settings page tabs — one ordered source the header rail iterates and the pane
// routes on (the page-scope echo of the global Settings overlay's tab list).

export type ProjectTabId = "overview" | "settings" | "notifications" | "commands";

export interface ProjectTab {
  id: ProjectTabId;
  label: string;
}

export const PROJECT_TABS: ProjectTab[] = [
  { id: "overview", label: "Overview" },
  { id: "settings", label: "Settings" },
  { id: "notifications", label: "Notifications" },
  { id: "commands", label: "Commands" },
];

// The DOM id of a tab's button — shared so the tabpanel can label itself by the active tab.
export function projectTabButtonId(id: ProjectTabId): string {
  return `project-settings-tab-${id}`;
}
