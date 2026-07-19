import { humanizeName } from "@/lib/humanize";
import type { TodoView } from "@/domain";

// The key standing for "this todo derives from no scratchpad". A todo without one is an ordinary,
// permanently valid todo, so it gets a first-class group rather than being hidden or flagged — it
// is simply the last one, where a reader expects the leftovers.
const UNLINKED = "unlinked";

/** The label the unlinked group carries. Plain and neutral: a statement of fact, not a warning. */
export const UNLINKED_GROUP_LABEL = "No scratchpad";

/** One scratchpad's todos on the board, with the label and collapse key its header needs. */
export interface TodoGroup {
  /** Stable across re-reads — the persisted collapse state and the React list key ride on it. */
  key: string;
  /** The scratchpad's prose title, or the unlinked group's label. */
  label: string;
  todos: TodoView[];
}

/**
 * Buckets todos by the scratchpad they derive from, in the order those scratchpads first appear
 * among the todos, with the unlinked ones last. Pure — the todos arrive from the live snapshot and
 * this only arranges them, so the board's shape stays trivially unit-testable and holds no IPC.
 *
 * Group order follows first appearance rather than the scratchpad roster, so the board's vertical
 * order tracks the todo order the user already sees in the flat view. Headers read the humanized
 * handle, matching the scratchpad panel, and a group is emitted only when it has todos — an empty
 * heading would be noise.
 */
export function groupTodosByScratchpad(todos: TodoView[]): TodoGroup[] {
  const byKey = new Map<string, TodoGroup>();
  for (const todo of todos) {
    const key = todo.scratchpad === null ? UNLINKED : String(todo.scratchpad.id);
    const existing = byKey.get(key);
    if (existing) {
      existing.todos.push(todo);
      continue;
    }
    byKey.set(key, {
      key,
      label: todo.scratchpad === null ? UNLINKED_GROUP_LABEL : humanizeName(todo.scratchpad.name),
      todos: [todo],
    });
  }
  const groups = [...byKey.values()];
  return [
    ...groups.filter((group) => group.key !== UNLINKED),
    ...groups.filter((group) => group.key === UNLINKED),
  ];
}
