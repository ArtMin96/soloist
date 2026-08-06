// @vitest-environment jsdom
import { act, cleanup, renderHook, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

// The read and the event subscription are the IPC boundary; mocking them leaves the hook's own
// behaviour — what it asks for, and when it asks for more — as what the test exercises.
vi.mock("@/api", () => ({
  gitDiff: vi.fn(),
  onDomainEvent: vi.fn(() => Promise.resolve(() => {})),
  onResync: vi.fn(() => Promise.resolve(() => {})),
}));

import { gitDiff } from "@/api";
import { useGitDiff } from "@/store/git/useGitDiff";
import type { FileDiff } from "@/domain";

const read = vi.mocked(gitDiff);

const PROJECT = 7;
const PATH = "src/main.rs";
const OTHER = "src/lib.rs";

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

function diffOf(path: string, truncated: boolean): FileDiff {
  return {
    path,
    original_path: null,
    target: "unstaged",
    binary: false,
    patch: `diff --git a/${path} b/${path}\n@@ -1 +1 @@\n-a\n+b\n`,
    hunks: [{ old_start: 1, old_lines: 1, new_start: 1, new_lines: 1 }],
    truncated,
  };
}

/** The extent the most recent read asked for. */
function extentAsked(): string {
  const calls = read.mock.calls;
  return String(calls[calls.length - 1]?.[3]);
}

describe("useGitDiff", () => {
  it("reads a capped diff by default", async () => {
    read.mockResolvedValue(diffOf(PATH, true));

    const { result } = renderHook(() => useGitDiff(PROJECT, PATH, "unstaged"));

    await waitFor(() => expect(result.current.diff).not.toBeNull());
    expect(result.current.diff?.truncated).toBe(true);
    expect(extentAsked()).toBe("capped");
  });

  it("asks again for the whole diff when the reader wants the rest of it", async () => {
    read.mockResolvedValue(diffOf(PATH, true));
    const { result } = renderHook(() => useGitDiff(PROJECT, PATH, "unstaged"));
    await waitFor(() => expect(result.current.diff).not.toBeNull());

    read.mockResolvedValue(diffOf(PATH, false));
    act(() => result.current.loadFull());

    await waitFor(() => expect(result.current.diff?.truncated).toBe(false));
    expect(extentAsked()).toBe("full");
  });

  it("starts capped again at the next file, so one long diff never commits the next one", async () => {
    read.mockResolvedValue(diffOf(PATH, true));
    const { result, rerender } = renderHook(
      ({ path }: { path: string }) => useGitDiff(PROJECT, path, "unstaged"),
      { initialProps: { path: PATH } },
    );
    await waitFor(() => expect(result.current.diff).not.toBeNull());
    act(() => result.current.loadFull());
    await waitFor(() => expect(extentAsked()).toBe("full"));

    read.mockResolvedValue(diffOf(OTHER, true));
    rerender({ path: OTHER });

    await waitFor(() => expect(result.current.diff?.path).toBe(OTHER));
    expect(extentAsked()).toBe("capped");
  });

  it("never shows one file's diff while another file's read is in flight", async () => {
    read.mockResolvedValue(diffOf(PATH, false));
    const { result, rerender } = renderHook(
      ({ path }: { path: string }) => useGitDiff(PROJECT, path, "unstaged"),
      { initialProps: { path: PATH } },
    );
    await waitFor(() => expect(result.current.diff?.path).toBe(PATH));

    let answer: (diff: FileDiff) => void = () => {};
    read.mockReturnValue(
      new Promise<FileDiff>((resolve) => {
        answer = resolve;
      }),
    );
    rerender({ path: OTHER });

    expect(result.current.diff).toBeNull();
    expect(result.current.loading).toBe(true);
    act(() => answer(diffOf(OTHER, false)));
    await waitFor(() => expect(result.current.diff?.path).toBe(OTHER));
  });

  it("reads nothing at all while no file is open", () => {
    renderHook(() => useGitDiff(PROJECT, null, "unstaged"));

    expect(read).not.toHaveBeenCalled();
  });
});
