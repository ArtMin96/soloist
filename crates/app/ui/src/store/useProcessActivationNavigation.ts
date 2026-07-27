import { useCallback, useLayoutEffect, useRef, useState } from "react";
import { isActive } from "@/lib/status";
import {
  activateProcess,
  forgetProcesses,
  mostRecentAvailableProcess,
} from "@/store/processActivationHistory";
import { useLatestRef } from "@/store/useLatestRef";
import type { ProcessView } from "@/domain";

interface ProcessActivationCallbacks {
  /** Clears project/orchestration whenever a process or the Start surface takes their place. */
  onClearAlternativeView: () => void;
  onStart: (id: number) => void;
  onRestart: (id: number) => void;
  onResume: (id: number) => void;
}

export interface ProcessActivationNavigation {
  selectedId: number | null;
  /** Records a semantic activation and opens its process pane. */
  selectProcess: (id: number) => void;
  /** Clears only process selection while another main-area view is being opened. */
  deselectProcess: () => void;
  /** Opens the canonical Start surface and clears every competing main-area view. */
  openStart: () => void;
  /** Synchronous selection access for stable hotkey handlers. */
  getSelectedId: () => number | null;
  /** Records one explicit Stop and navigates when it targets the visible pane. */
  processStopped: (id: number) => void;
  /** Starts the explicitly targeted process and makes its pane current. */
  startProcess: (id: number) => void;
  /** Restarts the explicitly targeted process and makes its pane current. */
  restartProcess: (id: number) => void;
  /** Resumes the explicitly targeted agent and makes its pane current. */
  resumeProcess: (id: number) => void;
  /** Records every live target of a project Stop All against the committed process snapshot. */
  projectStopped: (projectId: number) => void;
  /** Navigates only when a removal request executed immediately instead of opening confirmation. */
  removalRequested: (id: number) => void;
  /** Records a removal that was executed or observed externally. */
  processRemoved: (id: number) => void;
}

/**
 * Owns semantic process-pane activation and lifecycle navigation.
 *
 * The history is uncapped and independent from xterm's keep-alive pool. Mutable snapshots update
 * only after React commits: process data uses `useLatestRef`, while selection also updates inside
 * explicit callbacks so activate-then-stop sequences remain synchronous within one event turn.
 */
export function useProcessActivationNavigation(
  processes: ProcessView[],
  callbacks: ProcessActivationCallbacks,
): ProcessActivationNavigation {
  const [selectedId, setSelectedId] = useState<number | null>(null);
  const activationHistoryRef = useRef<number[]>([]);
  const selectedIdRef = useRef<number | null>(null);
  const knownProcessIdsRef = useRef(new Set<number>());
  const processesRef = useLatestRef(processes);
  const callbacksRef = useLatestRef(callbacks);

  useLayoutEffect(() => {
    selectedIdRef.current = selectedId;
  }, [selectedId]);

  const deselectProcess = useCallback(() => {
    selectedIdRef.current = null;
    setSelectedId(null);
  }, []);

  const openStart = useCallback(() => {
    selectedIdRef.current = null;
    setSelectedId(null);
    callbacksRef.current.onClearAlternativeView();
  }, [callbacksRef]);

  const selectProcess = useCallback(
    (id: number) => {
      activationHistoryRef.current = activateProcess(activationHistoryRef.current, id);
      selectedIdRef.current = id;
      setSelectedId(id);
      callbacksRef.current.onClearAlternativeView();
    },
    [callbacksRef],
  );

  const navigateAfterLifecycle = useCallback(
    (ids: readonly number[]) => {
      activationHistoryRef.current = forgetProcesses(activationHistoryRef.current, ids);
      const selected = selectedIdRef.current;
      if (selected === null || !ids.includes(selected)) return;

      const next = mostRecentAvailableProcess(
        activationHistoryRef.current,
        processesRef.current.map((process) => process.id),
      );
      if (next === null) openStart();
      else selectProcess(next);
    },
    [openStart, processesRef, selectProcess],
  );

  const processStopped = useCallback(
    (id: number) => navigateAfterLifecycle([id]),
    [navigateAfterLifecycle],
  );

  const startProcess = useCallback(
    (id: number) => {
      callbacksRef.current.onStart(id);
      selectProcess(id);
    },
    [callbacksRef, selectProcess],
  );

  const restartProcess = useCallback(
    (id: number) => {
      callbacksRef.current.onRestart(id);
      selectProcess(id);
    },
    [callbacksRef, selectProcess],
  );

  const resumeProcess = useCallback(
    (id: number) => {
      callbacksRef.current.onResume(id);
      selectProcess(id);
    },
    [callbacksRef, selectProcess],
  );

  const projectStopped = useCallback(
    (projectId: number) => {
      const stoppedIds = processesRef.current
        .filter((process) => process.project === projectId && isActive(process.status))
        .map((process) => process.id);
      navigateAfterLifecycle(stoppedIds);
    },
    [navigateAfterLifecycle, processesRef],
  );

  const removalRequested = useCallback(
    (id: number) => {
      const process = processesRef.current.find((candidate) => candidate.id === id);
      if (process === undefined || !isActive(process.status)) navigateAfterLifecycle([id]);
    },
    [navigateAfterLifecycle, processesRef],
  );

  const processRemoved = useCallback(
    (id: number) => navigateAfterLifecycle([id]),
    [navigateAfterLifecycle],
  );

  // Reconciliation and project teardown can remove a selected row without a UI action. Only ids
  // observed in the prior committed snapshot count as disappeared, so a newly created process
  // selected before its spawn delta arrives is not mistaken for removal.
  useLayoutEffect(() => {
    const currentIds = new Set(processes.map((process) => process.id));
    const disappeared = [...knownProcessIdsRef.current].filter((id) => !currentIds.has(id));
    knownProcessIdsRef.current = currentIds;
    if (disappeared.length > 0) navigateAfterLifecycle(disappeared);
  }, [processes, navigateAfterLifecycle]);

  const getSelectedId = useCallback(() => selectedIdRef.current, []);

  return {
    selectedId,
    selectProcess,
    deselectProcess,
    openStart,
    getSelectedId,
    processStopped,
    startProcess,
    restartProcess,
    resumeProcess,
    projectStopped,
    removalRequested,
    processRemoved,
  };
}
