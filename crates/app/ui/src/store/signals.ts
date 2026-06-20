import type { DomainEvent } from "@/domain";

// A coalesced CPU/memory reading for one process (bytes for rss; cpu_pct is per-core, so a
// busy multi-threaded process can exceed 100).
export interface ProcessMetrics {
  cpu_pct: number;
  rss: number;
}

// The event-derived signals the process list reads but the core does not keep on
// ProcessView: the latest CPU/memory reading per process (from MetricsTick, ~1 Hz per running
// group) and the current auto-restart attempt within the rate-limit window (from
// RestartScheduled). Kept apart from the read-model list so a ~1 Hz metric never churns the
// list projection.
export interface SignalState {
  metrics: Map<number, ProcessMetrics>;
  attempts: Map<number, number>;
}

export const EMPTY_SIGNALS: SignalState = { metrics: new Map(), attempts: new Map() };

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
    // A command that reaches Running has settled and a stopped one is at rest — either way the
    // restart progress no longer applies. RestartExhausted ends it too; the status glyph then
    // carries the exhausted state.
    case "ProcessStatusChanged":
      return event.to === "Running" || event.to === "Stopped"
        ? clearAttempt(state, event.id)
        : state;
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

function forget(state: SignalState, id: number): SignalState {
  if (!state.metrics.has(id) && !state.attempts.has(id)) return state;
  const metrics = new Map(state.metrics);
  const attempts = new Map(state.attempts);
  metrics.delete(id);
  attempts.delete(id);
  return { metrics, attempts };
}
