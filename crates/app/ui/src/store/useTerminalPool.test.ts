import { describe, expect, it } from "vitest";
import { nextPool } from "@/store/useTerminalPool";

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
