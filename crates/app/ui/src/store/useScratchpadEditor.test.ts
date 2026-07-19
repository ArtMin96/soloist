// @vitest-environment jsdom
import { act, renderHook, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { scratchpadLink, scratchpadRead, scratchpadRename } from "@/api";
import { useScratchpadEditor } from "@/store/useScratchpadEditor";
import { expectCopyLinkWritesCoreLink } from "@/test/copyLinkContract";
import type { ScratchpadView } from "@/domain";

// The real-window "Copy link" hop (writeText reaching the OS clipboard) is not verifiable under
// WebKitGTK/WebDriver, so it is covered here headlessly, via the shared copy-link contract: the
// editor's copy-link writes exactly the `solo://` link the core built for the scratchpad.
vi.mock("@/api", () => ({
  scratchpadRead: vi.fn(),
  scratchpadWrite: vi.fn(),
  scratchpadRename: vi.fn(),
  scratchpadLink: vi.fn(),
}));

const view = (name: string, revision = 3): ScratchpadView => ({
  id: 2,
  name,
  body: "the plan",
  rendered: `# ${name}\n\nthe plan`,
  tags: [],
  archived: false,
  revision,
});

/** Opens `name` in a fresh editor hook, with the read resolved. */
async function openedEditor(name: string) {
  vi.mocked(scratchpadRead).mockResolvedValue(view(name));
  const { result } = renderHook(() => useScratchpadEditor(7));
  act(() => result.current.open(name));
  await waitFor(() => expect(result.current.initialBody).toBe("the plan"));
  return result;
}

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

describe("useScratchpadEditor rename", () => {
  afterEach(() => vi.clearAllMocks());

  it("follows the open document to its new handle without re-reading it", async () => {
    const result = await openedEditor("release-plan");
    const mountKey = result.current.mountKey;
    vi.mocked(scratchpadRename).mockResolvedValue(view("Release plan"));

    await act(() => result.current.rename("Release plan"));

    expect(scratchpadRename).toHaveBeenCalledWith(7, "release-plan", "Release plan");
    expect(result.current.name).toBe("Release plan");
    // A rename is not an edit: the body is untouched, so the editor must not remount and throw
    // away an in-flight edit or its undo history.
    expect(result.current.mountKey).toBe(mountKey);
    expect(scratchpadRead).toHaveBeenCalledTimes(1);
  });

  it("rethrows a refusal and keeps the editor on the name it had", async () => {
    const result = await openedEditor("release-plan");
    vi.mocked(scratchpadRename).mockRejectedValue("a scratchpad named that already exists");

    await expect(result.current.rename("research")).rejects.toBe(
      "a scratchpad named that already exists",
    );
    expect(result.current.name).toBe("release-plan");
    // The refusal belongs to the rename field, not the editor's own error line.
    expect(result.current.error).toBeNull();
  });
});
