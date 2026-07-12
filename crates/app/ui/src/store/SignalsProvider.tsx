import { useEffect, useState, type ReactNode } from "react";
import { onDomainEvent } from "@/api";
import { createSignalStore } from "@/store/signalStore";
import { SignalsContext } from "@/store/signalsContext";

// Subscribes once to the core event stream and folds the coalesced per-process signals (CPU/memory
// + auto-restart attempt + agent activity) into an external store the leaves read by selector.
// Unlike the process list there is no snapshot to seed — these accrue from the live stream — so an
// empty start is correct. The store is created once and kept stable so consumer subscriptions never
// churn; a tick re-renders only the row whose telemetry changed, not every reader.
export function SignalsProvider({ children }: { children: ReactNode }) {
  const [store] = useState(createSignalStore);

  useEffect(() => {
    let cancelled = false;
    let unlisten: (() => void) | undefined;
    onDomainEvent((event) => store.apply(event))
      .then((stop) => {
        if (cancelled) stop();
        else unlisten = stop;
      })
      .catch(() => {});
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [store]);

  return <SignalsContext value={store}>{children}</SignalsContext>;
}
