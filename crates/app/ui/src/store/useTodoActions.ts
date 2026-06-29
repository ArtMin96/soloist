import { useCallback, useState } from "react";
import { todoComplete, todoLink } from "@/api";

export interface TodoActionsStore {
  /** The todo a write is in flight for, or null. */
  busyId: number | null;
  /** The last failure per todo (e.g. the core's blocked refusal), keyed by id. */
  errorById: Record<number, string>;
  complete: (id: number) => void;
  copyLink: (id: number) => void;
  clearError: (id: number) => void;
}

// The to-do board's write seam (the only place it reaches IPC). Completing a todo routes to the
// core, which refuses a still-blocked one with its `TodoBlocked` message — surfaced verbatim per
// todo, never pre-empted here (the gate is the core's single source of truth). Copy link writes the
// todo's `solo://` link to the clipboard. The board's data and its live refresh come from the
// snapshot hook; a successful write lands as a TodoChanged event that re-reads it.
export function useTodoActions(project: number): TodoActionsStore {
  const [busyId, setBusyId] = useState<number | null>(null);
  const [errorById, setErrorById] = useState<Record<number, string>>({});

  const setError = useCallback((id: number, message: string | null) => {
    setErrorById((prev) => {
      if (message == null) {
        if (prev[id] == null) return prev;
        const next = { ...prev };
        delete next[id];
        return next;
      }
      return { ...prev, [id]: message };
    });
  }, []);

  const complete = useCallback(
    (id: number) => {
      setBusyId(id);
      setError(id, null);
      todoComplete(project, id)
        .catch((reason) => setError(id, String(reason)))
        .finally(() => setBusyId((current) => (current === id ? null : current)));
    },
    [project, setError],
  );

  const copyLink = useCallback(
    (id: number) => {
      todoLink(project, id)
        .then((link) => navigator.clipboard?.writeText(link))
        .catch((reason) => setError(id, String(reason)));
    },
    [project, setError],
  );

  const clearError = useCallback((id: number) => setError(id, null), [setError]);

  return { busyId, errorById, complete, copyLink, clearError };
}
