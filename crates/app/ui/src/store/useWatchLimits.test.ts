// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { act, renderHook } from "@testing-library/react";

// The event subscription is the IPC boundary; mock it so the test drives the hook's own logic —
// which projects it reports limited, and when it stops.
vi.mock("@/api", () => ({
  onDomainEvent: vi.fn(() => Promise.resolve(() => {})),
}));

import { onDomainEvent } from "@/api";
import type { DomainEvent } from "@/domain";
import { useWatchLimits } from "@/store/useWatchLimits";

const subscribe = vi.mocked(onDomainEvent);

const STOREFRONT = 1;
const API = 2;

const refused: DomainEvent = {
  type: "WatchLimitChanged",
  project: STOREFRONT,
  limits: { restarts: { refused: "budget_exhausted" } },
};

afterEach(() => vi.clearAllMocks());

// Fires a captured `domain-event` into the hook's subscriber.
function fire(event: DomainEvent) {
  const handler = subscribe.mock.calls[0]?.[0];
  if (!handler) throw new Error("no event subscriber registered");
  act(() => handler(event));
}

describe("useWatchLimits", () => {
  it("reports nothing limited until the core says otherwise", () => {
    const { result } = renderHook(() => useWatchLimits());
    expect(result.current.size).toBe(0);
  });

  it("reports which of a project's watches were limited, and why", () => {
    const { result } = renderHook(() => useWatchLimits());
    fire(refused);
    expect(result.current.get(STOREFRONT)).toEqual({ restarts: { refused: "budget_exhausted" } });
  });

  it("keeps each project's limits to itself", () => {
    const { result } = renderHook(() => useWatchLimits());
    fire(refused);
    fire({
      type: "WatchLimitChanged",
      project: API,
      limits: { git_status: { refused: "unwatchable" } },
    });
    expect(result.current.get(STOREFRONT)).toEqual({ restarts: { refused: "budget_exhausted" } });
    expect(result.current.get(API)).toEqual({ git_status: { refused: "unwatchable" } });
  });

  it("reports a refusal that has since changed its reason", () => {
    const { result } = renderHook(() => useWatchLimits());
    fire(refused);
    fire({
      type: "WatchLimitChanged",
      project: STOREFRONT,
      limits: { restarts: { refused: "unavailable" } },
    });
    expect(result.current.get(STOREFRONT)).toEqual({ restarts: { refused: "unavailable" } });
  });

  // The core allocates a fresh `{refused: ...}` object every time it repeats an announcement, so a
  // hook that compared limits by identity would hand every project header a new map and re-render
  // the sidebar for an announcement that said nothing new.
  it("holds its map still when a repeated refusal carries the same reason", () => {
    const { result } = renderHook(() => useWatchLimits());
    fire(refused);
    const reported = result.current;
    fire({
      type: "WatchLimitChanged",
      project: STOREFRONT,
      limits: { restarts: { refused: "budget_exhausted" } },
    });
    expect(result.current).toBe(reported);
  });

  it("holds its map still when a repeated degradation is announced again", () => {
    const { result } = renderHook(() => useWatchLimits());
    fire({ type: "WatchLimitChanged", project: STOREFRONT, limits: { git_status: "degraded" } });
    const reported = result.current;
    fire({ type: "WatchLimitChanged", project: STOREFRONT, limits: { git_status: "degraded" } });
    expect(result.current).toBe(reported);
  });

  // A refusal and a degradation are different conditions for the same purpose, so the row must
  // update even though both count as "something is limited".
  it("reports a refusal easing into a degradation", () => {
    const { result } = renderHook(() => useWatchLimits());
    fire(refused);
    const reported = result.current;
    fire({
      type: "WatchLimitChanged",
      project: STOREFRONT,
      limits: { restarts: "degraded" },
    });
    expect(result.current).not.toBe(reported);
    expect(result.current.get(STOREFRONT)).toEqual({ restarts: "degraded" });
  });

  it("stops reporting a project whose watches were established again in full", () => {
    const { result } = renderHook(() => useWatchLimits());
    fire(refused);
    fire({ type: "WatchLimitChanged", project: STOREFRONT, limits: {} });
    expect(result.current.has(STOREFRONT)).toBe(false);
  });

  // A limit outlives the project it names unless the removal drops it, and a notice for a project
  // that is no longer listed has nothing left to attach to.
  it("stops reporting a project that was removed", () => {
    const { result } = renderHook(() => useWatchLimits());
    fire(refused);
    fire({ type: "ProjectRemoved", id: STOREFRONT });
    expect(result.current.has(STOREFRONT)).toBe(false);
  });
});
