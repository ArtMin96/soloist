import { describe, expect, it } from "vitest";
import {
  EMPTY_SCRATCHPAD_FILTER,
  filterScratchpads,
  isFilteringScratchpads,
  type ScratchpadFilter,
} from "@/store/scratchpadFilter";
import type { ScratchpadSummary } from "@/domain";

function pad(overrides: Partial<ScratchpadSummary> = {}): ScratchpadSummary {
  return {
    id: 1,
    name: "rich-editor-design",
    tags: [],
    archived: false,
    revision: 1,
    gist: "",
    updated_at: 0,
    ...overrides,
  };
}

const design = pad({ id: 1, name: "rich-editor-design", gist: "three revisions of the editor" });
const notes = pad({ id: 2, name: "release-notes", tags: ["release"], gist: "what shipped" });
const retired = pad({ id: 3, name: "old-plan", archived: true, gist: "superseded" });

const pads = [design, notes, retired];

function names(filter: Partial<ScratchpadFilter>): string[] {
  return filterScratchpads(pads, { ...EMPTY_SCRATCHPAD_FILTER, ...filter }).map((it) => it.name);
}

describe("filterScratchpads", () => {
  it("matches the handle a search names", () => {
    expect(names({ search: "release-notes" })).toEqual(["release-notes"]);
  });

  it("matches the title a reader sees, not only the handle it is written as", () => {
    // The board shows humanized titles, so the words on screen are what a search has to find.
    expect(names({ search: "rich editor" })).toEqual(["rich-editor-design"]);
  });

  it("matches the gist, so a search reaches text no title carries", () => {
    expect(names({ search: "what shipped" })).toEqual(["release-notes"]);
  });

  it("narrows to one side of the listing flag on the archived facet", () => {
    expect(names({ archived: "archived" })).toEqual(["old-plan"]);
    expect(names({ archived: "active" })).toEqual(["rich-editor-design", "release-notes"]);
  });

  it("ANDs a tag with the search rather than widening it", () => {
    expect(names({ tag: "release" })).toEqual(["release-notes"]);
    expect(names({ tag: "release", search: "rich" })).toEqual([]);
  });
});

describe("isFilteringScratchpads", () => {
  it("is false only for the unfiltered board", () => {
    expect(isFilteringScratchpads(EMPTY_SCRATCHPAD_FILTER)).toBe(false);
    expect(isFilteringScratchpads({ ...EMPTY_SCRATCHPAD_FILTER, search: "plan" })).toBe(true);
    expect(isFilteringScratchpads({ ...EMPTY_SCRATCHPAD_FILTER, archived: "active" })).toBe(true);
    expect(isFilteringScratchpads({ ...EMPTY_SCRATCHPAD_FILTER, tag: "release" })).toBe(true);
  });
});
