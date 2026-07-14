import { describe, expect, it } from "vitest";
import { applySignal, EMPTY_SIGNALS, seedActivity } from "@/store/signals";

describe("applySignal", () => {
  it("records the latest CPU/memory reading per process", () => {
    let state = applySignal(EMPTY_SIGNALS, {
      type: "MetricsTick",
      id: 1,
      cpu_pct: 12.5,
      rss: 4096,
    });
    state = applySignal(state, { type: "MetricsTick", id: 1, cpu_pct: 30, rss: 8192 });
    expect(state.metrics.get(1)).toEqual({ cpu_pct: 30, rss: 8192 });
  });

  it("tracks the auto-restart attempt and clears it once the command settles", () => {
    let state = applySignal(EMPTY_SIGNALS, { type: "RestartScheduled", id: 1, attempt: 3 });
    expect(state.attempts.get(1)).toBe(3);

    state = applySignal(state, {
      type: "ProcessStatusChanged",
      id: 1,
      from: "Starting",
      to: "Running",
      exit_code: null,
    });
    expect(state.attempts.has(1)).toBe(false);
  });

  it("clears the attempt when the command is held exhausted", () => {
    let state = applySignal(EMPTY_SIGNALS, { type: "RestartScheduled", id: 1, attempt: 10 });
    state = applySignal(state, { type: "RestartExhausted", id: 1 });
    expect(state.attempts.has(1)).toBe(false);
  });

  it("keeps the attempt through the restart cycle's transient states", () => {
    let state = applySignal(EMPTY_SIGNALS, { type: "RestartScheduled", id: 1, attempt: 2 });
    state = applySignal(state, {
      type: "ProcessStatusChanged",
      id: 1,
      from: "Crashed",
      to: "Starting",
      exit_code: null,
    });
    expect(state.attempts.get(1)).toBe(2);
  });

  it("records the latest agent activity, keeping it while the agent stays Running", () => {
    let state = applySignal(EMPTY_SIGNALS, {
      type: "AgentActivityChanged",
      id: 1,
      state: "Thinking",
    });
    state = applySignal(state, { type: "AgentActivityChanged", id: 1, state: "Working" });
    expect(state.activity.get(1)).toBe("Working");

    // A transition into Running does not drop activity.
    state = applySignal(state, {
      type: "ProcessStatusChanged",
      id: 1,
      from: "Starting",
      to: "Running",
      exit_code: null,
    });
    expect(state.activity.get(1)).toBe("Working");
  });

  it("clears agent activity when the agent leaves Running", () => {
    let state = applySignal(EMPTY_SIGNALS, {
      type: "AgentActivityChanged",
      id: 1,
      state: "Working",
    });
    state = applySignal(state, {
      type: "ProcessStatusChanged",
      id: 1,
      from: "Running",
      to: "Stopped",
      exit_code: 0,
    });
    expect(state.activity.has(1)).toBe(false);
  });

  it("forgets all signals when a process leaves the registry", () => {
    let state = applySignal(EMPTY_SIGNALS, {
      type: "MetricsTick",
      id: 1,
      cpu_pct: 5,
      rss: 1024,
    });
    state = applySignal(state, { type: "RestartScheduled", id: 1, attempt: 1 });
    state = applySignal(state, { type: "AgentActivityChanged", id: 1, state: "Working" });
    state = applySignal(state, { type: "ProcessRemoved", id: 1 });
    expect(state.metrics.has(1)).toBe(false);
    expect(state.attempts.has(1)).toBe(false);
    expect(state.activity.has(1)).toBe(false);
  });

  it("returns the same reference for an unrelated event", () => {
    const state = applySignal(EMPTY_SIGNALS, { type: "MetricsTick", id: 1, cpu_pct: 5, rss: 1 });
    const next = applySignal(state, { type: "TerminalBell", id: 1 });
    expect(next).toBe(state);
  });
});

describe("seedActivity", () => {
  it("replaces the activity map with the snapshot, dropping entries not in it", () => {
    // A stale badge from a dropped `AgentActivityChanged`: id 1 shows Working, but the true state
    // is Idle, and id 2 has left the registry entirely. The seed reconciles to the snapshot.
    let state = applySignal(EMPTY_SIGNALS, {
      type: "AgentActivityChanged",
      id: 1,
      state: "Working",
    });
    state = applySignal(state, { type: "AgentActivityChanged", id: 2, state: "Thinking" });

    const seeded = seedActivity(state, [{ id: 1, activity: "Idle" }]);
    expect(seeded.activity.get(1)).toBe("Idle");
    expect(seeded.activity.has(2)).toBe(false);
  });

  it("leaves metrics and attempts untouched — it reconciles only the idle badges", () => {
    let state = applySignal(EMPTY_SIGNALS, { type: "MetricsTick", id: 1, cpu_pct: 5, rss: 1024 });
    state = applySignal(state, { type: "RestartScheduled", id: 1, attempt: 2 });

    const seeded = seedActivity(state, [{ id: 9, activity: "Idle" }]);
    expect(seeded.metrics.get(1)).toEqual({ cpu_pct: 5, rss: 1024 });
    expect(seeded.attempts.get(1)).toBe(2);
    expect(seeded.activity.get(9)).toBe("Idle");
  });

  it("returns the same reference when the badge set is unchanged", () => {
    const state = applySignal(EMPTY_SIGNALS, {
      type: "AgentActivityChanged",
      id: 1,
      state: "Working",
    });
    expect(seedActivity(state, [{ id: 1, activity: "Working" }])).toBe(state);
  });
});
