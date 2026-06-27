import { useCallback, useEffect, useState, type ReactNode } from "react";
import { appearance as readAppearance, setAppearance as writeAppearance } from "@/api";
import {
  applyDarkClass,
  DEFAULT_APPEARANCE,
  interfaceRootFontPx,
  readThemeHint,
  resolveDark,
  systemPrefersDark,
  watchSystemDark,
  writeThemeHint,
} from "@/lib/appearance";
import { AppearanceContext } from "@/store/appearanceContext";
import { useLoadOnce } from "@/store/useLoadOnce";
import type { Appearance } from "@/domain";

// Loads the persisted appearance once, tracks the OS light/dark preference, and applies the
// resolved theme + interface scale to the document root — so the whole app (and the terminal,
// which reads the same document) restyles together, immediately and after restart. Mounted at
// the app root so the theme is always applied, not only while the Settings panel is open. This
// is the sole runtime authority for the `.dark` class; the entry point only seeds the first
// paint from the theme hint before this mounts.
export function AppearanceProvider({ children }: { children: ReactNode }) {
  // Seed the theme from the same webview-local hint the pre-paint used, so the provider's first
  // render matches what is already on screen (no flash) until the persisted record loads.
  const [appearance, setAppearance] = useState<Appearance>(() => ({
    ...DEFAULT_APPEARANCE,
    theme: readThemeHint() ?? DEFAULT_APPEARANCE.theme,
  }));
  const [systemDark, setSystemDark] = useState(systemPrefersDark);

  // The facade returns the documented defaults when nothing is stored, so this resolves to the
  // authoritative starting values (superseding the seeded theme) and refreshes the hint.
  useLoadOnce(readAppearance, (loaded) => {
    setAppearance(loaded);
    writeThemeHint(loaded.theme);
  });

  useEffect(() => watchSystemDark(setSystemDark), []);

  const dark = resolveDark(appearance.theme, systemDark);

  useEffect(() => {
    applyDarkClass(dark);
  }, [dark]);

  useEffect(() => {
    document.documentElement.style.fontSize = `${interfaceRootFontPx(appearance.interface_font_scale)}px`;
  }, [appearance.interface_font_scale]);

  // Optimistic update then persist; the facade auto-saves and echoes the stored value, which
  // reconciles the local state with what was written. If the persist fails, re-read the stored
  // record so the UI falls back to disk truth rather than silently diverging.
  const save = useCallback((next: Appearance) => {
    setAppearance(next);
    writeThemeHint(next.theme);
    void writeAppearance(next)
      .then((stored) => {
        setAppearance(stored);
        writeThemeHint(stored.theme);
      })
      .catch(() => {
        void readAppearance()
          .then((stored) => {
            setAppearance(stored);
            writeThemeHint(stored.theme);
          })
          .catch(() => {});
      });
  }, []);

  return (
    <AppearanceContext value={{ appearance, dark, setAppearance: save }}>
      {children}
    </AppearanceContext>
  );
}
