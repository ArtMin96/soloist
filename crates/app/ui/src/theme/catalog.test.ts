import { describe, expect, it } from "vitest";
import { BUILT_IN_THEMES } from "@/theme/catalog";
import { themeColorsForAppearance } from "@/theme/derive";
import { THEME_COLOR_ROLES } from "@/theme/roles";

describe("built-in theme catalog", () => {
  it("ships the six promised themes from complete T3-v1 assets", () => {
    expect(BUILT_IN_THEMES.map(({ id }) => id)).toEqual([
      "soloist-default",
      "poimandres-dark-theme",
      "catppuccin",
      "dracula",
      "tokyo-night",
      "github-light",
    ]);

    for (const theme of BUILT_IN_THEMES) {
      expect(Object.keys(theme.colors), theme.id).toHaveLength(THEME_COLOR_ROLES.length);
      expect(Object.keys(theme.colors).sort(), theme.id).toEqual([...THEME_COLOR_ROLES].sort());
    }
  });

  it("keeps Soloist Default paired so one ID is valid for both selections", () => {
    const theme = BUILT_IN_THEMES[0];
    expect(themeColorsForAppearance(theme, "light")).not.toBeNull();
    expect(themeColorsForAppearance(theme, "dark")).not.toBeNull();
  });

  it("uses the supplied Poimandres identity so importing that file conflicts with the built-in", () => {
    expect(BUILT_IN_THEMES.find(({ id }) => id === "poimandres-dark-theme")).toMatchObject({
      name: "Poimandres dark theme",
      author: "sbansal1999",
    });
  });
});
