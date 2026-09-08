import { MessageSquareIcon } from "lucide-react";
import { TagList } from "@/components/common/TagList";
import { TodoBlockerGate, TodoIdentity } from "@/components/orchestration/TodoBlockerGate";
import { Badge } from "@/components/ui/badge";
import { TODO_STATUS, TODO_STATUS_ICON, TODO_STATUS_TONE } from "@/lib/todo";
import { cn } from "@/lib/utils";
import type { TodoView } from "@/domain";

interface TodoRowSummaryProps {
  todo: TodoView;
  done: boolean;
}

// What a todo card says about itself in the list, on lines flush to the same left edge: title and
// declared status first, then operational metadata, with a wrapping tag rail only when tags exist.
// Separating them keeps every badge from competing with the title for the same few pixels. Nothing
// here is interactive — the whole block is the content of the row's own button, so any control
// would nest inside another one.
export function TodoRowSummary({ todo, done }: TodoRowSummaryProps) {
  const StatusIcon = TODO_STATUS_ICON[todo.doc.status];

  return (
    <>
      <div className="flex w-full min-w-0 items-center gap-2">
        <TodoIdentity id={todo.id} />
        <span
          data-todo-title
          className={cn(
            "type-body min-w-0 flex-1 truncate font-[550]",
            done ? "text-muted-foreground" : "text-foreground",
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
          <StatusIcon aria-hidden data-icon="inline-start" />
          <span className="type-label text-foreground">{TODO_STATUS[todo.doc.status]}</span>
        </Badge>
      </div>

      {(todo.blocked_by.length > 0 || todo.comments.length > 0) && (
        <div className="flex w-full min-w-0 items-center gap-2">
          <TodoBlockerGate blockerIds={todo.blocked_by} className="shrink" />
          {todo.comments.length > 0 && (
            <Badge
              data-todo-comments
              aria-label={`${todo.comments.length} ${todo.comments.length === 1 ? "comment" : "comments"}`}
              variant="muted"
              className="ml-auto shrink-0"
            >
              <MessageSquareIcon aria-hidden data-icon="inline-start" />
              {todo.comments.length}
            </Badge>
          )}
        </div>
      )}

      {todo.tags.length > 0 && (
        <span data-todo-tag-row className="flex w-full min-w-0 items-center">
          <TagList tags={todo.tags} wrap className="w-full" />
        </span>
      )}
    </>
  );
}
