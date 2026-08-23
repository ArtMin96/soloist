// @vitest-environment jsdom
import { act, renderHook, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { scratchpadLink, scratchpadRead, scratchpadRename, scratchpadWrite } from "@/api";
import { useScratchpadEditor } from "@/store/useScratchpadEditor";
import type { SaveOutcome } from "@/store/saveOutcome";
import { expectCopyLinkWritesCoreLink } from "@/test/copyLinkContract";
import {
  expectCloseDiscardsInFlightRead,
  expectSupersededReadIsDiscarded,
} from "@/test/loadRaceContract";
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

const view = (name: string, revision = 3, body = "the plan"): ScratchpadView => ({
  id: 2,
  name,
  body,
  rendered: `# ${name}\n\n${body}`,
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

describe("useScratchpadEditor open", () => {
  afterEach(() => vi.clearAllMocks());

  it("discards a superseded read that resolves after the current scratchpad", async () => {
    const { result } = await expectSupersededReadIsDiscarded({
      useStore: () => useScratchpadEditor(7),
      readFn: vi.mocked(scratchpadRead),
      open: (store, target) => store.open(target.name),
      snapshotOf: (store) => ({
        identity: store.name,
        content: store.initialBody,
        revision: store.baseRevision,
      }),
      snapshotIn: (target) => ({
        identity: target.name,
        content: target.body,
        revision: target.revision,
      }),
      first: view("release-plan", 3, "the plan"),
      second: view("research", 8, "the research notes"),
    });

    vi.mocked(scratchpadWrite).mockResolvedValueOnce(view("research", 9));
    await act(() => result.current.save("the research notes, edited"));
    expect(scratchpadWrite).toHaveBeenCalledWith(7, "research", "the research notes, edited", 8);
  });

  it("leaves the editor closed when an in-flight read resolves after close", () =>
    expectCloseDiscardsInFlightRead({
      useStore: () => useScratchpadEditor(7),
      readFn: vi.mocked(scratchpadRead),
      open: (store, target) => store.open(target.name),
      close: (store) => store.close(),
      snapshotOf: (store) => ({
        identity: store.name,
        content: store.initialBody,
        revision: store.baseRevision,
      }),
      target: view("release-plan", 3),
      mountKeyOf: (store) => store.mountKey,
    }));
});

describe("useScratchpadEditor save", () => {
  afterEach(() => vi.clearAllMocks());

  it("resolves to saved on a successful write", async () => {
    const result = await openedEditor("release-plan");
    vi.mocked(scratchpadWrite).mockResolvedValueOnce(view("release-plan", 4));

    await expect(result.current.save("edited")).resolves.toBe("saved");
    expect(result.current.conflict).toBeNull();
    expect(result.current.error).toBeNull();
  });

  it("resolves to refused when a refused write reveals a moved-on revision", async () => {
    const result = await openedEditor("release-plan");
    vi.mocked(scratchpadWrite).mockRejectedValueOnce("stale revision");
    vi.mocked(scratchpadRead).mockResolvedValueOnce(view("release-plan", 9));

    let outcome: SaveOutcome | undefined;
    await act(async () => {
      outcome = await result.current.save("edited");
    });

    expect(outcome).toBe("refused");
    expect(result.current.conflict).toEqual({ actual: 9 });
  });

  it("resolves to refused when a refused write is not a revision move", async () => {
    const result = await openedEditor("release-plan");
    vi.mocked(scratchpadWrite).mockRejectedValueOnce("invalid document");
    vi.mocked(scratchpadRead).mockResolvedValueOnce(view("release-plan", 3));

    let outcome: SaveOutcome | undefined;
    await act(async () => {
      outcome = await result.current.save("edited");
    });

    expect(outcome).toBe("refused");
    expect(result.current.error).toBe("invalid document");
  });
});

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
