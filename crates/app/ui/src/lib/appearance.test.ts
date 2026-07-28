// @vitest-environment jsdom
import { describe, expect, it } from "vitest";
import { Terminal } from "@xterm/xterm";
import {
  DEFAULT_APPEARANCE,
  monoFontOptions,
  TERMINAL_FIXED_OPTIONS,
  terminalFontFamily,
  terminalOptions,
} from "./appearance";
import { TERMINAL_MINIMUM_CONTRAST_RATIO } from "./terminalPalette";

describe("terminalFontFamily", () => {
  it("names families the platform installs, so nothing rests on the generic tail", () => {
    // The app bundles no font, so every family the stack names has to be one Ubuntu's own
    // packaging carries — a name absent from the box is skipped, and a stack of only absent names
    // renders whatever that machine resolves `monospace` to, which is not a decision the app made.
    expect(terminalFontFamily(null)).toBe('"Ubuntu Mono", "DejaVu Sans Mono", monospace');
  });

  it("puts a chosen family ahead of the whole stack", () => {
    expect(terminalFontFamily("Fira Code")).toBe(
      '"Fira Code", "Ubuntu Mono", "DejaVu Sans Mono", monospace',
    );
  });

  it("still prepends a family the stack already names", () => {
    // The repetition is deliberate rather than a case worth branching on: CSS reads the first
    // match and ignores the rest, so a chosen family is prepended unconditionally.
    expect(terminalFontFamily("Ubuntu Mono")).toBe(
      '"Ubuntu Mono", "Ubuntu Mono", "DejaVu Sans Mono", monospace',
    );
  });
});

describe("monoFontOptions", () => {
  it("offers the system default first, then only families the platform installs", () => {
    expect(monoFontOptions(null)).toEqual([
      { value: null, label: "System default" },
      { value: "Ubuntu Mono", label: "Ubuntu Mono" },
      { value: "DejaVu Sans Mono", label: "DejaVu Sans Mono" },
      { value: "Liberation Mono", label: "Liberation Mono" },
    ]);
  });

  it("keeps a stored family that the offered set does not carry", () => {
    // Anything chosen before the set was narrowed is still what the record holds, and a select
    // handed a value no item carries has nothing to show for it.
    expect(monoFontOptions("Fira Code")).toContainEqual({
      value: "Fira Code",
      label: "Fira Code",
    });
  });

  it("does not repeat a stored family the offered set already carries", () => {
    const values = monoFontOptions("Ubuntu Mono").map((option) => option.value);
    expect(values.filter((value) => value === "Ubuntu Mono")).toHaveLength(1);
  });

  it("reads a blank stored family as no choice, the way the stack reads it", () => {
    // The field is a free string, so a record can hold "" even though the control never writes
    // one. Both readers of that field have to agree about it: the stack already resolves "" to the
    // default, and an option carrying it would be a select item with an empty value.
    expect(monoFontOptions("")).toEqual(monoFontOptions(null));
    expect(terminalFontFamily("")).toBe('"Ubuntu Mono", "DejaVu Sans Mono", monospace');
  });
});

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
