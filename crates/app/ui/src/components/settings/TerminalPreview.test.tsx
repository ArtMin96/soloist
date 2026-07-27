// @vitest-environment jsdom
import { afterEach, describe, expect, it } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import { TerminalPreview } from "@/components/settings/TerminalPreview";
import { DEFAULT_APPEARANCE } from "@/lib/appearance";
import { ANSI_COLOR_NAMES, ansiColorLabel, terminalColors } from "@/lib/terminalPalette";
import { AppearanceContext } from "@/store/appearanceContext";

afterEach(cleanup);

function renderPreview(dark: boolean) {
  return render(
    <AppearanceContext value={{ appearance: DEFAULT_APPEARANCE, dark, setAppearance: () => {} }}>
      <TerminalPreview />
    </AppearanceContext>,
  );
}

// jsdom normalises an inline colour to `rgb(...)`, so compare renders against each other rather
// than against the source hex.
const swatchColor = (name: string) => screen.getByTitle(name).style.backgroundColor;

describe("Settings — terminal preview", () => {
  it("shows a swatch for every ANSI slot", () => {
    renderPreview(false);
    expect(screen.getByRole("list", { name: "ANSI palette" })).toBeTruthy();
    for (const name of ANSI_COLOR_NAMES) {
      expect(swatchColor(ansiColorLabel(name)), name).not.toBe("");
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
