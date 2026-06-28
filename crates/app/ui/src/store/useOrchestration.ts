import { useCallback, useEffect, useState } from "react";
import { onDomainEvent, orchestrationSnapshot } from "@/api";
import { buildOrchestrationTree, type OrchestrationTreeNode } from "@/store/orchestrationTree";
import type { DomainEvent } from "@/domain";

// Domain events that restructure or re-label the agent tree: a process entering or leaving the
// registry, a status or label change, or an agent activity transition. The snapshot is derived on
// read and its events carry ids only, so the hook re-reads the snapshot rather than folding deltas.
const TREE_EVENTS: ReadonlySet<DomainEvent["type"]> = new Set([
  "ProcessSpawned",
  "ProcessStatusChanged",
  "ProcessRemoved",
  "ProcessRenamed",
  "AgentActivityChanged",
]);

export interface OrchestrationStore {
  tree: OrchestrationTreeNode[];
  error: string | null;
  refresh: () => void;
}

// The orchestration tree read model for one project: seeds from the snapshot, then re-reads it
// when a process-lifecycle or agent-activity event signals a change. Re-reads are coalesced to one
// per animation frame, so a chatty run never thrashes the tree (CLAUDE.md §6). Holds no business
// logic — the nesting lives in the pure `buildOrchestrationTree`. A null project clears the tree.
export function useOrchestration(project: number | null): OrchestrationStore {
  const [tree, setTree] = useState<OrchestrationTreeNode[]>([]);
  const [error, setError] = useState<string | null>(null);

  const fail = useCallback((reason: unknown) => setError(String(reason)), []);

  const refresh = useCallback(() => {
    if (project == null) {
      setTree([]);
      return;
    }
    orchestrationSnapshot(project)
      .then((snapshot) => setTree(buildOrchestrationTree(snapshot.agents)))
      .catch(fail);
  }, [project, fail]);

  useEffect(() => {
    if (project == null) {
      setTree([]);
      return;
    }
    let cancelled = false;
    let unlisten: (() => void) | undefined;
    let frame: number | null = null;

    // Coalesce a burst of events into a single re-read on the next frame, so the tree updates at
    // most once per frame however chatty the workers are.
    const scheduleRefresh = () => {
      if (frame != null) return;
      frame = requestAnimationFrame(() => {
        frame = null;
        refresh();
      });
    };

    // Attach the listener before the first read, so an event emitted between the snapshot and the
    // subscription cannot be lost (snapshot-then-deltas).
    onDomainEvent((event) => {
      if (TREE_EVENTS.has(event.type)) scheduleRefresh();
    })
      .then((stop) => {
        if (cancelled) {
          stop();
          return;
        }
        unlisten = stop;
        refresh();
      })
      .catch(fail);

    return () => {
      cancelled = true;
      unlisten?.();
      if (frame != null) cancelAnimationFrame(frame);
    };
  }, [project, refresh, fail]);

  return { tree, error, refresh };
}
