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

// The project whose library the editor is opened over.
const OPEN_PROJECT = 7;

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

// Backs `templateRead`/`templateUpdate` with an in-memory template that honors the core's
// description contract: an omitted description keeps the stored one, a blank one clears it, and a
// write is refused unless the caller's revision guard still matches.
function backWithStore(initial: TemplateView) {
  let stored = initial;
  read.mockImplementation(async () => stored);
  update.mockImplementation(async (_kind, _project, _name, description, body, expectedRevision) => {
    if (expectedRevision !== stored.revision) throw "template revision conflict";
    stored = {
      ...stored,
      description: description === null ? stored.description : description.trim() || null,
      body,
      revision: stored.revision + 1,
    };
    return stored;
  });
}

describe("useTemplateEditor", () => {
  afterEach(() => vi.clearAllMocks());

  it("opens a template and seeds the editor with its body, description, and revision", async () => {
    read.mockResolvedValue(view(5));
    const { result } = renderHook(() => useTemplateEditor(OPEN_PROJECT));

    act(() => result.current.open("scratchpad", "global", "daily"));
    await waitFor(() => expect(result.current.initialBody).toBe("the body"));
    expect(result.current.initialDescription).toBe("a note");
    expect(result.current.baseRevision).toBe(5);
    expect(result.current.scope).toBe("global");
  });

  it("saves guarded by the base revision and bumps it on success", async () => {
    read.mockResolvedValue(view(5));
    const { result } = renderHook(() => useTemplateEditor(OPEN_PROJECT));
    act(() => result.current.open("scratchpad", "global", "daily"));
    await waitFor(() => expect(result.current.baseRevision).toBe(5));

    update.mockResolvedValue(view(6, "changed"));
    await act(async () => {
      await result.current.save("a note", "changed");
    });
    await waitFor(() => expect(result.current.baseRevision).toBe(6));
    expect(result.current.error).toBeNull();
  });

  it("clears a description, and it stays cleared when the template is reopened", async () => {
    backWithStore(view(5, "the body", "a note"));
    const { result } = renderHook(() => useTemplateEditor(OPEN_PROJECT));
    act(() => result.current.open("scratchpad", "global", "daily"));
    await waitFor(() => expect(result.current.initialDescription).toBe("a note"));

    await act(async () => {
      await result.current.save("   ", "changed");
    });

    // Reopening reads the template back from the store — the description is really gone, not just
    // blanked in the editor.
    act(() => result.current.open("scratchpad", "global", "daily"));
    await waitFor(() => expect(result.current.initialBody).toBe("changed"));
    expect(result.current.initialDescription).toBe("");
  });

  it("surfaces a moved revision as a conflict, keeping the local edits", async () => {
    read.mockResolvedValue(view(5));
    const { result } = renderHook(() => useTemplateEditor(OPEN_PROJECT));
    act(() => result.current.open("scratchpad", "global", "daily"));
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

  // A name can exist in both libraries, so the scope the row was opened from must ride along on
  // every read and write — otherwise a project template opens showing the global one's content.
  it("reads the library the template was opened from", async () => {
    read.mockImplementation(async (_kind, project) =>
      project == null ? view(1, "the global body") : view(1, "the project body"),
    );
    const { result } = renderHook(() => useTemplateEditor(OPEN_PROJECT));

    act(() => result.current.open("scratchpad", "project", "daily"));
    await waitFor(() => expect(result.current.initialBody).toBe("the project body"));
    expect(result.current.scope).toBe("project");

    act(() => result.current.open("scratchpad", "global", "daily"));
    await waitFor(() => expect(result.current.initialBody).toBe("the global body"));
  });

  it("surfaces a same-revision rejection as a plain error", async () => {
    read.mockResolvedValue(view(5));
    const { result } = renderHook(() => useTemplateEditor(OPEN_PROJECT));
    act(() => result.current.open("scratchpad", "global", "daily"));
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

  // Reloading is the only way out of a conflict, so it must actually adopt the winning revision —
  // clearing the banner while leaving the stale guard in place would send the next save straight
  // back into the same refusal.
  it("reloads onto the revision that won, clearing the conflict", async () => {
    read.mockResolvedValue(view(5));
    const { result } = renderHook(() => useTemplateEditor(OPEN_PROJECT));
    act(() => result.current.open("scratchpad", "global", "daily"));
    await waitFor(() => expect(result.current.baseRevision).toBe(5));

    update.mockRejectedValue("template revision conflict");
    read.mockResolvedValue(view(8, "the winning body", "their note"));
    await act(async () => {
      await result.current.save("a note", "mine");
    });
    await waitFor(() => expect(result.current.conflict).toEqual({ actual: 8 }));
    const beforeReload = result.current.mountKey;

    act(() => result.current.reload());

    await waitFor(() => expect(result.current.baseRevision).toBe(8));
    expect(result.current.initialBody).toBe("the winning body");
    expect(result.current.initialDescription).toBe("their note");
    expect(result.current.conflict).toBeNull();
    // The editor is uncontrolled, so only a remount re-seeds it with the body just loaded.
    expect(result.current.mountKey).toBeGreaterThan(beforeReload);
  });

  it("closing forgets the open template so the next open starts clean", async () => {
    read.mockResolvedValue(view(5));
    const { result } = renderHook(() => useTemplateEditor(OPEN_PROJECT));
    act(() => result.current.open("scratchpad", "global", "daily"));
    await waitFor(() => expect(result.current.baseRevision).toBe(5));

    act(() => result.current.close());

    expect(result.current.name).toBeNull();
    expect(result.current.kind).toBeNull();
    expect(result.current.scope).toBeNull();
    expect(result.current.initialBody).toBeNull();
    expect(result.current.initialDescription).toBe("");
    expect(result.current.baseRevision).toBeNull();

    // A stale guard left behind would let a save fire against a template the editor no longer shows.
    await act(async () => {
      await result.current.save("a note", "written after closing");
    });
    expect(update).not.toHaveBeenCalled();
  });
});
