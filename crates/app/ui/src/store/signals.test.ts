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

  it("forgets both signals when a process leaves the registry", () => {
    let state = applySignal(EMPTY_SIGNALS, {
      type: "MetricsTick",
      id: 1,
      cpu_pct: 5,
      rss: 1024,
    });
    state = applySignal(state, { type: "RestartScheduled", id: 1, attempt: 1 });
    state = applySignal(state, { type: "ProcessRemoved", id: 1 });
    expect(state.metrics.has(1)).toBe(false);
    expect(state.attempts.has(1)).toBe(false);
  });

  it("returns the same reference for an unrelated event", () => {
    const state = applySignal(EMPTY_SIGNALS, { type: "MetricsTick", id: 1, cpu_pct: 5, rss: 1 });
    const next = applySignal(state, { type: "TerminalBell", id: 1 });
    expect(next).toBe(state);
  });
});
