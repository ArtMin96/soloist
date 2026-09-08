import { startTransition, useCallback, useEffect, useMemo, useState } from "react";
import { onDomainEvent, orchestrationSnapshot } from "@/api";
import { failed, loading, ready, type Loadable } from "@/store/loadable";
import { buildOrchestrationTree, type OrchestrationTreeNode } from "@/store/orchestrationTree";
import { useReconcile } from "@/store/useReconcile";
import type {
  AgentMessageRecord,
  AgentNode,
  DiagramSummary,
  DomainEvent,
  ScratchpadSummary,
  TimerView,
  TodoView,
} from "@/domain";

// Domain events that change anything the orchestration surface renders: a process entering or
// leaving the registry, a status / label / activity change (the agent tree), or a todo, scratchpad,
// diagram, timer, or agent-message mutation (the coordination panels). The snapshot is derived on read and its
// events carry ids only, so the hook re-reads the one snapshot rather than folding deltas. Timer
// pause/resume events are included so the panel reflects the new status without polling.
const SNAPSHOT_EVENTS: ReadonlySet<DomainEvent["type"]> = new Set([
  "ProcessSpawned",
  "ProcessStatusChanged",
  "ProcessRemoved",
  "ProcessRenamed",
  "AgentActivityChanged",
  "TodoChanged",
  "AgentMessageChanged",
  "ScratchpadChanged",
  "DiagramChanged",
  "TimerArmed",
  "TimerFired",
  "TimerCleared",
  "TimerPaused",
  "TimerResumed",
]);

/** Everything the orchestration surface renders for one project, as one read. */
export interface OrchestrationReadModel {
  tree: OrchestrationTreeNode[];
  /** The flat agent list (registry order) — the tree's source, kept for id→label lookups. */
  agents: AgentNode[];
  todos: TodoView[];
  scratchpads: ScratchpadSummary[];
  /** One-line diagram summaries in the project. */
  diagrams: DiagramSummary[];
  /** Armed and paused timers in the project, ordered by id. */
  timers: TimerView[];
  /** Recorded agent-to-agent exchanges in the project, oldest first. */
  messages: AgentMessageRecord[];
}

export interface OrchestrationStore {
  /** The project's read model: loading until its first snapshot lands, then ready and kept live. */
  snapshot: Loadable<OrchestrationReadModel>;
  /** A re-read that failed while a snapshot is showing; cleared by the next successful read. */
  error: string | null;
  refresh: () => void;
}

/** A read model and the project it was read for, so staleness is derivable rather than reset. */
interface Held {
  forProject: number;
  model: OrchestrationReadModel;
}

/** Why a read failed, and the project it was read for. */
interface Failure {
  forProject: number;
  reason: string;
}

// The orchestration read model for one project — the agent tree plus the coordination state the
// panels render (todos, scratchpad summaries). Seeds from the snapshot, then re-reads it when a
// process-lifecycle, agent-activity, todo, or scratchpad event signals a change. Re-reads are
// coalesced to one per animation frame, so a chatty run never thrashes the surface. Holds no
// business logic — the tree nesting lives in the pure `buildOrchestrationTree`. Nothing is shown
// until this project's own snapshot lands, so an unread board is never mistaken for an empty one.
export function useOrchestration(project: number | null): OrchestrationStore {
  const [held, setHeld] = useState<Held | null>(null);
  const [failure, setFailure] = useState<Failure | null>(null);

  // Bound to the project the read was issued for, so a rejection that arrives after the user has
  // switched projects cannot be reported against the project they are now looking at.
  const failFor = useCallback(
    (forProject: number) => (reason: unknown) => setFailure({ forProject, reason: String(reason) }),
    [],
  );

  const refresh = useCallback(() => {
    if (project == null) return;
    orchestrationSnapshot(project)
      .then((snap) => {
        const model: OrchestrationReadModel = {
          tree: buildOrchestrationTree(snap.agents),
          agents: snap.agents,
          todos: snap.todos,
          scratchpads: snap.scratchpads,
          diagrams: snap.diagrams,
          timers: snap.timers,
          messages: snap.messages,
        };
        // Committing a whole read model is the expensive render on this surface, so it yields to
        // whatever urgent work (a keystroke, a view switch) is in flight. The healed read clears
        // the failure in the same transition, so fresh data is never painted beside stale news.
        startTransition(() => {
          setHeld({ forProject: project, model });
          setFailure(null);
        });
      })
      .catch(failFor(project));
  }, [project, failFor]);

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
      .catch(failFor(project));

    return () => {
      cancelled = true;
      unlisten?.();
      if (frame != null) cancelAnimationFrame(frame);
    };
  }, [project, refresh, failFor]);

  // Re-read on a backend resync signal or window focus, so a dropped coordination or lifecycle
  // delta never leaves the orchestration board stale. A no-op while no project is selected.
  useReconcile(refresh);

  // A model or a failure captured for another project (or before the first read answered) is not
  // this project's news: deriving that here means switching projects never flashes the previous
  // board or its complaint, and no effect has to reset state.
  const model = held?.forProject === project ? held.model : null;
  const reason = failure?.forProject === project ? failure.reason : null;

  // Where a read failure surfaces depends on whether there is anything to keep on screen: beside a
  // held model it is a re-read that failed, and with nothing held it is the phase itself.
  const snapshot = useMemo<Loadable<OrchestrationReadModel>>(() => {
    if (model != null) return ready(model);
    if (reason != null) return failed(reason);
    return loading();
  }, [model, reason]);

  return { snapshot, error: model != null ? reason : null, refresh };
}
