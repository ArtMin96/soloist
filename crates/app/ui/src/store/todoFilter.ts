import type { TodoStatus, TodoView } from "@/domain";

// The status facet: any status, or one declared `TodoStatus`. "all" is the unfiltered default,
// kept distinct from the enum so the board never emits a bare status string at the comparison site.
export type StatusFilter = TodoStatus | "all";

// The board's filter state: a text needle (matched against title or body), a status facet, and a
// single tag. Pure data — the todos arrive from the live snapshot and this only narrows them, so
// the visible set stays trivially unit-testable and holds no IPC.
export interface TodoFilter {
  search: string;
  status: StatusFilter;
  tag: string | null;
}

export const EMPTY_TODO_FILTER: TodoFilter = { search: "", status: "all", tag: null };

// The distinct tags across the todos, sorted — the tag facet's options.
export function todoTags(todos: TodoView[]): string[] {
  const distinct = new Set<string>();
  for (const todo of todos) for (const tag of todo.tags) distinct.add(tag);
  return [...distinct].sort();
}

// Narrows the todos to those matching every active facet: a blank search matches all; status
// "all" matches all; a null tag matches all. Search is case-insensitive over title and body.
export function filterTodos(todos: TodoView[], filter: TodoFilter): TodoView[] {
  const needle = filter.search.trim().toLowerCase();
  return todos.filter((todo) => {
    if (filter.status !== "all" && todo.doc.status !== filter.status) return false;
    if (filter.tag !== null && !todo.tags.includes(filter.tag)) return false;
    if (needle === "") return true;
    return (
      todo.doc.title.toLowerCase().includes(needle) || todo.doc.body.toLowerCase().includes(needle)
    );
  });
}

// Whether any facet is narrowing the list — the board picks its empty-state hint from this.
export function isFiltering(filter: TodoFilter): boolean {
  return filter.search.trim() !== "" || filter.status !== "all" || filter.tag !== null;
}
