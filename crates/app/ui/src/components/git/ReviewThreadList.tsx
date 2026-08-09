import { useState } from "react";
import { ChevronRightIcon, ExternalLinkIcon, SendIcon } from "lucide-react";
import { MarkdownView } from "@/components/editor/MarkdownView";
import { Button } from "@/components/ui/button";
import { threadPlace } from "@/lib/git";
import { cn } from "@/lib/utils";
import type { ReviewThread } from "@/domain";

const HAND_OFF_LABEL = "Hand to an agent";
const OPEN_LABEL = "Open this comment";
const WHOLE_CHANGE = "On the change";
const OUTDATED = "Outdated";
const NOTHING = "Nobody has written anything on this pull request yet.";

/**
 * Everything written on the pull request, one row per conversation.
 *
 * A row shows where it hangs and the first thing said, and opens to the full text of every comment
 * — which is also the only place the markdown renderer is mounted, so a long argument costs one
 * editor rather than one per comment. Settled conversations are kept behind their own disclosure:
 * the argument is often what explains the code, so it is put out of the way rather than thrown away.
 *
 * Presentational: props in, callbacks out.
 */
export function ReviewThreadList({
  threads,
  onHandOff,
  onOpen,
}: {
  threads: ReviewThread[];
  onHandOff: (thread: ReviewThread) => void;
  onOpen: (url: string) => void;
}) {
  const [settledShown, setSettledShown] = useState(false);
  const open = threads.filter((thread) => !thread.resolved);
  const settled = threads.filter((thread) => thread.resolved);

  if (threads.length === 0) {
    return <p className="px-1 text-[0.8125rem] text-muted-foreground">{NOTHING}</p>;
  }
  return (
    <div className="flex flex-col gap-1">
      <ul className="flex flex-col gap-1">
        {open.map((thread) => (
          <ThreadRow key={thread.id} thread={thread} onHandOff={onHandOff} onOpen={onOpen} />
        ))}
      </ul>
      {settled.length > 0 && (
        <>
          <Disclosure
            open={settledShown}
            label={`${settled.length} settled`}
            onToggle={() => setSettledShown((shown) => !shown)}
          />
          {settledShown && (
            <ul className="flex flex-col gap-1">
              {settled.map((thread) => (
                <ThreadRow key={thread.id} thread={thread} onHandOff={onHandOff} onOpen={onOpen} />
              ))}
            </ul>
          )}
        </>
      )}
    </div>
  );
}

function ThreadRow({
  thread,
  onHandOff,
  onOpen,
}: {
  thread: ReviewThread;
  onHandOff: (thread: ReviewThread) => void;
  onOpen: (url: string) => void;
}) {
  const [open, setOpen] = useState(false);
  const place = threadPlace(thread);
  const first = thread.comments[0];
  return (
    <li className="rounded-md border border-border">
      <div className="group/thread flex h-8 items-center gap-2 px-1">
        <Disclosure
          open={open}
          label={place ?? WHOLE_CHANGE}
          mono={place !== null}
          onToggle={() => setOpen((shown) => !shown)}
        />
        {thread.outdated && (
          <span className="shrink-0 type-label text-muted-foreground">{OUTDATED}</span>
        )}
        {first !== undefined && (
          <p className="min-w-0 flex-1 truncate text-[0.8125rem] text-muted-foreground">
            {`${first.author}: ${first.body}`}
          </p>
        )}
        <Button
          variant="ghost"
          size="icon-xs"
          aria-label={HAND_OFF_LABEL}
          className="shrink-0 opacity-0 transition-opacity group-hover/thread:opacity-100 focus-visible:opacity-100"
          onClick={() => onHandOff(thread)}
        >
          <SendIcon />
        </Button>
        {thread.url !== null && (
          <Button
            variant="ghost"
            size="icon-xs"
            aria-label={OPEN_LABEL}
            className="shrink-0 opacity-0 transition-opacity group-hover/thread:opacity-100 focus-visible:opacity-100"
            onClick={() => onOpen(thread.url ?? "")}
          >
            <ExternalLinkIcon />
          </Button>
        )}
      </div>
      {open && (
        <div className="flex flex-col gap-3 border-t border-border px-3 py-2">
          {/* Keyed by the address the service gave each comment, and by the thread's own for the
              one kind that has none — a submitted review's summary, which is always the only
              comment in its thread. Never by position: a conversation grows at the end and a row
              keyed by where it sat would take the next comment's text on the next read. */}
          {thread.comments.map((comment) => (
            <div key={comment.url ?? thread.id} className="flex flex-col gap-1">
              <p className="type-label text-muted-foreground">{comment.author}</p>
              <MarkdownView markdown={comment.body} ariaLabel={`${comment.author}'s comment`} />
            </div>
          ))}
        </div>
      )}
    </li>
  );
}

/** A disclosure control that names what it opens; the chevron turns rather than swapping glyph. */
function Disclosure({
  open,
  label,
  mono,
  onToggle,
}: {
  open: boolean;
  label: string;
  mono?: boolean;
  onToggle: () => void;
}) {
  return (
    <button
      type="button"
      aria-expanded={open}
      onClick={onToggle}
      className="flex min-w-0 shrink items-center gap-1 rounded-md px-1 py-0.5 text-left text-[0.8125rem] hover:bg-muted focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-ring"
    >
      <ChevronRightIcon
        aria-hidden
        className={cn(
          "size-3.5 shrink-0 text-muted-foreground transition-transform motion-reduce:transition-none",
          open && "rotate-90",
        )}
      />
      <span className={cn("truncate", mono === true && "font-mono")}>{label}</span>
    </button>
  );
}
