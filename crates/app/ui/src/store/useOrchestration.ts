import { useCallback, useEffect, useState } from "react";
import { onDomainEvent, orchestrationSnapshot } from "@/api";
import { buildOrchestrationTree, type OrchestrationTreeNode } from "@/store/orchestrationTree";
import type { AgentNode, DomainEvent, ScratchpadSummary, TodoView } from "@/domain";

// Domain events that change anything the orchestration surface renders: a process entering or
// leaving the registry, a status / label / activity change (the agent tree), or a todo or scratchpad
// mutation (the coordination panels). The snapshot is derived on read and its events carry ids only,
// so the hook re-reads the one snapshot rather than folding deltas.
const SNAPSHOT_EVENTS: ReadonlySet<DomainEvent["type"]> = new Set([
  "ProcessSpawned",
  "ProcessStatusChanged",
  "ProcessRemoved",
  "ProcessRenamed",
  "AgentActivityChanged",
  "TodoChanged",
  "ScratchpadChanged",
]);

export interface OrchestrationStore {
  tree: OrchestrationTreeNode[];
  /** The flat agent list (registry order) — the tree's source, kept for id→label lookups. */
  agents: AgentNode[];
  todos: TodoView[];
  scratchpads: ScratchpadSummary[];
  error: string | null;
  refresh: () => void;
}

const EMPTY: Omit<OrchestrationStore, "error" | "refresh"> = {
  tree: [],
  agents: [],
  todos: [],
  scratchpads: [],
};

// The orchestration read model for one project — the agent tree plus the coordination state the
// panels render (todos, scratchpad summaries). Seeds from the snapshot, then re-reads it when a
// process-lifecycle, agent-activity, todo, or scratchpad event signals a change. Re-reads are
// coalesced to one per animation frame, so a chatty run never thrashes the surface (CLAUDE.md §6).
// Holds no business logic — the tree nesting lives in the pure `buildOrchestrationTree`. A null
// project clears everything.
export function useOrchestration(project: number | null): OrchestrationStore {
  const [snapshot, setSnapshot] = useState(EMPTY);
  const [error, setError] = useState<string | null>(null);

  const fail = useCallback((reason: unknown) => setError(String(reason)), []);

  const refresh = useCallback(() => {
    if (project == null) {
      setSnapshot(EMPTY);
      return;
    }
    orchestrationSnapshot(project)
      .then((snap) =>
        setSnapshot({
          tree: buildOrchestrationTree(snap.agents),
          agents: snap.agents,
          todos: snap.todos,
          scratchpads: snap.scratchpads,
        }),
      )
      .catch(fail);
  }, [project, fail]);

  useEffect(() => {
    if (project == null) {
      setSnapshot(EMPTY);
      return;
    }
    let cancelled = false;
    let unlisten: (() => void) | undefined;
    let frame: number | null = null;

    // Coalesce a burst of events into a single re-read on the next frame, so the surface updates at
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
      .catch(fail);

    return () => {
      cancelled = true;
      unlisten?.();
      if (frame != null) cancelAnimationFrame(frame);
    };
  }, [project, refresh, fail]);

  return { ...snapshot, error, refresh };
}
