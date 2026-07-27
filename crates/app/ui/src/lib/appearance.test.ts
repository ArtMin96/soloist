import { describe, expect, it } from "vitest";
import { DEFAULT_APPEARANCE, terminalOptions } from "./appearance";
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

  it("keeps xterm's screen-reader mode off in a shipped build", () => {
    // Screen-reader mode maintains an accessibility DOM tree mirroring the viewport — an end-to-end
    // affordance (the WebDriver harness reads the terminal through it, since the GPU renderer draws
    // to a canvas the DOM cannot read). A shipped build must not pay for that tree: VITE_E2E is unset
    // outside the e2e build, so the option resolves off regardless of theme.
    expect(terminalOptions(DEFAULT_APPEARANCE, false).screenReaderMode).toBe(false);
    expect(terminalOptions(DEFAULT_APPEARANCE, true).screenReaderMode).toBe(false);
  });
});
