import { createContext, use, useCallback, useRef, useSyncExternalStore } from "react";
import type { AgentActivity } from "@/domain";
import type { ProcessMetrics, SignalState } from "@/store/signals";
import { EMPTY_STORE, type SignalStore } from "@/store/signalStore";

// Per-process telemetry is read at the leaves (every sidebar row, the terminal header) but would
// otherwise drill through three pass-through components, so it travels by context. The context
// carries the external store (not the state itself) so each `useSignal` reader subscribes to only
// its own process's slice — a ~1 Hz tick for one process re-renders just that process's reader, not
// every reader. The default is an empty store, so a component rendered without the provider (a
// focused test) reads an empty slice rather than throwing.
export const SignalsContext = createContext<SignalStore>(EMPTY_STORE);

export interface ProcessSignal {
  metrics?: ProcessMetrics;
  attempt?: number;
  activity?: AgentActivity;
}

// Two slices are equal when the id's telemetry is unchanged. Metrics compare by value (a fresh
// object is allocated each tick even when the numbers repeat) so an unchanged reading never
// re-renders; attempt and activity are primitives compared by identity.
function sameSignal(a: ProcessSignal, b: ProcessSignal): boolean {
  return (
    a.attempt === b.attempt &&
    a.activity === b.activity &&
    a.metrics?.cpu_pct === b.metrics?.cpu_pct &&
    a.metrics?.rss === b.metrics?.rss
  );
}

/** The telemetry for one process: its latest CPU/memory reading, current auto-restart attempt, and
 *  (for a running agent) its current activity — each `undefined` until one arrives. Re-renders the
 *  caller only when *this* process's slice changes, not on every other process's tick. */
export function useSignal(id: number): ProcessSignal {
  const store = use(SignalsContext);
  // `useSyncExternalStore` requires `getSnapshot` to return a stable reference while the selected
  // slice is unchanged, or it re-renders (and can loop). The whole `SignalState` reference changes
  // on every tick of any process, so cache this id's derived slice keyed on the state reference,
  // the id, and the slice value: a tick for another process reuses the cached slice (no re-render);
  // only a change to this id's telemetry — or a change of id — yields a new one. The id is part of
  // the key so a consumer whose id changes in place never reads the previous process's slice.
  const cache = useRef<{ state: SignalState; id: number; slice: ProcessSignal } | null>(null);
  const getSnapshot = useCallback(() => {
    const state = store.getSnapshot();
    const cached = cache.current;
    if (cached && cached.state === state && cached.id === id) return cached.slice;
    const slice: ProcessSignal = {
      metrics: state.metrics.get(id),
      attempt: state.attempts.get(id),
      activity: state.activity.get(id),
    };
    if (cached && cached.id === id && sameSignal(cached.slice, slice)) {
      cache.current = { state, id, slice: cached.slice };
      return cached.slice;
    }
    cache.current = { state, id, slice };
    return slice;
  }, [store, id]);
  return useSyncExternalStore(store.subscribe, getSnapshot, getSnapshot);
}
