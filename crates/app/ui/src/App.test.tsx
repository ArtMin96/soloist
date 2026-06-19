// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type { ProcessView } from "@/domain";

// The terminal hook drives the real xterm.js emulator against a measured DOM surface,
// which jsdom cannot provide; the real PTY/echo path is covered by the WebDriver e2e and
// by manual verification. Stubbing it lets the dashboard mount so this test can exercise
// the IPC -> read-model -> render/selection path that is the integration's real risk.
vi.mock("@/components/terminal/useTerminal", () => ({
  useTerminal: () => ({ hostRef: { current: null }, state: "not-started" as const }),
}));

import App from "@/App";

const STACK: ProcessView[] = [
  { id: 1, project: 1, kind: "Agent", label: "assistant", status: "Stopped", exit_code: null },
  { id: 2, project: 1, kind: "Terminal", label: "shell", status: "Running", exit_code: null },
  { id: 3, project: 1, kind: "Command", label: "build", status: "Stopped", exit_code: null },
  { id: 4, project: 1, kind: "Command", label: "web", status: "Running", exit_code: null },
];

// Stand in for the Tauri backend: answer the snapshot/identity commands with a fixture and
// let every other command (the event listener, the pty channel) resolve to undefined.
function mockBackend(processes: ProcessView[]) {
  mockIPC((cmd) => {
    if (cmd === "app_info") return { name: "soloist", version: "0.1.0" };
    if (cmd === "proc_list") return processes;
    return undefined;
  });
}

function row(id: number): HTMLElement {
  const element = document.querySelector<HTMLElement>(`[data-process-id="${id}"]`);
  if (!element) throw new Error(`no row for process ${id}`);
  return element;
}

afterEach(() => {
  cleanup();
  clearMocks();
});

describe("App dashboard", () => {
  it("renders the stack grouped by subtype with a status per row", async () => {
    mockBackend(STACK);
    render(<App />);

    const rows = await screen.findAllByRole("option");
    expect(rows).toHaveLength(4);

    // The three subtype groups are present as sentence-case headers.
    expect(screen.getByText("Agents")).toBeTruthy();
    expect(screen.getByText("Terminals")).toBeTruthy();
    expect(screen.getByText("Commands")).toBeTruthy();

    // Status is read from the value, not scraped text.
    expect(within(row(1)).getByText("assistant")).toBeTruthy();
    expect(row(1).querySelector("[data-status]")?.getAttribute("data-status")).toBe("Stopped");
    expect(row(2).querySelector("[data-status]")?.getAttribute("data-status")).toBe("Running");
  });

  it("derives control enabled-state from the status FSM", async () => {
    mockBackend(STACK);
    render(<App />);
    await screen.findAllByRole("option");

    // A stopped process can start, not stop; a running one is the inverse.
    const stopped = within(row(1));
    expect((stopped.getByLabelText("Start") as HTMLButtonElement).disabled).toBe(false);
    expect((stopped.getByLabelText("Stop") as HTMLButtonElement).disabled).toBe(true);

    const running = within(row(2));
    expect((running.getByLabelText("Start") as HTMLButtonElement).disabled).toBe(true);
    expect((running.getByLabelText("Stop") as HTMLButtonElement).disabled).toBe(false);
  });

  it("selects a process and opens its terminal pane", async () => {
    mockBackend(STACK);
    render(<App />);
    await screen.findAllByRole("option");

    // With a populated stack but nothing selected, the pane guides the next action.
    expect(screen.getByText(/Select a process in the sidebar/)).toBeTruthy();
    expect(row(1).getAttribute("aria-selected")).toBe("false");

    fireEvent.click(row(1));

    expect(row(1).getAttribute("aria-selected")).toBe("true");
    // The terminal pane mounts: the label now appears in both the row and the pane header.
    expect(screen.getAllByText("assistant").length).toBe(2);
    expect(screen.queryByText(/Select a process in the sidebar/)).toBeNull();
  });

  it("shows the no-config empty state when the stack is empty", async () => {
    mockBackend([]);
    render(<App />);

    await waitFor(() => {
      expect(screen.getByText(/No project loaded/)).toBeTruthy();
    });
    expect(screen.queryAllByRole("option")).toHaveLength(0);
  });
});
