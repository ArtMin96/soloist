import { useCallback, useState } from "react";
import { isActive } from "@/lib/status";
import { useLatestRef } from "@/store/useLatestRef";
import type { ProcessView } from "@/domain";

/** Removing a process, and the confirmation a live one has to pass first. */
export interface ProcessRemoval {
  /** The process a confirmation is open for, or `null` when none is pending. */
  pending: ProcessView | null;
  /** Asks to remove `id` — straight through when it rests, via `pending` while it is live. */
  request: (id: number) => void;
  /** Removes the pending process. */
  confirm: () => void;
  /** Abandons the pending removal, leaving the process untouched. */
  dismiss: () => void;
}

/**
 * The removal policy every surface shares: a resting process is forgotten immediately, because all
 * that is lost is a row and its scrollback, and clearing finished work out of the sidebar should
 * not cost a dialog. A *live* one is worth a confirmation first — removing it kills a running
 * child, which no undo brings back.
 *
 * The pending process is looked up from the current list rather than captured, so the dialog
 * always describes the process as it is now. Two cases fall out of that rather than needing their
 * own handling: a process that exits while its dialog is open keeps the dialog (the intent still
 * makes sense, and confirming still removes the row), and one already removed by another surface
 * — an agent over MCP, say — drops `pending` to `null`, closing a dialog that has nothing left to
 * ask about.
 */
export function useProcessRemoval(
  processes: ProcessView[],
  close: (id: number) => void,
): ProcessRemoval {
  const [pendingId, setPendingId] = useState<number | null>(null);
  const processesRef = useLatestRef(processes);
  const closeRef = useLatestRef(close);

  const request = useCallback(
    (id: number) => {
      const process = processesRef.current.find((candidate) => candidate.id === id);
      // An id the list no longer holds is already gone as far as this surface can tell, so there
      // is nothing to confirm; the call still goes through and the core answers for it.
      if (process !== undefined && isActive(process.status)) setPendingId(id);
      else closeRef.current(id);
    },
    [processesRef, closeRef],
  );

  const confirm = useCallback(() => {
    if (pendingId !== null) closeRef.current(pendingId);
    setPendingId(null);
  }, [pendingId, closeRef]);

  const dismiss = useCallback(() => setPendingId(null), []);

  return {
    pending: processes.find((candidate) => candidate.id === pendingId) ?? null,
    request,
    confirm,
    dismiss,
  };
}
