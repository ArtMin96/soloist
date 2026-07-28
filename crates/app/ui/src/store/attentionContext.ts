import { createContext, use, useMemo } from "react";
import { unreadProcessIds, unreadProjectIds } from "@/lib/attention";
import type { AttentionSnapshot, ProcessView } from "@/domain";

export interface AttentionMarks {
  /** The processes whose rows carry a marker. */
  processes: ReadonlySet<number>;
  /** The projects whose headers carry a dot, because something inside them is unread. */
  projects: ReadonlySet<number>;
}

const NOTHING_UNREAD: AttentionMarks = { processes: new Set(), projects: new Set() };

// Which rows and which project headers are marked. Read at the leaves — every sidebar row, every
// project header — but it would otherwise drill through the four pass-through components between
// the sidebar and a row, so it travels by context the way per-process telemetry does. The default
// is nothing unread, so a component rendered without the provider (a focused test) shows no marker
// rather than throwing.
export const AttentionContext = createContext<AttentionMarks>(NOTHING_UNREAD);

/** Whether this process has an alert waiting. */
export function useUnreadProcess(id: number): boolean {
  return use(AttentionContext).processes.has(id);
}

/** Whether any process in this project has an alert waiting. */
export function useUnreadProject(id: number): boolean {
  return use(AttentionContext).projects.has(id);
}

/** The marks the sidebar renders, derived from the core's snapshot and recomputed only when it
 *  or the stack changes. */
export function useAttentionMarks(
  snapshot: AttentionSnapshot,
  processes: ProcessView[],
): AttentionMarks {
  return useMemo(
    () => ({
      processes: unreadProcessIds(snapshot),
      projects: unreadProjectIds(snapshot, processes),
    }),
    [snapshot, processes],
  );
}
