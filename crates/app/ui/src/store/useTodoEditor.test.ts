// @vitest-environment jsdom
import { act, renderHook, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { todoCreate, todoUpdate } from "@/api";
import type { TodoView } from "@/domain";
import { useTodoEditor } from "@/store/useTodoEditor";

vi.mock("@/api", () => ({
  todoCreate: vi.fn(),
  todoUpdate: vi.fn(),
}));

const create = vi.mocked(todoCreate);
const update = vi.mocked(todoUpdate);

function view(id: number, revision: number): TodoView {
  return {
    id,
    doc: { title: "Ship it", body: "the body", status: "in_progress" },
    tags: [],
    blockers: [],
    blocked_by: [],
    blocked: false,
    comments: [],
    locked_by: null,
    revision,
  };
}

describe("useTodoEditor", () => {
  afterEach(() => vi.clearAllMocks());

  it("opens a create draft and posts the whole document, then closes", async () => {
    create.mockResolvedValue(view(9, 1));
    const { result } = renderHook(() => useTodoEditor(7));

    act(() => result.current.startCreate());
    expect(result.current.mode).toBe("create");
    expect(result.current.initial).toEqual({ title: "", body: "", status: "open" });
    expect(result.current.baseRevision).toBeNull();

    await act(async () => {
      await result.current.save({ title: "New", body: "b", status: "open" });
    });
    expect(create).toHaveBeenCalledWith(7, { title: "New", body: "b", status: "open" });
    expect(result.current.mode).toBeNull();
  });

  it("edits guarded by the base revision and bumps it on success", async () => {
    const { result } = renderHook(() => useTodoEditor(7));

    act(() => result.current.editTodo(view(3, 5)));
    expect(result.current.mode).toBe("edit");
    expect(result.current.editingId).toBe(3);
    expect(result.current.baseRevision).toBe(5);

    update.mockResolvedValue(view(3, 6));
    await act(async () => {
      await result.current.save({ title: "Ship it", body: "changed", status: "done" });
    });
    // The write carried the opened revision as its guard; success advances it for the next save.
    expect(update).toHaveBeenCalledWith(
      7,
      3,
      { title: "Ship it", body: "changed", status: "done" },
      5,
    );
    await waitFor(() => expect(result.current.baseRevision).toBe(6));
    expect(result.current.error).toBeNull();
  });

  it("surfaces a rejected write and keeps the surface open", async () => {
    const { result } = renderHook(() => useTodoEditor(7));

    act(() => result.current.editTodo(view(3, 5)));
    update.mockRejectedValue("todo is blocked by #2");
    await act(async () => {
      await result.current.save({ title: "Ship it", body: "b", status: "done" });
    });
    await waitFor(() => expect(result.current.error).toBe("todo is blocked by #2"));
    // The base revision is untouched (nothing was written) and the surface stays open to keep edits.
    expect(result.current.baseRevision).toBe(5);
    expect(result.current.mode).toBe("edit");
  });

  it("reload adopts the concurrent writer's document and revision", () => {
    const { result } = renderHook(() => useTodoEditor(7));

    act(() => result.current.editTodo(view(3, 5)));
    const concurrent: TodoView = {
      ...view(3, 8),
      doc: { title: "Ship it", body: "theirs", status: "open" },
    };
    act(() => result.current.reload(concurrent));
    expect(result.current.baseRevision).toBe(8);
    expect(result.current.initial).toEqual({ title: "Ship it", body: "theirs", status: "open" });
    expect(result.current.error).toBeNull();
  });
});
