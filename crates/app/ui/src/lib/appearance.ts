// The single place each closed Appearance enum is mapped to its concrete CSS / xterm value,
// and where the ordered option set + display label for every picker lives. The core stores
// only the closed enum; the frontend owns the concrete granularity (per DESIGN.md and the
// settings behavior contract), so the app tokens and the xterm.js renderer read the same
// values from here — no magic numbers scattered across components.

import type { Appearance, FontScale, FontWeight, LetterSpacing, LineHeight, Theme } from "@/domain";

// xterm's ITheme is structural; we only set the few colors that follow the app surface, so a
// minimal shape keeps the dependency off the emulator's full type.
export interface TerminalColors {
  background: string;
  foreground: string;
  cursor: string;
  cursorAccent: string;
  selectionBackground: string;
}

// Terminal font size (px) per step — xterm takes a px size directly.
const TERMINAL_FONT_PX: Record<FontScale, number> = {
  extra_small: 11,
  small: 12,
  medium: 13,
  large: 15,
  extra_large: 17,
};

// Interface scale factor per step — applied to the document root font size so the whole
// rem-based UI scales together (the terminal has its own size, above).
const INTERFACE_SCALE: Record<FontScale, number> = {
  extra_small: 0.875,
  small: 0.9375,
  medium: 1,
  large: 1.0625,
  extra_large: 1.125,
};

// The CSS / xterm numeric weight steps. A literal union (not a bare `number`) so the value is
// assignable to xterm's `fontWeight` option, whose type is the same closed set.
export type TerminalFontWeight = 100 | 200 | 300 | 400 | 500 | 600 | 700 | 800 | 900;

// A font weight maps to its CSS numeric weight.
const FONT_WEIGHT_VALUE: Record<FontWeight, TerminalFontWeight> = {
  w100: 100,
  w200: 200,
  w300: 300,
  w400: 400,
  w500: 500,
  w600: 600,
  w700: 700,
  w800: 800,
  w900: 900,
};

// Terminal line height (unitless multiple) — the Solo control ranges ~1.0–1.8, default ~1.1.
const LINE_HEIGHT_VALUE: Record<LineHeight, number> = {
  compact: 1,
  default: 1.1,
  comfortable: 1.4,
  spacious: 1.8,
};

// Terminal letter spacing (px) — the Solo control ranges ~0.5–1.3, default ~0.9.
const LETTER_SPACING_PX: Record<LetterSpacing, number> = {
  tight: 0.5,
  default: 0.9,
  wide: 1.1,
  wider: 1.3,
};

// The bundled monospace stack the terminal falls back to when no family is chosen.
const DEFAULT_MONO_STACK = '"Geist Mono Variable", ui-monospace, monospace';

const ROOT_FONT_PX = 16;

// The frontend's initial appearance, mirroring the core document defaults — the value held
// until the persisted record loads (which then supersedes it), and the fallback when a
// consumer renders without the provider (a focused test). Kept here with the other appearance
// concretions so there is one place the frontend's appearance constants live.
export const DEFAULT_APPEARANCE: Appearance = {
  theme: "system",
  interface_font_scale: "medium",
  terminal: {
    focus_on_click: false,
    font_family: null,
    font_weight: "w400",
    bold_font_weight: "w600",
    font_scale: "medium",
    line_height: "default",
    letter_spacing: "default",
  },
};

export function terminalFontPx(scale: FontScale): number {
  return TERMINAL_FONT_PX[scale];
}

export function interfaceRootFontPx(scale: FontScale): number {
  return ROOT_FONT_PX * INTERFACE_SCALE[scale];
}

export function fontWeightValue(weight: FontWeight): TerminalFontWeight {
  return FONT_WEIGHT_VALUE[weight];
}

export function lineHeightValue(height: LineHeight): number {
  return LINE_HEIGHT_VALUE[height];
}

export function letterSpacingPx(spacing: LetterSpacing): number {
  return LETTER_SPACING_PX[spacing];
}

// The CSS font-family stack for the terminal: the chosen family ahead of the bundled
// fallback, or the bundled stack alone when none is chosen.
export function terminalFontFamily(family: string | null): string {
  return family ? `"${family}", ${DEFAULT_MONO_STACK}` : DEFAULT_MONO_STACK;
}

// The webview-local synchronous mirror of the chosen theme. The persisted appearance loads
// async over IPC, so an explicit Light/Dark choice that differs from the OS preference would
// flash the OS theme on cold start; this hint — the one store readable synchronously before
// React mounts — lets the first paint be correct. The core stays authoritative: the hint is
// written through on every load/save and superseded by the loaded record.
const THEME_HINT_KEY = "soloist.theme-hint";

// The last chosen theme from the webview-local hint, or null when absent/unreadable (a headless
// test host has no localStorage). Never throws — a miss just means the OS preference is used.
export function readThemeHint(): Theme | null {
  try {
    const value = window.localStorage.getItem(THEME_HINT_KEY);
    return value === "light" || value === "dark" || value === "system" ? value : null;
  } catch {
    return null;
  }
}

// Persist the chosen theme for the next cold start's pre-paint. Never throws — a failed write
// just leaves the next start to fall back to the OS preference until the record loads.
export function writeThemeHint(theme: Theme): void {
  try {
    window.localStorage.setItem(THEME_HINT_KEY, theme);
  } catch {
    // Storage unavailable (a headless test host); the IPC-loaded record stays the source of truth.
  }
}

// The single place the `.dark` class is written on the document root — shared by the pre-paint
// hint (main entry) and the live provider, so there is one theme-application path.
export function applyDarkClass(dark: boolean): void {
  document.documentElement.classList.toggle("dark", dark);
}

// Resolve the effective dark/light from the theme choice and the OS preference (the latter
// only decides when the choice is "system").
export function resolveDark(theme: Theme, systemPrefersDark: boolean): boolean {
  switch (theme) {
    case "light":
      return false;
    case "dark":
      return true;
    case "system":
      return systemPrefersDark;
  }
}

// Read the OS dark-mode preference. Guarded because non-browser hosts (jsdom under test) have
// no `matchMedia`; there the preference is reported as light.
export function systemPrefersDark(): boolean {
  return typeof window !== "undefined" && typeof window.matchMedia === "function"
    ? window.matchMedia("(prefers-color-scheme: dark)").matches
    : false;
}

// Subscribe to OS light/dark changes (so a "System" theme follows them live); returns an
// unsubscribe. A no-op where `matchMedia` is unavailable.
export function watchSystemDark(onChange: (dark: boolean) => void): () => void {
  if (typeof window === "undefined" || typeof window.matchMedia !== "function") {
    return () => {};
  }
  const media = window.matchMedia("(prefers-color-scheme: dark)");
  const handler = (event: MediaQueryListEvent) => onChange(event.matches);
  media.addEventListener("change", handler);
  return () => media.removeEventListener("change", handler);
}

// The terminal's own surface palette, tracking the app light/dark theme. Program output keeps
// its own ANSI; only the chrome (background/foreground/cursor/selection) follows the theme.
// This is the one place the terminal chrome colors live — a surface distinct from the app
// `--background` tokens (DESIGN.md), kept as concrete hex because xterm.js cannot parse the
// OKLCH design tokens. The cursor's contrast color is always the surface behind it, so it is
// derived from the background rather than restated.
export function terminalColors(dark: boolean): TerminalColors {
  const surface = dark
    ? {
        background: "#1b1e25",
        foreground: "#e6e8ec",
        cursor: "#8ab4f8",
        selectionBackground: "#33405a",
      }
    : {
        background: "#fbfbfd",
        foreground: "#23262c",
        cursor: "#3b6fd4",
        selectionBackground: "#cfdcf5",
      };
  return { ...surface, cursorAccent: surface.background };
}

// The xterm.js options derived from the appearance document — applied at creation and pushed
// live on every change. `dark` is resolved separately (it depends on the OS preference).
export function terminalOptions(appearance: Appearance, dark: boolean) {
  const t = appearance.terminal;
  return {
    fontFamily: terminalFontFamily(t.font_family),
    fontSize: terminalFontPx(t.font_scale),
    fontWeight: fontWeightValue(t.font_weight),
    fontWeightBold: fontWeightValue(t.bold_font_weight),
    lineHeight: lineHeightValue(t.line_height),
    letterSpacing: letterSpacingPx(t.letter_spacing),
    theme: terminalColors(dark),
  };
}

// ── Picker option sets (ordered, labeled — one source for what each control offers) ─────────

export interface Option<T> {
  value: T;
  label: string;
}

export const THEME_OPTIONS: Option<Theme>[] = [
  { value: "light", label: "Light" },
  { value: "dark", label: "Dark" },
  { value: "system", label: "System" },
];

// Smallest → largest, for the A·A·A size steppers (interface and terminal).
export const FONT_SCALE_ORDER: FontScale[] = [
  "extra_small",
  "small",
  "medium",
  "large",
  "extra_large",
];

export const FONT_SCALE_LABEL: Record<FontScale, string> = {
  extra_small: "Extra small",
  small: "Small",
  medium: "Medium",
  large: "Large",
  extra_large: "Extra large",
};

export const FONT_WEIGHT_OPTIONS: Option<FontWeight>[] = (
  Object.keys(FONT_WEIGHT_VALUE) as FontWeight[]
).map((value) => ({ value, label: String(FONT_WEIGHT_VALUE[value]) }));

export const LINE_HEIGHT_OPTIONS: Option<LineHeight>[] = [
  { value: "compact", label: "Compact" },
  { value: "default", label: "Default" },
  { value: "comfortable", label: "Comfortable" },
  { value: "spacious", label: "Spacious" },
];

export const LETTER_SPACING_OPTIONS: Option<LetterSpacing>[] = [
  { value: "tight", label: "Tight" },
  { value: "default", label: "Default" },
  { value: "wide", label: "Wide" },
  { value: "wider", label: "Wider" },
];

// A curated set of common Linux monospace families; an uninstalled family falls back through
// the stack. `null` keeps the app's bundled default. (Probing actually-installed fonts is a
// later, separate concern; the core only stores the chosen name.)
export const MONO_FONT_OPTIONS: Option<string | null>[] = [
  { value: null, label: "System default" },
  { value: "JetBrains Mono", label: "JetBrains Mono" },
  { value: "Fira Code", label: "Fira Code" },
  { value: "Source Code Pro", label: "Source Code Pro" },
  { value: "Ubuntu Mono", label: "Ubuntu Mono" },
  { value: "DejaVu Sans Mono", label: "DejaVu Sans Mono" },
  { value: "Hack", label: "Hack" },
];
