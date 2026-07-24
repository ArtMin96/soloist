import { describe, expect, it } from "vitest";
import type { DiagramSummary } from "@/domain";
import { sortDiagrams } from "./diagramSort";

function diagram(name: string, updated_at: number): DiagramSummary {
  return { id: 1, name, tags: [], archived: false, revision: 1, gist: "", updated_at };
}

const names = (list: DiagramSummary[]) => list.map((d) => d.name);

describe("sortDiagrams", () => {
  it("orders by most-recently-written first for 'updated'", () => {
    const list = [diagram("old", 1_000), diagram("newest", 9_000), diagram("mid", 5_000)];
    expect(names(sortDiagrams(list, "updated"))).toEqual(["newest", "mid", "old"]);
  });

  it("breaks an updated tie by name so the order is stable", () => {
    // Two documents that predate the timestamp both sit at updated_at 0 — they fall back to name.
    const list = [diagram("zebra", 0), diagram("alpha", 0), diagram("recent", 100)];
    expect(names(sortDiagrams(list, "updated"))).toEqual(["recent", "alpha", "zebra"]);
  });

  it("orders alphabetically, case-insensitively, for 'name'", () => {
    const list = [diagram("Zebra", 9_000), diagram("apple", 1_000), diagram("Banana", 5_000)];
    expect(names(sortDiagrams(list, "name"))).toEqual(["apple", "Banana", "Zebra"]);
  });

  it("does not mutate the input array", () => {
    const list = [diagram("b", 1_000), diagram("a", 2_000)];
    const before = names(list);
    sortDiagrams(list, "name");
    expect(names(list)).toEqual(before);
  });
});
