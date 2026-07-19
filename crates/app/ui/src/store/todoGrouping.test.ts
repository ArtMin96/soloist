import { describe, expect, it } from "vitest";
import { groupTodosByScratchpad, UNLINKED_GROUP_LABEL } from "@/store/todoGrouping";
import type { ScratchpadRef, TodoView } from "@/domain";

const todo = (id: number, title: string, scratchpad: ScratchpadRef | null): TodoView => ({
  id,
  doc: { title, body: "", status: "open" },
  tags: [],
  blockers: [],
  blocked_by: [],
  blocked: false,
  comments: [],
  locked_by: null,
  scratchpad,
  revision: 1,
});

const plan: ScratchpadRef = { id: 4, name: "release-plan" };
const rollout: ScratchpadRef = { id: 9, name: "rollout-plan" };

describe("groupTodosByScratchpad", () => {
  it("buckets todos under the scratchpad they derive from, in first-appearance order", () => {
    const groups = groupTodosByScratchpad([
      todo(1, "ship", plan),
      todo(2, "announce", rollout),
      todo(3, "tag", plan),
    ]);

    expect(groups.map((group) => group.key)).toEqual(["4", "9"]);
    expect(groups[0].todos.map((t) => t.id)).toEqual([1, 3]);
    expect(groups[1].todos.map((t) => t.id)).toEqual([2]);
  });

  it("humanizes the scratchpad handle into the group's title", () => {
    const [group] = groupTodosByScratchpad([todo(1, "ship", plan)]);
    expect(group.label).toBe("Release plan");
  });

  it("puts the unlinked todos in a last group that is named, not flagged", () => {
    const groups = groupTodosByScratchpad([
      todo(1, "triage", null),
      todo(2, "ship", plan),
      todo(3, "sweep", null),
    ]);

    // Unlinked todos appeared first but their group sorts last — a first-class home at the bottom,
    // not a lead-with-the-problem bucket.
    expect(groups.map((group) => group.label)).toEqual(["Release plan", UNLINKED_GROUP_LABEL]);
    expect(groups[1].todos.map((t) => t.id)).toEqual([1, 3]);
  });

  it("groups every todo exactly once and emits no empty group", () => {
    const todos = [todo(1, "a", plan), todo(2, "b", null), todo(3, "c", rollout)];
    const groups = groupTodosByScratchpad(todos);

    expect(
      groups
        .flatMap((group) => group.todos)
        .map((t) => t.id)
        .sort(),
    ).toEqual([1, 2, 3]);
    expect(groups.every((group) => group.todos.length > 0)).toBe(true);
  });

  it("emits no groups at all for an empty board", () => {
    expect(groupTodosByScratchpad([])).toEqual([]);
  });

  it("keys a group by the scratchpad's durable id, so a rename does not reset its collapse", () => {
    const before = groupTodosByScratchpad([todo(1, "ship", plan)]);
    const after = groupTodosByScratchpad([todo(1, "ship", { id: plan.id, name: "cut-plan" })]);

    expect(after[0].key).toBe(before[0].key);
    expect(after[0].label).toBe("Cut plan");
  });
});
