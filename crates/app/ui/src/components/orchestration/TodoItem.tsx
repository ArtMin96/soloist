import { Lock } from "lucide-react";
import { TodoRowSummary } from "@/components/orchestration/TodoRowSummary";
import { Button } from "@/components/ui/button";
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

  return (
    // The card lifts to the next surface step on hover from anywhere in it, so it reads as one
    // object rather than as a row that happens to have a clickable region. Its two lines sit closer
    // to each other than one card sits to the next, which is what makes a card read as one thing.
    //
    // The edge is a transparency of the ink, not `--border`: that token resolves close to the pane
    // in both themes (1.31:1 light, 1.44:1 dark), so a card drawn with it reads in neither, and no
    // fill substitutes for it — any tint strong enough to read as an edge reads as a selected row.
    // The alpha is per-theme because one value cannot clear 3:1 on both grounds: ink on a near-white
    // pane needs roughly half again the opacity that ink on a near-black one does.
    //
    // Hover is carried by the border alone — the rest-to-hover fill step is worth only 1.05:1 — so
    // the two alphas have to stay far apart or the state change is imperceptible.
    //
    // The resting alphas are a ceiling, not a floor. At 45% the outline already matches the status
    // chips' own outlines for weight; raising it puts the card's structure above the content it
    // frames. The gap between cards is what does the grouping, so a stronger edge buys contrast the
    // boundary does not need.
    <div className="flex items-center gap-1 rounded-lg border border-foreground/45 bg-muted p-1 transition-colors duration-[var(--dur-fast)] hover:border-foreground/60 dark:border-foreground/32 dark:hover:border-foreground/45">
      {/* The card's own button and the agent control are siblings, not nested — a lock never buries
          an interactive control inside another one, so each is its own tab stop. */}
      <button
        type="button"
        data-todo-trigger
        onClick={onOpen}
        className="flex min-w-0 flex-1 flex-col gap-1 overflow-hidden rounded-md px-2 py-1.5 text-left outline-none focus-visible:ring-2 focus-visible:ring-sidebar-ring"
      >
        <TodoRowSummary todo={todo} done={done} />
      </button>
      {todo.locked_by != null && (
        <Button
          data-todo-agent
          data-process-id={todo.locked_by}
          variant="ghost"
          size="sm"
          disabled={onOpenAgent == null}
          onClick={() => onOpenAgent?.(todo.locked_by as number)}
          className="shrink-0 gap-1"
        >
          <Lock aria-hidden />
          {lockOwnerLabel ?? `#${todo.locked_by}`}
        </Button>
      )}
    </div>
  );
}
