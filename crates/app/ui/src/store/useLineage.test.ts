// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { act, renderHook, waitFor } from "@testing-library/react";

// The lineage read, the event subscription, and the resync signal are the IPC boundary; mock them
// so the test drives the hook's own logic — seeding the map, re-reading on a lifecycle event, and
// re-reading on a backend resync.
vi.mock("@/api", () => ({
  lineageEdges: vi.fn(),
  onDomainEvent: vi.fn(() => Promise.resolve(() => {})),
  onResync: vi.fn(() => Promise.resolve(() => {})),
}));

import { lineageEdges, onDomainEvent, onResync } from "@/api";
import type { DomainEvent, ProcessView } from "@/domain";
import { liveWorkerCount, useLineage } from "@/store/useLineage";

const read = vi.mocked(lineageEdges);
const subscribe = vi.mocked(onDomainEvent);
const resync = vi.mocked(onResync);

afterEach(() => vi.clearAllMocks());

// Fires a captured `domain-event` into the hook's subscriber.
function fire(event: DomainEvent) {
  const handler = subscribe.mock.calls[0]?.[0];
  if (!handler) throw new Error("no event subscriber registered");
  act(() => handler(event));
}

describe("useLineage", () => {
  it("seeds the child→parent map from the edges read", async () => {
    read.mockResolvedValue([{ child: 2, parent: 1 }]);
    const { result } = renderHook(() => useLineage());
    await waitFor(() => expect(result.current.get(2)).toBe(1));
    expect(result.current.size).toBe(1);
  });

  it("re-reads when a process leaves the registry", async () => {
    read.mockResolvedValue([{ child: 2, parent: 1 }]);
    const { result } = renderHook(() => useLineage());
    await waitFor(() => expect(result.current.size).toBe(1));

    read.mockResolvedValue([]);
    fire({ type: "ProcessRemoved", id: 1 });
    await waitFor(() => expect(result.current.size).toBe(0));
  });

  it("ignores events that cannot change lineage", async () => {
    read.mockResolvedValue([]);
    const { result } = renderHook(() => useLineage());
    await waitFor(() => expect(result.current.size).toBe(0));
    const readsAfterSeed = read.mock.calls.length;

    fire({ type: "MetricsTick", id: 1, cpu_pct: 1, rss: 1 });
    // The rAF the hook coalesces into would have fired well within this wait.
    await new Promise((resolve) => setTimeout(resolve, 50));
    expect(read.mock.calls.length).toBe(readsAfterSeed);
  });

  it("re-reads on a backend resync, healing a dropped lifecycle delta", async () => {
    read.mockResolvedValue([{ child: 2, parent: 1 }]);
    const { result } = renderHook(() => useLineage());
    await waitFor(() => expect(result.current.size).toBe(1));

    // A `ProcessRemoved` was dropped, so the map still nests the departed worker; a resync re-reads.
    read.mockResolvedValue([]);
    const handler = resync.mock.calls[0]?.[0];
    if (!handler) throw new Error("no resync subscriber registered");
    act(() => handler());
    await waitFor(() => expect(result.current.size).toBe(0));
  });
});

describe("liveWorkerCount", () => {
  const process = (id: number): ProcessView => ({
    id,
    project: 1,
    kind: "Agent",
    label: `Agent ${id}`,
    status: "Running",
    exit_code: null,
    requires_trust: false,
    resumable: false,
    ports: [],
    ready: "Ungated",
  });

  it("counts the workers a lead spawned", () => {
    const lineage = new Map([
      [2, 1],
      [3, 1],
    ]);
    expect(liveWorkerCount(lineage, [process(1), process(2), process(3)], 1)).toBe(2);
  });

  it("counts none for a lead that spawned nothing", () => {
    expect(liveWorkerCount(new Map([[2, 1]]), [process(1), process(2)], 2)).toBe(0);
  });

  it("ignores a worker that has already left the registry", () => {
    // An edge whose child is gone is not a live worker — the same rule the tree nesting applies,
    // and the reason the count is taken against the process list rather than the raw map.
    expect(liveWorkerCount(new Map([[2, 1]]), [process(1)], 1)).toBe(0);
  });

  it("does not count another lead's workers", () => {
    const lineage = new Map([
      [3, 1],
      [4, 2],
    ]);
    expect(liveWorkerCount(lineage, [process(1), process(2), process(3), process(4)], 1)).toBe(1);
  });
});
