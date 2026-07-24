import type { ProcStatus } from "@/domain";

// The single source for turning a process status into its display. DESIGN.md encodes
// status redundantly — glyph + color + label — so it survives color blindness and a
// grayscale screenshot; never by hue alone. The exhaustive Record makes the compiler
// require an entry for every status.
export interface StatusDisplay {
  /** Human label, e.g. "Running". */
  label: string;
  /** A shape that carries the state without color (●/◐/○/✕/⚠). */
  glyph: string;
  /** Tailwind text-color utility for the glyph, bound to a `--status-*` token. */
  toneClass: string;
  /** An in-flight state — the indicator pulses its glyph (reduced-motion: no pulse).
   *  Shared by the agent-activity display, where Thinking pulses the same way. */
  transitional: boolean;
}

export const STATUS: Record<ProcStatus, StatusDisplay> = {
  Running: {
    label: "Running",
    glyph: "●",
    toneClass: "text-status-running",
    transitional: false,
  },
  Starting: {
    label: "Starting",
    glyph: "◐",
    toneClass: "text-status-transition",
    transitional: true,
  },
  Restarting: {
    label: "Restarting",
    glyph: "◐",
    toneClass: "text-status-transition",
    transitional: true,
  },
  Stopping: {
    label: "Stopping",
    glyph: "◐",
    toneClass: "text-status-transition",
    transitional: true,
  },
  Stopped: {
    label: "Stopped",
    glyph: "○",
    toneClass: "text-status-stopped",
    transitional: false,
  },
  Crashed: {
    label: "Crashed",
    glyph: "✕",
    toneClass: "text-status-crashed",
    transitional: false,
  },
  RestartExhausted: {
    label: "Restart limit reached",
    glyph: "⚠",
    toneClass: "text-status-exhausted",
    transitional: false,
  },
};

/** Whether a process is currently running (the steady green state, not in-flight). */
export function isRunning(status: ProcStatus): boolean {
  return status === "Running";
}

/** In-flight toward Running — the states during which a pending auto-restart shows its
 *  "restarting k/N" progress. Stopping is in-flight too, but toward rest, so it is excluded. */
export function isStarting(status: ProcStatus): boolean {
  return status === "Starting" || status === "Restarting";
}

// Whether a process in each state has a live owning actor, mirroring core `ProcStatus::is_active`.
// An exhaustive Record rather than a condition chain for the same reason STATUS is one: a status
// added to the union stops the build here until it is answered for, instead of silently defaulting
// to inactive and quietly mis-enabling every control below.
const ACTIVE: Record<ProcStatus, boolean> = {
  Starting: true,
  Running: true,
  Restarting: true,
  Stopping: true,
  Stopped: false,
  Crashed: false,
  RestartExhausted: false,
};

/** Whether a process currently has a live owning actor (mirrors core `ProcStatus::is_active`). */
export function isActive(status: ProcStatus): boolean {
  return ACTIVE[status];
}

// What follows is which controls to *offer*, not which calls the core will accept — the two are
// deliberately different. The core takes a stop whenever a process is active (including one
// already stopping) and a restart from any state at all. These narrow that to the affordances
// worth showing, so the row does not offer a control whose effect the user cannot see. Keeping
// them here keeps presentation policy out of the domain; the core stays the authority on what is
// legal and refuses anything these let through.

/** Start begins a fresh run from an ordinary stop. Crash recovery is named Restart. */
export function canStart(status: ProcStatus): boolean {
  return status === "Stopped";
}

/** Stop is offered while live and not already stopping — a second stop would read as a no-op. */
export function canStop(status: ProcStatus): boolean {
  return isActive(status) && status !== "Stopping";
}

/** Restart cycles a live process or explicitly retries a failed/exhausted one. */
export function canRestart(status: ProcStatus): boolean {
  return status === "Running" || status === "Crashed" || status === "RestartExhausted";
}
