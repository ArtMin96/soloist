import { describe, expect, it } from "vitest";
import {
  distinctTags,
  isSearchingOrTagging,
  matchesSearchAndTag,
  type SearchTagFilter,
} from "@/store/boardFilter";

// A minimal board row — only a tag list is required of it; the searched fields are the caller's.
interface Row {
  id: number;
  name: string;
  gist: string;
  tags: string[];
}

const NOTHING: SearchTagFilter = { search: "", tag: null };

const ROWS: Row[] = [
  { id: 1, name: "release-plan", gist: "cut the deb", tags: ["ui", "editor"] },
  { id: 2, name: "gate", gist: "the blocker CHAIN", tags: ["core", "ui"] },
  { id: 3, name: "chain of custody", gist: "", tags: ["docs"] },
];

const fields = (row: Row) => [row.name, row.gist];

function matching(filter: SearchTagFilter): number[] {
  return ROWS.filter((row) => matchesSearchAndTag(row, filter, fields)).map((row) => row.id);
}

describe("matchesSearchAndTag", () => {
  it("matches every row when nothing is searched or tagged", () => {
    expect(matching(NOTHING)).toEqual([1, 2, 3]);
  });

  it("matches a trimmed, case-insensitive needle in any searched field", () => {
    // "chain" reads as the second row's body and the third row's name, in either case.
    expect(matching({ ...NOTHING, search: "  ChAiN " })).toEqual([2, 3]);
  });

  it("narrows to the single tag", () => {
    expect(matching({ ...NOTHING, tag: "ui" })).toEqual([1, 2]);
  });

  it("requires the needle and the tag together", () => {
    expect(matching({ search: "chain", tag: "ui" })).toEqual([2]);
    expect(matching({ search: "release", tag: "docs" })).toEqual([]);
  });
});

describe("isSearchingOrTagging", () => {
  it("is false only for a blank needle and no tag", () => {
    expect(isSearchingOrTagging(NOTHING)).toBe(false);
    expect(isSearchingOrTagging({ ...NOTHING, search: "   " })).toBe(false);
    expect(isSearchingOrTagging({ ...NOTHING, search: "chain" })).toBe(true);
    expect(isSearchingOrTagging({ ...NOTHING, tag: "ui" })).toBe(true);
  });
});

describe("distinctTags", () => {
  it("returns the sorted distinct tags", () => {
    expect(distinctTags(ROWS)).toEqual(["core", "docs", "editor", "ui"]);
  });

  it("returns nothing for untagged rows", () => {
    expect(distinctTags([{ tags: [] }])).toEqual([]);
  });
});
