import { useCallback, useEffect, useState } from "react";
import { lineageEdges, onDomainEvent } from "@/api";
import { useReconcile } from "@/store/useReconcile";
import type { DomainEvent } from "@/domain";

// Domain events that can change the lineage map: a spawn records an edge, a removal drops one,
// and a status change is the settle signal that follows a spawn (the edge is recorded just after
// `ProcessSpawned` is emitted, so the status re-read is what reliably picks it up).
const LINEAGE_EVENTS: ReadonlySet<DomainEvent["type"]> = new Set([
  "ProcessSpawned",
  "ProcessStatusChanged",
  "ProcessRemoved",
]);

const EMPTY: ReadonlyMap<number, number> = new Map();

// The live spawn-lineage map (worker id → lead id) across all projects — what the sidebar joins
// onto its process list to nest workers under their leads. Seeds from the one read, then re-reads
// on process lifecycle events, coalesced to one read per animation frame. Holds no business
// logic — the nesting itself lives in the pure grouping.
export function useLineage(): ReadonlyMap<number, number> {
  const [parents, setParents] = useState<ReadonlyMap<number, number>>(EMPTY);

  const refresh = useCallback(() => {
    lineageEdges()
      .then((edges) => setParents(new Map(edges.map((edge) => [edge.child, edge.parent]))))
      .catch(() => setParents(EMPTY));
  }, []);

  useEffect(() => {
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

    // Attach the listener before the first read, so an event emitted between the read and the
    // subscription cannot be lost (snapshot-then-deltas).
    onDomainEvent((event) => {
      if (LINEAGE_EVENTS.has(event.type)) scheduleRefresh();
    })
      .then((stop) => {
        if (cancelled) {
          stop();
          return;
        }
        unlisten = stop;
        refresh();
      })
      .catch(() => setParents(EMPTY));

    return () => {
      cancelled = true;
      unlisten?.();
      if (frame != null) cancelAnimationFrame(frame);
    };
  }, [refresh]);

  // Re-read on a backend resync signal or window focus, so a dropped lifecycle delta never leaves
  // the sidebar's worker nesting stale.
  useReconcile(refresh);

  return parents;
}
