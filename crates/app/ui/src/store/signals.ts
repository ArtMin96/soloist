import type { AgentActivity, DomainEvent } from "@/domain";

// A coalesced CPU/memory reading for one process, derived from the MetricsTick payload so its
// shape has a single source. cpu_pct is whole-machine (100 = every core busy, never above);
// rss is the group's memory in bytes (shared pages counted once).
export type ProcessMetrics = Pick<Extract<DomainEvent, { type: "MetricsTick" }>, "cpu_pct" | "rss">;

// The event-derived signals the process list reads but the core does not keep on
// ProcessView: the latest CPU/memory reading per process (from MetricsTick, ~1 Hz per running
// group), the current auto-restart attempt within the rate-limit window (from
// RestartScheduled), the current agent activity (from AgentActivityChanged), and an agent's
// latest idle summary (from AgentSummary — the opt-in auto-summarizer). Kept apart from the
// read-model list so a ~1 Hz signal never churns the list projection.
export interface SignalState {
  metrics: Map<number, ProcessMetrics>;
  attempts: Map<number, number>;
  activity: Map<number, AgentActivity>;
  summary: Map<number, string>;
}

export const EMPTY_SIGNALS: SignalState = {
  metrics: new Map(),
  attempts: new Map(),
  activity: new Map(),
  summary: new Map(),
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
    case "AgentSummary": {
      const summary = new Map(state.summary);
      summary.set(event.id, event.text);
      return { ...state, summary };
    }
    case "ProcessStatusChanged": {
      let next = state;
      // A command that reaches Running has settled and a stopped one is at rest — either way
      // the restart progress no longer applies.
      if (event.to === "Running" || event.to === "Stopped") next = clearAttempt(next, event.id);
      // Activity and its summary are only meaningful while running; drop both when an agent
      // leaves Running so a stopped row falls back to its status. A relaunch re-emits the
      // agent's first activity, and a fresh idle produces a new summary.
      if (event.to !== "Running") next = clearAgentSignals(next, event.id);
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

function clearAttempt(state: SignalState, id: number): SignalState {
  if (!state.attempts.has(id)) return state;
  const attempts = new Map(state.attempts);
  attempts.delete(id);
  return { ...state, attempts };
}

function clearAgentSignals(state: SignalState, id: number): SignalState {
  if (!state.activity.has(id) && !state.summary.has(id)) return state;
  const activity = new Map(state.activity);
  const summary = new Map(state.summary);
  activity.delete(id);
  summary.delete(id);
  return { ...state, activity, summary };
}

function forget(state: SignalState, id: number): SignalState {
  if (
    !state.metrics.has(id) &&
    !state.attempts.has(id) &&
    !state.activity.has(id) &&
    !state.summary.has(id)
  )
    return state;
  const metrics = new Map(state.metrics);
  const attempts = new Map(state.attempts);
  const activity = new Map(state.activity);
  const summary = new Map(state.summary);
  metrics.delete(id);
  attempts.delete(id);
  activity.delete(id);
  summary.delete(id);
  return { metrics, attempts, activity, summary };
}
