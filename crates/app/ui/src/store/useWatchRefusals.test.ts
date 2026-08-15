// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { act, renderHook } from "@testing-library/react";

// The event subscription is the IPC boundary; mock it so the test drives the hook's own logic —
// which projects it reports refused, and when it stops.
vi.mock("@/api", () => ({
  onDomainEvent: vi.fn(() => Promise.resolve(() => {})),
}));

import { onDomainEvent } from "@/api";
import type { DomainEvent } from "@/domain";
import { useWatchRefusals } from "@/store/useWatchRefusals";

const subscribe = vi.mocked(onDomainEvent);

const STOREFRONT = 1;
const API = 2;

const refused: DomainEvent = {
  type: "WatchRefusalChanged",
  project: STOREFRONT,
  refusals: { restarts: "budget_exhausted" },
};

afterEach(() => vi.clearAllMocks());

// Fires a captured `domain-event` into the hook's subscriber.
function fire(event: DomainEvent) {
  const handler = subscribe.mock.calls[0]?.[0];
  if (!handler) throw new Error("no event subscriber registered");
  act(() => handler(event));
}

describe("useWatchRefusals", () => {
  it("reports nothing refused until the core says otherwise", () => {
    const { result } = renderHook(() => useWatchRefusals());
    expect(result.current.size).toBe(0);
  });

  it("reports which of a project's watches were refused, and why", () => {
    const { result } = renderHook(() => useWatchRefusals());
    fire(refused);
    expect(result.current.get(STOREFRONT)).toEqual({ restarts: "budget_exhausted" });
  });

  it("keeps each project's refusals to itself", () => {
    const { result } = renderHook(() => useWatchRefusals());
    fire(refused);
    fire({
      type: "WatchRefusalChanged",
      project: API,
      refusals: { git_status: "unwatchable" },
    });
    expect(result.current.get(STOREFRONT)).toEqual({ restarts: "budget_exhausted" });
    expect(result.current.get(API)).toEqual({ git_status: "unwatchable" });
  });

  it("reports a refusal that has since changed its reason", () => {
    const { result } = renderHook(() => useWatchRefusals());
    fire(refused);
    fire({
      type: "WatchRefusalChanged",
      project: STOREFRONT,
      refusals: { restarts: "unavailable" },
    });
    expect(result.current.get(STOREFRONT)).toEqual({ restarts: "unavailable" });
  });

  // A repeat carries a fresh object, so a hook that took the new one anyway would hand every
  // project header a new map and re-render the sidebar for an announcement that said nothing.
  it("holds its map still when an announcement repeats what it already reports", () => {
    const { result } = renderHook(() => useWatchRefusals());
    fire(refused);
    const reported = result.current;
    fire({
      type: "WatchRefusalChanged",
      project: STOREFRONT,
      refusals: { restarts: "budget_exhausted" },
    });
    expect(result.current).toBe(reported);
  });

  it("stops reporting a project whose watches were established again", () => {
    const { result } = renderHook(() => useWatchRefusals());
    fire(refused);
    fire({ type: "WatchRefusalChanged", project: STOREFRONT, refusals: {} });
    expect(result.current.has(STOREFRONT)).toBe(false);
  });

  // A refusal outlives the project it names unless the removal drops it, and a notice for a project
  // that is no longer listed has nothing left to attach to.
  it("stops reporting a project that was removed", () => {
    const { result } = renderHook(() => useWatchRefusals());
    fire(refused);
    fire({ type: "ProjectRemoved", id: STOREFRONT });
    expect(result.current.has(STOREFRONT)).toBe(false);
  });
});
