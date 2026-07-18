// @vitest-environment jsdom
import { afterEach, describe, it, vi } from "vitest";
import { todoLink } from "@/api";
import { useTodoActions } from "@/store/useTodoActions";
import { expectCopyLinkWritesCoreLink } from "@/test/copyLinkContract";

// The real-window "Copy link" hop (writeText reaching the OS clipboard) is not verifiable under
// WebKitGTK/WebDriver, so it is covered here headlessly, via the shared copy-link contract: the
// board's copy-link writes exactly the `solo://` link the core built for the todo.
vi.mock("@/api", () => ({
  todoComplete: vi.fn(),
  todoLink: vi.fn(),
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
