import { useCallback, useEffect, useState } from "react";
import { onDomainEvent, orchestrationSnapshot } from "@/api";
import { buildOrchestrationTree, type OrchestrationTreeNode } from "@/store/orchestrationTree";
import type { AgentNode, DomainEvent, ScratchpadSummary, TimerView, TodoView } from "@/domain";

// Domain events that change anything the orchestration surface renders: a process entering or
// leaving the registry, a status / label / activity change (the agent tree), or a todo, scratchpad,
// or timer mutation (the coordination panels). The snapshot is derived on read and its events carry
// ids only, so the hook re-reads the one snapshot rather than folding deltas. Timer pause/resume
// events are included so the panel reflects the new status without polling.
const SNAPSHOT_EVENTS: ReadonlySet<DomainEvent["type"]> = new Set([
  "ProcessSpawned",
  "ProcessStatusChanged",
  "ProcessRemoved",
  "ProcessRenamed",
  "AgentActivityChanged",
  "TodoChanged",
  "ScratchpadChanged",
  "TimerArmed",
  "TimerFired",
  "TimerCleared",
  "TimerPaused",
  "TimerResumed",
]);

export interface OrchestrationStore {
  tree: OrchestrationTreeNode[];
  /** The flat agent list (registry order) — the tree's source, kept for id→label lookups. */
  agents: AgentNode[];
  todos: TodoView[];
  scratchpads: ScratchpadSummary[];
  /** Armed and paused timers in the project, ordered by id. */
  timers: TimerView[];
  error: string | null;
  refresh: () => void;
}

type Snapshot = Omit<OrchestrationStore, "error" | "refresh"> & { forProject: number | null };

const EMPTY: Snapshot = {
  forProject: null,
  tree: [],
  agents: [],
  todos: [],
  scratchpads: [],
  timers: [],
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
    if (project == null) return;
    orchestrationSnapshot(project)
      .then((snap) =>
        setSnapshot({
          forProject: project,
          tree: buildOrchestrationTree(snap.agents),
          agents: snap.agents,
          todos: snap.todos,
          scratchpads: snap.scratchpads,
          timers: snap.timers,
        }),
      )
      .catch(fail);
  }, [project, fail]);

  useEffect(() => {
    if (project == null) return;
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

  // A snapshot captured for another project (or before the first load) is stale: surface EMPTY
  // until this project's own data arrives, so switching projects never flashes the previous tree
  // and a null project shows nothing — deriving staleness here means no effect resets state.
  const view = snapshot.forProject === project ? snapshot : EMPTY;
  return {
    tree: view.tree,
    agents: view.agents,
    todos: view.todos,
    scratchpads: view.scratchpads,
    timers: view.timers,
    error,
    refresh,
  };
}
