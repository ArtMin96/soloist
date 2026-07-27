// @vitest-environment jsdom
import { act, renderHook } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { useProcessActivationNavigation } from "@/store/useProcessActivationNavigation";
import type { ProcessView } from "@/domain";

function process(id: number, status: ProcessView["status"] = "Running"): ProcessView {
  return {
    id,
    project: 1,
    kind: "Terminal",
    label: `Terminal ${id}`,
    status,
    exit_code: null,
    requires_trust: false,
    resumable: false,
    ports: [],
    ready: "Ungated",
  };
}

type ActivationCallbacks = Parameters<typeof useProcessActivationNavigation>[1];

function callbacks(overrides: Partial<ActivationCallbacks> = {}): ActivationCallbacks {
  return {
    onClearAlternativeView: vi.fn(),
    onStart: vi.fn(),
    onRestart: vi.fn(),
    onResume: vi.fn(),
    ...overrides,
  };
}

describe("useProcessActivationNavigation", () => {
  it("handles activate-activate-stop synchronously before React commits the selections", () => {
    const view = renderHook(() =>
      useProcessActivationNavigation([process(1), process(2)], callbacks()),
    );

    act(() => {
      view.result.current.selectProcess(1);
      view.result.current.selectProcess(2);
      view.result.current.processStopped(2);
    });

    expect(view.result.current.selectedId).toBe(1);
    expect(view.result.current.getSelectedId()).toBe(1);
  });

  it("forgets every live project target before choosing a fallback", () => {
    const view = renderHook(() =>
      useProcessActivationNavigation([process(1, "Stopped"), process(2), process(3)], callbacks()),
    );

    act(() => {
      view.result.current.selectProcess(1);
      view.result.current.selectProcess(3);
      view.result.current.selectProcess(2);
      view.result.current.projectStopped(1);
    });

    expect(view.result.current.selectedId).toBe(1);
  });

  it("does not navigate for a live removal request until removal is confirmed", () => {
    const view = renderHook(() =>
      useProcessActivationNavigation([process(1, "Stopped"), process(2)], callbacks()),
    );

    act(() => {
      view.result.current.selectProcess(1);
      view.result.current.selectProcess(2);
      view.result.current.removalRequested(2);
    });
    expect(view.result.current.selectedId).toBe(2);

    act(() => view.result.current.processRemoved(2));
    expect(view.result.current.selectedId).toBe(1);
  });

  it("falls back when the selected process disappears from a committed snapshot", () => {
    const handlers = callbacks();
    const view = renderHook(
      ({ processes }: { processes: ProcessView[] }) =>
        useProcessActivationNavigation(processes, handlers),
      { initialProps: { processes: [process(1), process(2)] } },
    );

    act(() => {
      view.result.current.selectProcess(1);
      view.result.current.selectProcess(2);
    });
    view.rerender({ processes: [process(1)] });

    expect(view.result.current.selectedId).toBe(1);
  });
});
