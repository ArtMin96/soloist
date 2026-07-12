// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, render, screen } from "@testing-library/react";
import { createSignalStore } from "@/store/signalStore";
import { SignalsContext, useSignal } from "@/store/signalsContext";

afterEach(cleanup);

// A leaf that reads one process's signal and reports each render, so a test can assert exactly which
// consumers re-rendered.
function Probe({ id, onRender }: { id: number; onRender: () => void }) {
  const { metrics } = useSignal(id);
  onRender();
  return <span data-testid={`probe-${id}`}>{metrics?.cpu_pct ?? "—"}</span>;
}

describe("useSignal", () => {
  it("re-renders only the consumer whose process ticked", () => {
    const store = createSignalStore();
    const renderOne = vi.fn();
    const renderTwo = vi.fn();
    render(
      <SignalsContext value={store}>
        <Probe id={1} onRender={renderOne} />
        <Probe id={2} onRender={renderTwo} />
      </SignalsContext>,
    );
    const baseOne = renderOne.mock.calls.length;
    const baseTwo = renderTwo.mock.calls.length;

    act(() => {
      store.apply({ type: "MetricsTick", id: 1, cpu_pct: 7, rss: 100 });
    });

    // Process 1's row updated; process 2's row did not re-render at all.
    expect(renderOne.mock.calls.length).toBe(baseOne + 1);
    expect(renderTwo.mock.calls.length).toBe(baseTwo);
    expect(screen.getByTestId("probe-1").textContent).toBe("7");
    expect(screen.getByTestId("probe-2").textContent).toBe("—");
  });

  it("does not re-render when the same reading repeats", () => {
    const store = createSignalStore();
    const renderOne = vi.fn();
    render(
      <SignalsContext value={store}>
        <Probe id={1} onRender={renderOne} />
      </SignalsContext>,
    );
    act(() => {
      store.apply({ type: "MetricsTick", id: 1, cpu_pct: 7, rss: 100 });
    });
    const afterFirst = renderOne.mock.calls.length;

    // A fresh reading object with identical values must not re-render (value-level equality).
    act(() => {
      store.apply({ type: "MetricsTick", id: 1, cpu_pct: 7, rss: 100 });
    });

    expect(renderOne.mock.calls.length).toBe(afterFirst);
  });
});
