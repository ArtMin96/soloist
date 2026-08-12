import type { ThemeColorRole } from "@/domain";

export interface ThemeColorRoleMeta {
  label: string;
  group: "Main" | "Status" | "Other";
}

// This exhaustive record is the editor/runtime vocabulary. Adding a role to the wire type cannot
// silently omit it from validation or the Advanced editor because TypeScript requires an entry.
export const THEME_COLOR_ROLE_META: Record<ThemeColorRole, ThemeColorRoleMeta> = {
  canvas: { label: "Canvas", group: "Main" },
  chrome: { label: "Chrome", group: "Main" },
  toolbar: { label: "Toolbar", group: "Main" },
  toolbarForeground: { label: "Toolbar text", group: "Main" },
  toolbarBorder: { label: "Toolbar border", group: "Main" },
  toolbarControl: { label: "Toolbar control", group: "Main" },
  toolbarControlForeground: { label: "Toolbar control text", group: "Main" },
  toolbarControlHover: { label: "Toolbar control hover", group: "Main" },
  surface: { label: "Surface", group: "Main" },
  surfaceRaised: { label: "Raised surface", group: "Main" },
  surfaceOverlay: { label: "Overlay surface", group: "Main" },
  text: { label: "Text", group: "Main" },
  textMuted: { label: "Muted text", group: "Main" },
  border: { label: "Border", group: "Main" },
  input: { label: "Input", group: "Main" },
  focus: { label: "Focus", group: "Main" },
  accent: { label: "Accent", group: "Main" },
  accentForeground: { label: "Accent text", group: "Main" },
  secondary: { label: "Secondary", group: "Main" },
  secondaryForeground: { label: "Secondary text", group: "Main" },
  muted: { label: "Muted", group: "Main" },
  mutedForeground: { label: "Muted foreground", group: "Main" },
  placeholder: { label: "Placeholder", group: "Other" },
  secondaryLabel: { label: "Secondary label", group: "Other" },
  iconMuted: { label: "Muted icon", group: "Other" },
  error: { label: "Error", group: "Status" },
  errorForeground: { label: "Error text", group: "Status" },
  errorSurface: { label: "Error surface", group: "Status" },
  warning: { label: "Warning", group: "Status" },
  warningForeground: { label: "Warning text", group: "Status" },
  warningSurface: { label: "Warning surface", group: "Status" },
  update: { label: "Update", group: "Status" },
  updateForeground: { label: "Update text", group: "Status" },
  updateSurface: { label: "Update surface", group: "Status" },
  accentSurface: { label: "Accent surface", group: "Main" },
  accentSurfaceForeground: { label: "Accent surface text", group: "Main" },
  messageSurface: { label: "Message surface", group: "Other" },
  messageForeground: { label: "Message text", group: "Other" },
  messageAction: { label: "Message action", group: "Other" },
  messageActionForeground: { label: "Message action text", group: "Other" },
  messageActionHover: { label: "Message action hover", group: "Other" },
  codeBackground: { label: "Code background", group: "Other" },
  codeForeground: { label: "Code text", group: "Other" },
  sidebar: { label: "Sidebar", group: "Main" },
  sidebarForeground: { label: "Sidebar text", group: "Main" },
  sidebarMutedForeground: { label: "Sidebar muted text", group: "Main" },
  sidebarControlSurface: { label: "Sidebar control", group: "Main" },
  sidebarRowHover: { label: "Sidebar row hover", group: "Main" },
  sidebarRowActive: { label: "Sidebar row active", group: "Main" },
  sidebarRowSelected: { label: "Sidebar row selected", group: "Main" },
  sidebarBorder: { label: "Sidebar border", group: "Main" },
  terminalBackground: { label: "Terminal background", group: "Other" },
  terminalForeground: { label: "Terminal text", group: "Other" },
  terminalCursor: { label: "Terminal cursor", group: "Other" },
  terminalSelection: { label: "Terminal selection", group: "Other" },
  terminalScrollbar: { label: "Terminal scrollbar", group: "Other" },
  terminalScrollbarHover: { label: "Terminal scrollbar hover", group: "Other" },
};

export const THEME_COLOR_ROLES = Object.keys(THEME_COLOR_ROLE_META) as ThemeColorRole[];

export const THEME_COLOR_GROUPS = (["Main", "Status", "Other"] as const).map((name) => ({
  name,
  roles: THEME_COLOR_ROLES.filter((role) => THEME_COLOR_ROLE_META[role].group === name),
}));
