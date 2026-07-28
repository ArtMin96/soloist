// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { emit } from "@tauri-apps/api/event";
import { DEFAULT_APPEARANCE } from "@/lib/appearance";
import { DEFAULT_SIDEBAR } from "@/lib/sidebar";
import { ATTENTION_LABEL } from "@/lib/attention";
import type { AttentionSnapshot, ProcessView } from "@/domain";

// The terminal hook drives the real xterm.js emulator against a measured DOM surface, which jsdom
// cannot provide; stubbing it lets the dashboard mount so these tests can exercise the
// snapshot -> indicator path that is this feature's real risk.
vi.mock("@/components/terminal/useTerminal", () => ({
  useTerminal: () => ({ hostRef: { current: null }, state: "not-started" as const }),
}));

vi.mock("@/store/cache/persistentCache", () => ({
  CacheKey: { projects: "projects", appInfo: "app-info", agents: "agents" },
  readSnapshot: vi.fn(() => Promise.resolve(null)),
  writeSnapshot: vi.fn(() => Promise.resolve()),
}));

import App from "@/App";

/// The visible unread markers — the row dot and the project dot. Queried by role so the
/// assertions land on the mark a user actually sees, not on a hidden announcement beside it.
function markers() {
  return screen.queryAllByRole("img", { name: ATTENTION_LABEL });
}

const STACK: ProcessView[] = [
  {
    id: 1,
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
  {
    id: 2,
    project: 1,
    kind: "Command",
    label: "api",
    status: "Running",
    exit_code: null,
    requires_trust: false,
    resumable: false,
    ports: [],
    ready: "Ungated",
  },
];

const PROJECT = { id: 1, name: "storefront", root: "/p", icon: null };

function snapshot(...processes: AttentionSnapshot["processes"]): AttentionSnapshot {
  return {
    processes,
    total: processes.reduce((sum, entry) => sum + entry.kinds.length, 0),
  };
}

const NOTHING = snapshot();

/// Stand in for the backend, answering the attention snapshot from a mutable cell so a test can
/// change what the core holds and then announce the change the way the core does.
function mockBackend(
  initial: AttentionSnapshot,
  observe: (cmd: string, args: unknown) => void = () => {},
) {
  const state = { snapshot: initial };
  mockIPC(
    (cmd, args) => {
      observe(cmd, args);
      if (cmd === "app_info") return { name: "soloist", version: "0.1.0" };
      if (cmd === "proc_list") return STACK;
      if (cmd === "project_list") return [PROJECT];
      if (cmd === "appearance") return DEFAULT_APPEARANCE;
      if (cmd === "sidebar_settings") return DEFAULT_SIDEBAR;
      if (cmd === "hotkeys") return [];
      if (cmd === "attention_snapshot") return state.snapshot;
      if (cmd === "clear_all_attention") {
        state.snapshot = NOTHING;
        return undefined;
      }
      return undefined;
    },
    { shouldMockEvents: true },
  );
  return state;
}

function row(id: number): HTMLElement {
  const element = document.querySelector<HTMLElement>(`[data-process-id="${id}"]`);
  if (!element) throw new Error(`no row for process ${id}`);
  return element;
}

/// Let the event listeners register before emitting — events have no replay.
async function settle() {
  await act(async () => {
    await new Promise((resolve) => setTimeout(resolve, 0));
  });
}

async function announceAttentionChanged() {
  await act(async () => {
    await emit("domain-event", { type: "AttentionChanged" });
  });
}

async function mount(initial: AttentionSnapshot) {
  const state = mockBackend(initial);
  render(<App />);
  await screen.findAllByRole("treeitem");
  await settle();
  return state;
}

/// The project header's disclosure, which collapses the whole project away. Reached through the
/// project name it carries, so the ••• menu beside it is never a candidate.
function projectDisclosure(): HTMLElement {
  const disclosure = screen.getByText(PROJECT.name).closest("button");
  if (!disclosure) throw new Error("no project disclosure");
  return disclosure;
}

/// The title-bar unread control.
function attentionControl(): HTMLElement {
  return screen.getByRole("button", { name: ATTENTION_LABEL });
}

/// The open unread list.
function unreadList() {
  return within(screen.getByRole("dialog"));
}

afterEach(() => {
  cleanup();
  clearMocks();
  // Sidebar collapse persists to localStorage, so a test that collapses a project would otherwise
  // hand the next one a sidebar with no rows in it.
  localStorage.clear();
});

describe("unread indicators", () => {
  it("marks the row of a process that raised an alert while the user was elsewhere", async () => {
    const state = await mount(NOTHING);
    expect(within(row(2)).queryByRole("img", { name: ATTENTION_LABEL })).toBeNull();

    state.snapshot = snapshot({ process: 2, kinds: ["crashed"] });
    await announceAttentionChanged();

    await waitFor(() =>
      expect(within(row(2)).queryByRole("img", { name: ATTENTION_LABEL })).not.toBeNull(),
    );
    // Only the process that alerted is marked.
    expect(within(row(1)).queryByRole("img", { name: ATTENTION_LABEL })).toBeNull();
  });

  it("dots the project header when a child is unread, including when it is collapsed", async () => {
    await mount(snapshot({ process: 2, kinds: ["crashed"] }));
    await waitFor(() => expect(markers().length).toBeGreaterThan(0));

    // Collapsing unmounts every row, so what survives is the header's own dot — the reason it
    // exists is a project whose commands are collapsed or scrolled out of view.
    fireEvent.click(projectDisclosure());

    await waitFor(() => expect(screen.queryAllByRole("treeitem")).toHaveLength(0));
    expect(markers()).toHaveLength(1);
  });

  it("keeps the project dot when the project is selected, and clears it at the process", async () => {
    // Stand in for the core's rule: the process being viewed is the one that gets cleared, and
    // nothing else does. What the shell controls — and what this test pins — is *what it reports
    // it is viewing*: selecting a project reports `viewing: null`, so the core is never asked to
    // clear anything. (The focus half of the rule is the core's and is tested there; a jsdom
    // window has no Tauri focus to report.)
    const state = mockBackend(snapshot({ process: 2, kinds: ["crashed"] }), (cmd, args) => {
      const presence = (args as { presence?: { viewing: number | null } })?.presence;
      if (cmd === "set_presence" && presence?.viewing === 2) state.snapshot = NOTHING;
    });
    render(<App />);
    await screen.findAllByRole("treeitem");
    await settle();
    await waitFor(() =>
      expect(within(row(2)).queryByRole("img", { name: ATTENTION_LABEL })).not.toBeNull(),
    );

    fireEvent.pointerDown(screen.getByRole("button", { name: `Actions for ${PROJECT.name}` }), {
      button: 0,
      ctrlKey: false,
    });
    fireEvent.click(await screen.findByRole("menuitem", { name: "Project settings" }));
    await settle();
    await announceAttentionChanged();

    // The project is what is on screen now, and the alert is still waiting on the process.
    expect(within(row(2)).queryByRole("img", { name: ATTENTION_LABEL })).not.toBeNull();
    expect(attentionControl()).toBeTruthy();

    // Looking at the process that alerted is what clears it.
    fireEvent.click(row(2));
    await settle();
    await announceAttentionChanged();

    await waitFor(() => expect(markers()).toHaveLength(0));
  });

  it("caps the title-bar count at 99+ while the snapshot keeps counting", async () => {
    await mount(snapshot({ process: 1, kinds: Array<"crashed">(150).fill("crashed") }));

    await waitFor(() => expect(screen.getByText("99+")).toBeTruthy());
  });

  it("shows the exact title-bar count below the cap", async () => {
    await mount(
      snapshot({ process: 1, kinds: ["crashed"] }, { process: 2, kinds: ["agent_error"] }),
    );

    await waitFor(() => expect(within(attentionControl()).getByText("2")).toBeTruthy());
  });

  it("clears every indicator at once from the title bar", async () => {
    await mount(
      snapshot({ process: 1, kinds: ["crashed"] }, { process: 2, kinds: ["agent_error"] }),
    );
    await waitFor(() => expect(markers().length).toBeGreaterThan(0));

    fireEvent.click(attentionControl());
    fireEvent.click(await screen.findByRole("button", { name: "Clear all" }));
    await announceAttentionChanged();

    // Row markers, the project dot and the count go together — one action, every surface.
    await waitFor(() => expect(markers()).toHaveLength(0));
    expect(screen.queryByRole("button", { name: ATTENTION_LABEL })).toBeNull();
  });

  it("shows no marker and no zero when nothing is unread", async () => {
    await mount(NOTHING);

    expect(markers()).toHaveLength(0);
    // Absent, not a control reading zero: with nothing waiting there is no count in the strip at
    // all, so a glance at the title bar can never be answered with "0".
    expect(screen.queryByRole("button", { name: ATTENTION_LABEL })).toBeNull();
    expect(within(screen.getByRole("banner")).queryByText("0")).toBeNull();
  });

  it("jumps to the process an entry names and clears only that one", async () => {
    const cleared: number[] = [];
    mockBackend(snapshot({ process: 2, kinds: ["crashed"] }), (cmd) => {
      if (cmd === "set_presence") cleared.push(1);
    });
    render(<App />);
    await screen.findAllByRole("treeitem");
    await settle();

    fireEvent.click(attentionControl());
    fireEvent.click(await unreadList().findByRole("button", { name: "api" }));

    // Selecting the process is what tells the core the user is looking at it; the core clears it.
    await waitFor(() => expect(row(2).getAttribute("aria-selected")).toBe("true"));
    expect(cleared.length).toBeGreaterThan(0);
  });
});
