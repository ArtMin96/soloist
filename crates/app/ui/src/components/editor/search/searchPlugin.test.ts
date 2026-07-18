// @vitest-environment jsdom
import { describe, expect, it } from "vitest";
import { findRanges, stepIndex } from "./searchPlugin";

describe("findRanges", () => {
  it("finds every match case-insensitively and reports their ranges", () => {
    expect(findRanges("The cat sat on the CAT mat", "cat")).toEqual([
      { from: 4, to: 7 },
      { from: 19, to: 22 },
    ]);
  });

  it("counts a single match", () => {
    expect(findRanges("one match here", "match")).toEqual([{ from: 4, to: 9 }]);
  });

  it("does not overlap: a match resumes past the previous one", () => {
    // "aa" inside "aaaa" is two matches (0-2, 2-4), never the overlapping 1-3.
    expect(findRanges("aaaa", "aa")).toEqual([
      { from: 0, to: 2 },
      { from: 2, to: 4 },
    ]);
  });

  it("returns nothing for an empty query", () => {
    expect(findRanges("anything at all", "")).toEqual([]);
  });

  it("returns nothing when the query is absent", () => {
    expect(findRanges("hello world", "zzz")).toEqual([]);
  });
});

describe("stepIndex", () => {
  it("advances to the next match", () => {
    expect(stepIndex(0, 3, 1)).toBe(1);
    expect(stepIndex(1, 3, 1)).toBe(2);
  });

  it("wraps past the last match back to the first", () => {
    expect(stepIndex(2, 3, 1)).toBe(0);
  });

  it("wraps before the first match round to the last", () => {
    expect(stepIndex(0, 3, -1)).toBe(2);
  });

  it("steps backward through the middle", () => {
    expect(stepIndex(2, 3, -1)).toBe(1);
  });

  it("stays put when there are no matches", () => {
    expect(stepIndex(0, 0, 1)).toBe(0);
    expect(stepIndex(0, 0, -1)).toBe(0);
  });
});
