// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { useState } from "react";
import { cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import { TodoBoard } from "@/components/orchestration/TodoBoard";
import { TooltipProvider } from "@/components/ui/tooltip";
import { UNLINKED_GROUP_LABEL } from "@/store/todoGrouping";
import type { TodoActionsStore } from "@/store/useTodoActions";
import type { TodoEditorStore } from "@/store/useTodoEditor";
import type { ScratchpadRef, ScratchpadSummary, TodoDoc, TodoView } from "@/domain";

// The board's own hooks are the only IPC on this surface; stubbing them keeps the test on the
// board's arrangement (grouping, the view toggle, which panel is showing) rather than on writes,
// which `useTodoEditor` and `useTodoActions` already cover. Each stub is typed as the store it
// stands in for, so a member added to the real hook fails the typecheck here instead of leaving the
// board under test wired to a shape the app no longer has.
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

// The edit session the stubbed hook reports, so a test can put the board in create or edit mode
// without driving IPC. Reset before each render.
const session: {
  mode: "create" | "edit" | null;
  editingId: number | null;
  initial: TodoDoc | null;
  baseRevision: number | null;
} = { mode: null, editingId: null, initial: null, baseRevision: null };

vi.mock("@/store/useTodoEditor", () => ({
  useTodoEditor: (): TodoEditorStore => {
    // `close` genuinely ends the session here, as the real hook's does, so a test can watch the
    // board end an edit rather than watch it call a spy — which would pass just as happily if the
    // board closed the wrong session, or closed one it should have kept.
    const [closed, setClosed] = useState(false);
    const open = !closed;
    return {
      mode: open ? session.mode : null,
      editingId: open ? session.editingId : null,
      initial: open ? session.initial : null,
      scratchpad: null,
      baseRevision: open ? session.baseRevision : null,
      mountKey: 0,
      error: null,
      startCreate: vi.fn(),
      editTodo: vi.fn(),
      close: () => setClosed(true),
      save: vi.fn(),
      reload: vi.fn(),
    };
  },
}));

// The create form and the edit surface both mount the lazy rich editor, which needs real layout;
// standing it in keeps this file on the board's arrangement.
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
  session.editingId = null;
  session.initial = null;
  session.baseRevision = null;
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

// The detail panel's Copy link and overflow controls are Tooltip triggers, which need a provider
// ancestor — supplied here rather than by the board itself, since the app supplies one once at its
// root and the board is never mounted without it in production.
function board(rows: TodoView[] = todos, overrides: Partial<Parameters<typeof TodoBoard>[0]> = {}) {
  return render(
    <TooltipProvider>
      <TodoBoard project={1} todos={rows} agents={[]} scratchpads={[pad]} {...overrides} />
    </TooltipProvider>,
  );
}

/** Re-renders the board under test with `props` merged over the defaults. */
function rerenderBoard(
  render: (ui: React.ReactElement) => void,
  props: Partial<Parameters<typeof TodoBoard>[0]>,
) {
  render(
    <TooltipProvider>
      <TodoBoard project={1} todos={todos} agents={[]} scratchpads={[pad]} {...props} />
    </TooltipProvider>,
  );
}

/** The board's group headers, in render order, read off the handle that carries the label itself. */
function groupLabels(): string[] {
  return [...document.querySelectorAll("[data-todo-group]")].map(
    (header) => header.getAttribute("data-todo-group") ?? "",
  );
}

/** Which panel the board is showing — the route, not merely which panel is mounted. */
function route(): string | null {
  return document.querySelector("[data-todo-route]")?.getAttribute("data-todo-route") ?? null;
}

function panel(name: "list" | "detail"): HTMLElement {
  return document.querySelector<HTMLElement>(`[data-todo-panel="${name}"]`) as HTMLElement;
}

/** The card that opens todo `id` — the same handle the end-to-end walks aim at. */
function card(id: number): HTMLElement {
  return document.querySelector<HTMLElement>(
    `[data-todo-id="${id}"] [data-todo-trigger]`,
  ) as HTMLElement;
}

function backButton(): HTMLElement {
  return panel("detail").querySelector<HTMLElement>("[data-todo-back]") as HTMLElement;
}

function searchBox(): HTMLInputElement {
  return screen.getByRole("searchbox", { name: "Search todos" }) as HTMLInputElement;
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

  it("names a scratchpad on its group header and never again on the card itself", () => {
    board();
    // Grouped: the header carries the title, and the card does not repeat it.
    expect(screen.queryAllByText("Release plan")).toHaveLength(1);
    expect(within(card(1)).queryByText("Release plan")).toBeNull();

    // Flattened, the header is gone — and the card still does not take it over. Provenance is
    // stated once, in the detail panel.
    fireEvent.click(screen.getByRole("radio", { name: "All" }));
    expect(screen.queryAllByText("Release plan")).toHaveLength(0);
    expect(screen.getByText("Ship the release")).toBeTruthy();
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

  it("flattens the board while a filter is active, and restores the groups when it clears", () => {
    board();
    expect(groupLabels()).toEqual(["Release plan", UNLINKED_GROUP_LABEL]);

    fireEvent.change(searchBox(), { target: { value: "triage" } });

    // A search is already a triage question, so the matches are not buried under headers.
    expect(groupLabels()).toEqual([]);
    expect(screen.getByText("Triage inbox")).toBeTruthy();
    expect(screen.queryByText("Ship the release")).toBeNull();

    fireEvent.change(searchBox(), { target: { value: "" } });
    expect(groupLabels()).toEqual(["Release plan", UNLINKED_GROUP_LABEL]);
  });

  it("keeps a filtered card free of the scratchpad too, once its header is gone", () => {
    board();
    // Grouped and unfiltered, the header carries the title and the card does not repeat it.
    expect(screen.queryAllByText("Release plan")).toHaveLength(1);

    fireEvent.change(searchBox(), { target: { value: "ship" } });

    expect(screen.queryAllByText("Release plan")).toHaveLength(0);
    expect(screen.getByText("Ship the release")).toBeTruthy();
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

  it("renders exactly one toolbar and no second filter strip", () => {
    board();

    expect(document.querySelectorAll("[data-todo-toolbar]")).toHaveLength(1);
  });

  it("hides New todo from the toolbar while the create form is open", () => {
    board();
    expect(screen.getByRole("button", { name: /New todo/ })).toBeTruthy();

    session.mode = "create";
    session.initial = { title: "", body: "", status: "open" };
    cleanup();
    board();

    expect(screen.queryByRole("button", { name: /New todo/ })).toBeNull();
  });

  it("hands the pane to a todo's detail when its card is opened", () => {
    board();
    expect(route()).toBe("list");

    fireEvent.click(card(1));

    expect(route()).toBe("detail");
    expect(within(panel("detail")).getByRole("heading", { name: "Ship the release" })).toBeTruthy();
  });

  it("moves focus to Back when the detail opens, so the inert list cannot strand it", () => {
    board();

    fireEvent.click(card(1));

    // The list goes inert the moment the detail shows; focus left on the card behind it would be
    // dropped to the document body and restart keyboard traversal at the top of the app.
    expect(document.activeElement).toBe(backButton());
  });

  it("takes the list out of the accessibility tree while the detail is showing, and back after", () => {
    board();
    expect(panel("list").hasAttribute("inert")).toBe(false);
    expect(panel("detail").hasAttribute("inert")).toBe(true);

    fireEvent.click(card(1));

    // Both panels stay mounted for the length of the swipe, so `inert` is the whole of what keeps
    // the off-screen one unreachable. This environment implements none of its behaviour — measured:
    // `HTMLElement.inert` is undefined, role queries still return inert content, and a button inside
    // an inert subtree still takes focus — so the attribute is the only half assertable here. What
    // it buys (tab traversal stopping at the panel edge) is the end-to-end walk's to prove.
    expect(panel("list").hasAttribute("inert")).toBe(true);
    expect(panel("detail").hasAttribute("inert")).toBe(false);

    fireEvent.click(backButton());

    expect(panel("list").hasAttribute("inert")).toBe(false);
    expect(panel("detail").hasAttribute("inert")).toBe(true);
  });

  it("returns to the list on Back, with focus back on the card it came from", () => {
    board();
    fireEvent.click(card(1));

    fireEvent.click(backButton());

    expect(route()).toBe("list");
    expect(document.activeElement).toBe(card(1));
  });

  it("ends an open edit session when Back leaves the todo it belongs to", () => {
    session.mode = "edit";
    session.editingId = 1;
    session.initial = { title: "Ship the release", body: "", status: "open" };
    session.baseRevision = 1;
    board();

    fireEvent.click(card(1));
    expect(panel("detail").querySelector("[data-todo-done]")).toBeTruthy();

    fireEvent.click(backButton());
    fireEvent.click(card(1));

    // Re-opening starts from the read view: an unsaved draft never outlives the panel showing it.
    expect(panel("detail").querySelector("[data-todo-done]")).toBeNull();
    expect(within(panel("detail")).getByRole("button", { name: /Edit/ })).toBeTruthy();
  });

  it("falls back to the list when the open todo vanishes from the snapshot", () => {
    const { rerender } = board();
    fireEvent.click(card(1));
    expect(route()).toBe("detail");

    // Deleted, or moved out of this project, while its panel was up.
    rerenderBoard(rerender, { todos: [todos[1]] });

    expect(route()).toBe("list");
    expect(document.querySelector("[data-todo-detail]")).toBeNull();
  });

  // Every navigation test below uses a nonce of its own. The board remembers the last activation it
  // acted on beyond its own lifetime — it has to, or a remount replays a stale one — and a real
  // nonce is minted per activation and never repeats, so unique values here match production rather
  // than working around the memory.
  it("opens the focusId todo's detail and focuses Back when focusNonce changes", () => {
    const { rerender } = board(todos, { focusId: 2, focusNonce: undefined });
    expect(route()).toBe("list");

    rerenderBoard(rerender, { focusId: 2, focusNonce: 10 });

    expect(route()).toBe("detail");
    expect(document.activeElement).toBe(backButton());
    expect(within(panel("detail")).getByRole("heading", { name: "Triage inbox" })).toBeTruthy();
  });

  it("opens the target once it arrives, when focusNonce was set before the todos did", () => {
    // Mirrors a pane that mounts fresh and asks for a target before its first snapshot lands: the
    // todo named by `focusId` is not in `todos` on the first render at all.
    const { rerender } = board([], { focusId: 2, focusNonce: 20 });
    expect(route()).toBe("list");

    rerenderBoard(rerender, { focusId: 2, focusNonce: 20 });

    expect(route()).toBe("detail");
    expect(document.activeElement).toBe(backButton());
  });

  it("opens on the list when an activation it already acted on is still standing at mount", () => {
    // The pane leaves `focus` set after acting on it, and unmounts the board whenever the user
    // switches to another orchestration view — so switching away and back re-delivers the same
    // activation to a fresh board. A nonce is one navigation, not a standing instruction to keep
    // reopening the detail, and the board a user opens must be the list.
    const first = board(todos, { focusId: 2, focusNonce: 30 });
    expect(route()).toBe("detail");
    first.unmount();

    board(todos, { focusId: 2, focusNonce: 30 });

    expect(route()).toBe("list");
  });

  it("reaches a target the list is hiding, without clearing the filter or expanding its group", () => {
    const { rerender } = board();
    // Arrange the list the way a real board could be when the navigation arrives: the target's
    // group collapsed, and a search that excludes it.
    fireEvent.click(screen.getByRole("button", { name: /Release plan/ }));
    fireEvent.change(searchBox(), { target: { value: "triage" } });
    expect(screen.queryByText("Ship the release")).toBeNull();

    rerenderBoard(rerender, { focusId: 1, focusNonce: 40 });

    // The detail panel shows the todo whatever the list is doing, so nothing has to be revealed to
    // reach it — and the list the reader returns to is still arranged the way they left it.
    expect(route()).toBe("detail");
    expect(within(panel("detail")).getByRole("heading", { name: "Ship the release" })).toBeTruthy();
    expect(searchBox().value).toBe("triage");
    expect(within(panel("list")).queryByText("Ship the release")).toBeNull();
  });

  it("keeps the search box live and settles the filtered list to the matching row, even over a large board", () => {
    // Filtering runs off a deferred copy of the toolbar's filter (see TodoBoard.tsx), so the search
    // box itself must never wait on it — this is the surface that would visibly lag if the input
    // were bound to anything but the live keystroke.
    const many = Array.from({ length: 3000 }, (_, i) => todo(i, `Task number ${i}`, null));
    board(many);

    fireEvent.change(searchBox(), { target: { value: "number 1234" } });

    // The typed text lands synchronously, regardless of how large the board behind it is.
    expect(searchBox().value).toBe("number 1234");
    // The deferred derivation settles to exactly the matching row — never the unfiltered 3000, and
    // never a mix of the two.
    const rows = document.querySelectorAll("[data-todo-id]");
    expect(rows).toHaveLength(1);
    expect(rows[0].getAttribute("data-todo-id")).toBe("1234");
  });
});
