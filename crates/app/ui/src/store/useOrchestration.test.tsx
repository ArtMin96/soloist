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
import { LoadStatus } from "@/store/loadable";
import { holdRead } from "@/test/heldRead";
import {
  useOrchestration,
  type OrchestrationReadModel,
  type OrchestrationStore,
} from "@/store/useOrchestration";

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

// The read model the hook is showing, refusing to answer while it has nothing to show — so a test
// asserting on the data can never pass against a board that is still waiting for its first read.
function modelOf(result: { current: OrchestrationStore }): OrchestrationReadModel {
  const { snapshot } = result.current;
  if (snapshot.status !== LoadStatus.Ready) {
    throw new Error(`the snapshot is ${snapshot.status}, not ready`);
  }
  return snapshot.value;
}

/** The hook's own domain-event subscriber, as the Tauri bridge would call it. */
function emit(event: Parameters<Parameters<typeof onDomainEvent>[0]>[0]) {
  const handler = domainEvent.mock.calls[0]?.[0];
  if (!handler) throw new Error("no domain-event subscriber registered");
  act(() => handler(event));
}

/** The hook's own resync subscriber, as the backend's reconcile signal would call it. */
function reconcile() {
  const handler = resync.mock.calls[0]?.[0];
  if (!handler) throw new Error("no resync subscriber registered");
  act(() => handler());
}

describe("useOrchestration", () => {
  it("seeds the board from the project's snapshot", async () => {
    read.mockResolvedValue(snapshotWith([1, 2]));
    const { result } = renderHook(() => useOrchestration(1));
    await waitFor(() => expect(modelOf(result).agents).toHaveLength(2));
  });

  it("reports loading until the first snapshot lands", async () => {
    const settle = holdRead(read);
    const { result } = renderHook(() => useOrchestration(1));

    // Nothing has been read yet, so there is nothing to show — an empty board is a different claim.
    expect(result.current.snapshot.status).toBe(LoadStatus.Loading);

    settle(snapshotWith([1, 2]));

    await waitFor(() => expect(modelOf(result).agents).toHaveLength(2));
  });

  it("reports a failed first read with nothing to show", async () => {
    read.mockRejectedValue(new Error("db locked"));
    const { result } = renderHook(() => useOrchestration(1));

    await waitFor(() =>
      expect(result.current.snapshot).toEqual({
        status: LoadStatus.Failed,
        error: "Error: db locked",
      }),
    );
  });

  it("keeps the last snapshot ready and reports a failed re-read", async () => {
    read.mockResolvedValue(snapshotWith([1, 2]));
    const { result } = renderHook(() => useOrchestration(1));
    await waitFor(() => expect(modelOf(result).agents).toHaveLength(2));

    read.mockRejectedValue(new Error("db locked"));
    reconcile();

    await waitFor(() => expect(result.current.error).toBe("Error: db locked"));
    // A refresh that could not be read is no reason to take away the board being read: the reader
    // keeps what they had, and is told the refresh failed.
    expect(modelOf(result).agents).toHaveLength(2);
  });

  it("clears the error once a re-read succeeds", async () => {
    read.mockRejectedValue(new Error("db locked"));
    const { result } = renderHook(() => useOrchestration(1));
    await waitFor(() => expect(result.current.snapshot.status).toBe(LoadStatus.Failed));

    read.mockResolvedValue(snapshotWith([1]));
    emit({ type: "ProcessRemoved", id: 2 });

    await waitFor(() => expect(modelOf(result).agents).toHaveLength(1));
    expect(result.current.error).toBeNull();
  });

  it("re-reads on a backend resync, healing a dropped coordination delta", async () => {
    read.mockResolvedValue(snapshotWith([1, 2]));
    const { result } = renderHook(() => useOrchestration(1));
    await waitFor(() => expect(modelOf(result).agents).toHaveLength(2));

    // A process-lifecycle delta was dropped, so the board still shows the departed agent; a resync
    // re-reads the snapshot and reconciles it.
    read.mockResolvedValue(snapshotWith([1]));
    reconcile();
    await waitFor(() => expect(modelOf(result).agents).toHaveLength(1));
  });

  it("re-reads the snapshot when a recorded agent message changes", async () => {
    read.mockResolvedValue(snapshotWith([1, 2]));
    const { result } = renderHook(() => useOrchestration(1));
    await waitFor(() => expect(modelOf(result).agents).toHaveLength(2));
    expect(modelOf(result).messages).toHaveLength(0);

    read.mockResolvedValue(snapshotWith([1, 2], [record(10)]));
    emit({ type: "AgentMessageChanged", project: 1, id: 10 });

    await waitFor(() => expect(modelOf(result).messages).toHaveLength(1));
    expect(modelOf(result).messages[0]?.delivery.message.body).toBe("review the parser");
  });

  it("coalesces a burst of message changes into one re-read", async () => {
    read.mockResolvedValue(snapshotWith([1, 2]));
    const { result } = renderHook(() => useOrchestration(1));
    await waitFor(() => expect(modelOf(result).agents).toHaveLength(2));
    const seeded = read.mock.calls.length;

    read.mockResolvedValue(snapshotWith([1, 2], [record(10), record(11), record(12)]));
    for (const id of [10, 11, 12]) {
      emit({ type: "AgentMessageChanged", project: 1, id });
    }

    await waitFor(() => expect(modelOf(result).messages).toHaveLength(3));
    expect(
      read.mock.calls.length - seeded,
      "a chatty run costs one re-read per frame, not one per message",
    ).toBe(1);
  });
});
