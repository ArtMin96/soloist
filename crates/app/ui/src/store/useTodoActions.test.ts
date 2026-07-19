// @vitest-environment jsdom
import { act, renderHook, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { todoCommentCreate, todoLink } from "@/api";
import { useTodoActions } from "@/store/useTodoActions";
import { expectCopyLinkWritesCoreLink } from "@/test/copyLinkContract";

// The real-window "Copy link" hop (writeText reaching the OS clipboard) is not verifiable under
// WebKitGTK/WebDriver, so it is covered here headlessly, via the shared copy-link contract: the
// board's copy-link writes exactly the `solo://` link the core built for the todo.
vi.mock("@/api", () => ({
  todoComplete: vi.fn(),
  todoLink: vi.fn(),
  todoCommentCreate: vi.fn(),
}));

describe("useTodoActions copy link", () => {
  afterEach(() => vi.clearAllMocks());

  it("writes the core's solo:// link for the todo to the clipboard", () =>
    expectCopyLinkWritesCoreLink({
      useStore: useTodoActions,
      linkFn: vi.mocked(todoLink),
      project: 7,
      target: 3,
      link: "solo://proj/7/todo/3",
    }));
});

describe("useTodoActions comment", () => {
  const comment = vi.mocked(todoCommentCreate);
  afterEach(() => vi.clearAllMocks());

  it("posts the comment through the core for the scoped project", async () => {
    comment.mockResolvedValue({} as never);
    const { result } = renderHook(() => useTodoActions(7));

    await act(async () => {
      await result.current.comment(3, "looks good");
    });
    expect(comment).toHaveBeenCalledWith(7, 3, "looks good");
    expect(result.current.errorById[3]).toBeUndefined();
  });

  it("surfaces a rejection in the todo's error slot and rethrows so the draft survives", async () => {
    comment.mockRejectedValue("no such todo");
    const { result } = renderHook(() => useTodoActions(7));

    await act(async () => {
      await expect(result.current.comment(3, "orphan note")).rejects.toBe("no such todo");
    });
    await waitFor(() => expect(result.current.errorById[3]).toBe("no such todo"));
  });
});
