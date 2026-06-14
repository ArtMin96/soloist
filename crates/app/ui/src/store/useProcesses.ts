import { useCallback, useEffect, useState } from "react";
import { listProcesses, onDomainEvent, spawnDemo, stopProcess } from "@/api";
import { applyEvent } from "@/store/projection";
import type { ProcessView } from "@/domain";

export interface ProcessStore {
  processes: ProcessView[];
  error: string | null;
  start: () => void;
  stop: (id: number) => void;
  refresh: () => void;
}

// The process read model: the single place the UI gets process data and the actions
// that mutate it. Seeds from a snapshot, then folds in live deltas
// (snapshot-then-deltas); `refresh` re-syncs after a dropped/lagged stream.
export function useProcesses(): ProcessStore {
  const [processes, setProcesses] = useState<ProcessView[]>([]);
  const [error, setError] = useState<string | null>(null);

  const fail = useCallback((reason: unknown) => setError(String(reason)), []);

  const refresh = useCallback(() => {
    listProcesses().then(setProcesses).catch(fail);
  }, [fail]);

  const start = useCallback(() => {
    spawnDemo().catch(fail);
  }, [fail]);

  const stop = useCallback(
    (id: number) => {
      stopProcess(id).catch(fail);
    },
    [fail],
  );

  useEffect(() => {
    const unlisten = onDomainEvent((event) => setProcesses((prev) => applyEvent(prev, event)));
    refresh();
    return () => {
      void unlisten.then((stopListening) => stopListening());
    };
  }, [refresh]);

  return { processes, error, start, stop, refresh };
}
