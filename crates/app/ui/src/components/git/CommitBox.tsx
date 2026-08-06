import { useId, useState } from "react";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import { Textarea } from "@/components/ui/textarea";
import type { FileChange } from "@/domain";

const PLACEHOLDER = "Message";
const COMMIT_LABEL = "Commit";
const AMEND_LABEL = "Amend";
const AMEND_HINT = "Replace the last commit instead of adding one";
const NOTHING_STAGED = "Nothing is staged to commit";

/**
 * The message, and what to do with it. Whether a commit is allowed is the core's answer — this
 * only declines to ask a question whose answer is already known: an empty message, and a first
 * commit with nothing to record. Amending needs neither, because it is how a message is
 * corrected.
 *
 * Presentational: it holds the message being typed and nothing else.
 */
export function CommitBox({
  changes,
  busy,
  onCommit,
}: {
  changes: FileChange[];
  busy: boolean;
  /** Resolves true when the commit was recorded, which is when the message is cleared. */
  onCommit: (message: string, amend: boolean) => Promise<boolean>;
}) {
  const amendId = useId();
  const [message, setMessage] = useState("");
  const [amend, setAmend] = useState(false);
  const staged = changes.some((change) => change.status.staged !== null);
  const ready = message.trim() !== "" && (amend || staged);

  const commit = () => {
    void onCommit(message, amend).then((recorded) => {
      if (recorded) {
        setMessage("");
        setAmend(false);
      }
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
        {!staged && !amend && (
          <p className="min-w-0 flex-1 truncate text-[0.6875rem] text-muted-foreground">
            {NOTHING_STAGED}
          </p>
        )}
        <Button size="sm" className="ms-auto" disabled={!ready || busy} onClick={commit}>
          {COMMIT_LABEL}
        </Button>
      </div>
    </div>
  );
}
