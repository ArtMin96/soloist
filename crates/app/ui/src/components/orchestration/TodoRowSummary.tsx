import { MessageSquareIcon, ShieldAlertIcon } from "lucide-react";
import { TagList } from "@/components/orchestration/TagList";
import { Badge } from "@/components/ui/badge";
import { TODO_STATUS, TODO_STATUS_ICON, TODO_STATUS_TONE, unmetBlockerLabel } from "@/lib/todo";
import { cn } from "@/lib/utils";
import type { TodoView } from "@/domain";

interface TodoRowSummaryProps {
  todo: TodoView;
  done: boolean;
}

// What a todo card says about itself in the list, on two lines flush to the same left edge: the
// title with its declared status, then the id, the blocker gate, the tags, and the comment count.
// Two lines rather than one because on a single baseline every one of those competes with the title
// for the same few pixels, and the title is the only thing a person scans for. Nothing here is
// interactive — the whole block is the content of the row's own button, so any control would nest
// inside another one.
export function TodoRowSummary({ todo, done }: TodoRowSummaryProps) {
  const StatusIcon = TODO_STATUS_ICON[todo.doc.status];

  return (
    <>
      <div className="flex w-full min-w-0 items-center gap-2">
        <span
          data-todo-title
          className={cn(
            "type-body min-w-0 flex-1 truncate font-[550]",
            done ? "text-muted-foreground line-through" : "text-foreground",
          )}
        >
          {todo.doc.title}
        </span>
        {/* The status keeps its natural width: it is one of four fixed strings, and a status
            clipped to "In prog…" is one you cannot read. The title is what yields on this line. */}
        <Badge
          data-todo-status
          data-status={todo.doc.status}
          variant="tinted"
          className={cn("shrink-0", TODO_STATUS_TONE[todo.doc.status])}
        >
          <StatusIcon aria-hidden className="shrink-0" />
          <span className="type-label text-foreground">{TODO_STATUS[todo.doc.status]}</span>
        </Badge>
      </div>

      <div className="flex w-full min-w-0 items-center gap-2">
        <span
          data-todo-ref
          className="type-label shrink-0 font-mono tabular-nums text-muted-foreground"
        >
          #{todo.id}
        </span>
        {todo.blocked_by.length > 0 && (
          // A shield, not the declared status's ban glyph: this is the derived gate, and the two
          // must not wear the same mark on a row that can show both at once.
          <span
            data-todo-blockers
            className="type-label flex min-w-0 shrink items-center gap-1 truncate text-muted-foreground"
          >
            <ShieldAlertIcon aria-hidden className="size-3.5 shrink-0 text-status-attention" />
            <span className="min-w-0 truncate">{unmetBlockerLabel(todo.blocked_by.length)}</span>
          </span>
        )}
        <TagList tags={todo.tags} />
        {todo.comments.length > 0 && (
          <span
            data-todo-comments
            className="type-label ml-auto flex shrink-0 items-center gap-1 text-muted-foreground"
          >
            <MessageSquareIcon aria-hidden className="size-3.5" />
            {todo.comments.length}
          </span>
        )}
      </div>
    </>
  );
}
