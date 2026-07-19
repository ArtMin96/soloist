// @vitest-environment jsdom
import { act, renderHook, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { templateRead, templateUpdate } from "@/api";
import type { TemplateView } from "@/domain";
import { useTemplateEditor } from "@/store/useTemplateEditor";

vi.mock("@/api", () => ({
  templateRead: vi.fn(),
  templateUpdate: vi.fn(),
}));

const read = vi.mocked(templateRead);
const update = vi.mocked(templateUpdate);

function view(
  revision: number,
  body = "the body",
  description: string | null = "a note",
): TemplateView {
  return {
    id: 3,
    kind: "scratchpad",
    name: "daily",
    description,
    body,
    placeholders: [],
    scope: "global",
    revision,
  };
}

describe("useTemplateEditor", () => {
  afterEach(() => vi.clearAllMocks());

  it("opens a template and seeds the editor with its body, description, and revision", async () => {
    read.mockResolvedValue(view(5));
    const { result } = renderHook(() => useTemplateEditor());

    act(() => result.current.open("scratchpad", "daily"));
    await waitFor(() => expect(result.current.initialBody).toBe("the body"));
    expect(result.current.initialDescription).toBe("a note");
    expect(result.current.baseRevision).toBe(5);
    expect(read).toHaveBeenCalledWith("scratchpad", "daily");
  });

  it("saves guarded by the base revision and bumps it on success", async () => {
    read.mockResolvedValue(view(5));
    const { result } = renderHook(() => useTemplateEditor());
    act(() => result.current.open("scratchpad", "daily"));
    await waitFor(() => expect(result.current.baseRevision).toBe(5));

    update.mockResolvedValue(view(6, "changed"));
    await act(async () => {
      await result.current.save("a note", "changed");
    });
    expect(update).toHaveBeenCalledWith("scratchpad", "daily", "a note", "changed", 5);
    await waitFor(() => expect(result.current.baseRevision).toBe(6));
    expect(result.current.error).toBeNull();
  });

  it("clears a blank description to null on save", async () => {
    read.mockResolvedValue(view(5));
    const { result } = renderHook(() => useTemplateEditor());
    act(() => result.current.open("scratchpad", "daily"));
    await waitFor(() => expect(result.current.baseRevision).toBe(5));

    update.mockResolvedValue(view(6, "b", null));
    await act(async () => {
      await result.current.save("   ", "b");
    });
    expect(update).toHaveBeenCalledWith("scratchpad", "daily", null, "b", 5);
  });

  it("surfaces a moved revision as a conflict, keeping the local edits", async () => {
    read.mockResolvedValue(view(5));
    const { result } = renderHook(() => useTemplateEditor());
    act(() => result.current.open("scratchpad", "daily"));
    await waitFor(() => expect(result.current.baseRevision).toBe(5));

    // The write is refused; the re-read shows a newer revision — a real conflict, not a validation error.
    update.mockRejectedValue("template revision conflict");
    read.mockResolvedValue(view(8));
    await act(async () => {
      await result.current.save("a note", "mine");
    });
    await waitFor(() => expect(result.current.conflict).toEqual({ actual: 8 }));
    // Nothing was clobbered; the guard is untouched so a reload is the only way forward.
    expect(result.current.baseRevision).toBe(5);
    expect(result.current.error).toBeNull();
  });

  it("surfaces a same-revision rejection as a plain error", async () => {
    read.mockResolvedValue(view(5));
    const { result } = renderHook(() => useTemplateEditor());
    act(() => result.current.open("scratchpad", "daily"));
    await waitFor(() => expect(result.current.baseRevision).toBe(5));

    // The write is refused but the revision did not move — an invalid document, surfaced verbatim.
    update.mockRejectedValue("template is not well-formed: the body is empty");
    read.mockResolvedValue(view(5));
    await act(async () => {
      await result.current.save("a note", "");
    });
    await waitFor(() =>
      expect(result.current.error).toBe("template is not well-formed: the body is empty"),
    );
    expect(result.current.conflict).toBeNull();
  });
});
