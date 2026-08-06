import { useCallback, useEffect, useState } from "react";
import { onDomainEvent } from "@/api";
import { useReconcile } from "@/store/useReconcile";
import type { DomainEvent } from "@/domain";

// The one event that changes anything a repository surface renders. It carries the project only,
// so a reader re-takes its whole read rather than folding a delta into it.
const SNAPSHOT_EVENTS: ReadonlySet<DomainEvent["type"]> = new Set(["GitStatusChanged"]);

export interface RepositoryRead<T> {
  /** What was read, or null until this request's own answer has arrived. */
  value: T | null;
  /** True until the first read for this request resolves. */
  loading: boolean;
  error: string | null;
  refresh: () => void;
}

/**
 * One read of a repository, re-taken whenever version control says something changed.
 *
 * `request` names what is being asked for — a project, a project and a path, a path and a
 * comparison. It identifies the answer as much as it triggers the read: an answer captured for
 * one request is never shown for another, so switching project or file reports nothing rather
 * than the previous repository's.
 *
 * A null request stops reading and keeps the last answer, because there is nothing it could be
 * confused with: a surface that is merely out of sight — a hidden tab — comes back to the tree
 * it had open rather than to an empty one.
 *
 * Re-reads are coalesced to one per animation frame, so a repository under active change costs
 * one query per frame rather than one per file. `read` must be stable and must change exactly
 * when `request` does.
 *
 * Holds no business logic: the core decides what a change is and whether it is worth announcing;
 * this only asks again.
 */
export function useRepositoryRead<T>(
  request: string | null,
  read: () => Promise<T>,
): RepositoryRead<T> {
  const [snapshot, setSnapshot] = useState<{ request: string | null; value: T | null }>({
    request: null,
    value: null,
  });
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(() => {
    if (request === null) return;
    read()
      .then((value) => {
        setSnapshot({ request, value });
        setError(null);
      })
      .catch((reason: unknown) => setError(String(reason)));
  }, [read, request]);

  useEffect(() => {
    if (request === null) return;
    let cancelled = false;
    let unlisten: (() => void) | undefined;
    let frame: number | null = null;

    const scheduleRefresh = () => {
      if (frame != null) return;
      frame = requestAnimationFrame(() => {
        frame = null;
        refresh();
      });
    };

    // Attach the listener before the first read, so a change announced between the snapshot and
    // the subscription cannot be lost (snapshot-then-deltas).
    onDomainEvent((event) => {
      if (SNAPSHOT_EVENTS.has(event.type)) scheduleRefresh();
    })
      .then((stop) => {
        if (cancelled) {
          stop();
          return;
        }
        unlisten = stop;
        refresh();
      })
      .catch((reason: unknown) => setError(String(reason)));

    return () => {
      cancelled = true;
      unlisten?.();
      if (frame != null) cancelAnimationFrame(frame);
    };
  }, [refresh, request]);

  // Re-read on a backend resync signal or window focus, so a dropped announcement never leaves a
  // surface stale.
  useReconcile(refresh);

  const answered = snapshot.request === request;
  return {
    value: answered || request === null ? snapshot.value : null,
    loading: request !== null && !answered,
    error,
    refresh,
  };
}
