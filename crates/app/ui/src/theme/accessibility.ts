import {
  THEME_APPEARANCES,
  type ThemeAppearance,
  type ThemeColors,
  type ThemeFile,
} from "@/domain";
import { contrastRatio, themeColorsForAppearance, themeSupportsAppearance } from "@/theme/derive";

interface ContrastPair {
  foreground: keyof ThemeColors;
  background: keyof ThemeColors;
  label: string;
  minimum: number;
}

const CONTRAST_PAIRS: ContrastPair[] = [
  { foreground: "text", background: "canvas", label: "Body text", minimum: 4.5 },
  { foreground: "textMuted", background: "canvas", label: "Muted text", minimum: 4.5 },
  {
    foreground: "accentForeground",
    background: "accent",
    label: "Accent controls",
    minimum: 4.5,
  },
  {
    foreground: "codeForeground",
    background: "codeBackground",
    label: "Code text",
    minimum: 4.5,
  },
  {
    foreground: "sidebarForeground",
    background: "sidebar",
    label: "Sidebar text",
    minimum: 4.5,
  },
  {
    foreground: "errorForeground",
    background: "errorSurface",
    label: "Error messages",
    minimum: 4.5,
  },
  {
    foreground: "warningForeground",
    background: "warningSurface",
    label: "Warning messages",
    minimum: 4.5,
  },
];

export interface ThemeContrastWarning {
  label: string;
  foreground: keyof ThemeColors;
  background: keyof ThemeColors;
  ratio: number;
  minimum: number;
}

export function themeContrastWarnings(colors: ThemeColors): ThemeContrastWarning[] {
  return CONTRAST_PAIRS.flatMap((pair) => {
    const ratio = contrastRatio(colors[pair.foreground], colors[pair.background]);
    return ratio < pair.minimum ? [{ ...pair, ratio }] : [];
  });
}

export interface ThemePaletteContrastWarnings {
  id: string;
  appearance: ThemeAppearance;
  warnings: ThemeContrastWarning[];
}

/**
 * The same pairs the editor shows, applied to every palette a set of themes publishes. Themes that
 * clear every pair are absent, so an empty result means the whole set is clean.
 */
export function paletteContrastWarnings(
  themes: readonly ThemeFile[],
): ThemePaletteContrastWarnings[] {
  return themes.flatMap((theme) => {
    const published = THEME_APPEARANCES.filter((appearance) =>
      themeSupportsAppearance(theme, appearance),
    );
    return published.flatMap((appearance) => {
      const colors = themeColorsForAppearance(theme, appearance);
      if (!colors) return [];
      const warnings = themeContrastWarnings(colors);
      return warnings.length > 0 ? [{ id: theme.id, appearance, warnings }] : [];
    });
  });
}
