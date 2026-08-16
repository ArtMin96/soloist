import { useEffect, useRef } from "react";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { plainReason } from "@/lib/plainText";
import type { TrustRequest } from "@/domain";

interface TrustRequestDialogProps {
  /** The open requests, oldest first; the first is decided and the rest queue behind it. */
  requests: TrustRequest[];
  onApprove: (request: TrustRequest) => void;
  onDeny: (request: TrustRequest) => void;
}

// A process asked to run a command line this project has never trusted (A6, the agent-initiated
// half). This is a different question from the `solo.yml` review that `TrustDialog` answers: there
// the user is confirming commands they wrote themselves, here something else is asking for
// arbitrary code execution and the user is the only gate. So the dialog is built against the one
// failure that actually matters — approval fatigue — rather than against the happy path:
//
//   - The command line is the largest, first, and most legible thing on screen; the reason is
//     context beneath it. Approving is impossible without the command being visible.
//   - The reason is the requester's own words, so it is rendered as an attributed quotation in
//     plain text: no markup, no links, control characters flattened, and a bounded height so a
//     long or hostile reason cannot push the command line or the buttons out of view.
//   - Deny is the low-friction path: it is the focused control on open and Escape takes it.
//     Approve is never focused by default, and there is no accent primary here, because Soloist
//     does not recommend approving.
//   - The asking process is named *and* numbered, so "who is asking" is answerable.
export function TrustRequestDialog({ requests, onApprove, onDeny }: TrustRequestDialogProps) {
  const request = requests[0] ?? null;
  const denyRef = useRef<HTMLButtonElement>(null);

  // Re-focus deny as each request in the queue comes forward, not only when the dialog opens: the
  // content changes underneath a dialog that stays mounted, and a focused Approve inherited from
  // the previous prompt is exactly the accident this guards against.
  useEffect(() => {
    if (request) denyRef.current?.focus();
  }, [request]);

  if (!request) return null;
  const env = Object.entries(request.review.env);
  const asker = `${request.requested_by_label} (process ${request.requested_by})`;

  return (
    <Dialog
      open
      onOpenChange={(next) => {
        // Escape resolves the request rather than hiding it: a prompt the user waved away would
        // reopen on the next render, and leaving it undecided strands the process that asked.
        if (!next) onDeny(request);
      }}
    >
      <DialogContent
        showCloseButton={false}
        className="max-w-lg"
        // A stray click outside must not decide anything either way.
        onPointerDownOutside={(event) => event.preventDefault()}
        onOpenAutoFocus={(event) => {
          event.preventDefault();
          denyRef.current?.focus();
        }}
      >
        <DialogHeader>
          <DialogTitle>Run a command {request.requested_by_label} asked for?</DialogTitle>
          <DialogDescription>
            This command is not in your solo.yml — {asker} is asking for it. Approving trusts this
            exact command line, working directory, and environment in this project, so it can run
            again later without asking. Revoke it any time in Project settings.
          </DialogDescription>
        </DialogHeader>

        <section
          aria-label="The command that would run"
          className="flex flex-col gap-1.5 rounded-md border border-border bg-muted px-3 py-2.5"
        >
          <code className="block break-all whitespace-pre-wrap font-mono text-sm text-foreground">
            {request.review.command}
          </code>
          <code className="block break-all whitespace-pre-wrap font-mono text-xs text-muted-foreground">
            in {request.review.working_dir ?? "the project root"}
          </code>
          {env.length > 0 && (
            <ul className="flex flex-col gap-0.5">
              {env.map(([key, value]) => (
                <li key={key}>
                  <code className="block break-all whitespace-pre-wrap font-mono text-xs text-muted-foreground">
                    {`${key}=${value}`}
                  </code>
                </li>
              ))}
            </ul>
          )}
        </section>

        <figure className="flex flex-col gap-1.5">
          <blockquote className="max-h-24 overflow-y-auto border-l-2 border-border pl-3 text-xs whitespace-pre-wrap text-muted-foreground">
            {plainReason(request.reason)}
          </blockquote>
          <figcaption className="text-xs text-muted-foreground">
            — {asker}, in its own words
          </figcaption>
        </figure>

        <DialogFooter className="sm:justify-between">
          {requests.length > 1 ? (
            <span className="text-xs text-muted-foreground">
              {requests.length - 1} more waiting
            </span>
          ) : (
            <span />
          )}
          <div className="flex gap-2">
            <Button variant="outline" ref={denyRef} onClick={() => onDeny(request)}>
              Deny
            </Button>
            <Button variant="ghost" onClick={() => onApprove(request)}>
              Approve
            </Button>
          </div>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
