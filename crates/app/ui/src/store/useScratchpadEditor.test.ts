// @vitest-environment jsdom
import { afterEach, describe, it, vi } from "vitest";
import { scratchpadLink } from "@/api";
import { useScratchpadEditor } from "@/store/useScratchpadEditor";
import { expectCopyLinkWritesCoreLink } from "@/test/copyLinkContract";

// The real-window "Copy link" hop (writeText reaching the OS clipboard) is not verifiable under
// WebKitGTK/WebDriver, so it is covered here headlessly, via the shared copy-link contract: the
// editor's copy-link writes exactly the `solo://` link the core built for the scratchpad.
vi.mock("@/api", () => ({
  scratchpadRead: vi.fn(),
  scratchpadWrite: vi.fn(),
  scratchpadLink: vi.fn(),
}));

describe("useScratchpadEditor copy link", () => {
  afterEach(() => vi.clearAllMocks());

  it("writes the core's solo:// link for the scratchpad to the clipboard", () =>
    expectCopyLinkWritesCoreLink({
      useStore: useScratchpadEditor,
      linkFn: vi.mocked(scratchpadLink),
      project: 7,
      target: 2,
      link: "solo://proj/7/scratchpad/2",
    }));
});
