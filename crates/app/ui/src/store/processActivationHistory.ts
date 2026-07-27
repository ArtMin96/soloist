/**
 * Application-owned activation history for xterm-backed process panes.
 *
 * Newest ids come first. This is intentionally independent from the terminal keep-alive pool:
 * navigation needs the complete visited history even after a pane has been evicted from that pool.
 */
export function activateProcess(history: readonly number[], id: number): number[] {
  return [id, ...history.filter((candidate) => candidate !== id)];
}

/** Permanently removes processes stopped or removed through a navigation lifecycle action. */
export function forgetProcesses(history: readonly number[], ids: Iterable<number>): number[] {
  const forgotten = new Set(ids);
  return history.filter((id) => !forgotten.has(id));
}

/** Returns the newest activated process that is still present, never an arbitrary sidebar row. */
export function mostRecentAvailableProcess(
  history: readonly number[],
  availableIds: Iterable<number>,
): number | null {
  const available = new Set(availableIds);
  return history.find((id) => available.has(id)) ?? null;
}
