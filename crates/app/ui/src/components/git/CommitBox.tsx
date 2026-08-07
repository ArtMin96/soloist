import { useId, useState } from "react";
import { SparklesIcon } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import { Textarea } from "@/components/ui/textarea";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import type { FileChange } from "@/domain";

const PLACEHOLDER = "Message";
const COMMIT_LABEL = "Commit";
const AMEND_LABEL = "Amend";
const AMEND_HINT = "Replace the last commit instead of adding one";
const NOTHING_STAGED = "Nothing is staged to commit";
const DRAFT_LABEL = "Draft a message";
const DRAFT_HINT = "Describe the staged change with your assist tool, to edit before committing";
const DRAFTING = "Drafting a message…";

/**
 * The message, and what to do with it. Whether a commit is allowed is the core's answer — this
 * only declines to ask a question whose answer is already known: an empty message, and a first
 * commit with nothing to record. Amending needs neither, because it is how a message is
 * corrected.
 *
 * A drafted message lands in the box like any other and is committed by the same button, because
 * it is a draft: nothing here treats it as more finished than what somebody typed.
 *
 * Presentational: it holds the message being typed and nothing else.
 */
export function CommitBox({
  changes,
  busy,
  draft,
  onCommit,
}: {
  changes: FileChange[];
  busy: boolean;
  /** Asking for a message, or `null` when no tool is configured to draft one. */
  draft: { drafting: boolean; request: () => Promise<string | null> } | null;
  /** Resolves true when the commit was recorded, which is when the message is cleared. */
  onCommit: (message: string, amend: boolean) => Promise<boolean>;
}) {
  const amendId = useId();
  const [message, setMessage] = useState("");
  const [amend, setAmend] = useState(false);
  const staged = changes.some((change) => change.status.staged !== null);
  const ready = message.trim() !== "" && (amend || staged);
  const drafting = draft?.drafting === true;

  const commit = () => {
    void onCommit(message, amend).then((recorded) => {
      if (recorded) {
        setMessage("");
        setAmend(false);
      }
    });
  };

  const requestDraft = () => {
    void draft?.request().then((drafted) => {
      if (drafted !== null) setMessage(drafted);
    });
  };

  return (
    <div className="flex shrink-0 flex-col gap-2 border-t border-sidebar-border p-3">
      <Textarea
        value={message}
        aria-label="Commit message"
        placeholder={PLACEHOLDER}
        rows={3}
        className="resize-none"
        onChange={(event) => setMessage(event.target.value)}
      />
      <div className="flex items-center gap-2">
        <Checkbox
          id={amendId}
          checked={amend}
          onCheckedChange={(checked) => setAmend(checked === true)}
        />
        <label htmlFor={amendId} className="text-[0.8125rem]" title={AMEND_HINT}>
          {AMEND_LABEL}
        </label>
        {drafting ? (
          <p className="min-w-0 flex-1 truncate text-[0.6875rem] text-muted-foreground">
            {DRAFTING}
          </p>
        ) : (
          !staged &&
          !amend && (
            <p className="min-w-0 flex-1 truncate text-[0.6875rem] text-muted-foreground">
              {NOTHING_STAGED}
            </p>
          )
        )}
        {/* Absent rather than disabled where no tool is configured: an action nobody may take is
            not an action. Disabled is kept for the one that is momentarily pending. */}
        {draft !== null && (
          <Tooltip>
            <TooltipTrigger asChild>
              <Button
                variant="ghost"
                size="icon-xs"
                className="ms-auto"
                aria-label={DRAFT_LABEL}
                disabled={!staged || drafting || busy}
                onClick={requestDraft}
              >
                <SparklesIcon className={drafting ? "motion-safe:animate-pulse" : undefined} />
              </Button>
            </TooltipTrigger>
            <TooltipContent>{DRAFT_HINT}</TooltipContent>
          </Tooltip>
        )}
        <Button
          size="sm"
          className={draft === null ? "ms-auto" : undefined}
          disabled={!ready || busy || drafting}
          onClick={commit}
        >
          {COMMIT_LABEL}
        </Button>
      </div>
    </div>
  );
}
