import { canRestart, canStart, canStop } from "@/lib/status";
import type { ProcessView, ProcStatus } from "@/domain";

/** A control action that can be run on a process. The closed set the control cluster and the
 *  palettes share, in canonical offer order (trust first, then the launch/stop verbs). */
export type ProcessActionKind = "trust" | "resume" | "start" | "stop" | "restart";

/** The status-derived inputs that decide which actions are currently available. */
export interface ProcessActionState {
  status: ProcStatus;
  requiresTrust: boolean;
  resumable: boolean;
}

/** The human label for each action — one source for every surface that names an action. */
export const PROCESS_ACTION_LABELS: Record<ProcessActionKind, string> = {
  trust: "Trust",
  resume: "Resume",
  start: "Start",
  stop: "Stop",
  restart: "Restart",
};

// The actions currently runnable on a process, derived once from the status FSM (lib/status)
// plus the trust and resume flags. This is the single source both the per-process control cluster
// and the action palettes read, so "what can I do to this process" lives in exactly one place. An
// untrusted command offers only Trust until trusted — start/resume are withheld because the core
// trust gate would refuse them regardless of which surface asked.
export function processActions(state: ProcessActionState): ProcessActionKind[] {
  const { status, requiresTrust, resumable } = state;
  const actions: ProcessActionKind[] = [];
  if (requiresTrust) actions.push("trust");
  if (canStart(status) && !requiresTrust) {
    if (resumable) actions.push("resume");
    actions.push("start");
  }
  if (canStop(status)) actions.push("stop");
  if (canRestart(status)) actions.push("restart");
  return actions;
}

/** The callbacks a surface supplies to run a process action. */
export interface ProcessActionHandlers {
  onTrust: (project: number, name: string) => void;
  onResume: (id: number) => void;
  onStart: (id: number) => void;
  onStop: (id: number) => void;
  onRestart: (id: number) => void;
}

/** One runnable action bound to its callback — what the palettes list and dispatch. */
export interface RunnableProcessAction {
  kind: ProcessActionKind;
  label: string;
  run: () => void;
}

// Binds a process's available actions (from `processActions`) to the caller's handlers. The palettes
// render this list directly, so neither re-derives the availability gating nor the run-dispatch.
export function runnableProcessActions(
  process: ProcessView,
  handlers: ProcessActionHandlers,
): RunnableProcessAction[] {
  return processActions({
    status: process.status,
    requiresTrust: process.requires_trust,
    resumable: process.resumable,
  }).map((kind) => ({
    kind,
    label: PROCESS_ACTION_LABELS[kind],
    run: runFor(kind, process, handlers),
  }));
}

function runFor(
  kind: ProcessActionKind,
  process: ProcessView,
  handlers: ProcessActionHandlers,
): () => void {
  switch (kind) {
    case "trust":
      return () => handlers.onTrust(process.project, process.label);
    case "resume":
      return () => handlers.onResume(process.id);
    case "start":
      return () => handlers.onStart(process.id);
    case "stop":
      return () => handlers.onStop(process.id);
    case "restart":
      return () => handlers.onRestart(process.id);
  }
}
