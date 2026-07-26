// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { act, renderHook, waitFor } from "@testing-library/react";

// The snapshot read, the event subscription, and the resync signal are the IPC boundary; mock them
// so the test drives the hook's own logic — seeding from the snapshot and re-reading on a resync.
vi.mock("@/api", () => ({
  orchestrationSnapshot: vi.fn(),
  onDomainEvent: vi.fn(() => Promise.resolve(() => {})),
  onResync: vi.fn(() => Promise.resolve(() => {})),
}));

import { onResync, orchestrationSnapshot } from "@/api";
import type { OrchestrationSnapshot } from "@/domain";
import { useOrchestration } from "@/store/useOrchestration";

const read = vi.mocked(orchestrationSnapshot);
const resync = vi.mocked(onResync);

afterEach(() => vi.clearAllMocks());

// A snapshot whose only varying part is its agent set, enough to observe a re-read landing.
function snapshotWith(agentIds: number[]): OrchestrationSnapshot {
  return {
    project: 1,
    agents: agentIds.map((id) => ({
      id,
      parent: null,
      label: `agent-${id}`,
      kind: "Agent",
      status: "Running",
      activity: null,
    })),
    todos: [],
    timers: [],
    leases: [],
    scratchpads: [],
    diagrams: [],
    kv: [],
  };
}

describe("useOrchestration", () => {
  it("seeds the board from the project's snapshot", async () => {
    read.mockResolvedValue(snapshotWith([1, 2]));
    const { result } = renderHook(() => useOrchestration(1));
    await waitFor(() => expect(result.current.agents).toHaveLength(2));
  });

  it("re-reads on a backend resync, healing a dropped coordination delta", async () => {
    read.mockResolvedValue(snapshotWith([1, 2]));
    const { result } = renderHook(() => useOrchestration(1));
    await waitFor(() => expect(result.current.agents).toHaveLength(2));

    // A process-lifecycle delta was dropped, so the board still shows the departed agent; a resync
    // re-reads the snapshot and reconciles it.
    read.mockResolvedValue(snapshotWith([1]));
    const handler = resync.mock.calls[0]?.[0];
    if (!handler) throw new Error("no resync subscriber registered");
    act(() => handler());
    await waitFor(() => expect(result.current.agents).toHaveLength(1));
  });
});
