import { describe, expect, it } from "vitest";
import { CHANGE, primaryChange, strongerChange, syncLabel } from "@/lib/git";
import type { ChangeKind } from "@/domain";

const EVERY_KIND: ChangeKind[] = [
  "modified",
  "type_changed",
  "added",
  "deleted",
  "renamed",
  "copied",
  "untracked",
  "conflicted",
];

describe("the change display map", () => {
  it("names every change in words as well as a letter, so a row never reads by colour alone", () => {
    for (const kind of EVERY_KIND) {
      expect(CHANGE[kind].letter).not.toBe("");
      expect(CHANGE[kind].label).not.toBe("");
    }
  });

  it("draws only a path that no longer exists as gone", () => {
    const gone = EVERY_KIND.filter((kind) => CHANGE[kind].gone);

    expect(gone).toEqual(["deleted"]);
  });

  it("tells a conflict apart from a deletion, the two states a red row could mean", () => {
    expect(CHANGE.conflicted.toneClass).not.toBe(CHANGE.deleted.toneClass);
  });

  it("tells the two changes that share the letter C apart by tone", () => {
    expect(CHANGE.copied.letter).toBe(CHANGE.conflicted.letter);
    expect(CHANGE.copied.toneClass).not.toBe(CHANGE.conflicted.toneClass);
  });
});

describe("primaryChange", () => {
  it("shows the working tree's change, which is the one the user can still act on", () => {
    expect(primaryChange({ staged: "added", unstaged: "modified" })).toBe("modified");
  });

  it("falls back to what is staged when the working tree matches the index", () => {
    expect(primaryChange({ staged: "added", unstaged: null })).toBe("added");
  });

  it("reports nothing for a path that differs on neither side", () => {
    expect(primaryChange({ staged: null, unstaged: null })).toBeNull();
  });
});

describe("strongerChange", () => {
  it("ranks an unresolved conflict above everything else", () => {
    for (const kind of EVERY_KIND) {
      expect(strongerChange(kind, "conflicted")).toBe("conflicted");
      expect(strongerChange("conflicted", kind)).toBe("conflicted");
    }
  });

  it("ranks a change to something tracked above a path version control has never seen", () => {
    expect(strongerChange("untracked", "modified")).toBe("modified");
    expect(strongerChange("modified", "untracked")).toBe("modified");
  });

  it("keeps the first of two equally strong changes, so folder order stays stable", () => {
    expect(strongerChange("added", "renamed")).toBe("added");
  });
});

describe("syncLabel", () => {
  it("says nothing when there is no comparison to make", () => {
    expect(syncLabel({ state: "unknown" })).toBeNull();
  });

  it("states each side's distance when the branch and its upstream have both moved", () => {
    expect(syncLabel({ state: "diverged", ahead: 2, behind: 3 })).toBe("2 ahead, 3 behind");
    expect(syncLabel({ state: "ahead", ahead: 1 })).toBe("1 ahead");
    expect(syncLabel({ state: "behind", behind: 4 })).toBe("4 behind");
    expect(syncLabel({ state: "up_to_date" })).toBe("Up to date");
  });
});
