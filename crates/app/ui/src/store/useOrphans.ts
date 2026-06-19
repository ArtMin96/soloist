import { useCallback, useEffect, useState } from "react";
import { onDomainEvent, orphansResolve } from "@/api";
import type { OrphanInfo } from "@/domain";

export interface OrphanStore {
  /** Surfaced leftover groups awaiting a decision; `null` when no dialog is open. */
  orphans: OrphanInfo[] | null;
  /** Reap one group (SIGKILL + forget), dropping it from the list. */
  killOne: (pgid: number) => void;
  /** Reap every listed group and close. */
  killAll: () => void;
  /** Dismiss without reaping — the groups keep running. */
  leave: () => void;
}

// Surfaces orphaned process groups for a Kill / Kill all / Leave decision. Subscribes to
// OrphansFound (emitted once on launch after reconciliation) and routes a resolution to
// the core, which SIGKILLs and forgets each chosen group; leaving just dismisses. App-
// level because orphans are a per-launch event, not a per-process one.
export function useOrphans(): OrphanStore {
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

  const killOne = useCallback((pgid: number) => {
    void orphansResolve([pgid]).catch(() => {});
    setOrphans((prev) => {
      const next = prev?.filter((orphan) => orphan.pgid !== pgid) ?? null;
      return next && next.length > 0 ? next : null;
    });
  }, []);

  const killAll = useCallback(() => {
    if (orphans) void orphansResolve(orphans.map((orphan) => orphan.pgid)).catch(() => {});
    setOrphans(null);
  }, [orphans]);

  const leave = useCallback(() => setOrphans(null), []);

  return { orphans, killOne, killAll, leave };
}
