import { describe, expect, it } from "vitest";
import { themeContrastWarnings } from "@/theme/accessibility";
import { contrastRatio, contrastSafeThemeColor } from "@/theme/derive";
import { BUILT_IN_THEMES } from "@/theme/catalog";

describe("theme accessibility", () => {
  it("adjusts syntax colors to readable contrast against a light code background", () => {
    const background = "#fefefe";
    const adjusted = contrastSafeThemeColor("#ffffff", [background]);

    expect(contrastRatio(adjusted, background)).toBeGreaterThanOrEqual(4.5);
  });

  it("reports low-contrast theme roles without rejecting the palette", () => {
    const colors = { ...BUILT_IN_THEMES[0].colors, text: "#ffffff", canvas: "#ffffff" };

    expect(themeContrastWarnings(colors)).toEqual(
      expect.arrayContaining([expect.objectContaining({ label: "Body text" })]),
    );
  });
});
