import { useCallback, useEffect, useState } from "react";
import { onDomainEvent, trustRequestApprove, trustRequestDeny } from "@/api";
import type { TrustRequest } from "@/domain";

export interface TrustRequestStore {
  /** The requests awaiting a decision, oldest first; the dialog shows the first. */
  requests: TrustRequest[];
  /** Approve the request, authorizing exactly the variant its review displays. */
  approve: (request: TrustRequest) => void;
  /** Decline it. Nothing is trusted, and the requester is told. */
  deny: (request: TrustRequest) => void;
}

// Open trust requests: a bound process asked the user to trust a command line that is not in this
// project's `solo.yml`, and only the person at the keyboard can answer.
//
// The queue is driven entirely by the event stream and never polled, because a request is
// ephemeral — it exists for this run only, and a `TrustRequestResolved` is the *whole* story of it
// ending, whether the user decided it, it aged out, or the process that asked closed. That last
// case is why removal is not conditional on why: a prompt for a process that no longer exists must
// disappear rather than invite a grant on a dead requester's behalf.
//
// A grant is refused unless the core can still re-derive the exact variant the dialog showed, so
// the hash travels back with the approval rather than the core trusting whatever is pending.
export function useTrustRequests(
  refresh: () => void,
  reportError: (reason: unknown) => void,
): TrustRequestStore {
  const [requests, setRequests] = useState<TrustRequest[]>([]);

  useEffect(() => {
    let cancelled = false;
    let unlisten: (() => void) | undefined;
    onDomainEvent((event) => {
      if (event.type === "TrustRequested") {
        setRequests((open) =>
          open.some((request) => request.id === event.request.id) ? open : [...open, event.request],
        );
      } else if (event.type === "TrustRequestResolved") {
        setRequests((open) => open.filter((request) => request.id !== event.id));
      }
    })
      .then((stop) => {
        if (cancelled) stop();
        else unlisten = stop;
      })
      .catch(reportError);
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [reportError]);

  // A decision leaves the queue only once the core accepted it: a refused approval keeps the
  // request on screen with the reason it was refused, rather than silently dropping the prompt.
  const decide = useCallback(
    (id: number, decision: Promise<void>, reload: boolean) => {
      decision
        .then(() => {
          setRequests((open) => open.filter((request) => request.id !== id));
          if (reload) refresh();
        })
        .catch(reportError);
    },
    [refresh, reportError],
  );

  const approve = useCallback(
    (request: TrustRequest) =>
      decide(request.id, trustRequestApprove(request.id, request.review.variant_hash), true),
    [decide],
  );

  const deny = useCallback(
    (request: TrustRequest) => decide(request.id, trustRequestDeny(request.id), false),
    [decide],
  );

  return { requests, approve, deny };
}
