import { useMemo, useState } from "react";
import { Plus } from "lucide-react";
import { TodoCreateForm } from "@/components/orchestration/TodoCreateForm";
import { TodoFilters } from "@/components/orchestration/TodoFilters";
import { TodoGroup } from "@/components/orchestration/TodoGroup";
import { TodoItem, type TodoEditState } from "@/components/orchestration/TodoItem";
import { SegmentedControl } from "@/components/SegmentedControl";
import { Button } from "@/components/ui/button";
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
import type { Option } from "@/lib/appearance";
import type { AgentNode, ScratchpadSummary, TodoView } from "@/domain";

/** How the board arranges its rows. Grouped is the default — most work comes out of a document. */
type BoardView = "grouped" | "all";

const BOARD_VIEWS: Option<BoardView>[] = [
  { value: "all", label: "All" },
  { value: "grouped", label: "By scratchpad" },
];

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
}: {
  project: number;
  todos: TodoView[];
  agents: AgentNode[];
  scratchpads: ScratchpadSummary[];
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
    <li key={todo.id}>
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
        showScratchpad={!grouped}
        scratchpads={scratchpads}
        edit={editStateFor(todo)}
      />
    </li>
  );

  // The board's one accent-filled default action — and only while there is no form open, since the
  // form's own Create is then the default and two filled buttons would each claim to be it.
  const creating = editor.mode === "create";
  const newTodo = (
    <Button size="sm" onClick={startCreate} className="shrink-0">
      <Plus aria-hidden /> New todo
    </Button>
  );

  return (
    <div className="flex h-full min-h-0 flex-col tracking-[var(--tracking-body)]">
      <div className="flex shrink-0 flex-col gap-1.5 border-b p-2">
        <TodoFilters
          filter={filter}
          tags={tags}
          onChange={setFilter}
          trailing={creating ? undefined : newTodo}
        />
        <SegmentedControl<BoardView>
          value={view}
          options={BOARD_VIEWS}
          onChange={setView}
          ariaLabel="Group todos"
        />
      </div>

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
          <p className="px-3 py-6 text-[0.8125rem] leading-relaxed text-muted-foreground">
            {isFiltering(filter)
              ? "No todos match your search."
              : "No todos yet. Create one, or let agents create them to hand off and order work — they appear here live, with their blockers, locks, and comments."}
          </p>
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
