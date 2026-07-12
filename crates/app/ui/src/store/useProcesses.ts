import { useCallback, useEffect, useRef, useState } from "react";
import {
  agentResume,
  onDomainEvent,
  procList,
  procRestart,
  procStart,
  procStop,
  stackRestartRunning,
  stackStart,
  stackStop,
} from "@/api";
import { applyEvent } from "@/store/projection";
import { useReconcile } from "@/store/useReconcile";
import type { DomainEvent, ProcessView } from "@/domain";

export interface ProcessStore {
  processes: ProcessView[];
  error: string | null;
  /** Surface a failure on the shared error banner (also used by sibling stores). */
  reportError: (reason: unknown) => void;
  clearError: () => void;
  refresh: () => void;
  start: (id: number) => void;
  stop: (id: number) => void;
  restart: (id: number) => void;
  /** Resume a stopped resumable agent's last session (vs `start`, which begins fresh). */
  resume: (id: number) => void;
  /** Bulk operations are scoped to a project so each project's header controls its own stack. */
  startAll: (project: number) => void;
  stopAll: (project: number) => void;
  restartRunning: (project: number) => void;
}

// The process read model: the single place the UI gets process data and the actions that
// mutate it. Seeds from a snapshot, then folds in live deltas (snapshot-then-deltas);
// `refresh` re-syncs after a dropped/lagged stream. Actions route to the core and never
// optimistically mutate the list — the resulting events do.
export function useProcesses(): ProcessStore {
  const [processes, setProcesses] = useState<ProcessView[]>([]);
  const [error, setError] = useState<string | null>(null);

  // Applying a snapshot replaces the whole list, so an event folded in while the snapshot was
  // in flight would be clobbered — and an unknown-id delta (a ProcessSpawned for a row the
  // snapshot predates) is a silent no-op in the projection, making the loss permanent. So while
  // a fetch is in flight events are buffered here and replayed on top of the snapshot (the folds
  // are idempotent), and a generation guard drops a superseded fetch so overlapping refreshes
  // (e.g. the post-trust refresh racing a spawn burst) resolve in order.
  const fetchingRef = useRef(false);
  const bufferRef = useRef<DomainEvent[]>([]);
  const genRef = useRef(0);

  const fail = useCallback((reason: unknown) => setError(String(reason)), []);
  const clearError = useCallback(() => setError(null), []);
  const refresh = useCallback(() => {
    const gen = ++genRef.current;
    fetchingRef.current = true;
    bufferRef.current = [];
    procList()
      .then((snapshot) => {
        if (gen !== genRef.current) return; // a newer refresh superseded this one
        const hydrated = bufferRef.current.reduce(applyEvent, snapshot);
        bufferRef.current = [];
        fetchingRef.current = false;
        setProcesses(hydrated);
      })
      .catch((reason) => {
        if (gen !== genRef.current) return;
        fetchingRef.current = false;
        fail(reason);
      });
  }, [fail]);

  const start = useCallback((id: number) => void procStart(id).catch(fail), [fail]);
  const stop = useCallback((id: number) => void procStop(id).catch(fail), [fail]);
  const restart = useCallback((id: number) => void procRestart(id).catch(fail), [fail]);
  const resume = useCallback((id: number) => void agentResume(id).catch(fail), [fail]);

  const startAll = useCallback((project: number) => void stackStart(project).catch(fail), [fail]);
  const stopAll = useCallback((project: number) => void stackStop(project).catch(fail), [fail]);
  const restartRunning = useCallback(
    (project: number) => void stackRestartRunning(project).catch(fail),
    [fail],
  );

  useEffect(() => {
    let cancelled = false;
    let unlisten: (() => void) | undefined;
    // Attach the listener before reading the snapshot, so an event emitted between the snapshot
    // and the subscription cannot be lost (snapshot-then-deltas). While a fetch is in flight the
    // event is buffered (replayed on top of the snapshot when it lands) rather than folded into a
    // list the snapshot is about to replace.
    const onEvent = (event: DomainEvent) => {
      if (fetchingRef.current) bufferRef.current.push(event);
      else setProcesses((prev) => applyEvent(prev, event));
    };
    onDomainEvent(onEvent)
      .then((stopListening) => {
        if (cancelled) {
          stopListening();
          return;
        }
        unlisten = stopListening;
        refresh();
      })
      .catch(fail);
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [refresh, fail]);

  // Re-sync on a backend resync signal or window focus, so a dropped delta never leaves the
  // list permanently stale.
  useReconcile(refresh);

  return {
    processes,
    error,
    reportError: fail,
    clearError,
    refresh,
    start,
    stop,
    restart,
    resume,
    startAll,
    stopAll,
    restartRunning,
  };
}
