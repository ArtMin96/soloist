import { describe, expect, it } from "vitest";
import { canMoveBy, moveBy, moveItem } from "@/lib/sortable";

describe("moveItem", () => {
  it("takes the item out of its place and puts it in the new one", () => {
    expect(moveItem(["a", "b", "c", "d"], 0, 2)).toEqual(["b", "c", "a", "d"]);
    expect(moveItem(["a", "b", "c", "d"], 3, 1)).toEqual(["a", "d", "b", "c"]);
  });

  it("leaves the list alone when the move goes nowhere", () => {
    expect(moveItem(["a", "b", "c"], 1, 1)).toEqual(["a", "b", "c"]);
  });

  it("never mutates the list it was given", () => {
    const original = ["a", "b", "c"];
    moveItem(original, 0, 2);
    expect(original).toEqual(["a", "b", "c"]);
  });

  it("changes nothing when an index is off the end", () => {
    // A move computed against a longer list than the one that arrived — no order is better than
    // a wrong one.
    expect(moveItem(["a", "b"], 0, 5)).toEqual(["a", "b"]);
    expect(moveItem(["a", "b"], -1, 0)).toEqual(["a", "b"]);
  });
});

describe("moveBy", () => {
  it("moves an item the given number of places", () => {
    expect(moveBy(["a", "b", "c"], "a", 1)).toEqual(["b", "a", "c"]);
    expect(moveBy(["a", "b", "c"], "c", -2)).toEqual(["c", "a", "b"]);
  });

  it("stops at the end rather than falling off it", () => {
    expect(moveBy(["a", "b", "c"], "a", -1)).toEqual(["a", "b", "c"]);
    expect(moveBy(["a", "b", "c"], "c", 3)).toEqual(["a", "b", "c"]);
  });

  it("changes nothing for an item that is not in the list", () => {
    expect(moveBy(["a", "b"], "z", 1)).toEqual(["a", "b"]);
  });
});

describe("canMoveBy", () => {
  it("reports the moves an item at each end can still make", () => {
    expect(canMoveBy(["a", "b", "c"], "a", -1)).toBe(false);
    expect(canMoveBy(["a", "b", "c"], "a", 1)).toBe(true);
    expect(canMoveBy(["a", "b", "c"], "c", 1)).toBe(false);
    expect(canMoveBy(["a", "b", "c"], "c", -1)).toBe(true);
  });

  it("reports no move for a list of one, or an item that is not in the list", () => {
    expect(canMoveBy(["only"], "only", -1)).toBe(false);
    expect(canMoveBy(["only"], "only", 1)).toBe(false);
    expect(canMoveBy(["a", "b"], "z", 1)).toBe(false);
  });
});
