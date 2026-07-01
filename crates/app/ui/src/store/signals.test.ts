import { describe, expect, it } from "vitest";
import { applySignal, EMPTY_SIGNALS } from "@/store/signals";

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

  it("records the latest idle summary, replacing an earlier one while the agent runs", () => {
    let state = applySignal(EMPTY_SIGNALS, {
      type: "AgentSummary",
      id: 1,
      text: "Reading the spec",
    });
    state = applySignal(state, { type: "AgentSummary", id: 1, text: "Writing the migration" });
    expect(state.summary.get(1)).toBe("Writing the migration");

    // A transition into Running does not drop the summary.
    state = applySignal(state, {
      type: "ProcessStatusChanged",
      id: 1,
      from: "Starting",
      to: "Running",
      exit_code: null,
    });
    expect(state.summary.get(1)).toBe("Writing the migration");
  });

  it("clears the idle summary when the agent leaves Running", () => {
    let state = applySignal(EMPTY_SIGNALS, { type: "AgentSummary", id: 1, text: "Writing tests" });
    state = applySignal(state, {
      type: "ProcessStatusChanged",
      id: 1,
      from: "Running",
      to: "Stopped",
      exit_code: 0,
    });
    expect(state.summary.has(1)).toBe(false);
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
    state = applySignal(state, { type: "AgentSummary", id: 1, text: "Running the suite" });
    state = applySignal(state, { type: "ProcessRemoved", id: 1 });
    expect(state.metrics.has(1)).toBe(false);
    expect(state.attempts.has(1)).toBe(false);
    expect(state.activity.has(1)).toBe(false);
    expect(state.summary.has(1)).toBe(false);
  });

  it("returns the same reference for an unrelated event", () => {
    const state = applySignal(EMPTY_SIGNALS, { type: "MetricsTick", id: 1, cpu_pct: 5, rss: 1 });
    const next = applySignal(state, { type: "TerminalBell", id: 1 });
    expect(next).toBe(state);
  });
});
