import { createContext, use } from "react";
import type {
  AppliedTheme,
  Appearance,
  Theme,
  ThemeAppearance,
  ThemeDefinition,
  ThemeFile,
  ThemeSelection,
} from "@/domain";
import { DEFAULT_APPEARANCE, resolveDark, systemPrefersDark } from "@/lib/appearance";
import { BUILT_IN_THEMES } from "@/theme/catalog";
import { defaultAppliedTheme } from "@/theme/runtime";

export type ThemeSelectionTarget = ThemeAppearance | "both";
export type ThemeImportConflictPolicy = "error" | "replace" | "keep_both";

// The live appearance read model: the document, the dark/light it resolves to (theme against
// the OS preference), and the auto-saving setter. Read at the leaves that restyle — the
// Appearance panel and the terminal — so it travels by context. The default is the documented
// defaults so a component rendered without the provider (a focused test) still works.
export interface AppearanceState {
  appearance: Appearance;
  dark: boolean;
  setAppearance: (next: Appearance) => void;
  resolvedAppearance: ThemeAppearance;
  selectedThemes: ThemeSelection;
  builtInThemes: ThemeDefinition[];
  customThemes: ThemeDefinition[];
  themes: ThemeDefinition[];
  appliedTheme: AppliedTheme;
  glassOpacity: number;
  themeDraft: ThemeFile | null;
  setAppearanceMode: (mode: Theme) => Promise<void>;
  selectTheme: (themeId: string, target?: ThemeSelectionTarget) => Promise<void>;
  setGlassOpacity: (opacity: number) => Promise<void>;
  resetGlassOpacity: () => Promise<void>;
  createCustomTheme: (theme: ThemeFile) => Promise<ThemeFile>;
  updateCustomTheme: (theme: ThemeFile) => Promise<ThemeFile>;
  removeCustomTheme: (themeId: string) => Promise<void>;
  duplicateTheme: (themeId: string) => Promise<ThemeFile>;
  importThemeJson: (json: string, conflict?: ThemeImportConflictPolicy) => Promise<ThemeFile>;
  serializeTheme: (themeId: string) => string;
  beginThemeDraft: (theme: ThemeFile) => void;
  updateThemeDraft: (theme: ThemeFile) => void;
  cancelThemeDraft: () => void;
  commitThemeDraft: (theme?: ThemeFile) => Promise<ThemeFile>;
}

const defaultDark = resolveDark(DEFAULT_APPEARANCE.theme, systemPrefersDark());
const defaultApplied = defaultAppliedTheme(defaultDark);

const DEFAULT_STATE: AppearanceState = {
  appearance: DEFAULT_APPEARANCE,
  dark: defaultDark,
  setAppearance: () => {},
  resolvedAppearance: defaultApplied.appearance,
  selectedThemes: DEFAULT_APPEARANCE.selected_themes,
  builtInThemes: BUILT_IN_THEMES,
  customThemes: [],
  themes: BUILT_IN_THEMES,
  appliedTheme: defaultApplied,
  glassOpacity: DEFAULT_APPEARANCE.glass_opacity,
  themeDraft: null,
  setAppearanceMode: async () => {},
  selectTheme: async () => {},
  setGlassOpacity: async () => {},
  resetGlassOpacity: async () => {},
  createCustomTheme: async (theme) => theme,
  updateCustomTheme: async (theme) => theme,
  removeCustomTheme: async () => {},
  duplicateTheme: async () => {
    throw new Error("AppearanceProvider is not mounted");
  },
  importThemeJson: async () => {
    throw new Error("AppearanceProvider is not mounted");
  },
  serializeTheme: () => "",
  beginThemeDraft: () => {},
  updateThemeDraft: () => {},
  cancelThemeDraft: () => {},
  commitThemeDraft: async (theme) => {
    if (!theme) throw new Error("No theme draft is active");
    return theme;
  },
};

// Focused component tests historically provide only the legacy three-field value. Keep the raw
// context partial and complete it at the hook boundary so those consumers stay lightweight while
// application code always receives the full runtime contract.
export const AppearanceContext = createContext<Partial<AppearanceState>>({});

/** The current appearance, the resolved dark/light, and the auto-saving setter. */
export function useAppearance(): AppearanceState {
  return { ...DEFAULT_STATE, ...use(AppearanceContext) };
}
