import { describe, expect, it } from "vitest";
import {
  bodyPreview,
  fireBadge,
  formatCountdown,
  formatPausedRemaining,
  groupByOwner,
} from "./timerPanel";
import type { TimerView } from "@/domain";

describe("fireBadge", () => {
  it("returns Scheduled for at timers", () => {
    expect(fireBadge({ kind: "at" })).toBe("Scheduled");
  });
  it("returns When any idle for when_idle_any", () => {
    expect(fireBadge({ kind: "when_idle_any", watched: [1, 2] })).toBe("When any idle");
  });
  it("returns When all idle for when_idle_all", () => {
    expect(fireBadge({ kind: "when_idle_all", watched: [3] })).toBe("When all idle");
  });
});

describe("formatCountdown", () => {
  it("returns 0s for zero", () => {
    expect(formatCountdown(0)).toBe("0s");
  });
  it("returns 0s for negative", () => {
    expect(formatCountdown(-500)).toBe("0s");
  });
  it("formats seconds only", () => {
    expect(formatCountdown(45_000)).toBe("45s");
  });
  it("formats minutes and seconds", () => {
    expect(formatCountdown(3 * 60 * 1000 + 7 * 1000)).toBe("3m 7s");
  });
  it("formats hours, minutes, and seconds", () => {
    expect(formatCountdown(2 * 3600 * 1000 + 15 * 60 * 1000 + 30 * 1000)).toBe("2h 15m 30s");
  });
  it("truncates sub-second remainder", () => {
    expect(formatCountdown(1_999)).toBe("1s");
  });
});

describe("formatPausedRemaining", () => {
  it("returns 0s remaining for zero or negative", () => {
    expect(formatPausedRemaining(0)).toBe("0s remaining");
    expect(formatPausedRemaining(-1)).toBe("0s remaining");
  });
  it("formats the frozen remainder directly, independent of the wall clock", () => {
    expect(formatPausedRemaining(45_000)).toBe("45s remaining");
    expect(formatPausedRemaining(70_000)).toBe("1m 10s remaining");
    expect(formatPausedRemaining(2 * 3600 * 1000 + 15 * 60 * 1000)).toBe("2h 15m remaining");
  });
  it("does not drift — the same remainder always reads the same", () => {
    expect(formatPausedRemaining(70_000)).toBe(formatPausedRemaining(70_000));
  });
});

describe("bodyPreview", () => {
  it("returns the first line when short", () => {
    expect(bodyPreview("review the output")).toBe("review the output");
  });
  it("uses only the first line of a multi-line body", () => {
    expect(bodyPreview("line one\nline two\nline three")).toBe("line one");
  });
  it("truncates lines longer than 60 chars", () => {
    const long = "a".repeat(65);
    const preview = bodyPreview(long);
    expect(preview.length).toBeLessThanOrEqual(60);
    expect(preview.endsWith("…")).toBe(true);
  });
  it("does not truncate exactly 60-char lines", () => {
    const exactly60 = "b".repeat(60);
    expect(bodyPreview(exactly60)).toBe(exactly60);
  });
});

function makeTimer(id: number, owner: number): TimerView {
  return {
    id,
    owner,
    body: "go",
    fire: { kind: "at" },
    status: "armed",
    deadline_unix_millis: 0,
    waiting_on: [],
    already_idle: false,
    paused_remaining_millis: null,
  };
}

describe("groupByOwner", () => {
  it("returns an empty map for no timers", () => {
    expect(groupByOwner([])).toEqual(new Map());
  });
  it("groups timers by their owner process id", () => {
    const timers = [makeTimer(1, 10), makeTimer(2, 20), makeTimer(3, 10)];
    const groups = groupByOwner(timers);
    expect(groups.get(10)).toHaveLength(2);
    expect(groups.get(20)).toHaveLength(1);
  });
});
