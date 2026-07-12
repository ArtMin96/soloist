import { describe, expect, it, vi } from "vitest";
import { createSignalStore, EMPTY_STORE, fixedSignalStore } from "@/store/signalStore";
import { EMPTY_SIGNALS } from "@/store/signals";

describe("createSignalStore", () => {
  it("folds an event into a new snapshot and notifies subscribers once", () => {
    const store = createSignalStore();
    const listener = vi.fn();
    store.subscribe(listener);
    const before = store.getSnapshot();

    store.apply({ type: "MetricsTick", id: 1, cpu_pct: 5, rss: 10 });

    expect(listener).toHaveBeenCalledTimes(1);
    const after = store.getSnapshot();
    expect(after).not.toBe(before);
    expect(after.metrics.get(1)).toEqual({ cpu_pct: 5, rss: 10 });
  });

  it("neither notifies nor churns the snapshot for an unrelated event", () => {
    const store = createSignalStore();
    store.apply({ type: "MetricsTick", id: 1, cpu_pct: 5, rss: 10 });
    const before = store.getSnapshot();
    const listener = vi.fn();
    store.subscribe(listener);

    // A bell carries no signal state, so the fold returns the same reference.
    store.apply({ type: "TerminalBell", id: 1 });

    expect(listener).not.toHaveBeenCalled();
    expect(store.getSnapshot()).toBe(before);
  });

  it("stops notifying after unsubscribe", () => {
    const store = createSignalStore();
    const listener = vi.fn();
    const unsubscribe = store.subscribe(listener);
    unsubscribe();

    store.apply({ type: "MetricsTick", id: 1, cpu_pct: 1, rss: 1 });

    expect(listener).not.toHaveBeenCalled();
  });
});

describe("fixedSignalStore", () => {
  it("returns its fixed snapshot and never notifies", () => {
    const store = fixedSignalStore(EMPTY_SIGNALS);
    const listener = vi.fn();
    store.subscribe(listener)();
    store.apply({ type: "MetricsTick", id: 1, cpu_pct: 1, rss: 1 });

    expect(store.getSnapshot()).toBe(EMPTY_SIGNALS);
    expect(listener).not.toHaveBeenCalled();
  });

  it("EMPTY_STORE reads the empty signals", () => {
    expect(EMPTY_STORE.getSnapshot()).toBe(EMPTY_SIGNALS);
  });
});
