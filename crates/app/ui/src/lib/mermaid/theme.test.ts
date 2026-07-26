// @vitest-environment jsdom
import { afterEach, describe, expect, it } from "vitest";
import { MERMAID_FONT_SIZE, MERMAID_THEME_TOKENS } from "./const";
import { isDarkTheme, mermaidThemeConfig, themeSignature } from "./theme";

afterEach(() => {
  document.documentElement.classList.remove("dark");
  for (const token of Object.values(MERMAID_THEME_TOKENS)) {
    document.documentElement.style.removeProperty(token);
  }
});

describe("themeSignature", () => {
  it("changes with the app palette, so a mounted diagram knows to re-render", () => {
    const light = themeSignature();
    document.documentElement.classList.add("dark");
    expect(themeSignature()).not.toBe(light);
    expect(isDarkTheme()).toBe(true);
  });
});

describe("mermaidThemeConfig", () => {
  it("reports dark mode inside themeVariables, which is where Mermaid reads it", () => {
    document.documentElement.classList.add("dark");

    const config = mermaidThemeConfig();

    // Mermaid assigns theme variables onto the theme instance before deriving the colours it was not
    // given, and every one of those derivations branches on this flag. At the config's top level it is
    // ignored, and a dark diagram comes out with light-mode derived colours.
    expect(config.themeVariables.darkMode).toBe(true);
    expect("darkMode" in config).toBe(false);
  });

  it("reports light mode the same way", () => {
    expect(mermaidThemeConfig().themeVariables.darkMode).toBe(false);
  });

  it("sizes every palette to the app's body type, not Mermaid's larger default", () => {
    expect(mermaidThemeConfig().themeVariables.fontSize).toBe(MERMAID_FONT_SIZE);
  });

  it("binds each Mermaid theme variable to the design token it names", () => {
    document.documentElement.style.setProperty(MERMAID_THEME_TOKENS.lineColor, "#123456");

    expect(mermaidThemeConfig().themeVariables.lineColor).toBe("#123456");
  });

  it("omits a variable whose token is not defined rather than sending a blank colour", () => {
    expect(mermaidThemeConfig().themeVariables.lineColor).toBeUndefined();
  });
});
