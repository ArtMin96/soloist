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

// The scratchpad board's hooks are the pane's remaining IPC. The `@/api` factory above throws on any
// export it does not name, so the boards' hooks are stubbed rather than the api mock extended.
vi.mock("@/store/useScratchpadActions", () => ({
  useScratchpadActions: (): ScratchpadActionsStore => ({
    error: null,
    createError: null,
    create: vi.fn(),
    archive: vi.fn(),
    exportMarkdown: vi.fn(),
    copyMarkdown: vi.fn(),
    clearError: vi.fn(),
  }),
}));

vi.mock("@/store/useScratchpadEditor", () => ({
  useScratchpadEditor: (): ScratchpadEditorStore => ({
    name: null,
    document: loading(),
    baseRevision: null,
    mountKey: 0,
    conflict: null,
    error: null,
    open: vi.fn(),
    close: vi.fn(),
    save: vi.fn(),
    reload: vi.fn(),
    rename: vi.fn(),
    copyLink: vi.fn(),
  }),
}));

import { orchestrationSnapshot } from "@/api";
import { CARD_ROW_ATTRIBUTE } from "@/components/common/CardRow";
import { OrchestrationPane } from "@/components/orchestration/OrchestrationPane";
import { loading } from "@/store/loadable";
import { TooltipProvider } from "@/components/ui/tooltip";
import type {
  OrchestrationSnapshot,
  ProjectView,
  ScratchpadSummary,
  TimerView,
  TodoView,
} from "@/domain";
import type { TodoActionsStore } from "@/store/useTodoActions";
import type { TodoEditorStore } from "@/store/useTodoEditor";
import type { ScratchpadActionsStore } from "@/store/useScratchpadActions";
import type { ScratchpadEditorStore } from "@/store/useScratchpadEditor";
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

const scratchpad: ScratchpadSummary = {
  id: 1,
  name: "release-plan",
  tags: [],
  archived: false,
  revision: 1,
  gist: "",
  updated_at: 0,
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

function showScratchpads() {
  fireEvent.click(screen.getByRole("radio", { name: "Scratchpads" }));
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
    // The board's own toolbar carries a polite count region for as long as it is mounted, so the
    // wait is what `aria-busy` marks — that, and only that, must be gone.
    expect(screen.queryByRole("status", { busy: true })).toBeNull();
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

  it("shows the scratchpad stand-in while the first snapshot is in flight, and the empty board only after it lands", async () => {
    const settle = holdRead(read);
    pane();

    showScratchpads();

    // A project with no scratchpads and a project whose scratchpads have not been read look nothing
    // alike to a reader: the empty state is a statement, not something to say while waiting.
    expect(screen.queryByText("No scratchpads yet")).toBeNull();
    const region = screen.getByRole("status");
    expect(region.getAttribute("aria-busy")).toBe("true");
    expect(region.textContent).toContain("Loading scratchpads");

    settle(snapshot());

    expect(await screen.findByText("No scratchpads yet")).toBeTruthy();
    expect(screen.queryByRole("status", { busy: true })).toBeNull();
  });

  it("renders the scratchpad board once the snapshot lands", async () => {
    read.mockResolvedValue(snapshot({ scratchpads: [scratchpad] }));
    pane();

    showScratchpads();

    expect(await screen.findByText("Release plan")).toBeTruthy();
    // The board's own card rows, so the pane is proven to mount the board rather than any other
    // surface that happens to name the same scratchpad.
    expect(document.querySelectorAll(`[${CARD_ROW_ATTRIBUTE}]`)).toHaveLength(1);
  });

  // The pane mounts *with* the activation already on its props. Opening a document from the
  // sidebar deselects the process and names the target in one commit, so the pane the
  // navigation lands in is always a fresh one — a switch that only reacts to `focus` changing after
  // mount leaves the reader on the agents tree instead of the item they asked for.
  it("opens on the view a navigation named, when that navigation is what mounted it", async () => {
    read.mockResolvedValue(snapshot({ todos: [todo] }));
    render(
      <TooltipProvider>
        <OrchestrationPane project={project} focus={{ view: "todos", id: todo.id, nonce: 1 }} />
      </TooltipProvider>,
    );

    // The board's own card, rather than the title anywhere on screen: the same navigation opens
    // that todo's detail panel, which names it too, and the question here is which *view* the pane
    // opened on.
    const card = await waitFor(() => {
      const rows = document.querySelectorAll(`[${CARD_ROW_ATTRIBUTE}]`);
      expect(rows).toHaveLength(1);
      return rows[0] as HTMLElement;
    });
    expect(within(card).getByText("Ship the release")).toBeTruthy();
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
