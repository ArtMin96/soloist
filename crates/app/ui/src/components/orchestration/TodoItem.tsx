import { ChevronRight, Link2, Lock } from "lucide-react";
import { Collapsible } from "radix-ui";
import { CommentList } from "@/components/orchestration/CommentList";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { TODO_STATUS } from "@/lib/todo";
import { cn } from "@/lib/utils";
import type { TodoView } from "@/domain";

interface TodoItemProps {
  todo: TodoView;
  open: boolean;
  onToggle: () => void;
  titleOf: (id: number) => string | undefined;
  lockOwnerLabel: string | undefined;
  busy: boolean;
  error: string | undefined;
  onComplete: () => void;
  onCopyLink: () => void;
}

// One todo on the board: a row with its declared status, the derived blocked gate, and its lock
// owner, expanding to the disciplined document, its blockers (the unmet ones stand out — the gate),
// its comments with their authors, and the actions. Presentational: completing routes to the
// core, which refuses a blocked todo with a message surfaced below — the UI never pre-empts the gate.
export function TodoItem({
  todo,
  open,
  onToggle,
  titleOf,
  lockOwnerLabel,
  busy,
  error,
  onComplete,
  onCopyLink,
}: TodoItemProps) {
  const done = todo.doc.status === "done";
  const unmet = new Set(todo.blocked_by);

  return (
    <Collapsible.Root open={open} onOpenChange={onToggle} className="border-b last:border-b-0">
      <Collapsible.Trigger className="flex w-full items-center gap-2 py-2 pr-2 pl-1 text-left outline-none hover:bg-sidebar-accent focus-visible:bg-sidebar-accent focus-visible:ring-2 focus-visible:ring-sidebar-ring">
        <ChevronRight
          aria-hidden
          className={cn(
            "size-3.5 shrink-0 text-muted-foreground transition-transform",
            open && "rotate-90",
          )}
        />
        <span
          className={cn(
            "min-w-0 flex-1 truncate text-[0.8125rem]",
            done ? "text-muted-foreground line-through" : "text-foreground",
          )}
        >
          {todo.doc.title}
        </span>
        {todo.blocked && (
          <Badge variant="outline" className="shrink-0 text-[0.6875rem]">
            Blocked
          </Badge>
        )}
        {todo.locked_by != null && (
          <Badge variant="secondary" className="shrink-0 gap-1 text-[0.6875rem]">
            <Lock aria-hidden className="size-3" />
            {lockOwnerLabel ?? `#${todo.locked_by}`}
          </Badge>
        )}
        <span className="shrink-0 text-[0.6875rem] text-muted-foreground">
          {TODO_STATUS[todo.doc.status]}
        </span>
      </Collapsible.Trigger>

      <Collapsible.Content className="flex flex-col gap-3 px-6 pb-3 text-[0.8125rem]">
        {todo.doc.description && (
          <p className="whitespace-pre-wrap text-foreground">{todo.doc.description}</p>
        )}

        <DetailList label="Acceptance criteria" items={todo.doc.acceptance_criteria} />
        <DetailList label="Risks" items={todo.doc.risks} />

        {todo.blockers.length > 0 && (
          <div className="flex flex-col gap-1">
            <span className="text-[0.6875rem] font-[550] text-muted-foreground">Blockers</span>
            <ul className="flex flex-col gap-0.5">
              {todo.blockers.map((id) => (
                <li key={id} className="flex items-center gap-2">
                  <span
                    className={cn(
                      "min-w-0 flex-1 truncate",
                      unmet.has(id) ? "text-foreground" : "text-muted-foreground line-through",
                    )}
                  >
                    {titleOf(id) ?? `Todo #${id}`}
                  </span>
                  <span className="shrink-0 text-[0.6875rem] text-muted-foreground">
                    {unmet.has(id) ? "open" : "done"}
                  </span>
                </li>
              ))}
            </ul>
          </div>
        )}

        <CommentList comments={todo.comments} />

        {error && (
          <p role="alert" className="text-[0.8125rem] text-destructive">
            {error}
          </p>
        )}

        <div className="flex items-center gap-2">
          {!done && (
            <Button size="sm" onClick={onComplete} disabled={busy}>
              {busy ? "Completing…" : "Complete"}
            </Button>
          )}
          <Button variant="ghost" size="sm" onClick={onCopyLink}>
            <Link2 aria-hidden /> Copy link
          </Button>
        </div>
      </Collapsible.Content>
    </Collapsible.Root>
  );
}

function DetailList({ label, items }: { label: string; items: string[] }) {
  if (items.length === 0) return null;
  return (
    <div className="flex flex-col gap-1">
      <span className="text-[0.6875rem] font-[550] text-muted-foreground">{label}</span>
      <ul className="flex list-disc flex-col gap-0.5 pl-4 text-foreground marker:text-muted-foreground">
        {items.map((item, index) => (
          // Read-only display of the disciplined document's lists; index is a stable key here.
          <li key={index}>{item}</li>
        ))}
      </ul>
    </div>
  );
}
