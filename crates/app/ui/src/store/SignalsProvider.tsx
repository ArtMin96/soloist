import { useEffect, useState, type ReactNode } from "react";
import { onDomainEvent } from "@/api";
import { applySignal, EMPTY_SIGNALS, type SignalState } from "@/store/signals";
import { SignalsContext } from "@/store/signalsContext";

// Subscribes once to the core event stream and projects the coalesced per-process signals
// (CPU/memory + auto-restart attempt) into context. Unlike the process list there is no
// snapshot to seed — these accrue from the live MetricsTick/RestartScheduled stream — so an
// empty start is correct.
export function SignalsProvider({ children }: { children: ReactNode }) {
  const [signals, setSignals] = useState<SignalState>(EMPTY_SIGNALS);

  useEffect(() => {
    let cancelled = false;
    let unlisten: (() => void) | undefined;
    onDomainEvent((event) => setSignals((prev) => applySignal(prev, event)))
      .then((stop) => {
        if (cancelled) stop();
        else unlisten = stop;
      })
      .catch(() => {});
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, []);

  return <SignalsContext value={signals}>{children}</SignalsContext>;
}
