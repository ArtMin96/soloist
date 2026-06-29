import { describe, expect, it } from "vitest";
import { commentAuthorLabel, TODO_STATUS } from "@/lib/todo";

describe("todo display helpers", () => {
  it("labels every todo status", () => {
    expect(TODO_STATUS.open).toBe("Open");
    expect(TODO_STATUS.in_progress).toBe("In progress");
    expect(TODO_STATUS.blocked).toBe("Blocked");
    expect(TODO_STATUS.done).toBe("Done");
  });

  it("names a comment author or marks it unattributed", () => {
    expect(commentAuthorLabel({ kind: "process", id: 4, label: "Web" })).toBe("Web");
    expect(commentAuthorLabel({ kind: "external", label: "raycast" })).toBe("raycast");
    expect(commentAuthorLabel(null)).toBe("unattributed");
  });
});
