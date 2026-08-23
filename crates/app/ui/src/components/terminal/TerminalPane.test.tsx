// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, render, screen } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { emit } from "@tauri-apps/api/event";
import { TooltipProvider } from "@/components/ui/tooltip";
import type { ProcessActionHandlers } from "@/lib/processActions";
import type { ProcessView } from "@/domain";

// The emulator hook drives xterm.js against a measured surface jsdom can't provide; stub
// it so the pane mounts and this test can exercise the title/bell chrome from real events.
vi.mock("@/components/terminal/useTerminal", () => ({
  useTerminal: () => ({ hostRef: { current: null }, state: "live" as const }),
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
});

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
