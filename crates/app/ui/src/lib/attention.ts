import type { AttentionSnapshot, ProcessAttention, ProcessView } from "@/domain";

// Nothing waiting. What every unread surface renders from before the first snapshot lands, so a
// cold start shows no marker rather than a flash of one.
export const NO_ATTENTION: AttentionSnapshot = { processes: [], total: 0 };

// What every unread indicator is called — the row marker, the project dot and the title-bar
// control all name the same state, so they read as one thing rather than three.
export const ATTENTION_LABEL = "Needs attention";

// The largest count the title bar prints; past it the reading is "99+". The snapshot keeps
// counting truthfully — this is presentation, and a surface that needs the real number reads
// `total`.
export const ATTENTION_DISPLAY_CAP = 99;

/** What the title-bar control reads: the exact total up to the cap, then "99+". */
export function attentionCountLabel(total: number): string {
  return total > ATTENTION_DISPLAY_CAP ? `${ATTENTION_DISPLAY_CAP}+` : String(total);
}

/** The processes with something waiting, as a set a row can test itself against. */
export function unreadProcessIds(snapshot: AttentionSnapshot): ReadonlySet<number> {
  return new Set(snapshot.processes.map((entry) => entry.process));
}

// The projects owning at least one unread process. The header dot reads this, which is why it
// still shows when a project's commands are collapsed or scrolled out of view — the dot is a
// property of the project, not of any row that happens to be on screen.
export function unreadProjectIds(
  snapshot: AttentionSnapshot,
  processes: ProcessView[],
): ReadonlySet<number> {
  const unread = unreadProcessIds(snapshot);
  return new Set(
    processes.filter((process) => unread.has(process.id)).map((process) => process.project),
  );
}

/** One line of the title bar's unread list: what the core reports, named for the user. */
export interface AttentionEntry extends ProcessAttention {
  label: string;
}

// The unread list, in the order the core keeps. A process the stack no longer holds is dropped:
// its row is gone, so an entry naming it could not be acted on. Nothing else is derived here — the
// kind and the count are the core's, so what the list reads and what the badge counts cannot
// disagree.
export function attentionEntries(
  snapshot: AttentionSnapshot,
  processes: ProcessView[],
): AttentionEntry[] {
  const byId = new Map(processes.map((process) => [process.id, process]));
  return snapshot.processes.flatMap((entry) => {
    const process = byId.get(entry.process);
    return process ? [{ ...entry, label: process.label }] : [];
  });
}
