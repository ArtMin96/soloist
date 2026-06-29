// Pure helpers for the timers panel. No IPC, no state — only derivations over `TimerView` values
// from the orchestration snapshot, so these are straightforwardly testable.

import type { FireCond, TimerView } from "@/domain";

/** Human-readable badge label for a `FireCond` variant. */
export function fireBadge(fire: FireCond): string {
  switch (fire.kind) {
    case "at":
      return "Scheduled";
    case "when_idle_any":
      return "When any idle";
    case "when_idle_all":
      return "When all idle";
  }
}

/**
 * Formats a duration (milliseconds) into a compact `Xh Ym Zs` string.
 * Negative or zero values render as "0s".
 */
export function formatCountdown(remainingMs: number): string {
  if (remainingMs <= 0) return "0s";
  const totalSecs = Math.floor(remainingMs / 1000);
  const h = Math.floor(totalSecs / 3600);
  const m = Math.floor((totalSecs % 3600) / 60);
  const s = totalSecs % 60;
  if (h > 0) return `${h}h ${m}m ${s}s`;
  if (m > 0) return `${m}m ${s}s`;
  return `${s}s`;
}

/**
 * Formats a paused timer's frozen remaining duration as `Xh Ym remaining`. Takes the milliseconds
 * that remained when the timer was paused (`TimerView.paused_remaining_millis`) directly, so the
 * value stays constant while paused instead of drifting with the wall clock.
 */
export function formatPausedRemaining(remainingMillis: number): string {
  const remaining = remainingMillis;
  if (remaining <= 0) return "0s remaining";
  const total = Math.floor(remaining / 1000);
  const h = Math.floor(total / 3600);
  const m = Math.floor((total % 3600) / 60);
  const s = total % 60;
  if (h > 0) return `${h}h ${m}m remaining`;
  if (m > 0) return `${m}m ${s}s remaining`;
  return `${s}s remaining`;
}

/**
 * Truncates a timer body for the panel preview. Bodies can be long agent instructions;
 * the panel shows only the first line, trimmed to ≤60 chars.
 */
export function bodyPreview(body: string): string {
  const firstLine = body.split("\n")[0] ?? body;
  return firstLine.length > 60 ? firstLine.slice(0, 57) + "…" : firstLine;
}

/** Groups timers by their owning agent's process id. */
export function groupByOwner(timers: TimerView[]): Map<number, TimerView[]> {
  const map = new Map<number, TimerView[]>();
  for (const t of timers) {
    const group = map.get(t.owner) ?? [];
    group.push(t);
    map.set(t.owner, group);
  }
  return map;
}
