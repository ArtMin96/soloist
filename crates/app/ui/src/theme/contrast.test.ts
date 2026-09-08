import { describe, expect, it } from "vitest";
import type { ThemeExtensions } from "@/domain";
import { BUILT_IN_THEMES } from "@/theme/catalog";
import { markBackgrounds } from "@/theme/derive";
import { appliedThemeFromFile } from "@/theme/runtime";

function luminance(hex: string): number {
  const channels = [1, 3, 5].map((index) => Number.parseInt(hex.slice(index, index + 2), 16));
  const linear = channels.map((channel) => {
    const value = channel / 255;
    return value <= 0.04045 ? value / 12.92 : ((value + 0.055) / 1.055) ** 2.4;
  });
  return 0.2126 * linear[0] + 0.7152 * linear[1] + 0.0722 * linear[2];
}

function contrast(first: string, second: string): number {
  const [high, low] = [luminance(first), luminance(second)].sort((a, b) => b - a);
  return (high + 0.05) / (low + 0.05);
}

const GIT_TEXT_ROLES = [
  "gitModified",
  "gitAdded",
  "gitDeleted",
  "gitConflicted",
  "gitIgnored",
  "gitBranchSynced",
  "gitBranchLocal",
] as const satisfies ReadonlyArray<keyof ThemeExtensions>;

const STATUS_GRAPHIC_ROLES = [
  "statusRunning",
  "statusTransition",
  "statusStopped",
  "statusCrashed",
  "statusExhausted",
  "statusAttention",
] as const satisfies ReadonlyArray<keyof ThemeExtensions>;

describe("derived Soloist theme contrast", () => {
  it.each(BUILT_IN_THEMES)("keeps $name git text readable on every ground it marks", (source) => {
    const theme = appliedThemeFromFile(source, source.appearance);
    if (!theme) throw new Error(`Could not apply ${source.id}`);
    const backgrounds = markBackgrounds(theme.colors);

    for (const role of GIT_TEXT_ROLES) {
      for (const background of backgrounds) {
        expect(
          contrast(theme.extensions[role], background),
          `${source.id}.${role}`,
        ).toBeGreaterThanOrEqual(4.5);
      }
    }
  });

  it.each(BUILT_IN_THEMES)(
    "keeps $name status marks distinguishable on every ground they mark",
    (source) => {
      const theme = appliedThemeFromFile(source, source.appearance);
      if (!theme) throw new Error(`Could not apply ${source.id}`);
      const backgrounds = markBackgrounds(theme.colors);

      for (const role of STATUS_GRAPHIC_ROLES) {
        for (const background of backgrounds) {
          expect(
            contrast(theme.extensions[role], background),
            `${source.id}.${role}`,
          ).toBeGreaterThanOrEqual(3);
        }
      }
    },
  );

  it("keeps the paired Soloist Default dark git text readable", () => {
    const source = BUILT_IN_THEMES.find(({ id }) => id === "soloist-default");
    if (!source) throw new Error("Missing Soloist Default");
    const theme = appliedThemeFromFile(source, "dark");
    if (!theme) throw new Error("Could not apply Soloist Default dark");
    const backgrounds = markBackgrounds(theme.colors);

    for (const role of GIT_TEXT_ROLES) {
      for (const background of backgrounds) {
        expect(
          contrast(theme.extensions[role], background),
          `soloist-default.dark.${role}`,
        ).toBeGreaterThanOrEqual(4.5);
      }
    }
  });

  it("preserves the explicit Soloist Default scrim and shadow alpha tokens", () => {
    const source = BUILT_IN_THEMES.find(({ id }) => id === "soloist-default");
    if (!source) throw new Error("Missing Soloist Default");
    const theme = appliedThemeFromFile(source, "light");
    if (!theme) throw new Error("Could not apply Soloist Default light");

    expect(theme.extensions.overlayScrim).toBe("#1b1e2573");
    expect(theme.extensions.shadowInk).toBe("#23262c33");
  });
});
