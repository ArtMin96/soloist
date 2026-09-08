import { describe, expect, it, vi } from "vitest";
import { documentItems } from "@/components/sidebar/documentItems";
import { TODO_STATUS_ICON, TODO_STATUS_TONE } from "@/lib/todo";
import type { DocumentParticipant, ProjectWork } from "@/domain";

function participant(process: number, role: DocumentParticipant["role"]): DocumentParticipant {
  return { process, label: `agent-${process}`, role };
}

const WORK: ProjectWork = {
  project: 1,
  todos: [
    {
      id: 7,
      title: "Ship the sidebar groups",
      status: "in_progress",
      participants: [participant(2, "reading"), participant(3, "implementing")],
    },
  ],
  scratchpads: [
    {
      id: 4,
      name: "release-readiness",
      participants: [participant(5, "editing")],
    },
  ],
};

const noop = () => {};

describe("documentItems", () => {
  it("returns nothing when no work has been read", () => {
    expect(documentItems(undefined, "todos", noop, noop)).toEqual([]);
    expect(documentItems(undefined, "scratchpads", noop, noop)).toEqual([]);
  });

  it("carries a todo's status icon, tone and id as its handle", () => {
    const [item] = documentItems(WORK, "todos", noop, noop);
    expect(item.label).toBe("Ship the sidebar groups");
    expect(item.icon).toBe(TODO_STATUS_ICON.in_progress);
    expect(item.tone).toBe(TODO_STATUS_TONE.in_progress);
    expect(item.handle).toBe(7);
  });

  it("takes the strongest role among a document's participants", () => {
    const [item] = documentItems(WORK, "todos", noop, noop);
    // implementing (process 3) outranks reading (process 2).
    expect(item.role).toBe("implementing");
  });

  it("activating a todo item calls onOpenTodo with its id", () => {
    const onOpenTodo = vi.fn();
    const [item] = documentItems(WORK, "todos", onOpenTodo, noop);
    item.activate();
    expect(onOpenTodo).toHaveBeenCalledWith(7);
  });

  it("carries a scratchpad's humanized name, a null tone, and its id as its handle", () => {
    const [item] = documentItems(WORK, "scratchpads", noop, noop);
    expect(item.label).toBe("Release readiness");
    expect(item.tone).toBeNull();
    expect(item.handle).toBe(4);
    expect(item.role).toBe("editing");
  });

  it("activating a scratchpad item calls onOpenScratchpad with its id", () => {
    const onOpenScratchpad = vi.fn();
    const [item] = documentItems(WORK, "scratchpads", noop, onOpenScratchpad);
    item.activate();
    expect(onOpenScratchpad).toHaveBeenCalledWith(4);
  });

  it("preserves the core's own order rather than re-sorting", () => {
    const twoTodos: ProjectWork = {
      project: 1,
      todos: [
        {
          id: 9,
          title: "second",
          status: "open",
          participants: [participant(1, "reading")],
        },
        {
          id: 7,
          title: "first",
          status: "open",
          participants: [participant(1, "reading")],
        },
      ],
      scratchpads: [],
    };
    const items = documentItems(twoTodos, "todos", noop, noop);
    expect(items.map((item) => item.handle)).toEqual([9, 7]);
  });
});
