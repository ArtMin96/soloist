import type { AppliedTheme, AppliedTerminalTheme } from "@/domain";
import { defaultAppliedTheme } from "@/theme/runtime";

export const ANSI_COLOR_NAMES = [
  "black",
  "red",
  "green",
  "yellow",
  "blue",
  "magenta",
  "cyan",
  "white",
  "brightBlack",
  "brightRed",
  "brightGreen",
  "brightYellow",
  "brightBlue",
  "brightMagenta",
  "brightCyan",
  "brightWhite",
] as const;

export type AnsiColorName = (typeof ANSI_COLOR_NAMES)[number];

export type TerminalColors = Pick<
  AppliedTerminalTheme,
  | "background"
  | "foreground"
  | "cursor"
  | "cursorAccent"
  | "selectionBackground"
  | "selectionInactiveBackground"
  | "scrollbarSliderBackground"
  | "scrollbarSliderHoverBackground"
  | "scrollbarSliderActiveBackground"
  | "overviewRulerBorder"
  | AnsiColorName
>;

export const TERMINAL_MINIMUM_CONTRAST_RATIO = 4.5;

function applied(value: boolean | AppliedTheme): AppliedTheme {
  return typeof value === "boolean" ? defaultAppliedTheme(value) : value;
}

// Kept as a projection rather than letting xterm read CSS. xterm requires parseable sRGB strings,
// while the same complete applied theme also powers previews and the initial prepaint cache.
export function terminalColors(theme: boolean | AppliedTheme): TerminalColors {
  const terminal = applied(theme).terminal;
  return {
    background: terminal.background,
    foreground: terminal.foreground,
    cursor: terminal.cursor,
    cursorAccent: terminal.cursorAccent,
    selectionBackground: terminal.selectionBackground,
    selectionInactiveBackground: terminal.selectionInactiveBackground,
    scrollbarSliderBackground: terminal.scrollbarSliderBackground,
    scrollbarSliderHoverBackground: terminal.scrollbarSliderHoverBackground,
    scrollbarSliderActiveBackground: terminal.scrollbarSliderActiveBackground,
    overviewRulerBorder: terminal.overviewRulerBorder,
    black: terminal.black,
    red: terminal.red,
    green: terminal.green,
    yellow: terminal.yellow,
    blue: terminal.blue,
    magenta: terminal.magenta,
    cyan: terminal.cyan,
    white: terminal.white,
    brightBlack: terminal.brightBlack,
    brightRed: terminal.brightRed,
    brightGreen: terminal.brightGreen,
    brightYellow: terminal.brightYellow,
    brightBlue: terminal.brightBlue,
    brightMagenta: terminal.brightMagenta,
    brightCyan: terminal.brightCyan,
    brightWhite: terminal.brightWhite,
  };
}

export interface SearchDecorationColors {
  matchBackground: string;
  matchBorder: string;
  matchOverviewRuler: string;
  activeMatchBackground: string;
  activeMatchBorder: string;
  activeMatchColorOverviewRuler: string;
}

export function searchDecorationColors(theme: boolean | AppliedTheme): SearchDecorationColors {
  const terminal = applied(theme).terminal;
  return {
    matchBackground: terminal.searchMatchBackground,
    matchBorder: terminal.searchMatchBorder,
    matchOverviewRuler: terminal.searchMatchOverviewRuler,
    activeMatchBackground: terminal.searchActiveMatchBackground,
    activeMatchBorder: terminal.searchActiveMatchBorder,
    activeMatchColorOverviewRuler: terminal.searchActiveMatchOverviewRuler,
  };
}

export function ansiColorLabel(name: AnsiColorName): string {
  const spaced = name.replace(/([A-Z])/g, " $1").toLowerCase();
  return spaced[0].toUpperCase() + spaced.slice(1);
}
