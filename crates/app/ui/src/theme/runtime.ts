import type {
  AppliedTheme,
  Appearance,
  ThemeAppearance,
  ThemeColorRole,
  ThemeDefinition,
  ThemeFile,
} from "@/domain";
import { BUILT_IN_THEMES, DEFAULT_THEME_ID } from "@/theme/catalog";
import { GLASS_OPACITY } from "@/theme/constraints";
import {
  deriveTerminalTheme,
  deriveThemeExtensions,
  themeColorsForAppearance,
  themeExtensionsForAppearance,
} from "@/theme/derive";

export const APPLIED_THEME_HINT_KEY = "soloist.applied-theme-hint";
const APPLIED_THEME_HINT_VERSION = 1;
const APPLIED_THEME_HINT_INTERVAL_MS = 250;
const MAX_COLOR_PERCENT = 100;
const GLASS_CONTROL_OPACITY_LIFT = 6;
const GLASS_HOVER_OPACITY_LIFT = 10;
const GLASS_ACTIVE_OPACITY_LIFT = 14;
const GLASS_BORDER_INK_MIX = 4;
const GLASS_HIGHLIGHT_MIX = { dark: 18, light: 28 } as const;
const GLASS_VARIABLE_PREFIX = "--glass-";
const TRANSITION_FREEZE_CSS = "*,*::before,*::after{transition:none!important}";

const THEME_ROLE_CSS_VARIABLE: Record<ThemeColorRole, `--theme-${string}`> = {
  canvas: "--theme-canvas",
  chrome: "--theme-chrome",
  toolbar: "--theme-toolbar",
  toolbarForeground: "--theme-toolbar-foreground",
  toolbarBorder: "--theme-toolbar-border",
  toolbarControl: "--theme-toolbar-control",
  toolbarControlForeground: "--theme-toolbar-control-foreground",
  toolbarControlHover: "--theme-toolbar-control-hover",
  surface: "--theme-surface",
  surfaceRaised: "--theme-surface-raised",
  surfaceOverlay: "--theme-surface-overlay",
  text: "--theme-text",
  textMuted: "--theme-text-muted",
  border: "--theme-border",
  input: "--theme-input",
  focus: "--theme-focus",
  accent: "--theme-accent",
  accentForeground: "--theme-accent-foreground",
  secondary: "--theme-secondary",
  secondaryForeground: "--theme-secondary-foreground",
  muted: "--theme-muted",
  mutedForeground: "--theme-muted-foreground",
  placeholder: "--theme-placeholder",
  secondaryLabel: "--theme-secondary-label",
  iconMuted: "--theme-icon-muted",
  error: "--theme-error",
  errorForeground: "--theme-error-foreground",
  errorSurface: "--theme-error-surface",
  warning: "--theme-warning",
  warningForeground: "--theme-warning-foreground",
  warningSurface: "--theme-warning-surface",
  update: "--theme-update",
  updateForeground: "--theme-update-foreground",
  updateSurface: "--theme-update-surface",
  accentSurface: "--theme-accent-surface",
  accentSurfaceForeground: "--theme-accent-surface-foreground",
  messageSurface: "--theme-message-surface",
  messageForeground: "--theme-message-foreground",
  messageAction: "--theme-message-action",
  messageActionForeground: "--theme-message-action-foreground",
  messageActionHover: "--theme-message-action-hover",
  codeBackground: "--theme-code-background",
  codeForeground: "--theme-code-foreground",
  sidebar: "--theme-sidebar",
  sidebarForeground: "--theme-sidebar-foreground",
  sidebarMutedForeground: "--theme-sidebar-muted-foreground",
  sidebarControlSurface: "--theme-sidebar-control-surface",
  sidebarRowHover: "--theme-sidebar-row-hover",
  sidebarRowActive: "--theme-sidebar-row-active",
  sidebarRowSelected: "--theme-sidebar-row-selected",
  sidebarBorder: "--theme-sidebar-border",
  terminalBackground: "--theme-terminal-background",
  terminalForeground: "--theme-terminal-foreground",
  terminalCursor: "--theme-terminal-cursor",
  terminalSelection: "--theme-terminal-selection",
  terminalScrollbar: "--theme-terminal-scrollbar",
  terminalScrollbarHover: "--theme-terminal-scrollbar-hover",
};

function themeSignature(theme: Omit<AppliedTheme, "signature">): string {
  const source = JSON.stringify([
    theme.id,
    theme.appearance,
    theme.colors,
    theme.extensions,
    theme.terminal,
    theme.glassOpacity,
  ]);
  let hash = 2166136261;
  for (let index = 0; index < source.length; index += 1) {
    hash ^= source.charCodeAt(index);
    hash = Math.imul(hash, 16777619);
  }
  return `${theme.id}:${theme.appearance}:${(hash >>> 0).toString(36)}`;
}

export function appliedThemeFromFile(
  theme: ThemeFile,
  appearance: ThemeAppearance,
  glassOpacity: number = GLASS_OPACITY.default,
): AppliedTheme | null {
  const colors = themeColorsForAppearance(theme, appearance);
  if (!colors) return null;
  const explicit = themeExtensionsForAppearance(theme, appearance);
  const extensions = deriveThemeExtensions(colors, appearance, explicit);
  const resolved = {
    id: theme.id,
    name: theme.name,
    appearance,
    colors,
    extensions,
    terminal: deriveTerminalTheme(colors, appearance, extensions, explicit),
    glassOpacity,
  };
  return { ...resolved, signature: themeSignature(resolved) };
}

export function resolveAppliedTheme(
  appearance: Appearance,
  themes: ThemeDefinition[],
  systemDark: boolean,
): AppliedTheme {
  const resolvedAppearance: ThemeAppearance =
    appearance.theme === "system" ? (systemDark ? "dark" : "light") : appearance.theme;
  const selectedId = appearance.selected_themes[resolvedAppearance];
  const selected = themes.find(({ id }) => id === selectedId);
  const fallback =
    themes.find(({ id }) => id === DEFAULT_THEME_ID) ??
    BUILT_IN_THEMES.find(({ id }) => id === DEFAULT_THEME_ID);
  const applied =
    (selected && appliedThemeFromFile(selected, resolvedAppearance, appearance.glass_opacity)) ??
    (fallback && appliedThemeFromFile(fallback, resolvedAppearance, appearance.glass_opacity));
  if (!applied) throw new Error("Soloist Default must provide light and dark palettes");
  return applied;
}

export function defaultAppliedTheme(dark: boolean): AppliedTheme {
  return resolveAppliedTheme(
    {
      theme: dark ? "dark" : "light",
      selected_themes: { light: DEFAULT_THEME_ID, dark: DEFAULT_THEME_ID },
      custom_themes: [],
      glass_opacity: GLASS_OPACITY.default,
      interface_font_scale: "medium",
      terminal: {
        focus_on_click: true,
        copy_on_select: false,
        font_family: null,
        font_weight: "w400",
        bold_font_weight: "w600",
        font_scale: "medium",
        line_height: "default",
        letter_spacing: "default",
        cursor_style: "block",
        cursor_inactive_style: "outline",
        cursor_blink: true,
      },
    },
    BUILT_IN_THEMES,
    dark,
  );
}

export type ThemeCssVariables = Record<`--${string}`, string>;

export function themeCssVariables(theme: AppliedTheme): ThemeCssVariables {
  const variables: ThemeCssVariables = {};
  for (const [role, variable] of Object.entries(THEME_ROLE_CSS_VARIABLE) as Array<
    [ThemeColorRole, `--theme-${string}`]
  >) {
    variables[variable] = theme.colors[role];
  }

  const controlOpacity = Math.min(
    MAX_COLOR_PERCENT,
    theme.glassOpacity + GLASS_CONTROL_OPACITY_LIFT,
  );
  const hoverOpacity = Math.min(MAX_COLOR_PERCENT, theme.glassOpacity + GLASS_HOVER_OPACITY_LIFT);
  const activeOpacity = Math.min(MAX_COLOR_PERCENT, theme.glassOpacity + GLASS_ACTIVE_OPACITY_LIFT);
  // A glass edge catches light, so the highlight is mixed from the palette's light end — the ink in
  // a dark theme, the canvas in a light one. Mixing it from the ink in a light theme lays a dark
  // line above the control's own border rather than a highlight above it.
  const lightEnd = theme.appearance === "dark" ? theme.colors.text : theme.colors.canvas;
  const glassHighlight = `color-mix(in srgb, ${lightEnd} ${GLASS_HIGHLIGHT_MIX[theme.appearance]}%, transparent)`;

  Object.assign(variables, {
    "--background": theme.colors.surface,
    "--foreground": theme.colors.text,
    "--card": theme.colors.surfaceRaised,
    "--card-foreground": theme.colors.text,
    "--popover": theme.colors.surfaceOverlay,
    "--popover-foreground": theme.colors.text,
    "--primary": theme.colors.accent,
    "--primary-foreground": theme.colors.accentForeground,
    "--secondary": theme.colors.secondary,
    "--secondary-foreground": theme.colors.secondaryForeground,
    "--muted": theme.colors.muted,
    "--muted-foreground": theme.colors.mutedForeground,
    "--accent": theme.colors.accentSurface,
    "--accent-foreground": theme.colors.accentSurfaceForeground,
    "--destructive": theme.colors.error,
    "--border": theme.colors.border,
    "--input": theme.colors.input,
    "--ring": theme.colors.focus,
    "--sidebar": theme.colors.sidebar,
    "--sidebar-foreground": theme.colors.sidebarForeground,
    "--sidebar-primary": theme.colors.accent,
    "--sidebar-primary-foreground": theme.colors.accentForeground,
    "--sidebar-accent": theme.colors.sidebarRowHover,
    "--sidebar-accent-foreground": theme.colors.sidebarForeground,
    "--sidebar-border": theme.colors.sidebarBorder,
    "--sidebar-ring": theme.colors.focus,
    "--status-running": theme.extensions.statusRunning,
    "--status-transition": theme.extensions.statusTransition,
    "--status-stopped": theme.extensions.statusStopped,
    "--status-crashed": theme.extensions.statusCrashed,
    "--status-exhausted": theme.extensions.statusExhausted,
    "--status-attention": theme.extensions.statusAttention,
    "--git-modified": theme.extensions.gitModified,
    "--git-added": theme.extensions.gitAdded,
    "--git-deleted": theme.extensions.gitDeleted,
    "--git-conflicted": theme.extensions.gitConflicted,
    "--git-ignored": theme.extensions.gitIgnored,
    "--git-branch-synced": theme.extensions.gitBranchSynced,
    "--git-branch-local": theme.extensions.gitBranchLocal,
    "--file-language-amber": theme.extensions.fileLanguageAmber,
    "--file-language-azure": theme.extensions.fileLanguageAzure,
    "--file-language-blue": theme.extensions.fileLanguageBlue,
    "--file-language-cyan": theme.extensions.fileLanguageCyan,
    "--file-language-green": theme.extensions.fileLanguageGreen,
    "--file-language-orange": theme.extensions.fileLanguageOrange,
    "--file-language-pink": theme.extensions.fileLanguagePink,
    "--file-language-red": theme.extensions.fileLanguageRed,
    "--file-language-violet": theme.extensions.fileLanguageViolet,
    "--dialog-overlay": theme.extensions.overlayScrim,
    "--shadow-ink": theme.extensions.shadowInk,
    "--terminal-background": theme.terminal.background,
    "--terminal-foreground": theme.terminal.foreground,
    "--glass-opacity": String(theme.glassOpacity / 100),
    "--glass-surface": `color-mix(in srgb, ${theme.colors.surfaceOverlay} ${theme.glassOpacity}%, transparent)`,
    "--glass-control-surface": `color-mix(in srgb, ${theme.colors.toolbarControl} ${controlOpacity}%, transparent)`,
    "--glass-control-hover": `color-mix(in srgb, ${theme.colors.toolbarControlHover} ${hoverOpacity}%, transparent)`,
    "--glass-control-active": `color-mix(in srgb, ${theme.colors.toolbarControlHover} ${activeOpacity}%, transparent)`,
    "--glass-border": `color-mix(in srgb, ${theme.colors.text} ${GLASS_BORDER_INK_MIX}%, ${theme.colors.border})`,
    "--glass-highlight": glassHighlight,
    "--glass-control-shadow": `inset 0 1px 0 ${glassHighlight}, 0 1px 3px -1px ${theme.extensions.shadowInk}`,
    "--glass-floating-shadow": `inset 0 1px 0 ${glassHighlight}, 0 18px 48px -20px ${theme.extensions.shadowInk}, 0 6px 16px -10px ${theme.extensions.shadowInk}`,
    "--glass-primary-shadow": `inset 0 1px 0 ${glassHighlight}, 0 2px 6px -2px ${theme.extensions.shadowInk}`,
  });
  return variables;
}

/**
 * Puts an applied palette on the document root, writing only the variables whose value changed and
 * touching nothing at all when the root already carries this palette.
 *
 * A palette swap additionally suspends transitions across the write, so the many properties that
 * would otherwise cross-fade between two palettes land in a single paint. A change confined to the
 * glass tint has no second palette to cross into, so it skips both the document-wide suspension and
 * the forced recalculation that flushes it — which is what a held opacity slider commits per frame.
 */
export function applyTheme(theme: AppliedTheme): void {
  const root = document.documentElement;
  const changed = Object.entries(themeCssVariables(theme)).filter(
    ([name, value]) => root.style.getPropertyValue(name) !== value,
  );
  if (changed.length === 0 && root.dataset.themeSignature === theme.signature) return;
  const paletteSwap =
    root.dataset.themeId !== theme.id ||
    root.dataset.themeAppearance !== theme.appearance ||
    changed.some(([name]) => !name.startsWith(GLASS_VARIABLE_PREFIX));
  const freeze = paletteSwap ? document.createElement("style") : null;
  if (freeze) {
    freeze.textContent = TRANSITION_FREEZE_CSS;
    document.head.appendChild(freeze);
  }
  for (const [name, value] of changed) {
    root.style.setProperty(name, value);
  }
  root.classList.toggle("dark", theme.appearance === "dark");
  root.dataset.themeId = theme.id;
  root.dataset.themeAppearance = theme.appearance;
  root.dataset.themeSignature = theme.signature;
  root.style.colorScheme = theme.appearance;
  if (freeze) {
    void window.getComputedStyle(root).backgroundColor;
    freeze.remove();
  }
}

export function readAppliedThemeHint(): AppliedTheme | null {
  try {
    const parsed = JSON.parse(window.localStorage.getItem(APPLIED_THEME_HINT_KEY) ?? "null") as {
      version?: unknown;
      theme?: unknown;
    } | null;
    if (parsed?.version !== APPLIED_THEME_HINT_VERSION || !parsed.theme) return null;
    const theme = parsed.theme as Partial<AppliedTheme>;
    if (
      typeof theme.id !== "string" ||
      typeof theme.name !== "string" ||
      (theme.appearance !== "light" && theme.appearance !== "dark") ||
      typeof theme.signature !== "string" ||
      !theme.colors ||
      !theme.extensions ||
      !theme.terminal
    ) {
      return null;
    }
    return theme as AppliedTheme;
  } catch {
    return null;
  }
}

let hintWriteTimer: ReturnType<typeof setTimeout> | null = null;
let pendingHint: AppliedTheme | null = null;

function storeAppliedThemeHint(theme: AppliedTheme): void {
  try {
    window.localStorage.setItem(
      APPLIED_THEME_HINT_KEY,
      JSON.stringify({ version: APPLIED_THEME_HINT_VERSION, theme }),
    );
  } catch {
    // The persisted core document remains authoritative when webview-local storage is unavailable.
  }
}

/**
 * Records the palette the next launch paints before React mounts.
 *
 * A change on its own is stored at once. A burst — a held opacity slider commits a palette per frame
 * — collapses onto one trailing write per interval, so serializing the palette and blocking on
 * storage costs that rather than one of each per frame. The last value in a burst is always written.
 */
export function writeAppliedThemeHint(theme: AppliedTheme): void {
  if (hintWriteTimer !== null) {
    pendingHint = theme;
    return;
  }
  storeAppliedThemeHint(theme);
  hintWriteTimer = setTimeout(() => {
    hintWriteTimer = null;
    const trailing = pendingHint;
    pendingHint = null;
    if (trailing) writeAppliedThemeHint(trailing);
  }, APPLIED_THEME_HINT_INTERVAL_MS);
}
