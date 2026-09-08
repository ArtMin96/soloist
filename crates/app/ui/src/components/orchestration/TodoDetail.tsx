import { Check, Lock } from "lucide-react";
import {
  DETAIL_DONE_ATTRIBUTE,
  DetailBackButton,
  DetailBody,
  DetailNotice,
  DetailPaneHeader,
  LABEL_FLOOR,
} from "@/components/common/DetailPane";
import { Section } from "@/components/common/Section";
import { TagList } from "@/components/common/TagList";
import { WELL } from "@/components/common/Well";
import { MarkdownView } from "@/components/editor/MarkdownView";
import { CommentThread } from "@/components/orchestration/CommentThread";
import { TodoActions } from "@/components/orchestration/TodoActions";
import { TodoBlockerGate, TodoIdentity } from "@/components/orchestration/TodoBlockerGate";
import { TodoEditor, type TodoConflict } from "@/components/orchestration/TodoEditor";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { humanizeName } from "@/lib/humanize";
import { TODO_STATUS, TODO_STATUS_ICON, TODO_STATUS_TONE } from "@/lib/todo";
import { cn } from "@/lib/utils";
import type { SaveOutcome } from "@/store/saveOutcome";
import type { ScratchpadSummary, TodoDoc, TodoView } from "@/domain";

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
        back={<DetailBackButton destination="todos" onClick={onBack} />}
        actions={
          edit ? (
            // Band 1 stays occupied in edit mode, so the way out of editing is never below the fold
            // on a long todo. `useAutosave` commits on unmount, which this triggers, so leaving does
            // not need an explicit flush of its own.
            <Button size="sm" onClick={edit.onDone} {...{ [DETAIL_DONE_ATTRIBUTE]: "" }}>
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
        title={
          <div className="flex min-w-0 items-start gap-2">
            <h2 className="type-title min-w-0 flex-1 font-[560] tracking-[var(--tracking-title)] text-pretty break-words text-foreground @max-[16rem]/detail-header:line-clamp-3">
              {todo.doc.title}
            </h2>
            <TodoIdentity id={todo.id} />
          </div>
        }
        meta={<TodoMetaRail todo={todo} />}
      />

      {error && <DetailNotice message={error} />}

      <DetailBody>
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

            <TodoProvenance todo={todo} lockOwnerLabel={lockOwnerLabel} onOpenAgent={onOpenAgent} />

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
      </DetailBody>
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
      <TodoBlockerGate blockerIds={todo.blocked_by} variant="detail" className="shrink-0" />

      <TagList tags={todo.tags} wrap />
    </div>
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
            <li key={id} className="flex items-center gap-3 px-3 py-2">
              <div className="flex min-w-0 flex-1 items-baseline gap-2">
                <span className="type-label shrink-0 font-mono tabular-nums text-muted-foreground">
                  Todo #{id}
                </span>
                {titleOf(id) && (
                  <span
                    className={cn(
                      "type-body min-w-0 truncate",
                      unmet.has(id) ? "text-foreground" : "text-muted-foreground",
                    )}
                  >
                    {titleOf(id)}
                  </span>
                )}
              </div>
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
