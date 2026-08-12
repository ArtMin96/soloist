// @vitest-environment jsdom

import { beforeEach, describe, expect, it } from "vitest";
import type { ThemeColorRole } from "@/domain";
import { DEFAULT_APPEARANCE } from "@/lib/appearance";
import { themeSignature as mermaidThemeSignature } from "@/lib/mermaid/theme";
import { searchDecorationColors, terminalColors } from "@/lib/terminalPalette";
import { BUILT_IN_THEMES } from "@/theme/catalog";
import { THEME_COLOR_ROLE_META } from "@/theme/roles";
import {
  APPLIED_THEME_HINT_KEY,
  appliedThemeFromFile,
  applyTheme,
  readAppliedThemeHint,
  resolveAppliedTheme,
  themeCssVariables,
  writeAppliedThemeHint,
} from "@/theme/runtime";

/** The naming rule each role's CSS variable follows: `sidebarRowHover` → `sidebar-row-hover`. */
function kebabCase(role: string): string {
  return role.replace(/[A-Z]/gu, (letter) => `-${letter.toLowerCase()}`);
}

describe("theme runtime", () => {
  beforeEach(() => {
    document.documentElement.className = "";
    document.documentElement.removeAttribute("data-theme-id");
    document.documentElement.removeAttribute("data-theme-signature");
    document.documentElement.removeAttribute("data-theme-appearance");
    document.documentElement.removeAttribute("style");
    localStorage.clear();
  });

  it("resolves the selected palette for the operating-system appearance", () => {
    const appearance = {
      ...DEFAULT_APPEARANCE,
      theme: "system" as const,
      selected_themes: { light: "github-light", dark: "poimandres-dark-theme" },
      custom_themes: [],
      glass_opacity: 80,
    };

    expect(resolveAppliedTheme(appearance, BUILT_IN_THEMES, false)).toMatchObject({
      id: "github-light",
      appearance: "light",
    });
    expect(resolveAppliedTheme(appearance, BUILT_IN_THEMES, true)).toMatchObject({
      id: "poimandres-dark-theme",
      appearance: "dark",
    });
  });

  it("updates root variables and the signature when switching between dark themes", () => {
    const poimandres = resolveAppliedTheme(
      {
        ...DEFAULT_APPEARANCE,
        theme: "dark",
        selected_themes: { light: "soloist-default", dark: "poimandres-dark-theme" },
        custom_themes: [],
        glass_opacity: 80,
      },
      BUILT_IN_THEMES,
      false,
    );
    const dracula = resolveAppliedTheme(
      {
        ...DEFAULT_APPEARANCE,
        theme: "dark",
        selected_themes: { light: "soloist-default", dark: "dracula" },
        custom_themes: [],
        glass_opacity: 80,
      },
      BUILT_IN_THEMES,
      false,
    );

    applyTheme(poimandres);
    const firstSignature = document.documentElement.dataset.themeSignature;
    const firstMermaidSignature = mermaidThemeSignature();
    applyTheme(dracula);

    expect(document.documentElement.classList.contains("dark")).toBe(true);
    expect(document.documentElement.dataset.themeId).toBe("dracula");
    expect(document.documentElement.dataset.themeAppearance).toBe("dark");
    expect(document.documentElement.dataset.themeSignature).not.toBe(firstSignature);
    expect(mermaidThemeSignature()).not.toBe(firstMermaidSignature);
    expect(document.documentElement.style.getPropertyValue("--background")).toBe(
      dracula.colors.canvas,
    );
    expect(terminalColors(dracula).background).not.toBe(terminalColors(poimandres).background);
    expect(dracula.extensions.gitAdded).not.toBe(poimandres.extensions.gitAdded);
    expect(terminalColors(dracula).magenta).not.toBe(terminalColors(poimandres).magenta);
    expect(searchDecorationColors(dracula).activeMatchBorder).toBe(
      dracula.terminal.searchActiveMatchBorder,
    );
  });

  it("prefers explicit Soloist extensions and derives every omitted role from that palette", () => {
    const source = BUILT_IN_THEMES.find(({ id }) => id === "dracula");
    if (!source) throw new Error("missing Dracula fixture");
    const explicit = {
      ...source,
      id: "custom-dracula",
      source: "custom" as const,
      extensions: { soloist: { statusRunning: "#123456" } },
    };
    const applied = resolveAppliedTheme(
      {
        ...DEFAULT_APPEARANCE,
        theme: "dark",
        selected_themes: { light: "soloist-default", dark: explicit.id },
        custom_themes: [explicit],
        glass_opacity: 80,
      },
      [...BUILT_IN_THEMES, explicit],
      false,
    );

    expect(applied.extensions.statusRunning).toBe("#123456");
    expect(applied.extensions.gitAdded).not.toBe("#123456");
    expect(applied.terminal.green).toBe("#123456");
  });

  it("changes the applied signature when only an explicit extension changes", () => {
    const source = BUILT_IN_THEMES.find(({ id }) => id === "dracula");
    if (!source) throw new Error("missing Dracula fixture");
    const first = appliedThemeFromFile(
      { ...source, extensions: { soloist: { statusRunning: "#123456" } } },
      "dark",
    );
    const second = appliedThemeFromFile(
      { ...source, extensions: { soloist: { statusRunning: "#654321" } } },
      "dark",
    );

    expect(second?.signature).not.toBe(first?.signature);
  });

  it("resolves the extension colors each appearance authored for its own palette", () => {
    const source = BUILT_IN_THEMES.find(({ id }) => id === "soloist-default");
    if (!source) throw new Error("missing Soloist Default fixture");
    const paired = {
      ...source,
      extensions: { soloist: { gitModified: "#b06c00", terminalAnsiRed: "#c9372c" } },
      variants: {
        dark: source.variants?.dark,
        extensions: { dark: { soloist: { gitModified: "#eba941", terminalAnsiRed: "#f75d59" } } },
      },
    };

    const light = appliedThemeFromFile(paired, "light");
    const dark = appliedThemeFromFile(paired, "dark");

    expect(light?.extensions.gitModified).toBe("#b06c00");
    expect(light?.terminal.red).toBe("#c9372c");
    expect(dark?.extensions.gitModified).toBe("#eba941");
    expect(dark?.terminal.red).toBe("#f75d59");
  });

  it("keeps a theme-level extension set on both appearances of a paired theme", () => {
    const source = BUILT_IN_THEMES.find(({ id }) => id === "soloist-default");
    if (!source) throw new Error("missing Soloist Default fixture");
    const shared = {
      ...source,
      extensions: { soloist: { gitModified: "#b06c00" } },
      variants: { dark: source.variants?.dark },
    };

    expect(appliedThemeFromFile(shared, "light")?.extensions.gitModified).toBe("#b06c00");
    expect(appliedThemeFromFile(shared, "dark")?.extensions.gitModified).toBe("#b06c00");
  });

  it("derives the paired default's dark extensions instead of repainting its light ones", () => {
    const source = BUILT_IN_THEMES.find(({ id }) => id === "soloist-default");
    if (!source) throw new Error("missing Soloist Default fixture");

    const light = appliedThemeFromFile(source, "light");
    const dark = appliedThemeFromFile(source, "dark");

    expect(light?.extensions.gitModified).toBe(source.extensions?.soloist?.gitModified);
    expect(dark?.extensions.gitModified).not.toBe(source.extensions?.soloist?.gitModified);
  });

  it("maps every applied palette field through one exhaustive CSS projection", () => {
    const theme = resolveAppliedTheme(
      {
        ...DEFAULT_APPEARANCE,
        theme: "dark",
        selected_themes: { light: "soloist-default", dark: "poimandres-dark-theme" },
        custom_themes: [],
        glass_opacity: 80,
      },
      BUILT_IN_THEMES,
      false,
    );

    const variables = themeCssVariables(theme);

    // Exhaustive against the role vocabulary rather than against a count: every role the editor
    // and validator know reaches CSS as `--theme-<role>` carrying that role's own colour, so a
    // role that stops being projected reddens here instead of quietly losing its paint.
    for (const role of Object.keys(THEME_COLOR_ROLE_META) as ThemeColorRole[]) {
      const variable: `--${string}` = `--theme-${kebabCase(role)}`;
      expect(theme.colors[role], role).toBeTruthy();
      expect(variables[variable], variable).toBe(theme.colors[role]);
    }

    expect(variables["--background"]).toBe(theme.colors.canvas);
    expect(variables["--terminal-background"]).toBe(theme.terminal.background);
    expect(variables["--status-crashed"]).toBe(theme.extensions.statusCrashed);
    expect(variables["--glass-opacity"]).toBe("0.8");
    expect(variables["--glass-surface"]).toContain(theme.colors.surfaceOverlay);
  });

  it("round-trips the complete custom palette needed for a flash-free prepaint", () => {
    const applied = resolveAppliedTheme(
      {
        ...DEFAULT_APPEARANCE,
        theme: "dark",
        selected_themes: { light: "soloist-default", dark: "poimandres-dark-theme" },
        custom_themes: [],
        glass_opacity: 80,
      },
      BUILT_IN_THEMES,
      false,
    );

    writeAppliedThemeHint(applied);

    expect(localStorage.getItem(APPLIED_THEME_HINT_KEY)).not.toBeNull();
    expect(readAppliedThemeHint()).toEqual(applied);
  });
});
