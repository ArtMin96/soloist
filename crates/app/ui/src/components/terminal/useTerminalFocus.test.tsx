// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, render } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { DEFAULT_APPEARANCE } from "@/lib/appearance";
import { FakeTerminal } from "@/test/fakeTerminal";
import type { Appearance, ProcessView } from "@/domain";

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

function Probe({ visible }: { visible: boolean }) {
  const { hostRef } = useTerminal(PROCESS, visible);
  return <div ref={hostRef} />;
}

async function settle() {
  await act(async () => {
    await new Promise((resolve) => setTimeout(resolve, 20));
  });
}

function withFocusOnClick(focus_on_click: boolean) {
  appearanceRef.current = {
    ...DEFAULT_APPEARANCE,
    terminal: { ...DEFAULT_APPEARANCE.terminal, focus_on_click },
  };
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

describe("focus on click", () => {
  it("hands a new pane the keyboard focus while on", async () => {
    withFocusOnClick(true);
    render(<Probe visible />);
    await settle();

    expect(FakeTerminal.live().focused).toBe(true);
  });

  it("leaves focus where it was for a new pane while off", async () => {
    // Off, opening a process shows its output without stealing the keystrokes the user is aiming
    // somewhere else; a click into the terminal is what starts typing into it.
    withFocusOnClick(false);
    render(<Probe visible />);
    await settle();

    expect(FakeTerminal.live().focused).toBe(false);
  });

  it("focuses the pane again when it is switched back to, while on", async () => {
    // A pooled pane stays mounted while hidden, so switching back to it never re-runs creation —
    // the show path has to honor the setting on its own or the switch only works the first time.
    withFocusOnClick(true);
    const view = render(<Probe visible />);
    await settle();
    const term = FakeTerminal.live();

    term.focused = false;
    await act(async () => view.rerender(<Probe visible={false} />));
    await act(async () => view.rerender(<Probe visible />));
    await settle();

    expect(term.focused).toBe(true);
  });

  it("leaves focus alone when switching back to a pane while off", async () => {
    withFocusOnClick(false);
    const view = render(<Probe visible />);
    await settle();
    const term = FakeTerminal.live();

    await act(async () => view.rerender(<Probe visible={false} />));
    await act(async () => view.rerender(<Probe visible />));
    await settle();

    expect(term.focused).toBe(false);
  });

  it("follows the setting after it is toggled on a mounted pane", async () => {
    // The setting is read live, not captured at mount: turning it off has to reach the pane the
    // user is already looking at, not only the next one they open.
    withFocusOnClick(true);
    const view = render(<Probe visible />);
    await settle();
    const term = FakeTerminal.live();

    withFocusOnClick(false);
    term.focused = false;
    await act(async () => view.rerender(<Probe visible={false} />));
    await act(async () => view.rerender(<Probe visible />));
    await settle();

    expect(term.focused).toBe(false);
  });
});
