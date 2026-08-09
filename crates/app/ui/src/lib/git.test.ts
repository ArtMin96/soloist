import { describe, expect, it } from "vitest";
import { branchStanding, CHANGE, primaryChange, strongerChange } from "@/lib/git";
import type { ChangeKind, SyncState } from "@/domain";

/** What the core reports when there is no comparison to make, whatever the reason. */
const UNCOMPARED: SyncState = { state: "unknown" };

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

describe("branchStanding", () => {
  it("tells a branch nobody has published from one that matches its upstream, in words", () => {
    const unpublished = branchStanding({ name: "spike", upstream: null, sync: UNCOMPARED });
    const matched = branchStanding({
      name: "main",
      upstream: "origin/main",
      sync: { state: "up_to_date" },
    });

    expect(unpublished.label).toBe("Local only");
    expect(matched.label).toBe("Up to date");
    expect(
      unpublished.label,
      "the two are a hue apart on screen, so the words are what tells them apart in grayscale",
    ).not.toBe(matched.label);
  });

  it("tells tracking nothing from tracking something nothing has fetched, since they want different things", () => {
    expect(branchStanding({ name: "spike", upstream: null, sync: UNCOMPARED }).label).toBe(
      "Local only",
    );
    expect(
      branchStanding({ name: "spike", upstream: "origin/spike", sync: UNCOMPARED }).label,
    ).toBe("Not fetched");
  });

  it("says nothing beside a detached head, which has no upstream to stand against", () => {
    expect(branchStanding({ name: null, upstream: null, sync: UNCOMPARED }).label).toBeNull();
  });

  it("states each side's distance when the branch and its upstream have both moved", () => {
    const diverged = branchStanding({
      name: "main",
      upstream: "origin/main",
      sync: { state: "diverged", ahead: 2, behind: 3 },
    });

    expect(diverged.label).toBe("2 ahead, 3 behind");
    expect(diverged.ahead).toBe(2);
    expect(diverged.behind).toBe(3);
    expect(
      branchStanding({ name: "main", upstream: "origin/main", sync: { state: "ahead", ahead: 1 } })
        .label,
    ).toBe("1 ahead");
    expect(
      branchStanding({
        name: "main",
        upstream: "origin/main",
        sync: { state: "behind", behind: 4 },
      }).label,
    ).toBe("4 behind");
  });

  it("spends the matched tone on the one standing that needs nothing doing", () => {
    const matched = branchStanding({
      name: "main",
      upstream: "origin/main",
      sync: { state: "up_to_date" },
    });
    const behind = branchStanding({
      name: "main",
      upstream: "origin/main",
      sync: { state: "behind", behind: 1 },
    });

    expect(matched.toneClass).not.toBe(behind.toneClass);
    expect(branchStanding({ name: "spike", upstream: null, sync: UNCOMPARED }).toneClass).toBe(
      behind.toneClass,
    );
  });
});
