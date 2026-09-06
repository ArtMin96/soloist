import { Lock } from "lucide-react";
import { CardRow } from "@/components/common/CardRow";
import { TodoRowSummary } from "@/components/orchestration/TodoRowSummary";
import { Button } from "@/components/ui/button";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import type { TodoView } from "@/domain";

interface TodoItemProps {
  todo: TodoView;
  /** Hands the pane over to this todo's detail panel. */
  onOpen: () => void;
  lockOwnerLabel: string | undefined;
  /**
   * Opens the agent this row is locked by. Absent when the caller offers no navigation — the
   * control still renders, but disabled, rather than disappearing and shifting the row.
   */
  onOpenAgent?: (process: number) => void;
}

// One todo in the board's list: a board row carrying its title, declared status, id, blocker gate,
// tags and comment count, and — while an agent holds it — the lock that names its owner beside the
// row's own trigger.
export function TodoItem({ todo, onOpen, lockOwnerLabel, onOpenAgent }: TodoItemProps) {
  const done = todo.doc.status === "done";
  const agentLabel = lockOwnerLabel ?? `#${todo.locked_by}`;

  const agentControl = todo.locked_by != null && (
    <Button
      data-todo-agent
      data-process-id={todo.locked_by}
      variant="ghost"
      size="sm"
      aria-label={`Open ${agentLabel} terminal`}
      disabled={onOpenAgent == null}
      onClick={() => onOpenAgent?.(todo.locked_by as number)}
      className="m-1 max-w-[45%] min-w-0 shrink-0"
    >
      <Lock aria-hidden data-icon="inline-start" />
      <span data-todo-agent-label className="min-w-0 truncate">
        {agentLabel}
      </span>
    </Button>
  );

  return (
    <CardRow
      onOpen={onOpen}
      aside={
        todo.locked_by != null &&
        (onOpenAgent ? (
          <Tooltip>
            <TooltipTrigger asChild>{agentControl}</TooltipTrigger>
            <TooltipContent>Open {agentLabel} terminal</TooltipContent>
          </Tooltip>
        ) : (
          agentControl
        ))
      }
    >
      <TodoRowSummary todo={todo} done={done} />
    </CardRow>
  );
}
