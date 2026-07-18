import { describe, expect, it } from "vitest";
import { DEFAULT_APPEARANCE, terminalOptions } from "./appearance";

describe("terminalOptions", () => {
  it("keeps xterm's screen-reader mode off in a shipped build", () => {
    // Screen-reader mode maintains an accessibility DOM tree mirroring the viewport — an end-to-end
    // affordance (the WebDriver harness reads the terminal through it, since the GPU renderer draws
    // to a canvas the DOM cannot read). A shipped build must not pay for that tree: VITE_E2E is unset
    // outside the e2e build, so the option resolves off regardless of theme.
    expect(terminalOptions(DEFAULT_APPEARANCE, false).screenReaderMode).toBe(false);
    expect(terminalOptions(DEFAULT_APPEARANCE, true).screenReaderMode).toBe(false);
  });
});
