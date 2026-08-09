import type { ProcessView } from "@/domain";

/**
 * The agent a handoff should reach given what the reader is looking at, or null to leave the choice
 * to the core.
 *
 * The one fact the core cannot know is which process is on screen, so it is the one thing the
 * surface supplies. Everything else stays the core's: a process that is not this project's running
 * agent is not named at all, and the core then falls back to the project's only running one — or
 * offers the context to copy when there is no single answer.
 */
export function handoffTarget(selected: ProcessView | null, project: number | null): number | null {
  if (selected === null || project === null) return null;
  if (selected.project !== project) return null;
  if (selected.kind !== "Agent" || selected.status !== "Running") return null;
  return selected.id;
}
