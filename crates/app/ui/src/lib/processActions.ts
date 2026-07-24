import { canRestart, canStart, canStop, isActive } from "@/lib/status";
import type { ProcessKind, ProcessView, ProcStatus } from "@/domain";

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
const PROCESS_ACTION_LABELS: Record<ProcessActionKind, string> = {
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

  // Active work wins over a newly-dirty trust flag: the existing child may keep running, Stop is
  // still safe, and Restart would be refused by the core's trust gate. Transitional processes
  // expose only cancellation while the actor can still receive it.
  if (canStop(status)) actions.push("stop");
  if (isActive(status)) {
    if (canRestart(status) && !requiresTrust) actions.push("restart");
    return actions;
  }

  // Trust gates every launch path. While resting it is the only honest next action.
  if (requiresTrust) return ["trust"];

  if (canStart(status)) {
    if (resumable) actions.push("resume");
    actions.push("start");
  }
  if (canRestart(status)) actions.push("restart");
  return actions;
}

/** Whether the row should keep its recovery/trust action visible without hover. */
export function shouldPersistProcessActions(state: ProcessActionState): boolean {
  const actions = processActions(state);
  if (actions.includes("trust")) return true;
  return !isActive(state.status) && actions.includes("restart");
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

/** One-click intent plus the actions progressively disclosed behind a menu. */
export interface ProcessActionPresentation<T> {
  primary: T | null;
  secondary: T[];
}

/**
 * Orders an already-resolved action list for dense process surfaces. Commands optimize for the
 * common supervisor operation (Restart); interactive agents and terminals optimize for Stop.
 * Availability stays entirely in `processActions`—this function chooses prominence only.
 */
export function presentProcessActions<T extends { kind: ProcessActionKind }>(
  kind: ProcessKind,
  status: ProcStatus,
  actions: T[],
): ProcessActionPresentation<T> {
  const preferred = status === "Running" && kind === "Command" ? "restart" : actions[0]?.kind;
  const primary = actions.find((action) => action.kind === preferred) ?? null;
  return {
    primary,
    secondary: primary == null ? [] : actions.filter((action) => action !== primary),
  };
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
    label: actionLabel(kind, process.resumable),
    run: runFor(kind, process, handlers),
  }));
}

function actionLabel(kind: ProcessActionKind, resumable: boolean): string {
  if (kind === "resume") return "Resume last session";
  if (kind === "start" && resumable) return "Start fresh";
  return PROCESS_ACTION_LABELS[kind];
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
