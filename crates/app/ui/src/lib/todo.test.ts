import { describe, expect, it } from "vitest";
import { commentAuthorLabel, TODO_STATUS } from "@/lib/todo";

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
});
