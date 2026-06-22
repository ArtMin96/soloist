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
  Running: { label: "Running", glyph: "●", toneClass: "text-status-running", transitional: false },
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
  Stopped: { label: "Stopped", glyph: "○", toneClass: "text-status-stopped", transitional: false },
  Crashed: { label: "Crashed", glyph: "✕", toneClass: "text-status-crashed", transitional: false },
  RestartExhausted: {
    label: "Exhausted",
    glyph: "⚠",
    toneClass: "text-status-exhausted",
    transitional: true,
  },
};

/** The auto-restart rate-limit gate, mirrored from the core: a crashed command is relaunched
 *  at most this many times within a 60-second window before it is held in RestartExhausted.
 *  Surfaced as the "restarting k/N" row affordance. */
export const RESTART_LIMIT = 10;

/** Whether a process is currently running (the steady green state, not in-flight). */
export function isRunning(status: ProcStatus): boolean {
  return status === "Running";
}

/** In-flight toward Running — the states during which a pending auto-restart shows its
 *  "restarting k/N" progress. Stopping is in-flight too, but toward rest, so it is excluded. */
export function isStarting(status: ProcStatus): boolean {
  return status === "Starting" || status === "Restarting";
}

/** Whether a process currently has a live owning actor (mirrors core `is_active`). */
export function isActive(status: ProcStatus): boolean {
  return (
    status === "Starting" ||
    status === "Running" ||
    status === "Restarting" ||
    status === "Stopping"
  );
}

/** Start is offered only from a resting state. */
export function canStart(status: ProcStatus): boolean {
  return !isActive(status);
}

/** Stop is offered while live and not already stopping. */
export function canStop(status: ProcStatus): boolean {
  return isActive(status) && status !== "Stopping";
}

/** Restart (stop + start) is offered for a running process. */
export function canRestart(status: ProcStatus): boolean {
  return status === "Running";
}
