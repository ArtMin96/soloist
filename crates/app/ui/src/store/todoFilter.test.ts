import { describe, expect, it } from "vitest";
import type { TodoStatus, TodoView } from "@/domain";
import { EMPTY_TODO_FILTER, filterTodos, isFiltering, todoTags } from "@/store/todoFilter";

// A minimal TodoView for filtering — only the fields the filter reads carry meaning here.
function todo(
  id: number,
  title: string,
  body: string,
  status: TodoStatus,
  tags: string[],
): TodoView {
  return {
    id,
    doc: { title, body, status },
    tags,
    blockers: [],
    blocked_by: [],
    blocked: false,
    comments: [],
    locked_by: null,
    revision: 1,
  };
}

const TODOS: TodoView[] = [
  todo(1, "Ship the editor", "wire the autosave", "in_progress", ["ui", "editor"]),
  todo(2, "Fix the gate", "blocker chain", "open", ["core"]),
  todo(3, "Write the docs", "PROGRESS and plan", "done", ["docs"]),
  todo(4, "Review the PR", "the AUTOSAVE path", "open", ["ui"]),
];

describe("filterTodos", () => {
  it("matches the empty filter to every todo", () => {
    expect(filterTodos(TODOS, EMPTY_TODO_FILTER)).toHaveLength(4);
  });

  it("searches title and body case-insensitively", () => {
    const byTitle = filterTodos(TODOS, { ...EMPTY_TODO_FILTER, search: "  Ship " });
    expect(byTitle.map((t) => t.id)).toEqual([1]);

    // "autosave" appears in #1's body and #4's body (upper-cased) — both match.
    const byBody = filterTodos(TODOS, { ...EMPTY_TODO_FILTER, search: "autosave" });
    expect(byBody.map((t) => t.id)).toEqual([1, 4]);
  });

  it("narrows by declared status", () => {
    const open = filterTodos(TODOS, { ...EMPTY_TODO_FILTER, status: "open" });
    expect(open.map((t) => t.id)).toEqual([2, 4]);
  });

  it("narrows by a single tag", () => {
    const ui = filterTodos(TODOS, { ...EMPTY_TODO_FILTER, tag: "ui" });
    expect(ui.map((t) => t.id)).toEqual([1, 4]);
  });

  it("combines facets with AND", () => {
    const openUiAutosave = filterTodos(TODOS, { search: "autosave", status: "open", tag: "ui" });
    expect(openUiAutosave.map((t) => t.id)).toEqual([4]);
  });
});

describe("todoTags", () => {
  it("returns the sorted distinct tags", () => {
    expect(todoTags(TODOS)).toEqual(["core", "docs", "editor", "ui"]);
  });
});

describe("isFiltering", () => {
  it("is false only for the empty filter", () => {
    expect(isFiltering(EMPTY_TODO_FILTER)).toBe(false);
    expect(isFiltering({ ...EMPTY_TODO_FILTER, search: "x" })).toBe(true);
    expect(isFiltering({ ...EMPTY_TODO_FILTER, status: "done" })).toBe(true);
    expect(isFiltering({ ...EMPTY_TODO_FILTER, tag: "ui" })).toBe(true);
  });
});
