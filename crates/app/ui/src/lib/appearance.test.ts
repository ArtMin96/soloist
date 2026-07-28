// @vitest-environment jsdom
import { describe, expect, it } from "vitest";
import { Terminal } from "@xterm/xterm";
import { DEFAULT_APPEARANCE, TERMINAL_FIXED_OPTIONS, terminalOptions } from "./appearance";
import { TERMINAL_MINIMUM_CONTRAST_RATIO } from "./terminalPalette";

describe("TERMINAL_FIXED_OPTIONS", () => {
  it("arms xterm's readability floor for colour the palette does not choose", () => {
    // The option defaults to 1 (off), which leaves 256-colour and truecolor output free to pick
    // a foreground invisible on the terminal surface. It is a top-level option, not a theme
    // field, so it only reaches the emulator through here.
    expect(TERMINAL_FIXED_OPTIONS.minimumContrastRatio).toBe(TERMINAL_MINIMUM_CONTRAST_RATIO);
  });

  it("selects the word under a right click", () => {
    // xterm derives this option's default from "are we on macOS", so it arrives off on our only
    // target — a right click would then open the context menu over an empty selection, which is
    // the one thing that menu exists to act on.
    expect(TERMINAL_FIXED_OPTIONS.rightClickSelectsWord).toBe(true);
  });

  it("keeps xterm's screen-reader mode off in a shipped build", () => {
    // Screen-reader mode maintains an accessibility DOM tree mirroring the viewport — an end-to-end
    // affordance (the WebDriver harness reads the terminal through it, since the GPU renderer draws
    // to a canvas the DOM cannot read). A shipped build must not pay for that tree: VITE_E2E is
    // unset outside the e2e build, so the option resolves off.
    expect(TERMINAL_FIXED_OPTIONS.screenReaderMode).toBe(false);
  });

  it("opens a terminal whose proposed-API surface is actually reachable", () => {
    // Asserted through the emulator rather than the option object: the gate's whole effect is that
    // reading `unicode` (the grapheme tables), `markers` and `registerDecoration` (the search
    // highlights) throws while it is shut, so a terminal built the way a pane is built has to be
    // able to reach them or the addons silently do nothing.
    for (const dark of [false, true]) {
      const term = new Terminal({
        ...TERMINAL_FIXED_OPTIONS,
        ...terminalOptions(DEFAULT_APPEARANCE, dark),
      });
      expect(() => term.unicode.activeVersion).not.toThrow();
      expect(() => term.markers).not.toThrow();
    }
  });
});
