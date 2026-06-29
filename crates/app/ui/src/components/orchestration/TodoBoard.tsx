import { useState } from "react";
import { TodoItem } from "@/components/orchestration/TodoItem";
import { useTodoActions } from "@/store/useTodoActions";
import type { AgentNode, TodoView } from "@/domain";

// The to-do board: the project's shared work items, each expandable to its document, blockers,
// comments, and actions. The todos come from the live snapshot (refreshed on TodoChanged); the only
// IPC here is the write hook (complete / copy link). Blocker titles and a lock owner's label are
// resolved from the same snapshot, so the board names them rather than showing bare ids.
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
  const [openId, setOpenId] = useState<number | null>(null);

  if (todos.length === 0) {
    return (
      <p className="px-3 py-6 text-[0.8125rem] leading-relaxed text-muted-foreground">
        No todos yet. Agents create them to hand off and order work — they will appear here live,
        with their blockers, locks, and comments.
      </p>
    );
  }

  const titleOf = (id: number) => todos.find((todo) => todo.id === id)?.doc.title;
  const labelOf = (id: number) => agents.find((agent) => agent.id === id)?.label;

  return (
    <div className="h-full overflow-auto">
      <ul className="flex flex-col px-1">
        {todos.map((todo) => (
          <li key={todo.id}>
            <TodoItem
              todo={todo}
              open={openId === todo.id}
              onToggle={() => setOpenId((current) => (current === todo.id ? null : todo.id))}
              titleOf={titleOf}
              lockOwnerLabel={todo.locked_by != null ? labelOf(todo.locked_by) : undefined}
              busy={actions.busyId === todo.id}
              error={actions.errorById[todo.id]}
              onComplete={() => actions.complete(todo.id)}
              onCopyLink={() => actions.copyLink(todo.id)}
            />
          </li>
        ))}
      </ul>
    </div>
  );
}
