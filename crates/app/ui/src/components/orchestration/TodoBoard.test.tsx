// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import { TodoBoard } from "@/components/orchestration/TodoBoard";
import { UNLINKED_GROUP_LABEL } from "@/store/todoGrouping";
import type { ScratchpadRef, ScratchpadSummary, TodoDoc, TodoView } from "@/domain";

// The board's own hooks are the only IPC on this surface; stubbing them keeps the test on the
// board's arrangement (grouping, the view toggle, what a row is told to show) rather than on writes,
// which `useTodoEditor` and `useTodoActions` already cover.
vi.mock("@/store/useTodoActions", () => ({
  useTodoActions: () => ({
    busyId: null,
    errorById: {},
    complete: vi.fn(),
    copyLink: vi.fn(),
    comment: vi.fn(),
  }),
}));
// The edit session the stubbed hook reports, so a test can put the board in create mode without
// driving IPC. Reset before each render.
const session: { mode: "create" | "edit" | null; initial: TodoDoc | null } = {
  mode: null,
  initial: null,
};

vi.mock("@/store/useTodoEditor", () => ({
  useTodoEditor: () => ({
    mode: session.mode,
    editingId: null,
    initial: session.initial,
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

// The create form mounts the lazy rich editor, which needs real layout; standing it in keeps this
// file on the board's arrangement.
vi.mock("@/components/editor/LazyRichTextEditor", () => ({
  LazyRichTextEditor: () => <div data-testid="rich-text" />,
}));

// The board persists its per-group collapse state through `localStorage`, which this environment
// does not provide; an in-memory stand-in makes that round trip real rather than silently swallowed.
const stored = new Map<string, string>();
vi.stubGlobal("localStorage", {
  getItem: (key: string) => stored.get(key) ?? null,
  setItem: (key: string, value: string) => void stored.set(key, value),
  removeItem: (key: string) => void stored.delete(key),
  clear: () => stored.clear(),
});

afterEach(() => {
  cleanup();
  localStorage.clear();
  session.mode = null;
  session.initial = null;
});

const plan: ScratchpadRef = { id: 4, name: "release-plan" };

const pad: ScratchpadSummary = {
  id: 4,
  name: "release-plan",
  tags: [],
  archived: false,
  revision: 1,
  gist: "",
  updated_at: 0,
};

const todo = (id: number, title: string, scratchpad: ScratchpadRef | null): TodoView => ({
  id,
  doc: { title, body: "", status: "open" },
  tags: [],
  blockers: [],
  blocked_by: [],
  blocked: false,
  comments: [],
  locked_by: null,
  scratchpad,
  revision: 1,
});

const todos = [todo(1, "Ship the release", plan), todo(2, "Triage inbox", null)];

function board(rows: TodoView[] = todos) {
  return render(<TodoBoard project={1} todos={rows} agents={[]} scratchpads={[pad]} />);
}

/** The board's group headers, in render order, read off their stable handle. */
function groupLabels(): string[] {
  return [...document.querySelectorAll("[data-todo-group]")].map(
    (header) => header.querySelector("span")?.textContent?.trim() ?? "",
  );
}

describe("TodoBoard", () => {
  it("groups by scratchpad on first open, without being asked", () => {
    board();

    expect(screen.getByRole("radio", { name: "By scratchpad" })).toHaveProperty(
      "ariaChecked",
      "true",
    );
    expect(groupLabels()).toEqual(["Release plan", UNLINKED_GROUP_LABEL]);
  });

  it("names the unlinked group plainly and keeps its rows visible", () => {
    board();

    expect(groupLabels()).toContain(UNLINKED_GROUP_LABEL);
    // Rows in it render exactly like any other group's — nothing is hidden behind the label.
    expect(screen.getByText("Triage inbox")).toBeTruthy();
  });

  it("flattens to one list when the view switches to All, and back again", () => {
    board();

    fireEvent.click(screen.getByRole("radio", { name: "All" }));
    expect(groupLabels()).toEqual([]);
    // Every todo is still on screen — flattening changes the arrangement, never the set.
    expect(screen.getByText("Ship the release")).toBeTruthy();
    expect(screen.getByText("Triage inbox")).toBeTruthy();

    fireEvent.click(screen.getByRole("radio", { name: "By scratchpad" }));
    expect(groupLabels()).toEqual(["Release plan", UNLINKED_GROUP_LABEL]);
  });

  it("names each row's scratchpad only in the flat view, where no header says it", () => {
    board();
    // Grouped: the header carries the title, so the row does not repeat it.
    expect(screen.getAllByText("Release plan")).toHaveLength(1);

    fireEvent.click(screen.getByRole("radio", { name: "All" }));
    const row = screen.getByText("Ship the release").closest("button");
    expect(within(row as HTMLElement).getByText("Release plan")).toBeTruthy();
  });

  it("collapses a group and remembers it across a remount", () => {
    const { unmount } = board();

    fireEvent.click(screen.getByRole("button", { name: /Release plan/ }));
    expect(screen.queryByText("Ship the release")).toBeNull();
    expect(screen.getByText("Triage inbox")).toBeTruthy();

    unmount();
    board();
    expect(screen.queryByText("Ship the release")).toBeNull();
  });

  it("narrows the groups to the todos a filter leaves, dropping one that empties", () => {
    board();

    fireEvent.change(screen.getByRole("searchbox", { name: "Search todos" }), {
      target: { value: "triage" },
    });

    expect(groupLabels()).toEqual([UNLINKED_GROUP_LABEL]);
    expect(screen.queryByText("Ship the release")).toBeNull();
  });

  it("offers one create action at a time — the form's Create replaces New todo, never joins it", () => {
    board();
    expect(screen.getByRole("button", { name: /New todo/ })).toBeTruthy();

    session.mode = "create";
    session.initial = { title: "", body: "", status: "open" };
    cleanup();
    board();

    expect(screen.queryByRole("button", { name: /New todo/ })).toBeNull();
    expect(screen.getByRole("button", { name: /Create todo/ })).toBeTruthy();
  });

  it("shows the empty state rather than an empty group when there are no todos", () => {
    board([]);

    expect(groupLabels()).toEqual([]);
    expect(screen.getByText(/No todos yet/)).toBeTruthy();
  });
});
