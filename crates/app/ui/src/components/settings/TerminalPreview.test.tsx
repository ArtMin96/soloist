// @vitest-environment jsdom
import { afterEach, describe, expect, it } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import { TerminalPreview } from "@/components/settings/TerminalPreview";
import { DEFAULT_APPEARANCE } from "@/lib/appearance";
import { ANSI_COLOR_NAMES, ansiColorLabel, terminalColors } from "@/lib/terminalPalette";
import { AppearanceContext } from "@/store/appearanceContext";
import { fakeAppearanceState } from "@/test/appearanceState";

afterEach(cleanup);

function renderPreview(dark: boolean) {
  return render(
    <AppearanceContext value={fakeAppearanceState(DEFAULT_APPEARANCE, dark)}>
      <TerminalPreview />
    </AppearanceContext>,
  );
}

// jsdom normalises an inline colour to `rgb(...)`, so the palette's hex has to be converted before
// it can be held against what a swatch actually painted.
const swatchColor = (name: string) => screen.getByTitle(name).style.backgroundColor;
const rgb = (hex: string) =>
  `rgb(${[1, 3, 5].map((i) => Number.parseInt(hex.slice(i, i + 2), 16)).join(", ")})`;

describe("Settings — terminal preview", () => {
  it("shows a swatch for every ANSI slot", () => {
    renderPreview(false);
    expect(screen.getByRole("list", { name: "ANSI palette" })).toBeTruthy();
    const colors = terminalColors(false);
    // Per slot, not merely "something was painted": a row that rendered one colour sixteen times
    // would satisfy the weaker check while showing the user nothing about the palette.
    for (const name of ANSI_COLOR_NAMES) {
      expect(swatchColor(ansiColorLabel(name)), name).toBe(rgb(colors[name]));
    }
    // A literal, because every other lookup here names a swatch through the same helper the
    // component names it with — so a helper that stopped rewriting the slot name would satisfy
    // them all. This is the one assertion that reads what a user actually hovers.
    expect(screen.getByTitle("Bright black")).toBeTruthy();
  });

  it("repaints the swatches when the theme flips", () => {
    renderPreview(false);
    const light = ANSI_COLOR_NAMES.map((name) => swatchColor(ansiColorLabel(name)));
    cleanup();
    renderPreview(true);
    const dark = ANSI_COLOR_NAMES.map((name) => swatchColor(ansiColorLabel(name)));

    // Every slot the two palettes genuinely disagree on has to move on screen; a preview reading
    // one fixed palette would report the same colour in both themes.
    const differing = ANSI_COLOR_NAMES.filter(
      (name) => terminalColors(false)[name] !== terminalColors(true)[name],
    );
    expect(differing.length).toBeGreaterThan(0);
    for (const name of differing) {
      const i = ANSI_COLOR_NAMES.indexOf(name);
      expect(light[i], name).not.toBe(dark[i]);
    }
  });
});
