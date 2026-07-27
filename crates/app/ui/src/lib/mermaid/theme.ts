// The theme bridge: turns the app's live OKLCH design tokens into a Mermaid theme configuration so a
// diagram is drawn in the same palette as everything around it, in both light and dark. Mermaid's
// colour engine rejects `oklch()`, so every token is converted to sRGB (see `color`) before it is
// handed over.

import { toRgb } from "./color";
import { MERMAID_FONT_SIZE, MERMAID_THEME_TOKENS } from "./const";

export interface MermaidThemeConfig {
  theme: "base";
  themeVariables: Record<string, string | boolean>;
  fontFamily: string;
}

/** True when the app is in dark mode — the single signal the rest of the UI keys off (the `.dark`
 * class on the document root, toggled by `applyDarkClass`). */
export function isDarkTheme(): boolean {
  return document.documentElement.classList.contains("dark");
}

/**
 * A cheap value that changes exactly when the diagram palette would change, so a rendered diagram
 * knows to re-render. Light and dark are the only palettes (token values are static within a mode),
 * so the mode name is a sufficient signature.
 */
export function themeSignature(): string {
  return isDarkTheme() ? "dark" : "light";
}

/**
 * Build the Mermaid theme configuration from the app's current tokens. Called per render so a
 * light/dark flip is picked up without caching stale colours.
 *
 * `darkMode` rides inside `themeVariables` rather than at the config's top level because that is
 * where Mermaid reads it: a theme's variables are assigned onto the theme instance before it derives
 * the colours it was not given (scale fills, alternating row bands, surface tints), and every one of
 * those derivations branches on `darkMode`. At the top level it is simply ignored, and a dark diagram
 * comes out with light-mode derived colours.
 */
export function mermaidThemeConfig(): MermaidThemeConfig {
  const root = getComputedStyle(document.documentElement);
  const themeVariables: Record<string, string | boolean> = {
    fontSize: MERMAID_FONT_SIZE,
    darkMode: isDarkTheme(),
  };
  for (const [variable, token] of Object.entries(MERMAID_THEME_TOKENS)) {
    const resolved = toRgb(root.getPropertyValue(token).trim());
    if (resolved) themeVariables[variable] = resolved;
  }

  return {
    theme: "base",
    themeVariables,
    // Diagram text uses the app's UI font, not Mermaid's serif default, so labels match the surface.
    fontFamily: getComputedStyle(document.body).fontFamily || "system-ui, sans-serif",
  };
}
