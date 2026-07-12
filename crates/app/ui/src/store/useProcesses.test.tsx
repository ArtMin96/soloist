// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { act, renderHook, waitFor } from "@testing-library/react";

// procList and the event subscription are the IPC boundary; mock them so the test drives the
// hook's own hydration logic — buffering deltas during the snapshot fetch and replaying them.
vi.mock("@/api", () => ({
  procList: vi.fn(),
  onDomainEvent: vi.fn(() => Promise.resolve(() => {})),
  procStart: vi.fn(),
  procStop: vi.fn(),
  procRestart: vi.fn(),
  agentResume: vi.fn(),
  stackStart: vi.fn(),
  stackStop: vi.fn(),
  stackRestartRunning: vi.fn(),
}));

import { onDomainEvent, procList } from "@/api";
import type { DomainEvent, ProcessView } from "@/domain";
import { useProcesses } from "@/store/useProcesses";

const list = vi.mocked(procList);
const subscribe = vi.mocked(onDomainEvent);

const spawned: DomainEvent = {
  type: "ProcessSpawned",
  id: 42,
  project: 1,
  kind: "Command",
  label: "Web",
  status: "Starting",
  requires_trust: false,
  resumable: false,
};

afterEach(() => vi.clearAllMocks());

async function firstEventHandler(): Promise<(event: DomainEvent) => void> {
  await waitFor(() => expect(subscribe).toHaveBeenCalled());
  const handler = subscribe.mock.calls[0]?.[0];
  if (!handler) throw new Error("no event subscriber registered");
  return handler;
}

describe("useProcesses hydration", () => {
  it("replays an event that arrives during the fetch instead of clobbering it", async () => {
    // Hold the snapshot pending so we can fire a spawn while it is in flight.
    let resolveSnapshot!: (rows: ProcessView[]) => void;
    list.mockReturnValue(
      new Promise<ProcessView[]>((resolve) => {
        resolveSnapshot = resolve;
      }),
    );

    const { result } = renderHook(() => useProcesses());
    const handler = await firstEventHandler();
    await waitFor(() => expect(list).toHaveBeenCalled());

    // A process spawns while the snapshot (which predates it) is still in flight.
    act(() => handler(spawned));

    // The snapshot resolves without that process; the buffered spawn must be replayed on top.
    await act(async () => {
      resolveSnapshot([]);
      await Promise.resolve();
    });

    expect(result.current.processes.map((p) => p.id)).toEqual([42]);
  });

  it("applies live events after hydration", async () => {
    list.mockResolvedValue([]);
    const { result } = renderHook(() => useProcesses());
    const handler = await firstEventHandler();
    await waitFor(() => expect(result.current.processes).toEqual([]));

    act(() => handler(spawned));
    await waitFor(() => expect(result.current.processes.map((p) => p.id)).toEqual([42]));
  });
});
