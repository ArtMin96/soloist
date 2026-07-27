// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, render } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { DEFAULT_APPEARANCE } from "@/lib/appearance";
import type { Appearance, CursorInactiveStyle, CursorStyle, ProcessView } from "@/domain";

// jsdom has no emulator surface, so the terminal is an options-recording fake. It seeds `options`
// from the constructor argument exactly as a real xterm does, which is what makes this suite able
// to tell the creation path from the live-restyle path: a cursor option that only ever reaches the
// constructor still leaves the mounted emulator unchanged when the setting is edited.
const { FakeTerminal } = vi.hoisted(() => {
  class FakeTerminal {
    static instances: FakeTerminal[] = [];
    options: Record<string, unknown>;
    disposed = false;
    cols = 80;
    rows = 24;
    constructor(options: Record<string, unknown>) {
      this.options = { ...options };
      FakeTerminal.instances.push(this);
    }
    loadAddon() {}
    open() {}
    focus() {}
    reset() {}
    write() {}
    dispose() {
      this.disposed = true;
    }
    onData() {
      return { dispose() {} };
    }
  }
  return { FakeTerminal };
});

vi.mock("@xterm/xterm", () => ({ Terminal: FakeTerminal }));
vi.mock("@xterm/addon-fit", () => ({
  FitAddon: class {
    fit() {}
  },
}));
vi.mock("@xterm/addon-search", () => ({
  SearchAddon: class {
    findNext() {}
    findPrevious() {}
    clearDecorations() {}
  },
}));
vi.mock("@/lib/terminalRenderer", () => ({
  activateTerminalRenderer: vi.fn().mockResolvedValue({ renderer: "dom", dispose() {} }),
}));

// The appearance the hook sees, swapped between renders so a test can edit a setting the way the
// Appearance panel does and watch the mounted emulator restyle.
const { appearanceRef } = vi.hoisted(() => ({
  appearanceRef: { current: null as unknown as Appearance },
}));
vi.mock("@/store/appearanceContext", () => ({
  useAppearance: () => ({ appearance: appearanceRef.current, dark: true }),
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

async function settle() {
  await act(async () => {
    await new Promise((resolve) => setTimeout(resolve, 20));
  });
}

function liveTerminal() {
  const term = FakeTerminal.instances.find((instance) => !instance.disposed);
  expect(term, "a mounted emulator").toBeDefined();
  return term as InstanceType<typeof FakeTerminal>;
}

// Mounts a pane, then edits the terminal appearance the way the settings panel does and lets the
// hook react. Returns the emulator that was mounted *before* the edit, so a caller can assert both
// what its options became and that it is still the same instance.
async function mountThenEdit(patch: Partial<Appearance["terminal"]>) {
  appearanceRef.current = DEFAULT_APPEARANCE;
  const view = render(<Probe />);
  await settle();
  const mounted = liveTerminal();

  appearanceRef.current = {
    ...DEFAULT_APPEARANCE,
    terminal: { ...DEFAULT_APPEARANCE.terminal, ...patch },
  };
  await act(async () => {
    view.rerender(<Probe />);
  });
  await settle();

  return mounted;
}

beforeEach(() => {
  FakeTerminal.instances = [];
  appearanceRef.current = DEFAULT_APPEARANCE;
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
  // applied only at construction would leave it untouched and redden every case here.
  const CURSOR_STYLES: CursorStyle[] = ["block", "underline", "bar"];
  it.each(CURSOR_STYLES)(
    "applies cursor style %s to the mounted emulator",
    async (cursor_style) => {
      const term = await mountThenEdit({ cursor_style });
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
      const term = await mountThenEdit({ cursor_inactive_style });
      expect(term.options.cursorInactiveStyle).toBe(cursor_inactive_style);
    },
  );

  it.each([false, true])(
    "applies cursor blink %s to the mounted emulator",
    async (cursor_blink) => {
      const term = await mountThenEdit({ cursor_blink });
      expect(term.options.cursorBlink).toBe(cursor_blink);
    },
  );

  // Restyling must not recreate the terminal: a remount would drop the emulator's scrollback and
  // force a re-attach. Instance identity is the headless half of that guarantee.
  it("restyles the same emulator instance rather than recreating it", async () => {
    const term = await mountThenEdit({ cursor_style: "bar" });

    expect(term.disposed).toBe(false);
    expect(liveTerminal()).toBe(term);
    expect(FakeTerminal.instances).toHaveLength(1);
  });
});
