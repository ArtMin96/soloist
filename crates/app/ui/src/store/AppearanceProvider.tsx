import { useCallback, useEffect, useMemo, useState, type ReactNode } from "react";
import { appearance as readAppearance, setAppearance as writeAppearance } from "@/api";
import {
  applyDarkClass,
  applyInterfaceRootFont,
  DEFAULT_APPEARANCE,
  readInterfaceScaleHint,
  readThemeHint,
  resolveDark,
  systemPrefersDark,
  watchSystemDark,
  writeInterfaceScaleHint,
  writeThemeHint,
} from "@/lib/appearance";
import { AppearanceContext } from "@/store/appearanceContext";
import { persistThenReconcile } from "@/store/persist";
import { useLoadOnce } from "@/store/useLoadOnce";
import type { Appearance } from "@/domain";

// Loads the persisted appearance once, tracks the OS light/dark preference, and applies the
// resolved theme + interface scale to the document root — so the whole app (and the terminal,
// which reads the same document) restyles together, immediately and after restart. Mounted at
// the app root so the theme is always applied, not only while the Settings panel is open. This
// is the sole runtime authority for the `.dark` class; the entry point only seeds the first
// paint from the theme hint before this mounts.
export function AppearanceProvider({ children }: { children: ReactNode }) {
  // Seed the theme and interface scale from the same webview-local hints the pre-paint used, so the
  // provider's first render matches what is already on screen (no flash, no reflow) until the
  // persisted record loads.
  const [appearance, setAppearance] = useState<Appearance>(() => ({
    ...DEFAULT_APPEARANCE,
    theme: readThemeHint() ?? DEFAULT_APPEARANCE.theme,
    interface_font_scale: readInterfaceScaleHint() ?? DEFAULT_APPEARANCE.interface_font_scale,
  }));
  const [systemDark, setSystemDark] = useState(systemPrefersDark);

  // Adopt an appearance into local state and refresh the pre-paint hints, so the next cold start is
  // correct. The one place this provider applies a value, whether seeded, loaded, or saved.
  const adopt = useCallback((next: Appearance) => {
    setAppearance(next);
    writeThemeHint(next.theme);
    writeInterfaceScaleHint(next.interface_font_scale);
  }, []);

  // The facade returns the documented defaults when nothing is stored, so this resolves to the
  // authoritative starting values (superseding the seeded hints) and refreshes them.
  useLoadOnce(readAppearance, adopt);

  useEffect(() => watchSystemDark(setSystemDark), []);

  const dark = resolveDark(appearance.theme, systemDark);

  useEffect(() => {
    applyDarkClass(dark);
  }, [dark]);

  useEffect(() => {
    applyInterfaceRootFont(appearance.interface_font_scale);
  }, [appearance.interface_font_scale]);

  // Optimistic update then persist; the facade auto-saves and echoes the stored value, which
  // reconciles the local state with what was written. If the persist fails, fall back to the stored
  // record so the UI never silently diverges (the shared reconcile the other settings hooks use).
  const save = useCallback(
    (next: Appearance) => {
      adopt(next);
      persistThenReconcile(writeAppearance(next), readAppearance, adopt);
    },
    [adopt],
  );

  // Memoized so a provider re-render (theme, interface scale, or OS light/dark change) only
  // propagates to consumers when the value they read actually changes — otherwise the fresh
  // object identity would re-render every consumer, including each live terminal, on every toggle.
  const value = useMemo(
    () => ({ appearance, dark, setAppearance: save }),
    [appearance, dark, save],
  );

  return <AppearanceContext value={value}>{children}</AppearanceContext>;
}
