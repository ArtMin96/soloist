// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";

// The orchestration read is the pane's own IPC; mocking it lets a test hold the project's first
// snapshot in flight, which is the moment the pane has to stand in for rather than paint as a
// settled, empty board.
vi.mock("@/api", () => ({
  orchestrationSnapshot: vi.fn(),
  onDomainEvent: vi.fn(() => Promise.resolve(() => {})),
  onResync: vi.fn(() => Promise.resolve(() => {})),
}));

// The to-do board's own hooks are the only other IPC on this surface; stubbing them keeps the test
// on what the pane shows while its read is in flight rather than on writes, which the board's and
// the hooks' own suites cover. Each stub is typed as the store it stands in for, so a member added
// to the real hook fails the typecheck here instead of leaving the board wired to a shape the app
// no longer has.
vi.mock("@/store/useTodoActions", () => ({
  useTodoActions: (): TodoActionsStore => ({
    busyId: null,
    errorById: {},
    complete: vi.fn(),
    copyLink: vi.fn(),
    comment: vi.fn(),
    clearError: vi.fn(),
  }),
}));

vi.mock("@/store/useTodoEditor", () => ({
  useTodoEditor: (): TodoEditorStore => ({
    mode: null,
    editingId: null,
    initial: null,
    scratchpad: null,
    baseRevision: null,
    mountKey: 0,
    error: null,
    startCreate: vi.fn(),
    editTodo: vi.fn(),
    close: vi.fn(),
    save: vi.fn(),
    reload: vi.fn(),
  }),
}));

import { orchestrationSnapshot } from "@/api";
import { OrchestrationPane } from "@/components/orchestration/OrchestrationPane";
import { TooltipProvider } from "@/components/ui/tooltip";
import type { OrchestrationSnapshot, ProjectView, TimerView, TodoView } from "@/domain";
import type { TodoActionsStore } from "@/store/useTodoActions";
import type { TodoEditorStore } from "@/store/useTodoEditor";
import { holdRead } from "@/test/heldRead";

const read = vi.mocked(orchestrationSnapshot);

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

const project: ProjectView = { id: 1, name: "storefront", root: "/p", icon: null };

const todo: TodoView = {
  id: 1,
  doc: { title: "Ship the release", body: "", status: "open" },
  tags: [],
  blockers: [],
  blocked_by: [],
  blocked: false,
  comments: [],
  locked_by: null,
  scratchpad: null,
  revision: 1,
};

function timer(id: number): TimerView {
  return {
    id,
    owner: 1,
    body: "check the build",
    fire: { kind: "at" },
    status: "armed",
    deadline_unix_millis: 1_700_000_000_000,
    waiting_on: [],
    already_idle: false,
    paused_remaining_millis: null,
  };
}

function snapshot(overrides: Partial<OrchestrationSnapshot> = {}): OrchestrationSnapshot {
  return {
    project: project.id,
    agents: [],
    todos: [],
    timers: [],
    leases: [],
    scratchpads: [],
    diagrams: [],
    kv: [],
    messages: [],
    ...overrides,
  };
}

// The Copy link and overflow controls inside the board are Tooltip triggers, which need a provider
// ancestor — supplied here as the app supplies one once at its root.
function pane() {
  return render(
    <TooltipProvider>
      <OrchestrationPane project={project} />
    </TooltipProvider>,
  );
}

/** Switches views the way a reader does — through the pane's segmented control. */
function showTodos() {
  fireEvent.click(screen.getByRole("radio", { name: "To-dos" }));
}

function timersOption(): HTMLElement {
  return screen.getByRole("radio", { name: /Timers/ });
}

describe("OrchestrationPane", () => {
  it("shows the to-do stand-in, not an empty state, while the first snapshot is in flight", () => {
    holdRead(read);
    pane();

    showTodos();

    // An unread board and a board with no work look nothing alike to a reader: the empty state is a
    // statement about the project, and it must never be made on the strength of a pending read.
    expect(screen.queryByText("No todos yet")).toBeNull();
    const region = screen.getByRole("status");
    expect(region.getAttribute("aria-busy")).toBe("true");
    expect(region.textContent).toContain("Loading to-dos");
  });

  it("renders the todos once the snapshot lands and no longer reports loading", async () => {
    read.mockResolvedValue(snapshot({ todos: [todo] }));
    pane();

    showTodos();

    expect(await screen.findByText("Ship the release")).toBeTruthy();
    expect(screen.queryByRole("status")).toBeNull();
  });

  it("offers a retry that re-reads when the first snapshot cannot be read", async () => {
    read.mockRejectedValueOnce(new Error("db locked"));
    read.mockResolvedValue(snapshot({ todos: [todo] }));
    pane();

    showTodos();

    expect(await screen.findByText("Could not load to-dos.")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Try again" }));

    // The retry is only worth anything if the read it runs reaches the board: the todo arriving is
    // the whole of the recovery.
    expect(await screen.findByText("Ship the release")).toBeTruthy();
    expect(screen.queryByText("Could not load to-dos.")).toBeNull();
  });

  it("keeps the timer count off the view switcher until the snapshot is ready", async () => {
    const settle = holdRead(read);
    pane();

    // A count is a fact about the project; there is no fact to state until the read answers.
    expect(within(timersOption()).queryByText("2")).toBeNull();

    settle(snapshot({ timers: [timer(1), timer(2)] }));

    await waitFor(() => expect(within(timersOption()).getByText("2")).toBeTruthy());
  });
});
