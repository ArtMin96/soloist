import { useCallback, useEffect, useState } from "react";
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
import type { ProcessView } from "@/domain";

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

  const fail = useCallback((reason: unknown) => setError(String(reason)), []);
  const clearError = useCallback(() => setError(null), []);
  const refresh = useCallback(() => {
    procList().then(setProcesses).catch(fail);
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
    // Attach the listener before reading the snapshot, so an event emitted between the
    // snapshot and the subscription cannot be lost (snapshot-then-deltas).
    onDomainEvent((event) => setProcesses((prev) => applyEvent(prev, event)))
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
