// @vitest-environment jsdom
import { renderHook, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { scratchpadLink } from "@/api";
import { useScratchpadEditor } from "@/store/useScratchpadEditor";

// The real-window "Copy link" hop (writeText reaching the OS clipboard) is not verifiable under
// WebKitGTK/WebDriver, so it is covered here headlessly: the editor's copy-link writes exactly the
// `solo://` link the core built for the scratchpad. The link's own construction is proven in the
// core (`coordination/link` + `facade/link` tests); this covers the UI wiring between them.
vi.mock("@/api", () => ({
  scratchpadRead: vi.fn(),
  scratchpadWrite: vi.fn(),
  scratchpadLink: vi.fn(),
}));

describe("useScratchpadEditor copy link", () => {
  afterEach(() => vi.clearAllMocks());

  it("writes the core's solo:// link for the scratchpad to the clipboard", async () => {
    const link = "solo://proj/7/scratchpad/2";
    vi.mocked(scratchpadLink).mockResolvedValue(link);
    const writeText = vi.fn().mockResolvedValue(undefined);
    Object.assign(navigator, { clipboard: { writeText } });

    const { result } = renderHook(() => useScratchpadEditor(7));
    result.current.copyLink(2);

    await waitFor(() => expect(writeText).toHaveBeenCalledWith(link));
    expect(scratchpadLink).toHaveBeenCalledWith(7, 2);
  });
});
