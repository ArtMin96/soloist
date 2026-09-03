import { Lock } from "lucide-react";
import { TodoRowSummary } from "@/components/orchestration/TodoRowSummary";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
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

// One todo in the board's list: a full-width card carrying its title, declared status, id, blocker
// gate, tags and comment count, and — while an agent holds it — the lock that names its owner. A
// card rather than a line because a single baseline gives the title no room against everything
// competing with it, and because the whole card is then one target worth aiming at.
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
    <Card data-todo-card size="sm" className="w-full gap-0 rounded-lg py-0">
      {/* The card's own button and the agent control are siblings, not nested — a lock never buries
          an interactive control inside another one, so each is its own tab stop. */}
      <CardContent className="flex items-stretch gap-0 p-0">
        <Button
          type="button"
          data-todo-trigger
          variant="ghost"
          onClick={onOpen}
          className="h-auto min-w-0 flex-1 flex-col items-stretch gap-1.5 overflow-hidden rounded-none px-2 py-1.5 text-left whitespace-normal active:not-aria-[haspopup]:scale-100 focus-visible:ring-inset"
        >
          <TodoRowSummary todo={todo} done={done} />
        </Button>
        {todo.locked_by != null &&
          (onOpenAgent ? (
            <Tooltip>
              <TooltipTrigger asChild>{agentControl}</TooltipTrigger>
              <TooltipContent>Open {agentLabel} terminal</TooltipContent>
            </Tooltip>
          ) : (
            agentControl
          ))}
      </CardContent>
    </Card>
  );
}
