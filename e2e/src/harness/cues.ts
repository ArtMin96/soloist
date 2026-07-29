import { rmSync, writeFileSync } from "node:fs";
import path from "node:path";

/**
 * The cues the fixture project's stub commands watch for, each a file dropped in the project
 * directory they run from.
 *
 * A stub acts on a cue rather than on start because starting a process from its row selects it,
 * and the core suppresses an alert about the process the user is looking at — so a signal that has
 * to reach the user must be raised after the spec has looked somewhere else.
 */
export const CUE = {
  /** `Faulty` exits nonzero. */
  crash: "cue-crash",
  /** `Signaller` rings the terminal bell. */
  bell: "cue-bell",
  /** `Signaller` raises a notification of its own over OSC 777. */
  notify: "cue-notify",
} as const;

export type Cue = (typeof CUE)[keyof typeof CUE];

/** Gives the cue, so the stub watching for it acts. */
export function give(projectRoot: string, cue: Cue): void {
  writeFileSync(path.join(projectRoot, cue), "");
}

/** Takes the cue back, re-arming the stub for the next time it is started. */
export function withdraw(projectRoot: string, cue: Cue): void {
  rmSync(path.join(projectRoot, cue), { force: true });
}
