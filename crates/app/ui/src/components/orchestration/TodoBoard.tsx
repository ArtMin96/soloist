import { useEffect, useMemo, useRef, useState } from "react";
import { TodoCreateForm } from "@/components/orchestration/TodoCreateForm";
import { TodoGroup } from "@/components/orchestration/TodoGroup";
import { TodoItem, type TodoEditState } from "@/components/orchestration/TodoItem";
import { TodoToolbar } from "@/components/orchestration/TodoToolbar";
import { Empty, EmptyDescription, EmptyHeader, EmptyTitle } from "@/components/ui/empty";
import { useCollapseState } from "@/store/useCollapseState";
import { useTodoActions } from "@/store/useTodoActions";
import { useTodoEditor } from "@/store/useTodoEditor";
import { groupTodosByScratchpad } from "@/store/todoGrouping";
import {
  EMPTY_TODO_FILTER,
  filterTodos,
  isFiltering,
  todoTags,
  type TodoFilter,
} from "@/store/todoFilter";
import type { BoardView } from "@/lib/todo";
import type { AgentNode, ScratchpadSummary, TodoView } from "@/domain";

/** Namespaces this board's persisted collapse keys so they cannot collide with the sidebar's. */
const COLLAPSE_PREFIX = "todos.scratchpad";

// The to-do board: the project's shared work items, filterable and fully editable. The todos come
// from the live snapshot (refreshed on TodoChanged); every write — create, edit, complete, comment —
// routes through the same core commands agents use (the editor and action hooks are the only IPC
// here). Editing is revision-guarded: the board watches the live revision to raise the conflict
// banner when a concurrent write moves a todo out from under an open editor. Blocker titles and a
// lock owner's label are resolved from the same snapshot, so the board names them, not bare ids.
//
// Rows are grouped by the scratchpad each todo derives from by default, because that is the shape
// the work actually has — tasks extracted from a plan belong under it. `All` flattens the board for
// triage, when the question is "what is open" rather than "what came from where". Filtering flattens
// it too: a search is already a triage question, and headers over one or two surviving rows each
// would bury the matches they are meant to organise. A flattened row names its own scratchpad, so
// nothing is lost with the headers. Grouping is a wrapper — the rows are identical in both views.
export function TodoBoard({
  project,
  todos,
  agents,
  scratchpads,
  onOpenAgent,
  focusId,
  focusNonce,
}: {
  project: number;
  todos: TodoView[];
  agents: AgentNode[];
  scratchpads: ScratchpadSummary[];
  /** Opens the agent a row is locked by — absent when the caller offers no navigation. */
  onOpenAgent?: (process: number) => void;
  /** The todo to expand and focus when `focusNonce` changes — cross-surface navigation, inbound. */
  focusId?: number;
  /** Bumped to re-trigger the focus above, even to repeat the same `focusId`. */
  focusNonce?: number;
}) {
  const actions = useTodoActions(project);
  const editor = useTodoEditor(project);
  const [openId, setOpenId] = useState<number | null>(null);
  const [filter, setFilter] = useState<TodoFilter>(EMPTY_TODO_FILTER);
  const [view, setView] = useState<BoardView>("grouped");
  const [collapsed, setCollapsed] = useCollapseState();

  const tags = useMemo(() => todoTags(todos), [todos]);
  const visible = useMemo(() => filterTodos(todos, filter), [todos, filter]);
  const groups = useMemo(() => groupTodosByScratchpad(visible), [visible]);
  const grouped = view === "grouped" && !isFiltering(filter);

  const titleOf = (id: number) => todos.find((todo) => todo.id === id)?.doc.title;
  const labelOf = (id: number) => agents.find((agent) => agent.id === id)?.label;

  const startCreate = () => {
    setOpenId(null);
    editor.startCreate();
  };

  const toggle = (id: number) => {
    setOpenId((current) => {
      const next = current === id ? null : id;
      // Collapsing the row being edited ends its edit session so a re-open starts from the read view.
      if (next !== id && editor.mode === "edit" && editor.editingId === id) editor.close();
      return next;
    });
  };

  // Whether the focus target has actually arrived in the live snapshot. Coming from a freshly
  // mounted pane, `focusNonce` can be set before the first snapshot lands — `targetPresent`
  // gates the reveal below on that, and its own presence in the effect's deps is what makes the
  // reveal retry once the todo shows up, rather than silently missing it.
  const targetPresent = focusId != null && todos.some((todo) => todo.id === focusId);
  const revealedNonceRef = useRef<number | undefined>(undefined);
  const pendingFocusRef = useRef<{ nonce: number; id: number } | null>(null);

  // Cross-surface navigation's inbound half, step one: once the target row exists, clear a
  // filter that hides it and expand its scratchpad group if the board is grouped and it is
  // collapsed, then hand off to the effect below — a collapsed Radix panel keeps its content
  // mounted but `hidden`, so focusing it only works once it opens.
  useEffect(() => {
    if (focusId == null || focusNonce == null || !targetPresent) return;
    if (revealedNonceRef.current === focusNonce) return;
    revealedNonceRef.current = focusNonce;

    setOpenId(focusId);
    if (!visible.some((todo) => todo.id === focusId)) setFilter(EMPTY_TODO_FILTER);
    const group = groupTodosByScratchpad(todos).find((candidate) =>
      candidate.todos.some((todo) => todo.id === focusId),
    );
    if (group) setCollapsed(`${COLLAPSE_PREFIX}.${group.key}`, false);

    pendingFocusRef.current = { nonce: focusNonce, id: focusId };
    // Only the reveal's own trigger conditions belong here — `visible`/`todos`/`collapsed` are
    // read for their value at reveal time, not to re-run the reveal when they later change.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [focusNonce, focusId, targetPresent]);

  // Step two: moves DOM focus once the trigger the step above revealed actually mounts —
  // retried on every render (no deps array) until it does, then self-clears. Queried by the
  // same `data-todo-*` handles the e2e layer uses, rather than threading a ref through the
  // Collapsible wrapper.
  useEffect(() => {
    const pending = pendingFocusRef.current;
    if (pending == null) return;
    const trigger = document.querySelector<HTMLElement>(
      `[data-todo-id="${pending.id}"] [data-todo-trigger]`,
    );
    if (trigger == null) return;
    trigger.scrollIntoView({ block: "nearest" });
    trigger.focus();
    pendingFocusRef.current = null;
  });

  // The edit surface for one row, present only while it is the one being edited. A concurrent write
  // that moves the live todo past the opened revision is the conflict the editor pauses on.
  const editStateFor = (todo: TodoView): TodoEditState | null => {
    if (editor.mode !== "edit" || editor.editingId !== todo.id || editor.initial == null) {
      return null;
    }
    const conflict =
      editor.baseRevision != null && todo.revision > editor.baseRevision
        ? { actual: todo.revision }
        : null;
    return {
      initial: editor.initial,
      initialScratchpad: editor.scratchpad,
      mountKey: editor.mountKey,
      conflict,
      error: editor.error,
      onSave: editor.save,
      onReload: () => editor.reload(todo),
      onDone: editor.close,
    };
  };

  const row = (todo: TodoView) => (
    <li key={todo.id} data-todo-id={todo.id}>
      <TodoItem
        todo={todo}
        open={openId === todo.id}
        onToggle={() => toggle(todo.id)}
        titleOf={titleOf}
        lockOwnerLabel={todo.locked_by != null ? labelOf(todo.locked_by) : undefined}
        busy={actions.busyId === todo.id}
        error={actions.errorById[todo.id]}
        onComplete={() => actions.complete(todo.id)}
        onCopyLink={() => actions.copyLink(todo.id)}
        onComment={(body) => actions.comment(todo.id, body)}
        onStartEdit={() => editor.editTodo(todo)}
        onOpenAgent={onOpenAgent}
        showScratchpad={!grouped}
        scratchpads={scratchpads}
        edit={editStateFor(todo)}
      />
    </li>
  );

  // The board's one accent-filled default action — and only while there is no form open, since the
  // form's own Create is then the default and two filled buttons would each claim to be it.
  const creating = editor.mode === "create";

  return (
    <div className="flex h-full min-h-0 flex-col tracking-[var(--tracking-body)]">
      <TodoToolbar
        filter={filter}
        tags={tags}
        onChange={setFilter}
        view={view}
        onViewChange={setView}
        shown={visible.length}
        total={todos.length}
        onCreate={creating ? undefined : startCreate}
      />

      {creating && editor.initial && (
        <TodoCreateForm
          onCreate={editor.save}
          scratchpads={scratchpads}
          onCancel={editor.close}
          error={editor.error}
        />
      )}

      <div className="min-h-0 flex-1 overflow-auto">
        {visible.length === 0 ? (
          <Empty>
            <EmptyHeader>
              {isFiltering(filter) ? (
                <EmptyDescription>No todos match your search.</EmptyDescription>
              ) : (
                <>
                  <EmptyTitle>No todos yet</EmptyTitle>
                  <EmptyDescription>
                    Create one, or let agents create them to hand off and order work — they appear
                    here live, with their blockers, locks, and comments.
                  </EmptyDescription>
                </>
              )}
            </EmptyHeader>
          </Empty>
        ) : grouped ? (
          // Sections are plain containers, not list items: the rows stay the only list entries, so a
          // row is addressed the same way whichever view is showing.
          <div className="flex flex-col px-1 pt-1">
            {groups.map((group) => (
              <TodoGroup
                key={group.key}
                label={group.label}
                count={group.todos.length}
                open={!collapsed[`${COLLAPSE_PREFIX}.${group.key}`]}
                onOpenChange={(open) => setCollapsed(`${COLLAPSE_PREFIX}.${group.key}`, !open)}
              >
                <ul className="flex flex-col">{group.todos.map(row)}</ul>
              </TodoGroup>
            ))}
          </div>
        ) : (
          <ul className="flex flex-col px-1">{visible.map(row)}</ul>
        )}
      </div>
    </div>
  );
}
