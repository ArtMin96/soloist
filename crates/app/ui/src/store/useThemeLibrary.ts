import { useCallback, useMemo, useState } from "react";
import {
  createTheme,
  duplicateTheme as duplicateStoredTheme,
  importTheme,
  inspectTheme,
  removeTheme,
  selectTheme as selectStoredTheme,
  setGlassOpacity as storeGlassOpacity,
  updateTheme,
} from "@/api";
import type { Appearance, Theme, ThemeDefinition, ThemeFile } from "@/domain";
import type { ThemeImportConflictPolicy, ThemeSelectionTarget } from "@/store/appearanceContext";
import {
  APPEARANCE_MUTATION_TARGET,
  type AppearanceMutationTarget,
} from "@/store/appearanceMutationQueue";
import { GLASS_OPACITY } from "@/theme/constraints";
import { themeDefinitions } from "@/theme/catalog";
import { themeSupportsAppearance } from "@/theme/derive";
import {
  serializeNewTheme,
  serializeTheme as serializeThemeFile,
  THEME_DRAFT_ID,
  ThemeImportConflictError,
} from "@/theme/io";

type AppearanceRef = { current: Appearance };
type UpdateAppearance = (project: (current: Appearance) => Appearance) => Promise<void>;
type RunCommand = (
  command: () => Promise<Appearance>,
  target?: AppearanceMutationTarget,
) => Promise<Appearance>;

function plainTheme(theme: ThemeFile | ThemeDefinition): ThemeFile {
  const file = { ...theme } as Partial<ThemeDefinition>;
  delete file.source;
  return file as ThemeFile;
}

function installedTheme(appearance: Appearance, themeId: string): ThemeFile {
  const theme = appearance.custom_themes.find(({ id }) => id === themeId);
  if (!theme) throw new Error(`The stored theme ${JSON.stringify(themeId)} was not returned`);
  return theme;
}

// Owns UI composition around the core's task-shaped theme commands. The Rust settings facade is
// authoritative for every library mutation and invariant; this hook only translates the panel's
// ergonomic targets, maintains the unsaved draft, and adopts each persisted Appearance response.
export function useThemeLibrary(
  appearanceRef: AppearanceRef,
  updateAppearance: UpdateAppearance,
  runCommand: RunCommand,
) {
  const [themeDraft, setThemeDraft] = useState<ThemeFile | null>(null);

  const setAppearanceMode = useCallback(
    (mode: Theme) => updateAppearance((current) => ({ ...current, theme: mode })),
    [updateAppearance],
  );

  const selectTheme = useCallback(
    async (themeId: string, target?: ThemeSelectionTarget) => {
      const definition = themeDefinitions(appearanceRef.current.custom_themes).find(
        ({ id }) => id === themeId,
      );
      if (!definition) throw new Error(`Theme ${JSON.stringify(themeId)} was not found`);
      const requested = target ?? definition.appearance;
      for (const appearance of ["light", "dark"] as const) {
        if (
          (requested === "both" || requested === appearance) &&
          themeSupportsAppearance(definition, appearance)
        ) {
          await runCommand(() => selectStoredTheme(appearance, themeId));
        }
      }
    },
    [appearanceRef, runCommand],
  );

  const setGlassOpacity = useCallback(
    async (opacity: number) => {
      await runCommand(() => storeGlassOpacity(opacity), APPEARANCE_MUTATION_TARGET.glassOpacity);
    },
    [runCommand],
  );
  const resetGlassOpacity = useCallback(
    () => setGlassOpacity(GLASS_OPACITY.default),
    [setGlassOpacity],
  );

  const createCustomTheme = useCallback(
    async (theme: ThemeFile) => {
      const file = await inspectTheme(
        theme.id === THEME_DRAFT_ID
          ? serializeNewTheme(plainTheme(theme))
          : serializeThemeFile(plainTheme(theme)),
      );
      const stored = await runCommand(() => createTheme(file));
      return installedTheme(stored, file.id);
    },
    [runCommand],
  );

  const updateCustomTheme = useCallback(
    async (theme: ThemeFile) => {
      const file = plainTheme(theme);
      const stored = await runCommand(() => updateTheme(file));
      return installedTheme(stored, file.id);
    },
    [runCommand],
  );

  const removeCustomTheme = useCallback(
    async (themeId: string) => {
      await runCommand(() => removeTheme(themeId));
    },
    [runCommand],
  );

  const duplicateTheme = useCallback(
    async (themeId: string) => {
      const before = new Set(appearanceRef.current.custom_themes.map(({ id }) => id));
      const stored = await runCommand(() => duplicateStoredTheme(themeId));
      const duplicate = stored.custom_themes.find(({ id }) => !before.has(id));
      if (!duplicate) throw new Error("The duplicated theme was not returned");
      return duplicate;
    },
    [appearanceRef, runCommand],
  );

  const importThemeJson = useCallback(
    async (json: string, conflict: ThemeImportConflictPolicy = "error") => {
      const incoming = await inspectTheme(json);
      const before = new Set(appearanceRef.current.custom_themes.map(({ id }) => id));
      let stored: Appearance;
      try {
        stored = await runCommand(() =>
          importTheme(json, conflict === "error" ? "reject" : conflict),
        );
      } catch (error) {
        // Core's reject policy decides the conflict; the library is consulted only to name the
        // clashing theme for the dialog that offers the explicit resolutions.
        const existing = themeDefinitions(appearanceRef.current.custom_themes).find(
          ({ id }) => id === incoming.id,
        );
        if (conflict === "error" && existing)
          throw new ThemeImportConflictError(existing, incoming);
        throw error;
      }
      const installed =
        conflict === "keep_both"
          ? stored.custom_themes.find(({ id }) => !before.has(id))
          : stored.custom_themes.find(({ id }) => id === incoming.id);
      if (!installed) throw new Error("The imported theme was not returned");
      return installed;
    },
    [appearanceRef, runCommand],
  );

  const serializeTheme = useCallback(
    (themeId: string) => {
      const theme = themeDefinitions(appearanceRef.current.custom_themes).find(
        ({ id }) => id === themeId,
      );
      if (!theme) throw new Error(`Theme ${JSON.stringify(themeId)} was not found`);
      return serializeThemeFile(plainTheme(theme));
    },
    [appearanceRef],
  );

  const beginThemeDraft = useCallback((theme: ThemeFile) => setThemeDraft(plainTheme(theme)), []);
  const updateThemeDraft = useCallback((theme: ThemeFile) => setThemeDraft(plainTheme(theme)), []);
  const cancelThemeDraft = useCallback(() => setThemeDraft(null), []);
  const commitThemeDraft = useCallback(
    async (provided?: ThemeFile) => {
      const draft = provided ? plainTheme(provided) : themeDraft;
      if (!draft) throw new Error("No theme draft is active");
      const saved = appearanceRef.current.custom_themes.some(({ id }) => id === draft.id)
        ? await updateCustomTheme(draft)
        : await createCustomTheme(draft);
      setThemeDraft(null);
      return saved;
    },
    [appearanceRef, createCustomTheme, themeDraft, updateCustomTheme],
  );

  return useMemo(
    () => ({
      themeDraft,
      setAppearanceMode,
      selectTheme,
      setGlassOpacity,
      resetGlassOpacity,
      createCustomTheme,
      updateCustomTheme,
      removeCustomTheme,
      duplicateTheme,
      importThemeJson,
      serializeTheme,
      beginThemeDraft,
      updateThemeDraft,
      cancelThemeDraft,
      commitThemeDraft,
    }),
    [
      beginThemeDraft,
      cancelThemeDraft,
      commitThemeDraft,
      createCustomTheme,
      duplicateTheme,
      importThemeJson,
      removeCustomTheme,
      resetGlassOpacity,
      selectTheme,
      serializeTheme,
      setAppearanceMode,
      setGlassOpacity,
      themeDraft,
      updateCustomTheme,
      updateThemeDraft,
    ],
  );
}
