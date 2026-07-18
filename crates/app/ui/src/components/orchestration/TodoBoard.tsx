import { useMemo, useState } from "react";
import { Plus } from "lucide-react";
import { TodoCreateForm } from "@/components/orchestration/TodoCreateForm";
import { TodoFilters } from "@/components/orchestration/TodoFilters";
import { TodoItem, type TodoEditState } from "@/components/orchestration/TodoItem";
import { Button } from "@/components/ui/button";
import { useTodoActions } from "@/store/useTodoActions";
import { useTodoEditor } from "@/store/useTodoEditor";
import {
  EMPTY_TODO_FILTER,
  filterTodos,
  isFiltering,
  todoTags,
  type TodoFilter,
} from "@/store/todoFilter";
import type { AgentNode, TodoView } from "@/domain";

// The to-do board: the project's shared work items, filterable and fully editable. The todos come
// from the live snapshot (refreshed on TodoChanged); every write — create, edit, complete, comment —
// routes through the same core commands agents use (the editor and action hooks are the only IPC
// here). Editing is revision-guarded: the board watches the live revision to raise the conflict
// banner when a concurrent write moves a todo out from under an open editor. Blocker titles and a
// lock owner's label are resolved from the same snapshot, so the board names them, not bare ids.
export function TodoBoard({
  project,
  todos,
  agents,
}: {
  project: number;
  todos: TodoView[];
  agents: AgentNode[];
}) {
  const actions = useTodoActions(project);
  const editor = useTodoEditor(project);
  const [openId, setOpenId] = useState<number | null>(null);
  const [filter, setFilter] = useState<TodoFilter>(EMPTY_TODO_FILTER);

  const tags = useMemo(() => todoTags(todos), [todos]);
  const visible = useMemo(() => filterTodos(todos, filter), [todos, filter]);

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
      mountKey: editor.mountKey,
      conflict,
      error: editor.error,
      onSave: editor.save,
      onReload: () => editor.reload(todo),
      onDone: editor.close,
    };
  };

  const newTodo = (
    <Button size="sm" onClick={startCreate} className="shrink-0">
      <Plus aria-hidden /> New todo
    </Button>
  );

  return (
    <div className="flex h-full min-h-0 flex-col">
      <div className="shrink-0 border-b p-2">
        <TodoFilters filter={filter} tags={tags} onChange={setFilter} trailing={newTodo} />
      </div>

      {editor.mode === "create" && editor.initial && (
        <TodoCreateForm onCreate={editor.save} onCancel={editor.close} error={editor.error} />
      )}

      <div className="min-h-0 flex-1 overflow-auto">
        {visible.length === 0 ? (
          <p className="px-3 py-6 text-[0.8125rem] leading-relaxed text-muted-foreground">
            {isFiltering(filter)
              ? "No todos match your search."
              : "No todos yet. Create one, or let agents create them to hand off and order work — they appear here live, with their blockers, locks, and comments."}
          </p>
        ) : (
          <ul className="flex flex-col px-1">
            {visible.map((todo) => (
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
                  edit={editStateFor(todo)}
                />
              </li>
            ))}
          </ul>
        )}
      </div>
    </div>
  );
}
