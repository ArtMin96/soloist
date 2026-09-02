// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { act, renderHook, waitFor } from "@testing-library/react";

// The read, the event subscription, and the resync signal are the IPC boundary; mock them so the
// test drives the hook's own logic — the enabled gate, staleness, and frame coalescing.
vi.mock("@/api", () => ({
  sessionWork: vi.fn(),
  onDomainEvent: vi.fn(() => Promise.resolve(() => {})),
  onResync: vi.fn(() => Promise.resolve(() => {})),
}));

import { onDomainEvent, sessionWork } from "@/api";
import type { SessionWork } from "@/domain";
import { useSessionWork } from "@/store/useSessionWork";

const read = vi.mocked(sessionWork);
const domainEvent = vi.mocked(onDomainEvent);

afterEach(() => vi.clearAllMocks());

function work(process: number, project = 1): SessionWork {
  return {
    process,
    project,
    todos: [
      {
        id: 1,
        title: "wire the header",
        status: "open",
        blocked: false,
        locked: true,
        access: "worked",
      },
    ],
    scratchpads: [],
  };
}

/** The hook's own domain-event subscriber, as the Tauri bridge would call it. */
function emit(event: Parameters<Parameters<typeof onDomainEvent>[0]>[0]) {
  const handler = domainEvent.mock.calls[0]?.[0];
  if (!handler) throw new Error("no domain-event subscriber registered");
  act(() => handler(event));
}

describe("useSessionWork", () => {
  it("reads nothing and subscribes to nothing while disabled", async () => {
    renderHook(() => useSessionWork(1, false));
    await act(async () => {
      await Promise.resolve();
    });
    expect(read).not.toHaveBeenCalled();
    expect(domainEvent).not.toHaveBeenCalled();
  });

  it("seeds from the process's session work when enabled", async () => {
    read.mockResolvedValue(work(1));
    const { result } = renderHook(() => useSessionWork(1, true));
    await waitFor(() => expect(result.current.work).not.toBeNull());
    expect(result.current.work?.process).toBe(1);
  });

  it("a SessionWorkChanged for this process triggers exactly one re-read", async () => {
    read.mockResolvedValue(work(1));
    const { result } = renderHook(() => useSessionWork(1, true));
    await waitFor(() => expect(result.current.work).not.toBeNull());
    const seeded = read.mock.calls.length;

    read.mockResolvedValue(work(1));
    emit({ type: "SessionWorkChanged", process: 1 });

    await waitFor(() => expect(read.mock.calls.length).toBe(seeded + 1));
  });

  it("a SessionWorkChanged for a different process triggers none", async () => {
    read.mockResolvedValue(work(1));
    const { result } = renderHook(() => useSessionWork(1, true));
    await waitFor(() => expect(result.current.work).not.toBeNull());
    const seeded = read.mock.calls.length;

    emit({ type: "SessionWorkChanged", process: 2 });
    await act(async () => {
      await new Promise((resolve) => requestAnimationFrame(resolve));
    });
    expect(read.mock.calls.length).toBe(seeded);
  });

  it("a TodoChanged for this work's project triggers a re-read; one for another project does not", async () => {
    read.mockResolvedValue(work(1, 7));
    const { result } = renderHook(() => useSessionWork(1, true));
    await waitFor(() => expect(result.current.work?.project).toBe(7));
    const seeded = read.mock.calls.length;

    emit({ type: "TodoChanged", project: 9, id: 1 });
    await act(async () => {
      await new Promise((resolve) => requestAnimationFrame(resolve));
    });
    expect(read.mock.calls.length).toBe(seeded);

    read.mockResolvedValue(work(1, 7));
    emit({ type: "TodoChanged", project: 7, id: 1 });
    await waitFor(() => expect(read.mock.calls.length).toBe(seeded + 1));
  });

  it("a burst of events in one frame coalesces to a single read", async () => {
    read.mockResolvedValue(work(1));
    const { result } = renderHook(() => useSessionWork(1, true));
    await waitFor(() => expect(result.current.work).not.toBeNull());
    const seeded = read.mock.calls.length;

    read.mockResolvedValue(work(1));
    emit({ type: "SessionWorkChanged", process: 1 });
    emit({ type: "SessionWorkChanged", process: 1 });
    emit({ type: "SessionWorkChanged", process: 1 });

    await waitFor(() => expect(read.mock.calls.length).toBe(seeded + 1));
    // Give any further (wrongly scheduled) frames a chance to land before asserting the count holds.
    await act(async () => {
      await new Promise((resolve) => requestAnimationFrame(resolve));
    });
    expect(
      read.mock.calls.length - seeded,
      "a chatty run costs one re-read per frame, not one per event",
    ).toBe(1);
  });

  it("a response that arrives after the process changed is discarded", async () => {
    let resolveFirst: (value: SessionWork | null) => void = () => {};
    let firstReadInFlight = false;
    read.mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          firstReadInFlight = true;
          resolveFirst = resolve;
        }),
    );
    const { result, rerender } = renderHook(({ process }) => useSessionWork(process, true), {
      initialProps: { process: 1 },
    });
    // Let the process-1 read genuinely start before switching, so its late resolution below is a
    // real stale-in-flight response rather than one that never got issued.
    await waitFor(() => expect(firstReadInFlight).toBe(true));

    read.mockResolvedValue(work(2));
    rerender({ process: 2 });
    await waitFor(() => expect(result.current.work?.process).toBe(2));

    act(() => resolveFirst(work(1)));
    await act(async () => {
      await Promise.resolve();
    });
    expect(
      result.current.work?.process,
      "the stale process-1 payload must never be shown once process 2 is current",
    ).not.toBe(1);
  });

  it("unmount removes the listener and cancels the pending frame", async () => {
    read.mockResolvedValue(work(1));
    const unlisten = vi.fn();
    domainEvent.mockImplementation(() => Promise.resolve(unlisten));
    const cancel = vi.spyOn(window, "cancelAnimationFrame");
    const { unmount } = renderHook(() => useSessionWork(1, true));
    await waitFor(() => expect(domainEvent).toHaveBeenCalled());

    read.mockResolvedValue(work(1));
    emit({ type: "SessionWorkChanged", process: 1 });
    unmount();

    expect(unlisten).toHaveBeenCalled();
    expect(cancel).toHaveBeenCalled();
    cancel.mockRestore();
  });

  it("a failed read surfaces its reason and does not throw", async () => {
    read.mockRejectedValue(new Error("boom"));
    const { result } = renderHook(() => useSessionWork(1, true));
    await waitFor(() => expect(result.current.error).toContain("boom"));
  });
});
