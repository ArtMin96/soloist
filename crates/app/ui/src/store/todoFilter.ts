import {
  isSearchingOrTagging,
  matchesSearchAndTag,
  type SearchTagFilter,
} from "@/store/boardFilter";
import type { TodoStatus, TodoView } from "@/domain";

// The status facet: any status, or one declared `TodoStatus`. "all" is the unfiltered default,
// kept distinct from the enum so the board never emits a bare status string at the comparison site.
export type StatusFilter = TodoStatus | "all";

// The board's filter state: the shared needle and tag, plus the to-do board's own status facet.
// Pure data — the todos arrive from the live snapshot and this only narrows them, so the visible
// set stays trivially unit-testable and holds no IPC.
export interface TodoFilter extends SearchTagFilter {
  status: StatusFilter;
}

export const EMPTY_TODO_FILTER: TodoFilter = { search: "", status: "all", tag: null };

// Narrows the todos to those matching every active facet: status "all" matches all, ANDed with the
// shared needle over title and body and the single tag.
export function filterTodos(todos: TodoView[], filter: TodoFilter): TodoView[] {
  return todos.filter(
    (todo) =>
      (filter.status === "all" || todo.doc.status === filter.status) &&
      matchesSearchAndTag(todo, filter, (t) => [t.doc.title, t.doc.body]),
  );
}

// Whether any facet is narrowing the list — the board picks its empty-state hint from this.
export function isFiltering(filter: TodoFilter): boolean {
  return filter.status !== "all" || isSearchingOrTagging(filter);
}
