import type { AgentActivity, AgentSignal, DomainEvent } from "@/domain";

// A coalesced CPU/memory reading for one process, derived from the MetricsTick payload so its
// shape has a single source. cpu_pct is whole-machine (100 = every core busy, never above);
// rss is the group's memory in bytes (shared pages counted once).
export type ProcessMetrics = Pick<Extract<DomainEvent, { type: "MetricsTick" }>, "cpu_pct" | "rss">;

// The event-derived signals the process list reads but the core does not keep on
// ProcessView: the latest CPU/memory reading per process (from MetricsTick, ~1 Hz per running
// group), the current auto-restart attempt within the rate-limit window (from
// RestartScheduled), and the current agent activity (from AgentActivityChanged). Kept apart
// from the read-model list so a ~1 Hz signal never churns the list projection.
export interface SignalState {
  metrics: Map<number, ProcessMetrics>;
  attempts: Map<number, number>;
  activity: Map<number, AgentActivity>;
}

export const EMPTY_SIGNALS: SignalState = {
  metrics: new Map(),
  attempts: new Map(),
  activity: new Map(),
};

// Fold one core event into the signal state. Pure, and returns the same reference when nothing
// changed so an unrelated event never forces a re-render. Holds no business logic — it mirrors
// the deltas the core emits, like the list projection.
export function applySignal(state: SignalState, event: DomainEvent): SignalState {
  switch (event.type) {
    case "MetricsTick": {
      const metrics = new Map(state.metrics);
      metrics.set(event.id, { cpu_pct: event.cpu_pct, rss: event.rss });
      return { ...state, metrics };
    }
    case "RestartScheduled": {
      const attempts = new Map(state.attempts);
      attempts.set(event.id, event.attempt);
      return { ...state, attempts };
    }
    case "AgentActivityChanged": {
      const activity = new Map(state.activity);
      activity.set(event.id, event.state);
      return { ...state, activity };
    }
    case "ProcessStatusChanged": {
      let next = state;
      // A command that reaches Running has settled and a stopped one is at rest — either way
      // the restart progress no longer applies.
      if (event.to === "Running" || event.to === "Stopped") next = clearAttempt(next, event.id);
      // Activity is only meaningful while running; drop it when an agent leaves Running so a
      // stopped row falls back to its status. A relaunch re-emits the agent's first activity.
      if (event.to !== "Running") next = clearActivity(next, event.id);
      return next;
    }
    // RestartExhausted ends restart progress too; the status glyph then carries the state.
    case "RestartExhausted":
      return clearAttempt(state, event.id);
    case "ProcessRemoved":
      return forget(state, event.id);
    default:
      return state;
  }
}

// Reconcile the activity map to the core's current agent-activity snapshot, dropping any entry not
// in it — an agent that left the registry, or one whose `AgentActivityChanged` was dropped during
// bus lag. The snapshot is the authoritative set, so no idle badge stays stale after a resync,
// focus, or reload. Returns the same reference when the map is unchanged, so a seed that changes
// nothing never notifies (mirroring `applySignal`). Metrics and restart attempts fold from their
// own deltas — metrics self-heal via the periodic tick — so the seed touches only activity.
export function seedActivity(state: SignalState, entries: readonly AgentSignal[]): SignalState {
  const activity = new Map<number, AgentActivity>(
    entries.map((entry) => [entry.id, entry.activity]),
  );
  if (sameActivity(state.activity, activity)) return state;
  return { ...state, activity };
}

function sameActivity(
  a: ReadonlyMap<number, AgentActivity>,
  b: ReadonlyMap<number, AgentActivity>,
): boolean {
  if (a.size !== b.size) return false;
  for (const [id, activity] of b) {
    if (a.get(id) !== activity) return false;
  }
  return true;
}

function clearAttempt(state: SignalState, id: number): SignalState {
  if (!state.attempts.has(id)) return state;
  const attempts = new Map(state.attempts);
  attempts.delete(id);
  return { ...state, attempts };
}

function clearActivity(state: SignalState, id: number): SignalState {
  if (!state.activity.has(id)) return state;
  const activity = new Map(state.activity);
  activity.delete(id);
  return { ...state, activity };
}

function forget(state: SignalState, id: number): SignalState {
  if (!state.metrics.has(id) && !state.attempts.has(id) && !state.activity.has(id)) return state;
  const metrics = new Map(state.metrics);
  const attempts = new Map(state.attempts);
  const activity = new Map(state.activity);
  metrics.delete(id);
  attempts.delete(id);
  activity.delete(id);
  return { metrics, attempts, activity };
}
