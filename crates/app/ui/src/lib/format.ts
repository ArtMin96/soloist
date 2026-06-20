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
