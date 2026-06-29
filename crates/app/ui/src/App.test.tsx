// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { emit } from "@tauri-apps/api/event";
import { DEFAULT_APPEARANCE } from "@/lib/appearance";
import { DEFAULT_SIDEBAR } from "@/lib/sidebar";
import type { ProcessView } from "@/domain";

// The terminal hook drives the real xterm.js emulator against a measured DOM surface,
// which jsdom cannot provide; the real PTY/echo path is covered by the WebDriver e2e and
// by manual verification. Stubbing it lets the dashboard mount so this test can exercise
// the IPC -> read-model -> render/selection path that is the integration's real risk.
vi.mock("@/components/terminal/useTerminal", () => ({
  useTerminal: () => ({ hostRef: { current: null }, state: "not-started" as const }),
}));

// The persisted read-model cache is the disk boundary (tauri-plugin-store); stub it so the
// dashboard revalidates against the mocked backend from a cold cache, deterministically.
vi.mock("@/store/cache/persistentCache", () => ({
  CacheKey: { projects: "projects", appInfo: "app-info", agents: "agents" },
  readSnapshot: vi.fn(() => Promise.resolve(null)),
  writeSnapshot: vi.fn(() => Promise.resolve()),
}));

import App from "@/App";

const STACK: ProcessView[] = [
  {
    id: 1,
    project: 1,
    kind: "Agent",
    label: "assistant",
    status: "Stopped",
    exit_code: null,
    requires_trust: false,
    resumable: false,
    ports: [],
    ready: "Ungated",
  },
  {
    id: 2,
    project: 1,
    kind: "Terminal",
    label: "shell",
    status: "Running",
    exit_code: null,
    requires_trust: false,
    resumable: false,
    ports: [],
    ready: "Ungated",
  },
  {
    id: 3,
    project: 1,
    kind: "Command",
    label: "build",
    status: "Stopped",
    exit_code: null,
    requires_trust: true,
    resumable: false,
    ports: [],
    ready: "Ungated",
  },
  {
    id: 4,
    project: 1,
    kind: "Command",
    label: "web",
    status: "Running",
    exit_code: null,
    requires_trust: false,
    resumable: false,
    ports: [],
    ready: "Ungated",
  },
];

// The loaded project the fixture stack belongs to. Named distinctly from the app so a
// header assertion can tell the project title apart from the toolbar's app name.
const PROJECT = { id: 1, name: "storefront", root: "/p", icon: null };

// Stand in for the Tauri backend: answer the snapshot/identity/project commands with a
// fixture and let every other command (the event listener, the pty channel) resolve to
// undefined.
function mockBackend(processes: ProcessView[]) {
  mockIPC((cmd) => {
    if (cmd === "app_info") return { name: "soloist", version: "0.1.0" };
    if (cmd === "proc_list") return processes;
    if (cmd === "project_list") return [PROJECT];
    if (cmd === "appearance") return DEFAULT_APPEARANCE;
    if (cmd === "sidebar_settings") return DEFAULT_SIDEBAR;
    if (cmd === "hotkeys") return [];
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

    // The project node titles the tree; its subtype subgroups nest beneath it.
    expect(screen.getByText("storefront")).toBeTruthy();

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

  it("blocks an untrusted command's start and trusts it from the row", async () => {
    let trusted: { project: number; name: string } | null = null;
    mockIPC((cmd, args) => {
      if (cmd === "app_info") return { name: "soloist", version: "0.1.0" };
      if (cmd === "proc_list") return STACK;
      if (cmd === "project_list") return [PROJECT];
      if (cmd === "appearance") return DEFAULT_APPEARANCE;
      if (cmd === "sidebar_settings") return DEFAULT_SIDEBAR;
      if (cmd === "hotkeys") return [];
      if (cmd === "config_trust") {
        trusted = args as { project: number; name: string };
        return undefined;
      }
      return undefined;
    });
    render(<App />);
    await screen.findAllByRole("option");

    // The untrusted command (row 3) cannot start; it offers a trust affordance instead.
    const untrusted = within(row(3));
    expect((untrusted.getByLabelText("Start") as HTMLButtonElement).disabled).toBe(true);

    fireEvent.click(untrusted.getByLabelText("Trust"));
    await waitFor(() => expect(trusted).toEqual({ project: 1, name: "build" }));
  });

  it("pops the trust dialog when a config change needs trust", async () => {
    mockIPC(
      (cmd) => {
        if (cmd === "app_info") return { name: "soloist", version: "0.1.0" };
        if (cmd === "proc_list") return STACK;
        if (cmd === "project_list") return [PROJECT];
        if (cmd === "appearance") return DEFAULT_APPEARANCE;
        if (cmd === "sidebar_settings") return DEFAULT_SIDEBAR;
        if (cmd === "hotkeys") return [];
        return undefined;
      },
      { shouldMockEvents: true },
    );
    render(<App />);
    await screen.findAllByRole("option");
    // Let the trust listener register before emitting — events have no replay.
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 0));
    });

    await act(async () => {
      await emit("domain-event", {
        type: "ConfigChanged",
        project: 1,
        requires_trust: true,
        diff: { added: ["Api"], updated: [], removed: [], renamed: [] },
        commands: [{ name: "Api", command: "cargo run", working_dir: null, env: {} }],
      });
    });

    expect(screen.getByText("Trust changed commands")).toBeTruthy();
    expect(screen.getByText("cargo run")).toBeTruthy();
  });
});
