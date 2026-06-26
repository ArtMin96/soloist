import { useCallback, useEffect, useState, type ReactNode } from "react";
import { appearance as readAppearance, setAppearance as writeAppearance } from "@/api";
import {
  DEFAULT_APPEARANCE,
  interfaceRootFontPx,
  resolveDark,
  systemPrefersDark,
  watchSystemDark,
} from "@/lib/appearance";
import { AppearanceContext } from "@/store/appearanceContext";
import type { Appearance } from "@/domain";

// Loads the persisted appearance once, tracks the OS light/dark preference, and applies the
// resolved theme + interface scale to the document root — so the whole app (and the terminal,
// which reads the same document) restyles together, immediately and after restart. Mounted at
// the app root so the theme is always applied, not only while the Settings panel is open.
export function AppearanceProvider({ children }: { children: ReactNode }) {
  const [appearance, setAppearance] = useState<Appearance>(DEFAULT_APPEARANCE);
  const [systemDark, setSystemDark] = useState(systemPrefersDark);

  // The facade returns the documented defaults when nothing is stored, so this resolves to the
  // authoritative starting values (superseding the local default).
  useEffect(() => {
    let cancelled = false;
    readAppearance()
      .then((loaded) => {
        if (!cancelled) setAppearance(loaded);
      })
      .catch(() => {});
    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => watchSystemDark(setSystemDark), []);

  const dark = resolveDark(appearance.theme, systemDark);

  useEffect(() => {
    document.documentElement.classList.toggle("dark", dark);
  }, [dark]);

  useEffect(() => {
    document.documentElement.style.fontSize = `${interfaceRootFontPx(appearance.interface_font_scale)}px`;
  }, [appearance.interface_font_scale]);

  // Optimistic update then persist; the facade auto-saves and echoes the stored value, which
  // reconciles the local state with what was written.
  const save = useCallback((next: Appearance) => {
    setAppearance(next);
    void writeAppearance(next)
      .then(setAppearance)
      .catch(() => {});
  }, []);

  return (
    <AppearanceContext value={{ appearance, dark, setAppearance: save }}>
      {children}
    </AppearanceContext>
  );
}
