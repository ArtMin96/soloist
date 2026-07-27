// @vitest-environment jsdom
import { useRef } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, render, screen } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { DEFAULT_APPEARANCE } from "@/lib/appearance";
import { FakeTerminal } from "@/test/fakeTerminal";
import type { Appearance, Binding, HotkeyBindingView, ProcessView } from "@/domain";

vi.mock("@xterm/xterm", async () => ({
  Terminal: (await import("@/test/fakeTerminal")).FakeTerminal,
}));
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

// The system clipboard the plugin commands reach, holding text so an assertion reads what ended up
// on it rather than whether a write was attempted.
const { system } = vi.hoisted(() => ({ system: { text: "" } }));
vi.mock("@tauri-apps/plugin-clipboard-manager", () => ({
  writeText: vi.fn(async (text: string) => {
    system.text = text;
  }),
  readText: vi.fn(async () => system.text),
}));

// The appearance the hooks see, swapped between renders so a test can edit a setting the way the
// Appearance panel does and watch an already-mounted pane react.
const { appearanceRef } = vi.hoisted(() => ({
  appearanceRef: { current: null as unknown as Appearance },
}));
vi.mock("@/store/appearanceContext", () => ({
  useAppearance: () => ({
    appearance: appearanceRef.current,
    dark: true,
    setAppearance: (next: Appearance) => {
      appearanceRef.current = next;
    },
  }),
}));

// The keymap the pane dispatches against. The chords mirror the core's code-defined defaults; the
// core owns them, and a headless pane has no IPC to load them over.
const { TERMINAL_KEYMAP } = vi.hoisted(() => {
  const chord = (key: string, shift: boolean): Binding => ({
    ctrl: true,
    alt: false,
    shift,
    super: false,
    key,
  });
  return {
    TERMINAL_KEYMAP: [
      {
        action: "copy_selection",
        scope: "terminal",
        binding: chord("C", true),
        is_default: true,
        conflict: false,
      },
      {
        action: "paste_clipboard",
        scope: "terminal",
        binding: chord("V", true),
        is_default: true,
        conflict: false,
      },
    ] as HotkeyBindingView[],
  };
});
vi.mock("@/store/hotkeysContext", () => ({
  useHotkeys: () => ({
    bindings: TERMINAL_KEYMAP,
    remap: () => {},
    disable: () => {},
    reset: () => {},
    resetAll: () => {},
  }),
}));

import { useTerminal } from "@/components/terminal/useTerminal";
import { useTerminalHotkeys } from "@/components/terminal/useTerminalHotkeys";

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

// The pane as `TerminalPane` composes it: the emulator's host inside the element whose
// capture-phase listener claims terminal-scope chords before the emulator can see them.
function Probe() {
  const paneRef = useRef<HTMLElement>(null);
  const { hostRef, clipboard } = useTerminal(PROCESS);
  useTerminalHotkeys({
    containerRef: paneRef,
    processes: [PROCESS],
    processId: PROCESS.id,
    clipboard,
  });
  return (
    <section ref={paneRef}>
      <div ref={hostRef} data-testid="terminal-host" />
    </section>
  );
}

/** Seed the system clipboard with what the user had copied before the test acts. */
function stubClipboard(initial = "") {
  system.text = initial;
  return system;
}

async function settle() {
  await act(async () => {
    await new Promise((resolve) => setTimeout(resolve, 20));
  });
}

async function mountPane() {
  render(<Probe />);
  await settle();
  return FakeTerminal.live();
}

function host(): HTMLElement {
  return screen.getByTestId("terminal-host");
}

// Keys that survive the pane's capture-phase interception reach the emulator, which is what hands
// them to the PTY. A chord the pane claims never arrives here.
function recordKeysReachingTheEmulator(): KeyboardEvent[] {
  const reached: KeyboardEvent[] = [];
  host().addEventListener("keydown", (event) => reached.push(event as KeyboardEvent));
  return reached;
}

// A key press as a keyboard really emits it: `key` carries the shifted character, so a caller
// passes "c" for Ctrl+C and "C" for Ctrl+Shift+C. The chord builder normalizes case, so what
// actually separates the two is the Shift flag — which is exactly what must not be ignored.
function press(key: string, { shift = false }: { shift?: boolean } = {}) {
  act(() => {
    host().dispatchEvent(
      new KeyboardEvent("keydown", {
        key,
        ctrlKey: true,
        shiftKey: shift,
        bubbles: true,
        cancelable: true,
      }),
    );
  });
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
  system.text = "";
});

describe("terminal copy and paste hotkeys", () => {
  it("copies exactly the selection", async () => {
    const clipboard = stubClipboard();
    const term = await mountPane();
    term.select("npm run dev");

    press("C", { shift: true });
    await settle();

    expect(clipboard.text).toBe("npm run dev");
  });

  it("leaves the clipboard alone when nothing is selected", async () => {
    // An empty write would replace whatever the user had copied with a blank — worse than the
    // shortcut doing nothing at all.
    const clipboard = stubClipboard("something the user copied earlier");
    await mountPane();

    press("C", { shift: true });
    await settle();

    expect(clipboard.text).toBe("something the user copied earlier");
  });

  it("pastes through the emulator, so bracketed-paste mode is honored", async () => {
    // Going through `term.paste` is what applies newline normalization and the bracketed-paste
    // markers; writing to the PTY directly would skip both.
    stubClipboard("echo hello");
    const term = await mountPane();

    press("V", { shift: true });
    await settle();

    expect(term.pasted).toEqual(["echo hello"]);
  });

  it("leaves Ctrl+C to the process on the PTY", async () => {
    // Swallowing this would make every interrupt impossible — the regression that would render the
    // terminal unusable.
    const clipboard = stubClipboard("untouched");
    const term = await mountPane();
    term.select("some output");
    const reached = recordKeysReachingTheEmulator();

    press("c");
    await settle();

    expect(reached.map((event) => event.key)).toEqual(["c"]);
    expect(clipboard.text).toBe("untouched");
  });

  it("leaves Ctrl+V to the process on the PTY", async () => {
    stubClipboard("clipboard contents");
    const term = await mountPane();
    const reached = recordKeysReachingTheEmulator();

    press("v");
    await settle();

    expect(reached.map((event) => event.key)).toEqual(["v"]);
    expect(term.pasted).toEqual([]);
  });

  it("claims the copy chord instead of forwarding it to the PTY", async () => {
    stubClipboard();
    const term = await mountPane();
    term.select("output");
    const reached = recordKeysReachingTheEmulator();

    press("C", { shift: true });
    await settle();

    expect(reached).toEqual([]);
  });
});

describe("copy on select", () => {
  // The setting is read live rather than captured when the pane mounts, so these edit it on an
  // already-mounted emulator: wiring that only consulted the setting at construction would leave
  // the pane the user is looking at behaving as it did before the switch moved.
  async function mountThenSet(copy_on_select: boolean) {
    const view = render(<Probe />);
    await settle();
    const term = FakeTerminal.live();

    appearanceRef.current = {
      ...DEFAULT_APPEARANCE,
      terminal: { ...DEFAULT_APPEARANCE.terminal, copy_on_select },
    };
    await act(async () => {
      view.rerender(<Probe />);
    });
    await settle();

    return term;
  }

  it("does not copy a selection while off", async () => {
    const clipboard = stubClipboard("untouched");
    const term = await mountThenSet(false);

    term.select("a selected line");
    await settle();

    expect(clipboard.text).toBe("untouched");
  });

  it("copies a selection as it is made while on", async () => {
    const clipboard = stubClipboard();
    const term = await mountThenSet(true);

    term.select("a selected line");
    await settle();

    expect(clipboard.text).toBe("a selected line");
  });

  it("keeps the last copy when the selection is cleared", async () => {
    // Clearing a selection fires the same event; copying then would blank the clipboard.
    const clipboard = stubClipboard();
    const term = await mountThenSet(true);
    term.select("a selected line");
    await settle();

    term.select("");
    await settle();

    expect(clipboard.text).toBe("a selected line");
  });
});
