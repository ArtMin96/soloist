import type {
  AppliedTerminalTheme,
  SoloistThemeExtensions,
  ThemeAppearance,
  ThemeColors,
  ThemeExtensions,
  ThemeFile,
} from "@/domain";
import { BUILT_IN_THEMES, DEFAULT_THEME_ID } from "@/theme/catalog";

const HEX_COLOR = /^#([0-9a-f]{3,4}|[0-9a-f]{6}|[0-9a-f]{8})$/i;

export function normalizeHexColor(value: string): string | null {
  if (!HEX_COLOR.test(value)) return null;
  const digits = value.slice(1).toLowerCase();
  if (digits.length === 3 || digits.length === 4) {
    return `#${digits
      .split("")
      .map((digit) => digit + digit)
      .join("")}`;
  }
  return `#${digits}`;
}

function channels(value: string): [number, number, number] {
  const normalized = normalizeHexColor(value);
  if (!normalized) throw new Error("Theme derivation requires a validated hex color");
  return [
    Number.parseInt(normalized.slice(1, 3), 16),
    Number.parseInt(normalized.slice(3, 5), 16),
    Number.parseInt(normalized.slice(5, 7), 16),
  ];
}

function hex(red: number, green: number, blue: number): string {
  return `#${[red, green, blue]
    .map((channel) => Math.round(channel).toString(16).padStart(2, "0"))
    .join("")}`;
}

export function mixHex(from: string, to: string, amount: number): string {
  const a = channels(from);
  const b = channels(to);
  const weight = Math.max(0, Math.min(1, amount));
  return hex(
    a[0] + (b[0] - a[0]) * weight,
    a[1] + (b[1] - a[1]) * weight,
    a[2] + (b[2] - a[2]) * weight,
  );
}

function withAlpha(color: string, alpha: number): string {
  const normalized = normalizeHexColor(color);
  if (!normalized) throw new Error("Theme derivation requires a validated hex color");
  return `${normalized.slice(0, 7)}${Math.round(Math.max(0, Math.min(1, alpha)) * 255)
    .toString(16)
    .padStart(2, "0")}`;
}

function relativeLuminance(color: string): number {
  const linear = channels(color).map((channel) => {
    const value = channel / 255;
    return value <= 0.04045 ? value / 12.92 : ((value + 0.055) / 1.055) ** 2.4;
  });
  return linear[0] * 0.2126 + linear[1] * 0.7152 + linear[2] * 0.0722;
}

export function contrastRatio(first: string, second: string): number {
  const lighter = Math.max(relativeLuminance(first), relativeLuminance(second));
  const darker = Math.min(relativeLuminance(first), relativeLuminance(second));
  return (lighter + 0.05) / (darker + 0.05);
}

function readableForeground(background: string, first: string, second: string): string {
  return contrastRatio(background, first) >= contrastRatio(background, second) ? first : second;
}

export function ensureThemeContrast(
  color: string,
  backgrounds: readonly string[],
  minimum: number,
  target: string,
): string {
  if (backgrounds.every((background) => contrastRatio(color, background) >= minimum)) return color;
  for (let step = 1; step <= 100; step += 1) {
    const adjusted = mixHex(color, target, step / 100);
    if (backgrounds.every((background) => contrastRatio(adjusted, background) >= minimum)) {
      return adjusted;
    }
  }
  return target;
}

export function contrastSafeThemeColor(
  color: string,
  backgrounds: readonly string[],
  minimum = 4.5,
): string {
  const endpoints = [hex(0, 0, 0), hex(255, 255, 255)];
  const worstRatio = (value: string) =>
    Math.min(...backgrounds.map((background) => contrastRatio(value, background)));
  const target = endpoints.reduce((best, candidate) => {
    return worstRatio(candidate) > worstRatio(best) ? candidate : best;
  });
  return ensureThemeContrast(color, backgrounds, minimum, target);
}

export function themeColorsForAppearance(
  theme: ThemeFile,
  appearance: ThemeAppearance,
): ThemeColors | null {
  if (theme.appearance === appearance) return theme.colors;
  return theme.variants?.[appearance] ?? null;
}

export function themeSupportsAppearance(theme: ThemeFile, appearance: ThemeAppearance): boolean {
  return themeColorsForAppearance(theme, appearance) !== null;
}

function defaultColors(appearance: ThemeAppearance): ThemeColors {
  const theme = BUILT_IN_THEMES.find(({ id }) => id === DEFAULT_THEME_ID);
  const colors = theme && themeColorsForAppearance(theme, appearance);
  if (!colors) throw new Error("Soloist Default must provide light and dark palettes");
  return colors;
}

// Basic mode deliberately has only two inputs. Every dependent role is derived here so the editor,
// preview, saved file, and live app cannot use subtly different recipes.
export function deriveThemeColors(
  appearance: ThemeAppearance,
  background: string,
  accent: string,
): ThemeColors {
  const base = defaultColors(appearance);
  const ink = readableForeground(background, base.text, base.canvas);
  const raised = mixHex(background, ink, appearance === "dark" ? 0.055 : 0.025);
  const control = mixHex(background, ink, appearance === "dark" ? 0.1 : 0.055);
  const hover = mixHex(background, ink, appearance === "dark" ? 0.15 : 0.09);
  const border = mixHex(background, ink, appearance === "dark" ? 0.18 : 0.13);
  const mutedText = mixHex(ink, background, 0.42);
  const accentSurface = mixHex(background, accent, appearance === "dark" ? 0.18 : 0.12);

  return {
    ...base,
    canvas: background,
    chrome: background,
    toolbar: raised,
    toolbarForeground: ink,
    toolbarBorder: border,
    toolbarControl: control,
    toolbarControlForeground: ink,
    toolbarControlHover: hover,
    surface: background,
    surfaceRaised: raised,
    surfaceOverlay: raised,
    text: ink,
    textMuted: mutedText,
    border,
    input: border,
    focus: accent,
    accent,
    accentForeground: readableForeground(accent, base.text, base.canvas),
    secondary: control,
    secondaryForeground: ink,
    muted: raised,
    mutedForeground: mutedText,
    placeholder: mutedText,
    secondaryLabel: mutedText,
    iconMuted: mutedText,
    accentSurface,
    accentSurfaceForeground: ink,
    messageSurface: control,
    messageForeground: ink,
    messageAction: hover,
    messageActionForeground: ink,
    messageActionHover: mixHex(hover, accent, 0.15),
    codeBackground: mixHex(background, ink, appearance === "dark" ? 0.025 : 0.015),
    codeForeground: ink,
    sidebar: raised,
    sidebarForeground: ink,
    sidebarMutedForeground: mutedText,
    sidebarControlSurface: control,
    sidebarRowHover: control,
    sidebarRowActive: accentSurface,
    sidebarRowSelected: accentSurface,
    sidebarBorder: border,
    terminalBackground: mixHex(background, ink, appearance === "dark" ? 0.025 : 0.015),
    terminalForeground: ink,
    terminalCursor: accent,
    terminalSelection: mixHex(background, accent, appearance === "dark" ? 0.35 : 0.22),
    terminalScrollbar: withAlpha(ink, 0.22),
    terminalScrollbarHover: withAlpha(ink, 0.38),
  };
}

function hueFrom(reference: string, hue: number, lightnessShift = 0): string {
  const [red, green, blue] = channels(reference).map((channel) => channel / 255);
  const maximum = Math.max(red, green, blue);
  const minimum = Math.min(red, green, blue);
  const lightness = (maximum + minimum) / 2;
  const delta = maximum - minimum;
  const saturation = delta === 0 ? 0 : delta / (1 - Math.abs(2 * lightness - 1));
  const chroma = (1 - Math.abs(2 * (lightness + lightnessShift) - 1)) * Math.max(0.45, saturation);
  const segment = (((hue % 360) + 360) % 360) / 60;
  const secondary = chroma * (1 - Math.abs((segment % 2) - 1));
  const [r, g, b] =
    segment < 1
      ? [chroma, secondary, 0]
      : segment < 2
        ? [secondary, chroma, 0]
        : segment < 3
          ? [0, chroma, secondary]
          : segment < 4
            ? [0, secondary, chroma]
            : segment < 5
              ? [secondary, 0, chroma]
              : [chroma, 0, secondary];
  const adjustedLightness = Math.max(0.18, Math.min(0.82, lightness + lightnessShift));
  const match = adjustedLightness - chroma / 2;
  return hex((r + match) * 255, (g + match) * 255, (b + match) * 255);
}

export function deriveThemeExtensions(
  colors: ThemeColors,
  appearance: ThemeAppearance,
  explicit: Partial<SoloistThemeExtensions> = {},
): ThemeExtensions {
  const railBackgrounds = [
    colors.sidebar,
    colors.sidebarRowHover,
    colors.sidebarRowActive,
    colors.sidebarRowSelected,
  ];
  const statusColor = (color: string) =>
    ensureThemeContrast(color, railBackgrounds, 3, colors.sidebarForeground);
  const gitColor = (color: string) =>
    ensureThemeContrast(color, railBackgrounds, 4.5, colors.sidebarForeground);
  const fileColor = (color: string) =>
    ensureThemeContrast(color, railBackgrounds, 3, colors.sidebarForeground);
  const statusRunning = statusColor(
    hueFrom(colors.accent, 145, appearance === "dark" ? 0.04 : -0.05),
  );
  const statusExhausted = statusColor(mixHex(colors.error, colors.codeBackground, 0.14));
  const derived: ThemeExtensions = {
    statusRunning,
    statusStopped: statusColor(colors.textMuted),
    statusExhausted,
    statusAttention: statusColor(colors.warning),
    gitBranchLocal: gitColor(hueFrom(colors.accent, 300)),
    fileLanguageAmber: fileColor(colors.warning),
    fileLanguageAzure: fileColor(colors.accent),
    fileLanguageBlue: fileColor(mixHex(colors.accent, colors.update, 0.25)),
    fileLanguageCyan: fileColor(colors.update),
    fileLanguageGreen: fileColor(statusRunning),
    fileLanguageOrange: fileColor(mixHex(colors.warning, colors.error, 0.35)),
    fileLanguagePink: fileColor(hueFrom(colors.accent, 335)),
    fileLanguageRed: fileColor(colors.error),
    fileLanguageViolet: fileColor(
      hueFrom(colors.accent, 285, appearance === "dark" ? 0.12 : -0.04),
    ),
    statusTransition: statusColor(colors.warning),
    statusCrashed: statusColor(colors.error),
    gitModified: gitColor(colors.warning),
    gitAdded: gitColor(statusRunning),
    gitDeleted: gitColor(colors.error),
    gitConflicted: gitColor(statusExhausted),
    gitIgnored: gitColor(colors.textMuted),
    gitBranchSynced: gitColor(statusRunning),
    overlayScrim: withAlpha(colors.codeBackground, appearance === "dark" ? 0.62 : 0.45),
    shadowInk: withAlpha(
      appearance === "dark" ? colors.codeBackground : colors.codeForeground,
      appearance === "dark" ? 0.5 : 0.2,
    ),
  };
  for (const key of Object.keys(derived) as Array<keyof ThemeExtensions>) {
    const override = explicit[key];
    if (override) derived[key] = override;
  }
  return derived;
}

export function deriveTerminalTheme(
  colors: ThemeColors,
  appearance: ThemeAppearance,
  explicit: Partial<SoloistThemeExtensions> = {},
): AppliedTerminalTheme {
  const background = colors.terminalBackground;
  const foreground = colors.terminalForeground;
  const extensions = deriveThemeExtensions(colors, appearance, explicit);
  const emphasize = (color: string) =>
    mixHex(color, foreground, appearance === "dark" ? 0.32 : 0.16);
  const derived: AppliedTerminalTheme = {
    background,
    foreground,
    cursor: colors.terminalCursor,
    cursorAccent: background,
    selectionBackground: colors.terminalSelection,
    selectionInactiveBackground: mixHex(
      background,
      foreground,
      appearance === "dark" ? 0.18 : 0.12,
    ),
    scrollbarSliderBackground: colors.terminalScrollbar,
    scrollbarSliderHoverBackground: colors.terminalScrollbarHover,
    scrollbarSliderActiveBackground: withAlpha(foreground, 0.5),
    overviewRulerBorder: mixHex(background, foreground, 0.1),
    black: mixHex(background, foreground, appearance === "dark" ? 0.12 : 0.86),
    red: colors.error,
    green: extensions.statusRunning,
    yellow: colors.warning,
    blue: colors.accent,
    magenta: extensions.fileLanguageViolet,
    cyan: colors.update,
    white: mixHex(background, foreground, appearance === "dark" ? 0.84 : 0.18),
    brightBlack: mixHex(background, foreground, appearance === "dark" ? 0.6 : 0.65),
    brightRed: emphasize(colors.error),
    brightGreen: emphasize(extensions.statusRunning),
    brightYellow: emphasize(colors.warning),
    brightBlue: emphasize(colors.accent),
    brightMagenta: emphasize(extensions.fileLanguageViolet),
    brightCyan: emphasize(colors.update),
    brightWhite: appearance === "dark" ? foreground : mixHex(background, foreground, 0.01),
    searchMatchBackground: mixHex(background, foreground, appearance === "dark" ? 0.16 : 0.15),
    searchMatchBorder: mixHex(background, foreground, appearance === "dark" ? 0.58 : 0.62),
    searchMatchOverviewRuler: mixHex(background, foreground, 0.45),
    searchActiveMatchBackground: mixHex(
      background,
      colors.accent,
      appearance === "dark" ? 0.55 : 0.3,
    ),
    searchActiveMatchBorder: foreground,
    searchActiveMatchOverviewRuler: emphasize(colors.focus),
  };
  const overrideMap: Partial<Record<keyof AppliedTerminalTheme, keyof SoloistThemeExtensions>> = {
    selectionInactiveBackground: "terminalSelectionInactive",
    scrollbarSliderActiveBackground: "terminalScrollbarActive",
    overviewRulerBorder: "terminalOverviewRulerBorder",
    black: "terminalAnsiBlack",
    red: "terminalAnsiRed",
    green: "terminalAnsiGreen",
    yellow: "terminalAnsiYellow",
    blue: "terminalAnsiBlue",
    magenta: "terminalAnsiMagenta",
    cyan: "terminalAnsiCyan",
    white: "terminalAnsiWhite",
    brightBlack: "terminalAnsiBrightBlack",
    brightRed: "terminalAnsiBrightRed",
    brightGreen: "terminalAnsiBrightGreen",
    brightYellow: "terminalAnsiBrightYellow",
    brightBlue: "terminalAnsiBrightBlue",
    brightMagenta: "terminalAnsiBrightMagenta",
    brightCyan: "terminalAnsiBrightCyan",
    brightWhite: "terminalAnsiBrightWhite",
    searchMatchBackground: "terminalSearchMatchBackground",
    searchMatchBorder: "terminalSearchMatchBorder",
    searchMatchOverviewRuler: "terminalSearchMatchOverviewRuler",
    searchActiveMatchBackground: "terminalSearchActiveMatchBackground",
    searchActiveMatchBorder: "terminalSearchActiveMatchBorder",
    searchActiveMatchOverviewRuler: "terminalSearchActiveMatchOverviewRuler",
  };
  for (const [target, source] of Object.entries(overrideMap) as Array<
    [keyof AppliedTerminalTheme, keyof SoloistThemeExtensions]
  >) {
    const override = explicit[source];
    if (override) derived[target] = override;
  }
  return derived;
}
