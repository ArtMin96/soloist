// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { act, renderHook, waitFor } from "@testing-library/react";

// The lineage read and the event subscription are the IPC boundary; mock them so the test
// drives the hook's own logic — seeding the map and re-reading on a lifecycle event.
vi.mock("@/api", () => ({
  lineageEdges: vi.fn(),
  onDomainEvent: vi.fn(() => Promise.resolve(() => {})),
}));

import { lineageEdges, onDomainEvent } from "@/api";
import type { DomainEvent } from "@/domain";
import { useLineage } from "@/store/useLineage";

const read = vi.mocked(lineageEdges);
const subscribe = vi.mocked(onDomainEvent);

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
});
