// @vitest-environment jsdom
import { renderHook, waitFor } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { nextPool, useTerminalPool } from "@/store/useTerminalPool";

describe("nextPool", () => {
  it("promotes a newly-selected process to the front", () => {
    expect(nextPool([2, 1], 3, [1, 2, 3], 6)).toEqual([3, 2, 1]);
  });

  it("moves an already-pooled selection back to the front", () => {
    expect(nextPool([1, 2, 3], 3, [1, 2, 3], 6)).toEqual([3, 1, 2]);
  });

  it("drops processes that no longer exist", () => {
    expect(nextPool([1, 2, 3], 2, [2, 3], 6)).toEqual([2, 3]);
  });

  it("evicts the least-recently-selected once past the cap", () => {
    expect(nextPool([3, 2, 1], 4, [1, 2, 3, 4], 3)).toEqual([4, 3, 2]);
  });

  it("ignores a selection that does not exist yet", () => {
    expect(nextPool([1], 99, [1], 6)).toEqual([1]);
  });

  it("keeps the pool (filtered) when nothing is selected", () => {
    expect(nextPool([1, 2], null, [1, 2, 3], 6)).toEqual([1, 2]);
  });

  it("returns the same reference when nothing changed", () => {
    const prev = [1, 2];
    expect(nextPool(prev, 1, [1, 2], 6)).toBe(prev);
  });
});

describe("useTerminalPool", () => {
  it("promotes a newly-selected process, and does not re-run for an equal-membership array", async () => {
    const { result, rerender } = renderHook(
      ({ selectedId, existingIds }: { selectedId: number | null; existingIds: number[] }) =>
        useTerminalPool(selectedId, existingIds),
      { initialProps: { selectedId: 1, existingIds: [1, 2] } },
    );
    await waitFor(() => expect(result.current).toEqual([1]));

    rerender({ selectedId: 2, existingIds: [1, 2] });
    await waitFor(() => expect(result.current).toEqual([2, 1]));

    // A fresh array with the same members must not disturb the pool.
    const beforeRerender = result.current;
    rerender({ selectedId: 2, existingIds: [1, 2] });
    expect(result.current).toBe(beforeRerender);
  });

  it("drops a pooled process once it leaves the registry", async () => {
    const { result, rerender } = renderHook(
      ({ selectedId, existingIds }: { selectedId: number | null; existingIds: number[] }) =>
        useTerminalPool(selectedId, existingIds),
      { initialProps: { selectedId: 1, existingIds: [1, 2] } },
    );
    await waitFor(() => expect(result.current).toEqual([1]));

    // Process 1 closed; the selection moves to the survivor.
    rerender({ selectedId: 2, existingIds: [2] });
    await waitFor(() => expect(result.current).toEqual([2]));
  });
});
