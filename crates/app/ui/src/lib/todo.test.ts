import { describe, expect, it } from "vitest";
import type { TodoStatus } from "@/domain";
import {
  commentAuthorLabel,
  TODO_STATUS,
  TODO_STATUS_ICON,
  TODO_STATUS_TONE,
  unmetBlockerLabel,
} from "@/lib/todo";

const STATUSES: TodoStatus[] = ["open", "blocked", "in_progress", "done"];

describe("todo display helpers", () => {
  it("labels every todo status with a distinct, non-empty label", () => {
    // `Record<TodoStatus, string>` already forces a label per status at compile time, so the runtime
    // invariants worth guarding are the ones the type cannot catch: every label is non-empty and no
    // two statuses collide on one label (which would make them indistinguishable in the UI).
    const labels = Object.values(TODO_STATUS);
    expect(labels.length).toBeGreaterThan(0);
    expect(labels.every((label) => label.trim().length > 0)).toBe(true);
    expect(new Set(labels).size).toBe(labels.length);
  });

  it("names a comment author or marks it unattributed", () => {
    expect(commentAuthorLabel({ kind: "process", id: 4, label: "Web" })).toBe("Web");
    expect(commentAuthorLabel({ kind: "external", label: "raycast" })).toBe("raycast");
    expect(commentAuthorLabel(null)).toBe("unattributed");
  });

  it("gives every todo status an icon", () => {
    for (const status of STATUSES) {
      expect(TODO_STATUS_ICON[status]).toBeDefined();
    }
  });

  it("gives every todo status a tone no other status shares", () => {
    // A repeated tone would make two statuses look alike at a glance, which is the whole point of
    // colouring them; the `Record` type only guarantees an entry exists, not that it is distinct.
    const tones = STATUSES.map((status) => TODO_STATUS_TONE[status]);
    expect(tones.every((tone) => tone.trim().length > 0)).toBe(true);
    expect(new Set(tones).size).toBe(tones.length);
  });

  it("reads one unmet blocker singular and any other count plural", () => {
    expect(unmetBlockerLabel(1)).toBe("1 unmet blocker");
    expect(unmetBlockerLabel(3)).toBe("3 unmet blockers");
    expect(unmetBlockerLabel(0)).toBe("0 unmet blockers");
  });
});
