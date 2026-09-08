// @vitest-environment jsdom
import { act, renderHook, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { exportMarkdown, scratchpadArchive, scratchpadWrite } from "@/api";
import { writeClipboard } from "@/lib/clipboard";
import { useScratchpadActions } from "@/store/useScratchpadActions";
import type { SaveOutcome } from "@/store/saveOutcome";
import type { ScratchpadView } from "@/domain";

// Every action here is an IPC hop or an OS one, so both boundaries are mocked: the assertions are
// about what the board hands the core and the clipboard, and what it does with a refusal.
vi.mock("@/api", () => ({
  scratchpadWrite: vi.fn(),
  scratchpadArchive: vi.fn(),
  exportMarkdown: vi.fn(),
}));

vi.mock("@/lib/clipboard", () => ({ writeClipboard: vi.fn() }));

const view = (name: string): ScratchpadView => ({
  id: 4,
  name,
  body: "the plan",
  rendered: `# ${name}\n\nthe plan`,
  tags: [],
  archived: false,
  revision: 1,
});

afterEach(() => vi.clearAllMocks());

describe("useScratchpadActions create", () => {
  it("writes the new scratchpad with no revision guard, so the core owns the name check", async () => {
    vi.mocked(scratchpadWrite).mockResolvedValue(view("release-plan"));
    const { result } = renderHook(() => useScratchpadActions(7));

    let outcome: SaveOutcome | undefined;
    await act(async () => {
      outcome = await result.current.create("release-plan", "the plan");
    });

    expect(outcome).toBe("saved");
    expect(scratchpadWrite).toHaveBeenCalledWith(7, "release-plan", "the plan", null);
    expect(result.current.createError).toBeNull();
  });

  it("resolves refused with the core's reason rather than throwing at the form", async () => {
    vi.mocked(scratchpadWrite).mockRejectedValue("a scratchpad named that already exists");
    const { result } = renderHook(() => useScratchpadActions(7));

    let outcome: SaveOutcome | undefined;
    await act(async () => {
      outcome = await result.current.create("release-plan", "the plan");
    });

    expect(outcome).toBe("refused");
    expect(result.current.createError).toBe("a scratchpad named that already exists");
  });
});

describe("useScratchpadActions archive", () => {
  it("archives through the core and surfaces a refusal until it is cleared", async () => {
    vi.mocked(scratchpadArchive).mockRejectedValue("no such scratchpad");
    const { result } = renderHook(() => useScratchpadActions(7));

    act(() => result.current.archive("release-plan", true));

    expect(scratchpadArchive).toHaveBeenCalledWith(7, "release-plan", true);
    await waitFor(() => expect(result.current.error).toBe("no such scratchpad"));

    act(() => result.current.clearError());
    expect(result.current.error).toBeNull();
  });
});

describe("useScratchpadActions export and copy", () => {
  it("exports the scratchpad as its name over its body", async () => {
    vi.mocked(exportMarkdown).mockResolvedValue(true);
    const { result } = renderHook(() => useScratchpadActions(7));

    act(() => result.current.exportMarkdown("release-plan", "the plan"));

    expect(exportMarkdown).toHaveBeenCalledWith("release-plan", "# release-plan\n\nthe plan");
    await waitFor(() => expect(result.current.error).toBeNull());
  });

  it("copies the same text the export writes", async () => {
    vi.mocked(writeClipboard).mockResolvedValue(undefined);
    const { result } = renderHook(() => useScratchpadActions(7));

    act(() => result.current.copyMarkdown("release-plan", "the plan"));

    expect(writeClipboard).toHaveBeenCalledWith("# release-plan\n\nthe plan");
  });

  it("surfaces a refused export", async () => {
    vi.mocked(exportMarkdown).mockRejectedValue("could not write the file");
    const { result } = renderHook(() => useScratchpadActions(7));

    act(() => result.current.exportMarkdown("release-plan", "the plan"));

    await waitFor(() => expect(result.current.error).toBe("could not write the file"));
  });
});
