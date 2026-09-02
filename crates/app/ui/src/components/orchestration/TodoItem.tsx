import { ChevronRight, Link2, Lock, MessageSquareIcon, Pencil } from "lucide-react";
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from "@/components/ui/collapsible";
import { MarkdownView } from "@/components/editor/MarkdownView";
import { CommentComposer } from "@/components/orchestration/CommentComposer";
import { CommentList } from "@/components/orchestration/CommentList";
import { TagList } from "@/components/orchestration/TagList";
import { TodoEditor, type TodoConflict } from "@/components/orchestration/TodoEditor";
import { Button } from "@/components/ui/button";
import { humanizeName } from "@/lib/humanize";
import { TODO_STATUS, TODO_STATUS_ICON, unmetBlockerLabel } from "@/lib/todo";
import { cn } from "@/lib/utils";
import type { SaveOutcome } from "@/store/saveOutcome";
import type { ScratchpadSummary, TodoDoc, TodoView } from "@/domain";

// The edit surface's state for this row, present only while it is being edited. The board owns the
// single edit session (one todo at a time) and hands it here so the expanded row swaps its read
// view for the editor.
export interface TodoEditState {
  initial: TodoDoc;
  initialScratchpad: number | null;
  mountKey: number;
  conflict: TodoConflict | null;
  error: string | null;
  onSave: (doc: TodoDoc, scratchpad: number | null) => Promise<SaveOutcome>;
  onReload: () => void;
  onDone: () => void;
}

interface TodoRowMetaProps {
  todo: TodoView;
  done: boolean;
  showScratchpad: boolean;
}

// The trigger's passive info — id, title, status, the unmet-blocker count, the scratchpad, tags,
// and the comment count — split out so `TodoItem` itself only has to reason about the row's
// interactive structure (the trigger/agent-control split, the read/edit swap), not every optional
// span this declares.
function TodoRowMeta({ todo, done, showScratchpad }: TodoRowMetaProps) {
  const StatusIcon = TODO_STATUS_ICON[todo.doc.status];
  return (
    <>
      <span data-todo-ref className="type-label shrink-0 text-muted-foreground">
        #{todo.id}
      </span>
      <span
        data-todo-title
        className={cn(
          "min-w-0 flex-1 truncate text-[0.8125rem] leading-4",
          done ? "text-muted-foreground line-through" : "text-foreground",
        )}
      >
        {todo.doc.title}
      </span>
      <span
        data-todo-status
        data-status={todo.doc.status}
        className="type-label flex min-w-0 shrink items-center gap-1 text-muted-foreground"
      >
        <StatusIcon aria-hidden className="size-3.5 shrink-0" />
        <span className="min-w-0 truncate">{TODO_STATUS[todo.doc.status]}</span>
      </span>
      {todo.blocked_by.length > 0 && (
        <span
          data-todo-blockers
          className="type-label min-w-0 shrink truncate text-muted-foreground"
        >
          {unmetBlockerLabel(todo.blocked_by.length)}
        </span>
      )}
      {showScratchpad && todo.scratchpad && (
        <span
          data-todo-scratchpad
          className="type-label min-w-0 shrink truncate text-muted-foreground"
        >
          {humanizeName(todo.scratchpad.name)}
        </span>
      )}
      <TagList tags={todo.tags} />
      {todo.comments.length > 0 && (
        <span
          data-todo-comments
          className="type-label flex shrink-0 items-center gap-1 text-muted-foreground"
        >
          <MessageSquareIcon aria-hidden className="size-3" />
          {todo.comments.length}
        </span>
      )}
    </>
  );
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
  /**
   * Opens the agent this row is locked by. Absent when the caller offers no navigation — the
   * control still renders, but disabled, rather than disappearing and shifting the row.
   */
  onOpenAgent?: (process: number) => void;
  /**
   * Whether the row names the scratchpad it derives from. False while the board groups by
   * scratchpad, where the group header already says it and repeating it on every row would be
   * noise; true in the flat view and whenever a filter has flattened the grouping.
   */
  showScratchpad: boolean;
  /** The project's scratchpads, offered in the edit surface's picker. */
  scratchpads: ScratchpadSummary[];
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
  onOpenAgent,
  showScratchpad,
  scratchpads,
  edit,
}: TodoItemProps) {
  const done = todo.doc.status === "done";
  const unmet = new Set(todo.blocked_by);

  return (
    // No separator between rows: a native list draws its structure with the rows themselves, and
    // the open row is marked by a quiet tonal fill instead of a rule.
    <Collapsible
      open={open}
      onOpenChange={onToggle}
      className="rounded-md data-[state=open]:bg-muted/40"
    >
      {/* The trigger and the agent control are siblings, not nested — a lock never buries an
          interactive control inside another one, so each is its own tab stop. */}
      <div className="flex items-center gap-1 pr-1">
        <CollapsibleTrigger
          data-todo-trigger
          className="flex min-h-7 min-w-0 flex-1 items-center gap-2 overflow-hidden rounded-md px-2 py-1 text-left outline-none hover:bg-sidebar-accent focus-visible:bg-sidebar-accent focus-visible:ring-2 focus-visible:ring-sidebar-ring"
        >
          <ChevronRight
            aria-hidden
            className={cn(
              "size-3.5 shrink-0 text-muted-foreground transition-transform duration-[var(--dur-control)] ease-spring-settle",
              open && "rotate-90",
            )}
          />
          <TodoRowMeta todo={todo} done={done} showScratchpad={showScratchpad} />
        </CollapsibleTrigger>
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
            <Lock aria-hidden className="size-3" />
            {lockOwnerLabel ?? `#${todo.locked_by}`}
          </Button>
        )}
      </div>

      {/* Indented to the row's title, so the document reads as belonging to the row above it. */}
      <CollapsibleContent className="flex flex-col gap-3 pt-1 pr-2 pb-3 pl-8 text-[0.8125rem]">
        {edit ? (
          <TodoEditor
            key={edit.mountKey}
            initial={edit.initial}
            initialScratchpad={edit.initialScratchpad}
            scratchpads={scratchpads}
            conflict={edit.conflict}
            error={edit.error}
            onSave={edit.onSave}
            onReload={edit.onReload}
            onDone={edit.onDone}
          />
        ) : (
          <>
            {todo.doc.body && (
              <MarkdownView markdown={todo.doc.body} ariaLabel={`${todo.doc.title} body`} />
            )}

            {todo.blockers.length > 0 && (
              <div className="flex flex-col gap-1">
                <span className="type-label font-[550] text-muted-foreground">Blockers</span>
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
      </CollapsibleContent>
    </Collapsible>
  );
}
