import { createContext, useContext } from "react";
import { EMPTY_SIGNALS, type ProcessMetrics, type SignalState } from "@/store/signals";

// Per-process telemetry is read at the leaves (every sidebar row, the terminal header) but
// would otherwise drill through three pass-through components, so it travels by context. The
// default is the empty state, so a component rendered without the provider (a focused test)
// sees no signals rather than throwing.
export const SignalsContext = createContext<SignalState>(EMPTY_SIGNALS);

export interface ProcessSignal {
  metrics?: ProcessMetrics;
  attempt?: number;
}

/** The telemetry for one process: its latest CPU/memory reading and current auto-restart
 *  attempt, each `undefined` until one arrives. */
export function useSignal(id: number): ProcessSignal {
  const { metrics, attempts } = useContext(SignalsContext);
  return { metrics: metrics.get(id), attempt: attempts.get(id) };
}
