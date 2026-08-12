import { describe, expect, it } from "vitest";
import { paletteContrastWarnings, themeContrastWarnings } from "@/theme/accessibility";
import { contrastRatio, contrastSafeThemeColor } from "@/theme/derive";
import { BUILT_IN_THEMES } from "@/theme/catalog";

describe("theme accessibility", () => {
  it("adjusts syntax colors to readable contrast against a light code background", () => {
    const background = "#fefefe";
    const adjusted = contrastSafeThemeColor("#ffffff", [background]);

    expect(contrastRatio(adjusted, background)).toBeGreaterThanOrEqual(4.5);
  });

  it("finds every published built-in palette clean, including a paired theme's second one", () => {
    // Driven by the closed appearance list rather than the keys of the `variants` object, which
    // also carries the per-appearance extension sets and so is not an appearance map.
    expect(paletteContrastWarnings(BUILT_IN_THEMES)).toEqual([]);
  });

  it("reports low-contrast theme roles without rejecting the palette", () => {
    const colors = { ...BUILT_IN_THEMES[0].colors, text: "#ffffff", canvas: "#ffffff" };

    expect(themeContrastWarnings(colors)).toEqual(
      expect.arrayContaining([expect.objectContaining({ label: "Body text" })]),
    );
  });
});
