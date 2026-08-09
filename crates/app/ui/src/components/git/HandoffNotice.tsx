import { CopyIcon } from "lucide-react";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import type { Handoff } from "@/domain";

const COPY_TITLE = "Nothing is running to hand this to";
const COPY_BODY =
  "No single agent of this project is running, so the context is here to take. Start an agent, or paste this into one yourself.";
const COPY_LABEL = "Copy the context";
const DISMISS_LABEL = "Close";

/**
 * What became of a handoff.
 *
 * A delivery says so quietly and gets out of the way; having nowhere to deliver opens the context
 * for the reader to take, because the alternative — a button that does nothing when no agent is
 * running — is the silent no-op this exists to avoid.
 *
 * Presentational: props in, callbacks out.
 */
export function HandoffNotice({
  handoff,
  onCopy,
  onDismiss,
}: {
  handoff: Handoff | null;
  onCopy: (text: string) => void;
  onDismiss: () => void;
}) {
  if (handoff === null || handoff.delivery === "delivered") return null;
  return (
    <Dialog open onOpenChange={(next) => !next && onDismiss()}>
      <DialogContent className="max-w-2xl">
        <DialogHeader>
          <DialogTitle>{COPY_TITLE}</DialogTitle>
          <DialogDescription>{COPY_BODY}</DialogDescription>
        </DialogHeader>
        <pre className="max-h-72 overflow-auto rounded-md border border-border bg-muted p-3 font-mono text-[0.8125rem] whitespace-pre-wrap">
          {handoff.text}
        </pre>
        <DialogFooter>
          <Button variant="ghost" onClick={onDismiss}>
            {DISMISS_LABEL}
          </Button>
          <Button onClick={() => onCopy(handoff.text)}>
            <CopyIcon aria-hidden />
            {COPY_LABEL}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
