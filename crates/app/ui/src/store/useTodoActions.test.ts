// @vitest-environment jsdom
import { renderHook, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { todoLink } from "@/api";
import { useTodoActions } from "@/store/useTodoActions";

// The real-window "Copy link" hop (writeText reaching the OS clipboard) is not verifiable under
// WebKitGTK/WebDriver, so it is covered here headlessly: the board's copy-link writes exactly the
// `solo://` link the core built for the todo. The link's own construction is proven in the core
// (`coordination/link` + `facade/link` tests); this covers the UI wiring between them.
vi.mock("@/api", () => ({
  todoComplete: vi.fn(),
  todoLink: vi.fn(),
}));

describe("useTodoActions copy link", () => {
  afterEach(() => vi.clearAllMocks());

  it("writes the core's solo:// link for the todo to the clipboard", async () => {
    const link = "solo://proj/7/todo/3";
    vi.mocked(todoLink).mockResolvedValue(link);
    const writeText = vi.fn().mockResolvedValue(undefined);
    Object.assign(navigator, { clipboard: { writeText } });

    const { result } = renderHook(() => useTodoActions(7));
    result.current.copyLink(3);

    await waitFor(() => expect(writeText).toHaveBeenCalledWith(link));
    expect(todoLink).toHaveBeenCalledWith(7, 3);
  });
});
