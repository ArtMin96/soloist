import { describe, expect, it } from "vitest";
import { kindCollapseKey, projectCollapseKey } from "@/store/projects/view";
import { workCollapseKey } from "@/store/projects/work";

describe("workCollapseKey", () => {
  it("produces distinct keys per project and per section", () => {
    expect(workCollapseKey(1, "todos")).not.toBe(workCollapseKey(1, "scratchpads"));
    expect(workCollapseKey(1, "todos")).not.toBe(workCollapseKey(2, "todos"));
  });

  it("is stable across calls with the same arguments", () => {
    expect(workCollapseKey(1, "todos")).toBe(workCollapseKey(1, "todos"));
  });

  it("does not collide with projectCollapseKey or kindCollapseKey output", () => {
    const workKeys = new Set([workCollapseKey(1, "todos"), workCollapseKey(1, "scratchpads")]);
    expect(workKeys.has(projectCollapseKey(1))).toBe(false);
    expect(workKeys.has(kindCollapseKey(1, "Agent"))).toBe(false);
  });
});
