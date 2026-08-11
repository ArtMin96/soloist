import type { ThemeDefinition, ThemeFile } from "@/domain";
import catalog from "../../../../../themes/builtins/catalog.json";

const BUILT_IN_FILES = catalog as ThemeFile[];

export const DEFAULT_THEME_ID = "soloist-default";

export const BUILT_IN_THEMES: ThemeDefinition[] = BUILT_IN_FILES.map((theme) => ({
  ...theme,
  source: "built_in",
}));

export function themeDefinitions(customThemes: ThemeFile[]): ThemeDefinition[] {
  return [
    ...BUILT_IN_THEMES,
    ...customThemes.map((theme) => ({ ...theme, source: "custom" as const })),
  ];
}
