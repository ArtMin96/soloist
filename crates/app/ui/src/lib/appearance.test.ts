// @vitest-environment jsdom
import { describe, expect, it } from "vitest";
import { Terminal } from "@xterm/xterm";
import { DEFAULT_APPEARANCE, TERMINAL_OVERVIEW_RULER_WIDTH, terminalOptions } from "./appearance";
import { TERMINAL_MINIMUM_CONTRAST_RATIO } from "./terminalPalette";

describe("terminalOptions", () => {
  it("arms xterm's readability floor for colour the palette does not choose", () => {
    // The option defaults to 1 (off), which leaves 256-colour and truecolor output free to pick
    // a foreground invisible on the terminal surface. It is a top-level option, not a theme
    // field, so it only reaches the emulator through here.
    expect(terminalOptions(DEFAULT_APPEARANCE, false).minimumContrastRatio).toBe(
      TERMINAL_MINIMUM_CONTRAST_RATIO,
    );
    expect(terminalOptions(DEFAULT_APPEARANCE, true).minimumContrastRatio).toBe(
      TERMINAL_MINIMUM_CONTRAST_RATIO,
    );
  });

  it("selects the word under a right click", () => {
    // xterm derives this option's default from "are we on macOS", so it arrives off on our only
    // target — a right click would then open the context menu over an empty selection, which is
    // the one thing that menu exists to act on.
    expect(terminalOptions(DEFAULT_APPEARANCE, false).rightClickSelectsWord).toBe(true);
    expect(terminalOptions(DEFAULT_APPEARANCE, true).rightClickSelectsWord).toBe(true);
  });

  it("keeps xterm's screen-reader mode off in a shipped build", () => {
    // Screen-reader mode maintains an accessibility DOM tree mirroring the viewport — an end-to-end
    // affordance (the WebDriver harness reads the terminal through it, since the GPU renderer draws
    // to a canvas the DOM cannot read). A shipped build must not pay for that tree: VITE_E2E is unset
    // outside the e2e build, so the option resolves off regardless of theme.
    expect(terminalOptions(DEFAULT_APPEARANCE, false).screenReaderMode).toBe(false);
    expect(terminalOptions(DEFAULT_APPEARANCE, true).screenReaderMode).toBe(false);
  });

  it("opens a terminal whose proposed-API surface is actually reachable", () => {
    // Asserted through the emulator rather than the option object: the gate's whole effect is that
    // reading `unicode` (the grapheme tables), `markers` and `registerDecoration` (the search
    // highlights) throws while it is shut, so a terminal built from these options has to be able
    // to reach them or the addons silently do nothing.
    for (const dark of [false, true]) {
      const term = new Terminal(terminalOptions(DEFAULT_APPEARANCE, dark));
      expect(() => term.unicode.activeVersion).not.toThrow();
      expect(() => term.markers).not.toThrow();
    }
  });

  it("gives the overview ruler a width, without which it is never rendered", () => {
    // The emulator draws no ruler at all until a width is set, so search matches outside the
    // visible screen would go unmarked — the same shape of failure as a setting nothing reads.
    for (const dark of [false, true]) {
      expect(terminalOptions(DEFAULT_APPEARANCE, dark).overviewRuler).toEqual({
        width: TERMINAL_OVERVIEW_RULER_WIDTH,
      });
    }
  });
});
