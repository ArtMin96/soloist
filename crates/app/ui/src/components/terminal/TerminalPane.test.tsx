// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, render, screen } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { emit } from "@tauri-apps/api/event";
import { TooltipProvider } from "@/components/ui/tooltip";
import type { ProcessActionHandlers } from "@/lib/processActions";
import type { ProcessView, SessionWork } from "@/domain";

// The emulator hook drives xterm.js against a measured surface jsdom can't provide; stub
// it so the pane mounts and this test can exercise the title/bell chrome from real events.
vi.mock("@/components/terminal/useTerminal", () => ({
  useTerminal: () => ({ hostRef: { current: null }, state: "live" as const }),
}));

// The session-work read model reaches IPC; stubbed here so the header wiring — which arguments
// reach the hook, and what the pane does with its result — is exercised without a Tauri runtime.
const useSessionWorkMock =
  vi.fn<
    (process: number, enabled: boolean) => { work: SessionWork | null; error: string | null }
  >();
vi.mock("@/store/useSessionWork", () => ({
  useSessionWork: (process: number, enabled: boolean) => useSessionWorkMock(process, enabled),
}));

import { TerminalPane } from "@/components/terminal/TerminalPane";

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

const noop = () => {};

const NOOP_HANDLERS: ProcessActionHandlers = {
  onTrust: noop,
  onResume: noop,
  onStart: noop,
  onStop: noop,
  onRestart: noop,
  onRemove: noop,
};

function renderPane() {
  render(
    <TooltipProvider>
      <TerminalPane process={PROCESS} handlers={NOOP_HANDLERS} />
    </TooltipProvider>,
  );
}

// Let the listener registered in the pane's effect resolve before emitting (events have
// no replay, so emitting too early would be missed).
async function flush() {
  await act(async () => {
    await new Promise((resolve) => setTimeout(resolve, 0));
  });
}

afterEach(() => {
  cleanup();
  clearMocks();
  useSessionWorkMock.mockReset();
  useSessionWorkMock.mockReturnValue({ work: null, error: null });
});

// A default so every test not concerned with session work never sees `work` as `undefined`.
useSessionWorkMock.mockReturnValue({ work: null, error: null });

describe("TerminalPane Trust control", () => {
  const UNTRUSTED: ProcessView = {
    id: 8,
    project: 3,
    kind: "Command",
    label: "build",
    status: "Stopped",
    exit_code: null,
    requires_trust: true,
    resumable: false,
    ports: [],
    ready: "Ungated",
  };

  // The header carries the process's project and name; Trust needs both to open the right
  // review. This is the same contract `runFor` dispatches in lib/processActions.
  it("passes the process's project and name to Trust, not just its id", async () => {
    mockIPC(() => {}, { shouldMockEvents: true });
    const onTrust = vi.fn();
    render(
      <TooltipProvider>
        <TerminalPane process={UNTRUSTED} handlers={{ ...NOOP_HANDLERS, onTrust }} />
      </TooltipProvider>,
    );
    await flush();

    screen.getByLabelText("Trust").click();
    expect(onTrust).toHaveBeenCalledWith(UNTRUSTED.project, UNTRUSTED.label);
  });
});

describe("TerminalPane chrome", () => {
  it("shows the label until an OSC title arrives, then the title", async () => {
    mockIPC(() => {}, { shouldMockEvents: true });
    renderPane();
    await flush();
    expect(screen.getByText("assistant")).toBeTruthy();

    await act(async () => {
      await emit("domain-event", {
        type: "TerminalTitleChanged",
        id: 7,
        title: "claude — working",
      });
    });
    expect(screen.getByText("claude — working")).toBeTruthy();
    expect(screen.queryByText("assistant")).toBeNull();
  });

  it("ignores a title meant for a different process", async () => {
    mockIPC(() => {}, { shouldMockEvents: true });
    renderPane();
    await flush();

    await act(async () => {
      await emit("domain-event", { type: "TerminalTitleChanged", id: 99, title: "other" });
    });
    expect(screen.getByText("assistant")).toBeTruthy();
  });

  it("raises a bell indicator when the process rings the bell", async () => {
    mockIPC(() => {}, { shouldMockEvents: true });
    renderPane();
    await flush();
    expect(screen.queryByLabelText("Terminal bell")).toBeNull();

    await act(async () => {
      await emit("domain-event", { type: "TerminalBell", id: 7 });
    });
    expect(screen.getByLabelText("Terminal bell")).toBeTruthy();
  });
});

const SESSION_WORK: SessionWork = {
  process: 7,
  project: 1,
  todos: [
    { id: 1, title: "held todo", status: "open", blocked: false, locked: true, access: "worked" },
  ],
  scratchpads: [],
};

// Mirrors the real hook's contract (`work` is `null` while disabled) rather than a fixed return,
// so a test here proves the pane passes the right `enabled` flag through — not just that the bar
// renders when told to.
function stubSessionWork(work: SessionWork) {
  useSessionWorkMock.mockImplementation((_process, enabled) => ({
    work: enabled ? work : null,
    error: null,
  }));
}

describe("TerminalPane session work", () => {
  it("shows the session-work bar for an Agent process", () => {
    stubSessionWork(SESSION_WORK);
    renderPane();
    expect(document.querySelector("[data-session-work]")).not.toBeNull();
  });

  it("shows nothing for a Command or a Terminal process", () => {
    stubSessionWork(SESSION_WORK);
    const command: ProcessView = { ...PROCESS, id: 20, kind: "Command" };
    const terminal: ProcessView = { ...PROCESS, id: 21, kind: "Terminal" };

    const { rerender } = render(
      <TooltipProvider>
        <TerminalPane process={command} handlers={NOOP_HANDLERS} />
      </TooltipProvider>,
    );
    expect(document.querySelector("[data-session-work]")).toBeNull();

    rerender(
      <TooltipProvider>
        <TerminalPane process={terminal} handlers={NOOP_HANDLERS} />
      </TooltipProvider>,
    );
    expect(document.querySelector("[data-session-work]")).toBeNull();
  });

  it("passes enabled: false to the hook for a hidden pooled pane", () => {
    stubSessionWork(SESSION_WORK);
    render(
      <TooltipProvider>
        <TerminalPane process={PROCESS} visible={false} handlers={NOOP_HANDLERS} />
      </TooltipProvider>,
    );
    expect(document.querySelector("[data-session-work]")).toBeNull();
    expect(useSessionWorkMock).toHaveBeenCalledWith(PROCESS.id, false);
  });

  it("keeps the process controls rendered when the bar is full", () => {
    stubSessionWork({
      process: 7,
      project: 1,
      todos: Array.from({ length: 10 }, (_, index) => ({
        id: index,
        title: `todo ${index} with a fairly long title to fill the header row`,
        status: "open",
        blocked: false,
        locked: true,
        access: "worked",
      })),
      scratchpads: [],
    });
    renderPane();
    expect(screen.getByLabelText("Stop")).toBeTruthy();
  });
});
