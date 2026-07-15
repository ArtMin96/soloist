import { useCallback, useEffect, useState, type ReactNode } from "react";
import { agentActivity, onDomainEvent } from "@/api";
import { createSignalStore } from "@/store/signalStore";
import { SignalsContext } from "@/store/signalsContext";
import { useReconcile } from "@/store/useReconcile";

// Subscribes once to the core event stream and folds the coalesced per-process signals (CPU/memory
// + auto-restart attempt + agent activity) into an external store the leaves read by selector.
// Agent activity is edge-triggered (`AgentActivityChanged` only fires on a change), so a dropped
// delta or a webview reload would leave a badge permanently stale; the store is therefore seeded
// from the agent-activity snapshot on mount and re-seeded on resync/focus (`useReconcile`), which
// self-heals it. Metrics and attempts still accrue from the live stream — metrics re-publish on a
// periodic tick — so they need no seed. The store is created once and kept stable so consumer
// subscriptions never churn; a tick re-renders only the row whose telemetry changed, not every
// reader.
export function SignalsProvider({ children }: { children: ReactNode }) {
  const [store] = useState(createSignalStore);

  // Reconcile the idle badges to the core's current agent-activity snapshot. Stable so the
  // subscription and `useReconcile` attach once per mount.
  const reseed = useCallback(() => {
    agentActivity()
      .then((signals) => store.seed(signals))
      .catch(() => {});
  }, [store]);

  useEffect(() => {
    let cancelled = false;
    let unlisten: (() => void) | undefined;
    // Attach the listener before the first seed, so an `AgentActivityChanged` emitted between the
    // snapshot and the subscription cannot be lost (snapshot-then-deltas).
    onDomainEvent((event) => store.apply(event))
      .then((stop) => {
        if (cancelled) {
          stop();
          return;
        }
        unlisten = stop;
        reseed();
      })
      .catch(() => {});
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [store, reseed]);

  // Re-seed on a backend resync signal or window focus, so a dropped `AgentActivityChanged` never
  // leaves an idle badge permanently stale.
  useReconcile(reseed);

  return <SignalsContext value={store}>{children}</SignalsContext>;
}
