/**
 * Moves the item at `from` to `to`, returning a new array. Indices outside the array leave it
 * unchanged, so a move computed against a list that has since shrunk is a no-op rather than a
 * corrupted order.
 */
export function moveItem<T>(items: readonly T[], from: number, to: number): T[] {
  const next = [...items];
  if (!inRange(next, from) || !inRange(next, to)) return next;
  const [moved] = next.splice(from, 1);
  next.splice(to, 0, moved);
  return next;
}

/**
 * Moves `id` `delta` places through `items` — negative is toward the front. A move that would
 * leave the list is clamped to its end, and an unknown id changes nothing.
 */
export function moveBy<T>(items: readonly T[], id: T, delta: number): T[] {
  const from = items.indexOf(id);
  if (from === -1) return [...items];
  return moveItem(items, from, clamp(from + delta, 0, items.length - 1));
}

/** Whether `id` can still move `delta` places — false at the end it is already against. */
export function canMoveBy<T>(items: readonly T[], id: T, delta: number): boolean {
  const from = items.indexOf(id);
  return from !== -1 && inRange(items, from + delta);
}

function inRange(items: readonly unknown[], index: number): boolean {
  return index >= 0 && index < items.length;
}

function clamp(value: number, min: number, max: number): number {
  return Math.min(Math.max(value, min), max);
}
