import type { Appearance } from "@/domain";
import type { AppearanceState } from "@/store/appearanceContext";
import { BUILT_IN_THEMES, themeDefinitions } from "@/theme/catalog";
import { resolveAppliedTheme } from "@/theme/runtime";

export function fakeAppearanceState(appearance: Appearance, dark: boolean): AppearanceState {
  const themes = themeDefinitions(appearance.custom_themes);
  return {
    appearance,
    dark,
    setAppearance: () => {},
    resolvedAppearance: dark ? "dark" : "light",
    selectedThemes: appearance.selected_themes,
    builtInThemes: BUILT_IN_THEMES,
    customThemes: themes.filter(({ source }) => source === "custom"),
    themes,
    appliedTheme: resolveAppliedTheme(appearance, BUILT_IN_THEMES, dark),
    glassOpacity: appearance.glass_opacity,
    themeDraft: null,
    setAppearanceMode: async () => {},
    selectTheme: async () => {},
    setGlassOpacity: async () => {},
    resetGlassOpacity: async () => {},
    createCustomTheme: async (theme) => theme,
    updateCustomTheme: async (theme) => theme,
    removeCustomTheme: async () => {},
    duplicateTheme: async () => {
      throw new Error("No theme was duplicated in this test");
    },
    importThemeJson: async () => {
      throw new Error("No theme was imported in this test");
    },
    serializeTheme: () => "",
    beginThemeDraft: () => {},
    updateThemeDraft: () => {},
    cancelThemeDraft: () => {},
    commitThemeDraft: async (theme) => {
      if (!theme) throw new Error("No draft");
      return theme;
    },
  };
}
