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

import { onDomainEvent, onResync, orchestrationSnapshot } from "@/api";
import type { AgentMessageRecord, OrchestrationSnapshot } from "@/domain";
import { useOrchestration } from "@/store/useOrchestration";

const read = vi.mocked(orchestrationSnapshot);
const resync = vi.mocked(onResync);
const domainEvent = vi.mocked(onDomainEvent);

afterEach(() => vi.clearAllMocks());

// A snapshot whose only varying part is its agent set, enough to observe a re-read landing.
function snapshotWith(
  agentIds: number[],
  messages: AgentMessageRecord[] = [],
): OrchestrationSnapshot {
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
    messages,
  };
}

function record(id: number): AgentMessageRecord {
  return {
    delivery: {
      message: {
        id,
        project: 1,
        sender: 1,
        recipient: 2,
        kind: "direct",
        body: "review the parser",
        todo_id: null,
      },
      outcome: "queued",
    },
    at_unix_millis: 1_700_000_000_000,
    truncated: false,
  };
}

/** The hook's own domain-event subscriber, as the Tauri bridge would call it. */
function emit(event: Parameters<Parameters<typeof onDomainEvent>[0]>[0]) {
  const handler = domainEvent.mock.calls[0]?.[0];
  if (!handler) throw new Error("no domain-event subscriber registered");
  act(() => handler(event));
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

  it("re-reads the snapshot when a recorded agent message changes", async () => {
    read.mockResolvedValue(snapshotWith([1, 2]));
    const { result } = renderHook(() => useOrchestration(1));
    await waitFor(() => expect(result.current.agents).toHaveLength(2));
    expect(result.current.messages).toHaveLength(0);

    read.mockResolvedValue(snapshotWith([1, 2], [record(10)]));
    emit({ type: "AgentMessageChanged", project: 1, id: 10 });

    await waitFor(() => expect(result.current.messages).toHaveLength(1));
    expect(result.current.messages[0]?.delivery.message.body).toBe("review the parser");
  });

  it("coalesces a burst of message changes into one re-read", async () => {
    read.mockResolvedValue(snapshotWith([1, 2]));
    const { result } = renderHook(() => useOrchestration(1));
    await waitFor(() => expect(result.current.agents).toHaveLength(2));
    const seeded = read.mock.calls.length;

    read.mockResolvedValue(snapshotWith([1, 2], [record(10), record(11), record(12)]));
    for (const id of [10, 11, 12]) {
      emit({ type: "AgentMessageChanged", project: 1, id });
    }

    await waitFor(() => expect(result.current.messages).toHaveLength(3));
    expect(
      read.mock.calls.length - seeded,
      "a chatty run costs one re-read per frame, not one per message",
    ).toBe(1);
  });
});
