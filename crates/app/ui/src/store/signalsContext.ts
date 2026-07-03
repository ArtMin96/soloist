import { createContext, use } from "react";
import type { AgentActivity } from "@/domain";
import { EMPTY_SIGNALS, type ProcessMetrics, type SignalState } from "@/store/signals";

// Per-process telemetry is read at the leaves (every sidebar row, the terminal header) but
// would otherwise drill through three pass-through components, so it travels by context. The
// default is the empty state, so a component rendered without the provider (a focused test)
// sees no signals rather than throwing.
export const SignalsContext = createContext<SignalState>(EMPTY_SIGNALS);

export interface ProcessSignal {
  metrics?: ProcessMetrics;
  attempt?: number;
  activity?: AgentActivity;
}

/** The telemetry for one process: its latest CPU/memory reading, current auto-restart
 *  attempt, and (for a running agent) its current activity — each `undefined` until one
 *  arrives. */
export function useSignal(id: number): ProcessSignal {
  const { metrics, attempts, activity } = use(SignalsContext);
  return { metrics: metrics.get(id), attempt: attempts.get(id), activity: activity.get(id) };
}
