// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { act, renderHook, waitFor } from "@testing-library/react";

// The decisions and the event subscription are the IPC boundary; mocking them leaves the hook's
// own logic — what enters the queue, what leaves it, and what a refused decision does — as the
// thing under test.
vi.mock("@/api", () => ({
  onDomainEvent: vi.fn(() => Promise.resolve(() => {})),
  trustRequestApprove: vi.fn(),
  trustRequestDeny: vi.fn(),
}));

import { onDomainEvent, trustRequestApprove, trustRequestDeny } from "@/api";
import type { DomainEvent, TrustRequest } from "@/domain";
import { useTrustRequests } from "@/store/useTrustRequests";

const subscribe = vi.mocked(onDomainEvent);
const approve = vi.mocked(trustRequestApprove);
const deny = vi.mocked(trustRequestDeny);

const REQUEST: TrustRequest = {
  id: 7,
  project: 1,
  requested_by: 42,
  requested_by_label: "lead",
  review: {
    name: "build",
    variant_hash: "variant-v1",
    command: "npm run build",
    working_dir: null,
    env: {},
  },
  reason: "the release build needs it",
  expires_unix_millis: 0,
};

const RAISED: DomainEvent = { type: "TrustRequested", project: 1, request: REQUEST };

afterEach(() => vi.clearAllMocks());

function fire(event: DomainEvent) {
  const handler = subscribe.mock.calls[0]?.[0];
  if (!handler) throw new Error("no event subscriber registered");
  act(() => handler(event));
}

describe("useTrustRequests", () => {
  it("queues a request the core raised", () => {
    const { result } = renderHook(() => useTrustRequests(vi.fn(), vi.fn()));
    fire(RAISED);
    expect(result.current.requests).toEqual([REQUEST]);
  });

  it("queues a second ask for the same request only once", () => {
    const { result } = renderHook(() => useTrustRequests(vi.fn(), vi.fn()));
    fire(RAISED);
    fire(RAISED);
    expect(result.current.requests).toHaveLength(1);
  });

  it("drops a request whose requester closed, without the user deciding it", () => {
    const { result } = renderHook(() => useTrustRequests(vi.fn(), vi.fn()));
    fire(RAISED);

    fire({ type: "TrustRequestResolved", project: 1, id: 7, state: "withdrawn" });

    // Leaving it up would invite the user to authorize a command on behalf of a process that no
    // longer exists.
    expect(result.current.requests).toEqual([]);
    expect(approve).not.toHaveBeenCalled();
    expect(deny).not.toHaveBeenCalled();
  });

  it("approves the exact variant the prompt displayed", async () => {
    approve.mockResolvedValue(undefined);
    const refresh = vi.fn();
    const { result } = renderHook(() => useTrustRequests(refresh, vi.fn()));
    fire(RAISED);

    act(() => result.current.approve(REQUEST));

    // The hash travels back with the decision, so the core grants what was on screen rather than
    // whatever happens to be pending.
    expect(approve).toHaveBeenCalledWith(7, "variant-v1");
    await waitFor(() => expect(result.current.requests).toEqual([]));
    expect(refresh).toHaveBeenCalled();
  });

  it("keeps a refused approval on screen and reports why", async () => {
    approve.mockRejectedValue(new Error("this request no longer authorizes the reviewed command"));
    const reportError = vi.fn();
    const { result } = renderHook(() => useTrustRequests(vi.fn(), reportError));
    fire(RAISED);

    act(() => result.current.approve(REQUEST));

    await waitFor(() => expect(reportError).toHaveBeenCalled());
    expect(result.current.requests).toEqual([REQUEST]);
  });

  it("denies without re-reading the process list, since nothing became runnable", async () => {
    deny.mockResolvedValue(undefined);
    const refresh = vi.fn();
    const { result } = renderHook(() => useTrustRequests(refresh, vi.fn()));
    fire(RAISED);

    act(() => result.current.deny(REQUEST));

    await waitFor(() => expect(result.current.requests).toEqual([]));
    expect(deny).toHaveBeenCalledWith(7);
    expect(refresh).not.toHaveBeenCalled();
  });
});
