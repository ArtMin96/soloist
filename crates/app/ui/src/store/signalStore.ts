import { applySignal, EMPTY_SIGNALS, seedActivity, type SignalState } from "@/store/signals";
import type { AgentSignal, DomainEvent } from "@/domain";

// A framework-free external store holding the coalesced per-process signals, read by the leaves via
// `useSyncExternalStore` (see `signalsContext`). It exists so a ~1 Hz MetricsTick re-renders only the
// row whose telemetry changed, not every consumer: React Context has no selector, so a whole-object
// context value forces every `useSignal` reader to re-render on every tick. The fold itself stays the
// pure `applySignal` — this only adds subscription + the current snapshot around it.
export interface SignalStore {
  /** Subscribe to state changes; returns an unsubscribe. Stable identity for `useSyncExternalStore`. */
  subscribe: (listener: () => void) => () => void;
  /** The current immutable snapshot. A new reference only when a fold actually changed the state. */
  getSnapshot: () => SignalState;
  /** Fold one core event in, notifying subscribers only when the state reference changed. */
  apply: (event: DomainEvent) => void;
  /** Reconcile the idle badges to the core's current agent-activity snapshot — the resync/reload
   *  backstop for a dropped `AgentActivityChanged` — notifying only if the set actually changed. */
  seed: (entries: readonly AgentSignal[]) => void;
}

export function createSignalStore(): SignalStore {
  let state = EMPTY_SIGNALS;
  const listeners = new Set<() => void>();
  return {
    subscribe(listener) {
      listeners.add(listener);
      return () => {
        listeners.delete(listener);
      };
    },
    getSnapshot: () => state,
    apply(event) {
      // `applySignal` returns the same reference when nothing changed, so an unrelated event never
      // notifies — no snapshot churn, no wasted wake-ups.
      const next = applySignal(state, event);
      if (next === state) return;
      state = next;
      for (const listener of listeners) listener();
    },
    seed(entries) {
      // Same no-churn rule as `apply`: `seedActivity` returns the same reference when the badge set
      // is unchanged, so a routine resync that finds nothing new never re-renders a reader.
      const next = seedActivity(state, entries);
      if (next === state) return;
      state = next;
      for (const listener of listeners) listener();
    },
  };
}

// A read-only store over a fixed snapshot: it never notifies. Backs the provider-less default
// (below) and lets a focused test render consumers against a specific signal state.
export function fixedSignalStore(state: SignalState): SignalStore {
  return {
    subscribe: () => () => {},
    getSnapshot: () => state,
    apply: () => {},
    seed: () => {},
  };
}

// The default for a consumer rendered without a provider (a focused test): an empty, silent store,
// so `useSignal` reads the empty slice rather than throwing.
export const EMPTY_STORE: SignalStore = fixedSignalStore(EMPTY_SIGNALS);
