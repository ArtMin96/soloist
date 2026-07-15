import { useCallback, useEffect, useState } from "react";
import { onDomainEvent, orphansResolve } from "@/api";
import type { OrphanInfo } from "@/domain";

export interface OrphanStore {
  /** Surfaced leftover groups awaiting a decision; `null` when no dialog is open. */
  orphans: OrphanInfo[] | null;
  /** Reap one group (SIGKILL + forget); drops it only once the core confirms. */
  killOne: (pgid: number) => void;
  /** Reap every listed group and close; keeps the list if any kill fails. */
  killAll: () => void;
  /** Dismiss without reaping — the groups keep running. */
  leave: () => void;
}

// Surfaces orphaned process groups for a Kill / Kill all / Leave decision. Subscribes to
// OrphansFound (emitted once on launch after reconciliation) and routes a resolution to
// the core, which SIGKILLs and forgets each chosen group; leaving just dismisses. A row is
// dropped only after the core confirms the kill — a failed SIGKILL surfaces the error via
// `reportError` and keeps the row, so the still-running leftover stays actionable. App-
// level because orphans are a per-launch event, not a per-process one.
export function useOrphans(reportError: (reason: unknown) => void): OrphanStore {
  const [orphans, setOrphans] = useState<OrphanInfo[] | null>(null);

  useEffect(() => {
    let cancelled = false;
    let unlisten: (() => void) | undefined;
    onDomainEvent((event) => {
      if (event.type === "OrphansFound") setOrphans(event.orphans);
    })
      .then((stop) => {
        if (cancelled) stop();
        else unlisten = stop;
      })
      .catch(() => {});
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, []);

  const killOne = useCallback(
    (pgid: number) => {
      orphansResolve([pgid])
        .then(() =>
          setOrphans((prev) => {
            const next = prev?.filter((orphan) => orphan.pgid !== pgid) ?? null;
            return next && next.length > 0 ? next : null;
          }),
        )
        .catch(reportError);
    },
    [reportError],
  );

  // Reap each group independently through `killOne`, so on a partial failure the ones
  // that were killed drop and only the ones that failed stay listed with their error.
  const killAll = useCallback(() => {
    orphans?.forEach((orphan) => killOne(orphan.pgid));
  }, [orphans, killOne]);

  const leave = useCallback(() => setOrphans(null), []);

  return { orphans, killOne, killAll, leave };
}
