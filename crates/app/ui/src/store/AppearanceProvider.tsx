import { useCallback, useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { appearance as readAppearance, setAppearance as writeAppearance } from "@/api";
import type { Appearance } from "@/domain";
import {
  applyInterfaceRootFont,
  DEFAULT_APPEARANCE,
  readInterfaceScaleHint,
  readThemeHint,
  systemPrefersDark,
  watchSystemDark,
  writeInterfaceScaleHint,
  writeThemeHint,
} from "@/lib/appearance";
import { AppearanceContext } from "@/store/appearanceContext";
import {
  createAppearanceMutationQueue,
  type AppearanceMutationTarget,
} from "@/store/appearanceMutationQueue";
import { useLoadOnce } from "@/store/useLoadOnce";
import { useThemeLibrary } from "@/store/useThemeLibrary";
import { BUILT_IN_THEMES, themeDefinitions } from "@/theme/catalog";
import { GLASS_OPACITY } from "@/theme/constraints";
import {
  appliedThemeFromFile,
  applyTheme,
  readAppliedThemeHint,
  resolveAppliedTheme,
  writeAppliedThemeHint,
} from "@/theme/runtime";

function normalizedAppearance(value: Appearance): Appearance {
  return {
    ...DEFAULT_APPEARANCE,
    ...value,
    selected_themes: value.selected_themes ?? DEFAULT_APPEARANCE.selected_themes,
    custom_themes: value.custom_themes ?? [],
    glass_opacity: value.glass_opacity ?? GLASS_OPACITY.default,
    terminal: { ...DEFAULT_APPEARANCE.terminal, ...value.terminal },
  };
}

type RunThemeCommand = (
  command: () => Promise<Appearance>,
  target?: AppearanceMutationTarget,
) => Promise<Appearance>;

function appearanceProjection(current: Appearance, next: Appearance) {
  const patch: Partial<Appearance> = {};
  if (current.theme !== next.theme) patch.theme = next.theme;
  if (current.selected_themes !== next.selected_themes)
    patch.selected_themes = next.selected_themes;
  if (current.custom_themes !== next.custom_themes) patch.custom_themes = next.custom_themes;
  if (current.glass_opacity !== next.glass_opacity) patch.glass_opacity = next.glass_opacity;
  if (current.interface_font_scale !== next.interface_font_scale) {
    patch.interface_font_scale = next.interface_font_scale;
  }
  if (current.terminal !== next.terminal) patch.terminal = next.terminal;
  return (latest: Appearance): Appearance => normalizedAppearance({ ...latest, ...patch });
}

// Loads the durable Appearance document, resolves its selected palette against the OS, and applies
// the complete theme atomically. Library mutations live in useThemeLibrary; this component owns
// only persistence ordering, prepaint reconciliation, and the live application boundary.
export function AppearanceProvider({ children }: { children: ReactNode }) {
  const [appearance, setAppearanceState] = useState<Appearance>(() => ({
    ...DEFAULT_APPEARANCE,
    theme: readThemeHint() ?? DEFAULT_APPEARANCE.theme,
    interface_font_scale: readInterfaceScaleHint() ?? DEFAULT_APPEARANCE.interface_font_scale,
  }));
  const [systemDark, setSystemDark] = useState(systemPrefersDark);
  const [loaded, setLoaded] = useState(false);
  const [prepaintHint] = useState(readAppliedThemeHint);
  const appearanceRef = useRef(appearance);

  const adopt = useCallback((value: Appearance) => {
    const next = normalizedAppearance(value);
    appearanceRef.current = next;
    setAppearanceState(next);
    writeThemeHint(next.theme);
    writeInterfaceScaleHint(next.interface_font_scale);
  }, []);

  // `current` never runs during render: the queue only calls it from `update`/`task`, which this file
  // calls solely from event-driven callbacks, never from render. No effect-deferred alternative works
  // here — the queue is stateful (pending mutations live in its closure) and the hooks below need it
  // synchronously on the first render, so it must exist before any effect could construct it.
  // eslint-disable-next-line react-hooks/refs -- see above
  const [mutationQueue] = useState(() =>
    createAppearanceMutationQueue({
      write: writeAppearance,
      read: readAppearance,
      current: () => appearanceRef.current,
      adopt,
    }),
  );

  useLoadOnce(readAppearance, (next) => {
    adopt(next);
    setLoaded(true);
  });
  useEffect(() => watchSystemDark(setSystemDark), []);

  const allThemes = useMemo(
    () => themeDefinitions(appearance.custom_themes),
    [appearance.custom_themes],
  );
  const customThemes = useMemo(
    () => allThemes.filter(({ source }) => source === "custom"),
    [allThemes],
  );
  const resolved = useMemo(
    () => resolveAppliedTheme(appearance, allThemes, systemDark),
    [allThemes, appearance, systemDark],
  );

  const saveAsync = useCallback(
    async (next: Appearance) => {
      const normalized = normalizedAppearance(next);
      await mutationQueue.update(appearanceProjection(appearance, normalized));
    },
    [appearance, mutationQueue],
  );
  const save = useCallback((next: Appearance) => void saveAsync(next).catch(() => {}), [saveAsync]);
  const updateAppearance = useCallback(
    (project: (current: Appearance) => Appearance) => saveAsync(project(appearanceRef.current)),
    [saveAsync],
  );
  const runThemeCommand = useCallback<RunThemeCommand>(
    (command, target) => mutationQueue.task(command, target),
    [mutationQueue],
  );
  const library = useThemeLibrary(appearanceRef, updateAppearance, runThemeCommand);

  const draft = library.themeDraft;
  const draftApplied = useMemo(
    () => (draft ? appliedThemeFromFile(draft, draft.appearance, appearance.glass_opacity) : null),
    [appearance.glass_opacity, draft],
  );
  const appliedTheme = draftApplied ?? (!loaded && prepaintHint ? prepaintHint : resolved);
  const dark = appliedTheme.appearance === "dark";

  useEffect(() => {
    applyTheme(appliedTheme);
    writeAppliedThemeHint(appliedTheme);
  }, [appliedTheme]);
  useEffect(() => {
    applyInterfaceRootFont(appearance.interface_font_scale);
  }, [appearance.interface_font_scale]);

  const value = useMemo(
    () => ({
      appearance,
      dark,
      setAppearance: save,
      resolvedAppearance: appliedTheme.appearance,
      selectedThemes: appearance.selected_themes,
      builtInThemes: BUILT_IN_THEMES,
      customThemes,
      themes: allThemes,
      appliedTheme,
      glassOpacity: appearance.glass_opacity,
      ...library,
    }),
    [allThemes, appearance, appliedTheme, customThemes, dark, library, save],
  );

  return <AppearanceContext value={value}>{children}</AppearanceContext>;
}
