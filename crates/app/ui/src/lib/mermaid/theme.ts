// The theme bridge: turns the app's live OKLCH design tokens into a Mermaid theme configuration so a
// diagram is drawn in the same palette as everything around it, in both light and dark. Mermaid's
// color engine cannot parse `oklch()`, so every token is resolved to a computed rgb string through a
// probe element before it is handed over.

import { MERMAID_FONT_SIZE, MERMAID_THEME_TOKENS } from "./const";

export interface MermaidThemeConfig {
  theme: "base";
  darkMode: boolean;
  themeVariables: Record<string, string>;
  fontFamily: string;
  fontSize: string;
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

/** Resolve a batch of raw CSS color values (possibly `oklch(...)`) to rgb strings via one shared probe
 * element. In a real webview `getComputedStyle().color` returns rgb; a headless renderer returns the
 * input unchanged, which Mermaid then ignores — acceptable, since diagram color is a real-window
 * concern the tests do not assert. */
function resolveColors(raws: string[]): string[] {
  const probe = document.createElement("span");
  probe.setAttribute("aria-hidden", "true");
  probe.style.position = "absolute";
  probe.style.visibility = "hidden";
  probe.style.pointerEvents = "none";
  document.body.appendChild(probe);
  try {
    return raws.map((raw) => {
      if (!raw) return raw;
      probe.style.color = raw;
      return getComputedStyle(probe).color || raw;
    });
  } finally {
    probe.remove();
  }
}

/** Build the Mermaid theme configuration from the app's current tokens. Called per render so a
 * light/dark flip is picked up without caching stale colors. */
export function mermaidThemeConfig(): MermaidThemeConfig {
  const root = getComputedStyle(document.documentElement);
  const names = Object.keys(MERMAID_THEME_TOKENS);
  const raws = names.map((name) => root.getPropertyValue(MERMAID_THEME_TOKENS[name]).trim());
  const resolved = resolveColors(raws);

  const themeVariables: Record<string, string> = { fontSize: MERMAID_FONT_SIZE };
  names.forEach((name, i) => {
    if (resolved[i]) themeVariables[name] = resolved[i];
  });

  // Diagram text uses the app's UI font, not Mermaid's serif default, so labels match the surface.
  const fontFamily = getComputedStyle(document.body).fontFamily || "system-ui, sans-serif";

  return {
    theme: "base",
    darkMode: isDarkTheme(),
    themeVariables,
    fontFamily,
    fontSize: MERMAID_FONT_SIZE,
  };
}
