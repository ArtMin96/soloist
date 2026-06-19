import { useCallback, useEffect, useState } from "react";
import {
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
  /** The project the loaded stack belongs to; `null` before anything loads. */
  projectId: number | null;
  error: string | null;
  /** Surface a failure on the shared error banner (also used by sibling stores). */
  reportError: (reason: unknown) => void;
  clearError: () => void;
  refresh: () => void;
  start: (id: number) => void;
  stop: (id: number) => void;
  restart: (id: number) => void;
  startAll: () => void;
  stopAll: () => void;
  restartRunning: () => void;
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

  const projectId = processes[0]?.project ?? null;

  const start = useCallback((id: number) => void procStart(id).catch(fail), [fail]);
  const stop = useCallback((id: number) => void procStop(id).catch(fail), [fail]);
  const restart = useCallback((id: number) => void procRestart(id).catch(fail), [fail]);

  const startAll = useCallback(() => {
    if (projectId !== null) void stackStart(projectId).catch(fail);
  }, [projectId, fail]);
  const stopAll = useCallback(() => {
    if (projectId !== null) void stackStop(projectId).catch(fail);
  }, [projectId, fail]);
  const restartRunning = useCallback(() => {
    if (projectId !== null) void stackRestartRunning(projectId).catch(fail);
  }, [projectId, fail]);

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
    projectId,
    error,
    reportError: fail,
    clearError,
    refresh,
    start,
    stop,
    restart,
    startAll,
    stopAll,
    restartRunning,
  };
}
