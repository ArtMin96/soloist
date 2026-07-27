import { describe, expect, it } from "vitest";
import {
  ANSI_COLOR_NAMES,
  TERMINAL_MINIMUM_CONTRAST_RATIO,
  type AnsiColorName,
  terminalColors,
} from "./terminalPalette";

// WCAG 2.x relative luminance and contrast ratio, over the hex the palette actually emits.
function luminance(hex: string): number {
  const n = Number.parseInt(hex.slice(1), 16);
  const channels = [(n >> 16) & 255, (n >> 8) & 255, n & 255].map((v) => {
    const c = v / 255;
    return c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
  });
  return 0.2126 * channels[0] + 0.7152 * channels[1] + 0.0722 * channels[2];
}

function contrast(a: string, b: string): number {
  const [hi, lo] = [luminance(a), luminance(b)].sort((x, y) => y - x);
  return (hi + 0.05) / (lo + 0.05);
}

// `over` laid on `under` at `alpha`, the way the renderer resolves a translucent selection.
function blend(under: string, over: string, alpha: number): string {
  const channels = (hex: string) => [1, 3, 5].map((i) => Number.parseInt(hex.slice(i, i + 2), 16));
  const [u, o] = [channels(under), channels(over)];
  return `#${o
    .map((v, i) =>
      Math.round(u[i] + (v - u[i]) * alpha)
        .toString(16)
        .padStart(2, "0"),
    )
    .join("")}`;
}

// The slots whose ANSI role *is* the near-background tone of their own theme, so demanding
// 4.5:1 of them would invert what the slot means (`\e[47m` on the light theme has to paint a
// pale panel). Named here rather than imported, so widening the palette's exemptions in the
// source reddens this test instead of silently agreeing with it.
const SURFACE_END: Record<"light" | "dark", AnsiColorName[]> = {
  light: ["white", "brightWhite"],
  dark: ["black"],
};

// Bold output renders in the bright set (`drawBoldTextInBrightColors` defaults on), so a bright
// hue must never be less legible than its normal twin. The achromatic pair is excluded because
// ANSI fixes its meaning the other way round: `brightBlack` is the dim slot, and `brightWhite`
// the surface end of a light theme — neither is an emphatic form of its twin.
const CHROMATIC = ["red", "green", "yellow", "blue", "magenta", "cyan"] as const;

const THEMES = [
  { name: "light" as const, dark: false },
  { name: "dark" as const, dark: true },
];

describe("terminalColors", () => {
  it.each(THEMES)("exposes every ANSI slot in the $name theme", ({ dark }) => {
    const colors = terminalColors(dark);
    for (const name of ANSI_COLOR_NAMES) {
      expect(colors[name], name).toMatch(/^#[0-9a-f]{6}$/);
    }
  });

  it.each(THEMES)("gives the $name scrollbar slider a parseable alpha", ({ dark }) => {
    const colors = terminalColors(dark);
    // xterm reads a hex colour by length: only 4, 5, 7 and 9 characters are understood, and a
    // mistyped alpha lands on a length that parses as nothing at all rather than erroring.
    for (const slider of [
      colors.scrollbarSliderBackground,
      colors.scrollbarSliderHoverBackground,
      colors.scrollbarSliderActiveBackground,
    ]) {
      expect(slider).toMatch(/^#[0-9a-f]{8}$/);
    }
  });

  it.each(THEMES)(
    "clears AA against the $name background on every slot but its surface end",
    ({ name, dark }) => {
      const colors = terminalColors(dark);
      const failures = ANSI_COLOR_NAMES.filter(
        (slot) =>
          !SURFACE_END[name].includes(slot) &&
          contrast(colors[slot], colors.background) < TERMINAL_MINIMUM_CONTRAST_RATIO,
      ).map(
        (slot) =>
          `${slot} ${colors[slot]} ${contrast(colors[slot], colors.background).toFixed(2)}:1`,
      );
      expect(failures).toEqual([]);
    },
  );

  it.each(THEMES)(
    "clears AA over a $name selection too, not just the bare surface",
    ({ name, dark }) => {
      const colors = terminalColors(dark);
      // xterm forces an opaque `selectionBackground` to 30% and blends it over the terminal
      // background, so a selected cell sits on this — not on the raw selection hex. Every slot
      // loses a little contrast there, which is where the dim slot would otherwise dip under the
      // bar and get visibly recoloured the moment a user drags across it.
      for (const selection of [colors.selectionBackground, colors.selectionInactiveBackground]) {
        const behind = blend(colors.background, selection, 0.3);
        const failures = ANSI_COLOR_NAMES.filter(
          (slot) =>
            !SURFACE_END[name].includes(slot) &&
            contrast(colors[slot], behind) < TERMINAL_MINIMUM_CONTRAST_RATIO,
        ).map((slot) => `${slot} ${colors[slot]} ${contrast(colors[slot], behind).toFixed(2)}:1`);
        expect(failures, behind).toEqual([]);
      }
    },
  );

  it.each(THEMES)("keeps bold at least as legible as normal in the $name theme", ({ dark }) => {
    const colors = terminalColors(dark);
    const regressions = CHROMATIC.filter((slot) => {
      const bright = `bright${slot[0].toUpperCase()}${slot.slice(1)}` as AnsiColorName;
      return (
        contrast(colors[bright], colors.background) < contrast(colors[slot], colors.background)
      );
    });
    expect(regressions).toEqual([]);
  });

  it.each(THEMES)("keeps the $name surface-end slots next to the background", ({ name, dark }) => {
    const colors = terminalColors(dark);
    // The exemption is only honest while these really are the near-background tone. A slot that
    // drifted into mid-tone would be exempt from the AA check without deserving it.
    for (const slot of SURFACE_END[name]) {
      expect(contrast(colors[slot], colors.background), slot).toBeLessThan(2);
    }
  });

  it("gives the unfocused selection its own tone in both themes", () => {
    // xterm falls back to the active selection colour when this is unset, which would leave a
    // background window looking focused.
    for (const dark of [false, true]) {
      const colors = terminalColors(dark);
      expect(colors.selectionInactiveBackground).not.toBe(colors.selectionBackground);
    }
  });
});
