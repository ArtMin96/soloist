import {
  Check,
  ChevronLeft,
  Link2,
  Lock,
  MoreHorizontal,
  Pencil,
  ShieldAlert,
  TriangleAlert,
} from "lucide-react";
import { Section } from "@/components/common/Section";
import { WELL } from "@/components/common/Well";
import { MarkdownView } from "@/components/editor/MarkdownView";
import { CommentThread } from "@/components/orchestration/CommentThread";
import { DETAIL_MEASURE, DetailPaneHeader } from "@/components/orchestration/DetailPaneHeader";
import { TagList } from "@/components/orchestration/TagList";
import { TodoEditor, type TodoConflict } from "@/components/orchestration/TodoEditor";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuGroup,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { Separator } from "@/components/ui/separator";
import { Spinner } from "@/components/ui/spinner";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { humanizeName } from "@/lib/humanize";
import { TODO_STATUS, TODO_STATUS_ICON, TODO_STATUS_TONE, unmetBlockerLabel } from "@/lib/todo";
import { cn } from "@/lib/utils";
import type { SaveOutcome } from "@/store/saveOutcome";
import type { ScratchpadSummary, TodoDoc, TodoView } from "@/domain";

// A control's label, shown only while the pane can carry it. It stays in the accessibility tree at
// every width — a control that loses its name when a split narrows is a control nobody can identify
// — so it is hidden the screen-reader way rather than removed. Paired with a collapse to a square
// button, since a hidden label still leaves its icon gap and its horizontal padding behind.
const LABEL_WIDE = "@max-[20rem]/detail-header:sr-only";
const SQUARE_WIDE =
  "@max-[20rem]/detail-header:size-7 @max-[20rem]/detail-header:justify-center @max-[20rem]/detail-header:gap-0 @max-[20rem]/detail-header:p-0";

// The last label to go. Complete holds its word through every regime but the narrowest, because it
// is the primary action and the only one with a running state to say.
const LABEL_FLOOR = "@max-[12rem]/detail-header:sr-only";
const SQUARE_FLOOR =
  "@max-[12rem]/detail-header:size-7 @max-[12rem]/detail-header:justify-center @max-[12rem]/detail-header:gap-0 @max-[12rem]/detail-header:p-0";

// Below this the two secondary actions stop being rendered inline and become menu items instead.
// The pair and the trigger are mutually exclusive rather than both-rendered-one-hidden, so `Edit`
// is never two tab stops.
const INLINE_ABOVE = "@max-[15rem]/detail-header:hidden";
const MENU_BELOW = "@min-[15rem]/detail-header:hidden";

// The edit surface's state for the open todo, present only while it is being edited. The board owns
// the single edit session (one todo at a time) and hands it here so the panel swaps its read view
// for the editor.
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

interface TodoDetailProps {
  todo: TodoView;
  /** Returns to the board list. */
  onBack: () => void;
  titleOf: (id: number) => string | undefined;
  lockOwnerLabel: string | undefined;
  /**
   * Opens the agent this todo is locked by. Absent when the caller offers no navigation — the
   * control still renders, but disabled, rather than disappearing and leaving the lock unnamed.
   */
  onOpenAgent?: (process: number) => void;
  busy: boolean;
  error: string | undefined;
  onComplete: () => void;
  onCopyLink: () => void;
  onComment: (body: string) => Promise<void>;
  onStartEdit: () => void;
  /** The project's scratchpads, offered in the edit surface's picker. */
  scratchpads: ScratchpadSummary[];
  /** Non-null only while this todo is being edited. */
  edit: TodoEditState | null;
}

// One todo, full width: the board hands the whole pane over to it rather than unfolding a row, so
// the document, its provenance, its blockers and its discussion each get a region of their own.
// Blockers lead, because they are the gate on completing and outrank the prose. The header and the
// refusal strip are pinned; everything else scrolls under them. Two modes share the panel, exactly
// as the row did: the read view below, and — while editing — the editor in place of everything
// under the header. Presentational; completing routes to the core, which refuses a blocked todo
// with a message surfaced in the strip (the UI never pre-empts the gate), and editing and
// commenting route to the same core commands agents use.
export function TodoDetail({
  todo,
  onBack,
  titleOf,
  lockOwnerLabel,
  onOpenAgent,
  busy,
  error,
  onComplete,
  onCopyLink,
  onComment,
  onStartEdit,
  scratchpads,
  edit,
}: TodoDetailProps) {
  const done = todo.doc.status === "done";

  return (
    <article data-todo-detail={todo.id} className="flex h-full min-h-0 flex-col">
      <DetailPaneHeader
        back={
          <Button
            data-todo-back
            variant="ghost"
            size="sm"
            onClick={onBack}
            aria-label="Back to todos"
            className={cn(
              "-ml-2.5 min-w-0 text-muted-foreground @max-[20rem]/detail-header:ml-0",
              SQUARE_WIDE,
            )}
          >
            <ChevronLeft aria-hidden data-icon="inline-start" />
            {/* Names the destination, which is what a back control in a two-panel board should say. */}
            <span className={LABEL_WIDE}>Todos</span>
          </Button>
        }
        actions={
          edit ? (
            // Band 1 stays occupied in edit mode, so the way out of editing is never below the fold
            // on a long todo. `useAutosave` commits on unmount, which this triggers, so leaving does
            // not need an explicit flush of its own.
            <Button size="sm" onClick={edit.onDone} data-todo-done>
              <Check aria-hidden data-icon="inline-start" />
              <span className={LABEL_FLOOR}>Done</span>
            </Button>
          ) : (
            <TodoActions
              done={done}
              busy={busy}
              onComplete={onComplete}
              onCopyLink={onCopyLink}
              onStartEdit={onStartEdit}
            />
          )
        }
        title={todo.doc.title}
        meta={<TodoMetaRail todo={todo} />}
      />

      {/* Pinned rather than scrolled: this answers a control in the header, so it may not be
          somewhere the reader has already scrolled past by the time it appears. */}
      {error && (
        <Alert variant="destructive" className="shrink-0 rounded-none border-x-0 border-t-0">
          <TriangleAlert aria-hidden />
          <AlertDescription>{error}</AlertDescription>
        </Alert>
      )}

      <div className="min-h-0 flex-1 overflow-auto">
        {/* Capped so a long body reads at a comfortable measure instead of running the pane's width,
            and held to the same column the pinned header uses so the two share a left edge. */}
        <div className={cn(DETAIL_MEASURE, "flex flex-col gap-5 py-4")}>
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
            />
          ) : (
            <>
              {todo.blockers.length > 0 && <TodoBlockers todo={todo} titleOf={titleOf} />}

              <TodoProvenance
                todo={todo}
                lockOwnerLabel={lockOwnerLabel}
                onOpenAgent={onOpenAgent}
              />

              <Section title="Description">
                {todo.doc.body ? (
                  // The renderer reads its Markdown once, so it is remounted whenever the document
                  // itself moves. Only a doc write bumps the revision — a comment does not — so this
                  // follows an agent's edit without flashing on every posted comment.
                  <MarkdownView
                    key={`${todo.id}:${todo.revision}`}
                    markdown={todo.doc.body}
                    ariaLabel={`${todo.doc.title} body`}
                  />
                ) : (
                  <p className="type-body text-muted-foreground">No description.</p>
                )}
              </Section>

              {/* Keyed by the todo, so a draft written here never follows the panel to another one. */}
              <CommentThread key={todo.id} comments={todo.comments} onComment={onComment} />
            </>
          )}
        </div>
      </div>
    </article>
  );
}

// The header's third band: the todo's identifiers and state as a rail of uniform-height chips,
// centred against each other. Everything a reader scans for that is not the title lives here, which
// is what leaves the title alone on its own line.
function TodoMetaRail({ todo }: { todo: TodoView }) {
  const StatusIcon = TODO_STATUS_ICON[todo.doc.status];
  return (
    <div className="flex min-h-6 flex-wrap items-center gap-x-2.5 gap-y-1.5">
      <span className="type-label shrink-0 font-mono tabular-nums text-muted-foreground">
        #{todo.id}
      </span>

      {/* The tone dresses the glyph and the chip's tint; the label stays ink, because a
          `--status-*` hue measures as low as 2.48:1 and cannot carry text. */}
      <Badge
        data-todo-status
        data-status={todo.doc.status}
        variant="tinted"
        className={cn("shrink-0", TODO_STATUS_TONE[todo.doc.status])}
      >
        <StatusIcon aria-hidden data-icon="inline-start" />
        <span className="text-foreground">{TODO_STATUS[todo.doc.status]}</span>
      </Badge>

      {/* The gate on the primary action sitting two bands above, so it belongs in the masthead
          rather than only in the Blockers section further down. */}
      {todo.blocked_by.length > 0 && (
        <Badge variant="outline" className="shrink-0">
          <ShieldAlert aria-hidden data-icon="inline-start" className="text-status-attention" />
          {unmetBlockerLabel(todo.blocked_by.length)}
        </Badge>
      )}

      <TagList tags={todo.tags} wrap />
    </div>
  );
}

interface TodoActionsProps {
  /** A completed todo offers no Complete — the action is spent, not merely unavailable. */
  done: boolean;
  busy: boolean;
  onComplete: () => void;
  onCopyLink: () => void;
  onStartEdit: () => void;
}

// The header's action cluster, ordered secondary → separator → primary so Complete is the terminal
// element of the row and unmistakably the one action that finishes the todo. Three shed steps carry
// it down to a 184px pane: labels go first, then the secondary pair folds into a menu, and Complete
// keeps its word longest. Icon-only actions use the shared tooltip so their meaning remains visible
// to pointer and keyboard users after their labels shed.
function TodoActions({ done, busy, onComplete, onCopyLink, onStartEdit }: TodoActionsProps) {
  return (
    <>
      <div className={cn("flex items-center gap-1", INLINE_ABOVE)}>
        <Button
          variant="ghost"
          size="sm"
          onClick={onStartEdit}
          className={cn("text-muted-foreground", SQUARE_WIDE)}
        >
          <Pencil aria-hidden data-icon="inline-start" />
          <span className={LABEL_WIDE}>Edit</span>
        </Button>

        <Tooltip>
          <TooltipTrigger asChild>
            <Button
              variant="ghost"
              size="icon-sm"
              onClick={onCopyLink}
              aria-label="Copy link to todo"
              className="text-muted-foreground"
            >
              <Link2 aria-hidden />
            </Button>
          </TooltipTrigger>
          <TooltipContent>Copy link to todo</TooltipContent>
        </Tooltip>

        {/* Only ever drawn beside Complete: on a done todo it would point at nothing. `h-4` beats
            the primitive's `self-stretch`, which would otherwise run the rule the band's full height. */}
        {!done && <Separator orientation="vertical" className="mx-1 h-4" />}
      </div>

      <DropdownMenu>
        <Tooltip>
          <TooltipTrigger asChild>
            <DropdownMenuTrigger asChild>
              <Button
                variant="ghost"
                size="icon-sm"
                aria-label="More actions"
                className={cn("text-muted-foreground", MENU_BELOW)}
              >
                <MoreHorizontal aria-hidden />
              </Button>
            </DropdownMenuTrigger>
          </TooltipTrigger>
          <TooltipContent>More todo actions</TooltipContent>
        </Tooltip>
        <DropdownMenuContent align="end">
          <DropdownMenuGroup>
            <DropdownMenuItem onSelect={onStartEdit}>
              <Pencil aria-hidden /> Edit todo
            </DropdownMenuItem>
            <DropdownMenuItem onSelect={onCopyLink}>
              <Link2 aria-hidden /> Copy link to todo
            </DropdownMenuItem>
          </DropdownMenuGroup>
        </DropdownMenuContent>
      </DropdownMenu>

      {!done && (
        <Button
          size="sm"
          onClick={onComplete}
          disabled={busy}
          aria-busy={busy}
          className={SQUARE_FLOOR}
        >
          {/* `aria-hidden` suppresses the spinner's built-in "Loading" so the state is announced
              once, by `aria-busy`. The text swap remains the carrier when reduced motion freezes
              the spin. */}
          {busy ? (
            <Spinner aria-hidden data-icon="inline-start" />
          ) : (
            <Check aria-hidden data-icon="inline-start" />
          )}
          <span className={LABEL_FLOOR}>{busy ? "Completing…" : "Complete"}</span>
        </Button>
      )}
    </>
  );
}

interface TodoProvenanceProps {
  todo: TodoView;
  lockOwnerLabel: string | undefined;
  onOpenAgent?: (process: number) => void;
}

// Where the todo came from and who holds it. Both fields are always named, absent or not: a missing
// scratchpad or lock is information, and hiding the row would leave the reader to guess whether it
// was empty or simply not shown.
function TodoProvenance({ todo, lockOwnerLabel, onOpenAgent }: TodoProvenanceProps) {
  const lockedBy = todo.locked_by;
  return (
    <Section title="Details">
      <dl
        className={cn(WELL, "grid grid-cols-[7rem_1fr] items-center gap-x-3 gap-y-1 px-3 py-2.5")}
      >
        <dt className="type-label text-muted-foreground">Scratchpad</dt>
        <dd data-todo-scratchpad className="type-body min-w-0 truncate">
          {todo.scratchpad ? (
            humanizeName(todo.scratchpad.name)
          ) : (
            <span className="text-muted-foreground">Not derived from a scratchpad</span>
          )}
        </dd>

        <dt className="type-label text-muted-foreground">Locked by</dt>
        <dd className="min-w-0">
          {lockedBy == null ? (
            <span className="type-body text-muted-foreground">Not locked</span>
          ) : (
            <Button
              data-todo-agent
              data-process-id={lockedBy}
              variant="ghost"
              size="sm"
              disabled={onOpenAgent == null}
              onClick={() => onOpenAgent?.(lockedBy)}
              className="-ml-2 max-w-full"
            >
              <Lock aria-hidden data-icon="inline-start" />
              <span className="min-w-0 truncate">{lockOwnerLabel ?? `#${lockedBy}`}</span>
            </Button>
          )}
        </dd>
      </dl>
    </Section>
  );
}

// The todos this one waits on, at the top of the panel because they are the gate on completing it.
// `blocked_by` is the unmet subset the core derives — those are the ones still holding it; the rest
// are already done and shown with quieter text. The list scrolls past six entries rather than growing without
// bound, so a heavily blocked todo cannot push its own description off the pane.
function TodoBlockers({
  todo,
  titleOf,
}: {
  todo: TodoView;
  titleOf: (id: number) => string | undefined;
}) {
  const unmet = new Set(todo.blocked_by);
  return (
    <Section
      title="Blockers"
      // Short form here, not `unmetBlockerLabel`: the masthead already carries the full sentence,
      // and under a heading that says "Blockers" the count pairs with "All met" without repeating it.
      aside={todo.blocked_by.length > 0 ? `${todo.blocked_by.length} unmet` : "All met"}
    >
      <div className={cn(WELL, "overflow-hidden")}>
        <ul className="flex max-h-[13.125rem] flex-col divide-y divide-border overflow-y-auto">
          {todo.blockers.map((id) => (
            <li key={id} className="flex items-center gap-2 px-3 py-2">
              <span
                className={cn(
                  "type-body min-w-0 flex-1 truncate",
                  unmet.has(id) ? "text-foreground" : "text-muted-foreground",
                )}
              >
                {titleOf(id) ?? `Todo #${id}`}
              </span>
              <Badge variant={unmet.has(id) ? "outline" : "muted"} className="shrink-0">
                {unmet.has(id) ? "open" : "done"}
              </Badge>
            </li>
          ))}
        </ul>
      </div>
    </Section>
  );
}
