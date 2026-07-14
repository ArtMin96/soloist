// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { act, renderHook, waitFor } from "@testing-library/react";

// orphansResolve and the event subscription are the IPC boundary; mock them so the test
// drives the hook's own logic — surfacing groups, and dropping/keeping rows by the core's
// actual outcome.
vi.mock("@/api", () => ({
  orphansResolve: vi.fn(),
  onDomainEvent: vi.fn(() => Promise.resolve(() => {})),
}));

import { onDomainEvent, orphansResolve } from "@/api";
import type { DomainEvent } from "@/domain";
import { useOrphans } from "@/store/useOrphans";

const resolve = vi.mocked(orphansResolve);
const subscribe = vi.mocked(onDomainEvent);

const found: DomainEvent = {
  type: "OrphansFound",
  orphans: [
    { name: "web", command: "npm run dev", pgid: 555 },
    { name: "worker", command: "node worker.js", pgid: 556 },
  ],
};

afterEach(() => vi.clearAllMocks());

// Fires a captured `domain-event` into the hook's subscriber.
function fire(event: DomainEvent) {
  const handler = subscribe.mock.calls[0]?.[0];
  if (!handler) throw new Error("no event subscriber registered");
  act(() => handler(event));
}

describe("useOrphans", () => {
  it("surfaces leftover groups when reconciliation announces them", () => {
    const { result } = renderHook(() => useOrphans(vi.fn()));
    fire(found);
    expect(result.current.orphans?.map((o) => o.pgid)).toEqual([555, 556]);
  });

  it("drops a row only after the core confirms the kill", async () => {
    resolve.mockResolvedValue(undefined);
    const { result } = renderHook(() => useOrphans(vi.fn()));
    fire(found);

    act(() => result.current.killOne(555));

    await waitFor(() => expect(result.current.orphans?.map((o) => o.pgid)).toEqual([556]));
    expect(resolve).toHaveBeenCalledWith([555]);
  });

  it("keeps the row and reports the error when a kill fails", async () => {
    resolve.mockRejectedValue("Could not stop leftover process: pgid 555 (EPERM)");
    const reportError = vi.fn();
    const { result } = renderHook(() => useOrphans(reportError));
    fire(found);

    act(() => result.current.killOne(555));

    await waitFor(() =>
      expect(reportError).toHaveBeenCalledWith("Could not stop leftover process: pgid 555 (EPERM)"),
    );
    // The still-running leftover stays actionable in the dialog.
    expect(result.current.orphans?.map((o) => o.pgid)).toEqual([555, 556]);
  });

  it("keeps every row and reports when kill-all fails", async () => {
    resolve.mockRejectedValue("Could not stop leftover process: pgid 555 (EPERM)");
    const reportError = vi.fn();
    const { result } = renderHook(() => useOrphans(reportError));
    fire(found);

    act(() => result.current.killAll());

    // Each group is reaped independently, so both failures surface and both rows stay.
    await waitFor(() => expect(reportError).toHaveBeenCalledTimes(2));
    expect(result.current.orphans?.map((o) => o.pgid)).toEqual([555, 556]);
  });

  it("drops only the killed rows when kill-all partially fails", async () => {
    resolve.mockImplementation((pgids: number[]) =>
      pgids.includes(556) ? Promise.reject("pgid 556 (EPERM)") : Promise.resolve(),
    );
    const reportError = vi.fn();
    const { result } = renderHook(() => useOrphans(reportError));
    fire(found);

    act(() => result.current.killAll());

    // 555 was reaped (row drops); 556 failed (row stays, error surfaced).
    await waitFor(() => expect(result.current.orphans?.map((o) => o.pgid)).toEqual([556]));
    expect(reportError).toHaveBeenCalledTimes(1);
  });

  it("closes the dialog when kill-all succeeds", async () => {
    resolve.mockResolvedValue(undefined);
    const { result } = renderHook(() => useOrphans(vi.fn()));
    fire(found);

    act(() => result.current.killAll());

    await waitFor(() => expect(result.current.orphans).toBeNull());
    expect(resolve).toHaveBeenCalledWith([555]);
    expect(resolve).toHaveBeenCalledWith([556]);
  });
});
