// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, render } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { DEFAULT_APPEARANCE, terminalOptions } from "@/lib/appearance";
import { FakeTerminal } from "@/test/fakeTerminal";
import type { Appearance, CursorInactiveStyle, CursorStyle, ProcessView } from "@/domain";

// jsdom has no emulator surface, so the terminal is the shared recording fake. It seeds `options`
// from the constructor argument exactly as a real xterm does, which is what makes this suite able
// to tell the creation path from the live-restyle path: a cursor option that only ever reaches the
// constructor still leaves the mounted emulator unchanged when the setting is edited.
vi.mock("@xterm/xterm", async () => ({
  Terminal: (await import("@/test/fakeTerminal")).FakeTerminal,
}));
vi.mock("@xterm/addon-fit", () => ({
  FitAddon: class {
    fit() {}
  },
}));
vi.mock("@xterm/addon-search", async () => ({
  SearchAddon: (await import("@/test/fakeSearchAddon")).FakeSearchAddon,
}));
vi.mock("@/lib/terminalRenderer", () => ({
  activateTerminalRenderer: vi.fn().mockResolvedValue({ renderer: "dom", dispose() {} }),
}));

// The appearance the hook sees, swapped between renders so a test can edit a setting the way the
// Appearance panel does and watch the mounted emulator restyle. The resolved theme is swappable for
// the same reason: it is the one input to the projection that is not a field of the document.
const { appearanceRef, darkRef } = vi.hoisted(() => ({
  appearanceRef: { current: null as unknown as Appearance },
  darkRef: { current: true },
}));
vi.mock("@/store/appearanceContext", () => ({
  useAppearance: () => ({ appearance: appearanceRef.current, dark: darkRef.current }),
}));

import { useTerminal } from "@/components/terminal/useTerminal";

const PROCESS: ProcessView = {
  id: 7,
  project: 1,
  kind: "Agent",
  label: "assistant",
  status: "Running",
  exit_code: null,
  requires_trust: false,
  resumable: false,
  ports: [],
  ready: "Ungated",
};

function Probe() {
  const { hostRef } = useTerminal(PROCESS);
  return <div ref={hostRef} />;
}

// Long enough for the mount's async work — the renderer activation and the font-ready re-fit — to
// resolve before a test asserts on the emulator.
const EFFECTS_SETTLE_MS = 20;

async function settle() {
  await act(async () => {
    await new Promise((resolve) => setTimeout(resolve, EFFECTS_SETTLE_MS));
  });
}

function liveTerminal() {
  const term = FakeTerminal.instances.find((instance) => !instance.disposed);
  expect(term, "a mounted emulator").toBeDefined();
  return term as InstanceType<typeof FakeTerminal>;
}

function withTerminal(patch: Partial<Appearance["terminal"]>): Appearance {
  return { ...DEFAULT_APPEARANCE, terminal: { ...DEFAULT_APPEARANCE.terminal, ...patch } };
}

// Any member of a closed set other than the one under test. A case that starts from its own target
// edits the value to itself, so the constructor alone satisfies it and it can no longer tell a live
// restyle from no restyle at all — every case below starts somewhere else.
function otherThan<T>(set: readonly T[], value: T): T {
  const alternative = set.find((candidate) => candidate !== value);
  if (alternative === undefined) throw new Error(`no alternative to ${String(value)}`);
  return alternative;
}

// Mounts a pane holding `seed`, then edits the terminal appearance to `edit` the way the settings
// panel does and lets the hook react. Returns the emulator that was mounted *before* the edit, so a
// caller can assert both what its options became and that it is still the same instance.
async function mountThenEdit(
  seed: Partial<Appearance["terminal"]>,
  edit: Partial<Appearance["terminal"]>,
  themes: { seed: boolean; edit: boolean } = { seed: true, edit: true },
) {
  appearanceRef.current = withTerminal(seed);
  darkRef.current = themes.seed;
  const view = render(<Probe />);
  await settle();
  const mounted = liveTerminal();

  appearanceRef.current = withTerminal(edit);
  darkRef.current = themes.edit;
  await act(async () => {
    view.rerender(<Probe />);
  });
  await settle();

  return mounted;
}

beforeEach(() => {
  FakeTerminal.instances = [];
  appearanceRef.current = DEFAULT_APPEARANCE;
  darkRef.current = true;
  vi.stubGlobal(
    "ResizeObserver",
    class {
      observe() {}
      unobserve() {}
      disconnect() {}
    },
  );
  mockIPC((cmd) => (cmd === "pty_attach" ? 1 : null));
});

afterEach(() => {
  cleanup();
  clearMocks();
  vi.unstubAllGlobals();
});

describe("terminal cursor settings reach the live emulator", () => {
  // The defaults must survive the trip into a freshly created emulator, or an untouched install
  // silently renders a different cursor than the settings document claims.
  it("creates a pane with the documented cursor defaults", async () => {
    render(<Probe />);
    await settle();

    const term = liveTerminal();
    expect(term.options.cursorStyle).toBe("block");
    expect(term.options.cursorInactiveStyle).toBe("outline");
    expect(term.options.cursorBlink).toBe(true);
  });

  // Every value of the closed set must actually reach `term.options` — the assertion the dead
  // `focus_on_click` setting never had. Editing the setting must restyle the emulator the user is
  // already looking at, so these assert against the instance mounted *before* the change: an option
  // applied only at construction would leave it holding the seed and redden every case here.
  const CURSOR_STYLES: CursorStyle[] = ["block", "underline", "bar"];
  it.each(CURSOR_STYLES)(
    "applies cursor style %s to the mounted emulator",
    async (cursor_style) => {
      const term = await mountThenEdit(
        { cursor_style: otherThan(CURSOR_STYLES, cursor_style) },
        { cursor_style },
      );
      expect(term.options.cursorStyle).toBe(cursor_style);
    },
  );

  const CURSOR_INACTIVE_STYLES: CursorInactiveStyle[] = [
    "outline",
    "block",
    "bar",
    "underline",
    "none",
  ];
  it.each(CURSOR_INACTIVE_STYLES)(
    "applies unfocused cursor style %s to the mounted emulator",
    async (cursor_inactive_style) => {
      const term = await mountThenEdit(
        { cursor_inactive_style: otherThan(CURSOR_INACTIVE_STYLES, cursor_inactive_style) },
        { cursor_inactive_style },
      );
      expect(term.options.cursorInactiveStyle).toBe(cursor_inactive_style);
    },
  );

  it.each([false, true])(
    "applies cursor blink %s to the mounted emulator",
    async (cursor_blink) => {
      const term = await mountThenEdit({ cursor_blink: !cursor_blink }, { cursor_blink });
      expect(term.options.cursorBlink).toBe(cursor_blink);
    },
  );

  // Restyling must not recreate the terminal: a remount would drop the emulator's scrollback and
  // force a re-attach. Instance identity is the headless half of that guarantee.
  it("restyles the same emulator instance rather than recreating it", async () => {
    const term = await mountThenEdit({ cursor_style: "block" }, { cursor_style: "bar" });

    expect(term.disposed).toBe(false);
    expect(liveTerminal()).toBe(term);
    expect(FakeTerminal.instances).toHaveLength(1);
  });
});

// Two appearances that disagree about everything the projection derives — including the theme,
// which is why the pair flips `dark` too. Written as whole documents rather than as a diff so the
// disagreement is visible, and checked below rather than trusted.
const PLAIN: Partial<Appearance["terminal"]> = {
  font_family: null,
  font_scale: "small",
  font_weight: "w300",
  bold_font_weight: "w500",
  line_height: "compact",
  letter_spacing: "tight",
  cursor_style: "block",
  cursor_inactive_style: "outline",
  cursor_blink: true,
};

const ELABORATE: Partial<Appearance["terminal"]> = {
  font_family: "Fira Code",
  font_scale: "large",
  font_weight: "w600",
  bold_font_weight: "w800",
  line_height: "spacious",
  letter_spacing: "wider",
  cursor_style: "bar",
  cursor_inactive_style: "none",
  cursor_blink: false,
};

const THEMES = { seed: true, edit: false };

const projected = (terminal: Partial<Appearance["terminal"]>, dark: boolean) =>
  terminalOptions(withTerminal(terminal), dark) as Record<string, unknown>;

// The guarantee the whole appearance surface rests on, stated once over the projection's own keys
// rather than as a list anyone has to remember to extend: an option added to `terminalOptions` and
// forgotten in the restyle effect is a setting that works on the next pane the user opens and does
// nothing to the one in front of them — the dead-setting defect, arriving by omission.
describe("the live restyle re-applies the whole appearance projection", () => {
  // The fixture's own precondition. If a future option is derived from a field these two documents
  // happen to agree on, the emulator would already hold the right value from construction and the
  // case below would pass while that option was never re-assigned at all. Reddening here is how
  // that key gets covered instead of silently slipping through.
  it("moves every derived option, so none can be right by accident", () => {
    const before = projected(PLAIN, THEMES.seed);
    const after = projected(ELABORATE, THEMES.edit);

    for (const key of Object.keys(after)) {
      expect(before[key], `${key} must differ between the two appearances`).not.toEqual(after[key]);
    }
  });

  it("assigns each of them to the emulator already on screen", async () => {
    const term = await mountThenEdit(PLAIN, ELABORATE, THEMES);
    const after = projected(ELABORATE, THEMES.edit);

    for (const [key, value] of Object.entries(after)) {
      expect(term.options[key], `${key} never reached the mounted emulator`).toEqual(value);
    }
  });
});
