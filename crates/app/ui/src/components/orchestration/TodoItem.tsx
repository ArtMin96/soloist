import { ChevronRight, Link2, Lock, Pencil } from "lucide-react";
import { Collapsible } from "radix-ui";
import { CommentComposer } from "@/components/orchestration/CommentComposer";
import { CommentList } from "@/components/orchestration/CommentList";
import { TodoEditor, type TodoConflict } from "@/components/orchestration/TodoEditor";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { TODO_STATUS } from "@/lib/todo";
import { cn } from "@/lib/utils";
import type { TodoDoc, TodoView } from "@/domain";

// The edit surface's state for this row, present only while it is being edited. The board owns the
// single edit session (one todo at a time) and hands it here so the expanded row swaps its read
// view for the editor.
export interface TodoEditState {
  initial: TodoDoc;
  mountKey: number;
  conflict: TodoConflict | null;
  error: string | null;
  onSave: (doc: TodoDoc) => Promise<void>;
  onReload: () => void;
  onDone: () => void;
}

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
  onComment: (body: string) => Promise<void>;
  onStartEdit: () => void;
  /** Non-null only while this row is the one being edited. */
  edit: TodoEditState | null;
}

// One todo on the board: a row with its declared status, the derived blocked gate, and its lock
// owner, expanding to its document and actions. Two expanded modes share the row: the read view
// (Markdown body, blockers, comments, a comment composer, and the actions) and, while editing, the
// inline editor in place of the read view. Presentational — completing routes to the core, which
// refuses a blocked todo with a message surfaced below (the UI never pre-empts the gate); creating,
// editing, and commenting all route to the same core commands agents use.
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
  onComment,
  onStartEdit,
  edit,
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
          <Badge variant="outline" className="shrink-0">
            Blocked
          </Badge>
        )}
        {todo.locked_by != null && (
          <Badge variant="muted" className="shrink-0 gap-1">
            <Lock aria-hidden className="size-3" />
            {lockOwnerLabel ?? `#${todo.locked_by}`}
          </Badge>
        )}
        <span className="shrink-0 text-[0.6875rem] text-muted-foreground">
          {TODO_STATUS[todo.doc.status]}
        </span>
      </Collapsible.Trigger>

      <Collapsible.Content className="flex flex-col gap-3 px-6 pb-3 text-[0.8125rem]">
        {edit ? (
          <TodoEditor
            key={edit.mountKey}
            initial={edit.initial}
            conflict={edit.conflict}
            error={edit.error}
            onSave={edit.onSave}
            onReload={edit.onReload}
            onDone={edit.onDone}
          />
        ) : (
          <>
            {todo.doc.body && (
              <p className="whitespace-pre-wrap text-foreground">{todo.doc.body}</p>
            )}

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

            <div className="flex flex-col gap-1.5">
              <CommentList comments={todo.comments} />
              <CommentComposer onSubmit={onComment} />
            </div>

            {error && (
              <p role="alert" className="text-[0.8125rem] text-destructive">
                {error}
              </p>
            )}

            <div className="flex items-center gap-2">
              <Button variant="ghost" size="sm" onClick={onStartEdit}>
                <Pencil aria-hidden /> Edit
              </Button>
              {!done && (
                <Button size="sm" onClick={onComplete} disabled={busy}>
                  {busy ? "Completing…" : "Complete"}
                </Button>
              )}
              <Button variant="ghost" size="sm" onClick={onCopyLink}>
                <Link2 aria-hidden /> Copy link
              </Button>
            </div>
          </>
        )}
      </Collapsible.Content>
    </Collapsible.Root>
  );
}
