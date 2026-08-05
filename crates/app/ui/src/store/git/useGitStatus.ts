import { useCallback, useEffect, useState } from "react";
import { gitStatus, onDomainEvent } from "@/api";
import { useReconcile } from "@/store/useReconcile";
import type { DomainEvent, GitStatus } from "@/domain";

// The one event that changes anything the git rail renders. It carries the project only, so the
// hook re-reads the snapshot rather than folding a delta.
const SNAPSHOT_EVENTS: ReadonlySet<DomainEvent["type"]> = new Set(["GitStatusChanged"]);

export interface GitStatusStore {
  /** The working tree, or null for a project that is not a repository. */
  status: GitStatus | null;
  /** True until the first read for this project resolves — the rail waits rather than claiming
   *  a repository has no changes when it has simply not been read yet. */
  loading: boolean;
  error: string | null;
  refresh: () => void;
}

interface Snapshot {
  forProject: number | null;
  status: GitStatus | null;
}

const EMPTY: Snapshot = { forProject: null, status: null };

/**
 * A project's working-tree status, seeded from the snapshot and re-read whenever version
 * control says something changed. Re-reads are coalesced to one per animation frame, so a
 * repository under active change costs one query per frame rather than one per file.
 *
 * Holds no business logic: the core decides what a change is and whether it is worth
 * announcing; this only asks again. A null project clears everything.
 */
export function useGitStatus(project: number | null): GitStatusStore {
  const [snapshot, setSnapshot] = useState(EMPTY);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(() => {
    if (project == null) return;
    gitStatus(project)
      .then((status) => {
        setSnapshot({ forProject: project, status });
        setError(null);
      })
      .catch((reason: unknown) => setError(String(reason)));
  }, [project]);

  useEffect(() => {
    if (project == null) return;
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
  }, [project, refresh]);

  // Re-read on a backend resync signal or window focus, so a dropped announcement never leaves
  // the rail stale.
  useReconcile(refresh);

  // A snapshot captured for another project is stale: report nothing for this one until its own
  // read arrives, so switching projects never shows the previous repository's changes.
  const fresh = snapshot.forProject === project;
  return {
    status: fresh ? snapshot.status : null,
    loading: project != null && !fresh,
    error,
    refresh,
  };
}
