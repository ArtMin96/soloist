import { useEffect, useRef, useState } from "react";

// The most terminals kept alive at once. Enough that switching between recently-viewed processes
// is instant (each keeps its xterm + live PTY stream mounted), but bounded well under WebKitGTK's
// 16-live-WebGL-context cap — each live terminal holds one context, and the oldest would be
// force-lost past the cap. The least-recently-selected terminal is evicted beyond this.
export const TERMINAL_POOL_CAP = 6;

// The next keep-alive pool (most-recently-selected first): the current pool filtered to processes
// that still exist, with the current selection promoted to the front, capped so the
// least-recently-viewed terminal is dropped. Returns the same array reference when nothing changed,
// so an unrelated render never re-creates the terminal set. Pure — the hook below drives it.
export function nextPool(
  prev: number[],
  selectedId: number | null,
  existing: Iterable<number>,
  cap: number,
): number[] {
  const alive = new Set(existing);
  let pool = prev.filter((id) => alive.has(id));
  if (selectedId !== null && alive.has(selectedId)) {
    pool = [selectedId, ...pool.filter((id) => id !== selectedId)];
  }
  if (pool.length > cap) pool = pool.slice(0, cap);
  const unchanged = pool.length === prev.length && pool.every((id, i) => id === prev[i]);
  return unchanged ? prev : pool;
}

// Tracks which process terminals to keep mounted (xterm + PTY stream alive) so switching back to a
// recently-viewed process is instant. The selection is always included; the least-recently-selected
// process is evicted once the pool is full, and a process that leaves the registry drops out.
// Returns the pooled ids, most-recently-selected first.
export function useTerminalPool(
  selectedId: number | null,
  existingIds: number[],
  cap: number = TERMINAL_POOL_CAP,
): number[] {
  const [pool, setPool] = useState<number[]>([]);
  // Depend on the *membership* of the process set (its ordered ids joined), not the array identity,
  // so a fresh `existingIds` array with the same members does not re-run the effect. The latest
  // array is read from a ref inside the effect so it is not itself a dependency.
  const existingKey = existingIds.join(",");
  const existingRef = useRef(existingIds);
  existingRef.current = existingIds;
  useEffect(() => {
    setPool((prev) => nextPool(prev, selectedId, existingRef.current, cap));
  }, [selectedId, existingKey, cap]);
  return pool;
}
