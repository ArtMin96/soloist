// @vitest-environment jsdom

import { beforeEach, describe, expect, it } from "vitest";
import { DEFAULT_APPEARANCE } from "@/lib/appearance";
import { themeSignature as mermaidThemeSignature } from "@/lib/mermaid/theme";
import { searchDecorationColors, terminalColors } from "@/lib/terminalPalette";
import { BUILT_IN_THEMES } from "@/theme/catalog";
import {
  APPLIED_THEME_HINT_KEY,
  appliedThemeFromFile,
  applyTheme,
  readAppliedThemeHint,
  resolveAppliedTheme,
  themeCssVariables,
  writeAppliedThemeHint,
} from "@/theme/runtime";

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

    expect(Object.keys(variables).length).toBeGreaterThan(80);
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
