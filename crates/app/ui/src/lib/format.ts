// Formatting for the at-a-glance process telemetry — CPU, memory, ports — rendered in the
// monospace data face so digits align. Pure functions, unit-tested directly.

/** A whole-machine CPU percentage as a compact integer, e.g. 4 -> "4%". Normalised so 100%
 *  is every core busy; it never exceeds 100. */
export function formatCpu(pct: number): string {
  return `${Math.round(pct)}%`;
}

/** Resident memory (bytes) as a compact human size: whole KB/MB up to a gigabyte, then one
 *  decimal GB. Binary units (1024), matching how process monitors report RSS. */
export function formatRss(bytes: number): string {
  const kib = bytes / 1024;
  if (kib < 1024) return `${Math.round(kib)} KB`;
  const mib = kib / 1024;
  if (mib < 1024) return `${Math.round(mib)} MB`;
  return `${(mib / 1024).toFixed(1)} GB`;
}

/** Listening ports as a compact primary + overflow count, e.g. [5173] -> ":5173",
 *  [5173, 9229] -> ":5173 +1". Empty -> null (nothing to show). */
export function formatPorts(ports: number[]): string | null {
  if (ports.length === 0) return null;
  const [first, ...rest] = ports;
  return rest.length ? `:${first} +${rest.length}` : `:${first}`;
}

// The stamp a document written before the core recorded write times carries: an absent time, not a
// write at the epoch.
const NO_WRITE_TIME = 0;
const MILLIS_PER_MINUTE = 60_000;
const MILLIS_PER_HOUR = 60 * MILLIS_PER_MINUTE;
const MILLIS_PER_DAY = 24 * MILLIS_PER_HOUR;
// Past a week, counting days stops helping and the calendar date reads faster.
const CALENDAR_FROM_DAYS = 7;

/** How long ago a document was last written, e.g. "just now", "5 min ago", "3 h ago", "2 d ago",
 *  and the calendar date from a week back — null when it carries no write time, which must render
 *  as nothing rather than as 1970. Pure: the caller passes the clock reading. */
export function formatUpdatedAt(updatedAtMillis: number, nowMillis: number): string | null {
  if (updatedAtMillis === NO_WRITE_TIME) return null;
  const elapsed = nowMillis - updatedAtMillis;
  if (elapsed < MILLIS_PER_MINUTE) return "just now";
  if (elapsed < MILLIS_PER_HOUR) return `${Math.floor(elapsed / MILLIS_PER_MINUTE)} min ago`;
  if (elapsed < MILLIS_PER_DAY) return `${Math.floor(elapsed / MILLIS_PER_HOUR)} h ago`;
  const days = Math.floor(elapsed / MILLIS_PER_DAY);
  if (days < CALENDAR_FROM_DAYS) return `${days} d ago`;
  const written = new Date(updatedAtMillis);
  const sameYear = written.getFullYear() === new Date(nowMillis).getFullYear();
  return new Intl.DateTimeFormat(undefined, {
    month: "short",
    day: "numeric",
    year: sameYear ? undefined : "numeric",
  }).format(written);
}
