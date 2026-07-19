import { describe, expect, it } from "vitest";
import type { ScratchpadSummary } from "@/domain";
import { sortScratchpads } from "./scratchpadSort";

function pad(name: string, updated_at: number): ScratchpadSummary {
  return { id: 1, name, tags: [], archived: false, revision: 1, gist: "", updated_at };
}

const names = (list: ScratchpadSummary[]) => list.map((p) => p.name);

describe("sortScratchpads", () => {
  it("orders by most-recently-written first for 'updated'", () => {
    const list = [pad("old", 1_000), pad("newest", 9_000), pad("mid", 5_000)];
    expect(names(sortScratchpads(list, "updated"))).toEqual(["newest", "mid", "old"]);
  });

  it("breaks an updated tie by name so the order is stable", () => {
    // Two documents that predate the timestamp both sit at updated_at 0 — they fall back to name.
    const list = [pad("zebra", 0), pad("alpha", 0), pad("recent", 100)];
    expect(names(sortScratchpads(list, "updated"))).toEqual(["recent", "alpha", "zebra"]);
  });

  it("orders alphabetically, case-insensitively, for 'name'", () => {
    const list = [pad("Zebra", 9_000), pad("apple", 1_000), pad("Banana", 5_000)];
    expect(names(sortScratchpads(list, "name"))).toEqual(["apple", "Banana", "Zebra"]);
  });

  it("does not mutate the input array", () => {
    const list = [pad("b", 1_000), pad("a", 2_000)];
    const before = names(list);
    sortScratchpads(list, "name");
    expect(names(list)).toEqual(before);
  });
});
