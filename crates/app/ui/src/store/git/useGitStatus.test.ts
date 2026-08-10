// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { act, renderHook, waitFor } from "@testing-library/react";

// The status read, the event subscription, and the resync signal are the IPC boundary; mocking
// them leaves the hook's own behaviour — seed, re-read, coalesce, discard a stale project — as
// what the test exercises.
vi.mock("@/api", () => ({
  gitStatus: vi.fn(),
  onDomainEvent: vi.fn(() => Promise.resolve(() => {})),
  onResync: vi.fn(() => Promise.resolve(() => {})),
}));

import { gitStatus, onDomainEvent } from "@/api";
import type { DomainEvent, GitStatus } from "@/domain";
import { useGitStatus } from "@/store/git/useGitStatus";

const read = vi.mocked(gitStatus);
const subscribe = vi.mocked(onDomainEvent);

afterEach(() => vi.clearAllMocks());

function statusWith(paths: string[]): GitStatus {
  return {
    branch: { name: "main", upstream: null, sync: { state: "unknown" } },
    changes: paths.map((path) => ({
      path,
      status: { staged: null, unstaged: "modified" },
      original_path: null,
    })),
    merging: false,
    capabilities: {
      pull: false,
      push: true,
      stash: paths.length > 0,
      discardablePaths: paths,
    },
    changeCounts: { added: 0, removed: 0 },
  };
}

/** Delivers `event` to the hook's subscriber, as the backend would. */
function announce(event: DomainEvent): void {
  const handler = subscribe.mock.calls[0]?.[0];
  if (!handler) throw new Error("no domain-event subscriber registered");
  act(() => handler(event));
}

/** Runs whatever the hook scheduled for the next frame. */
async function nextFrame(): Promise<void> {
  await act(async () => {
    await new Promise((resolve) => requestAnimationFrame(() => resolve(null)));
  });
}

describe("useGitStatus", () => {
  it("seeds the rail from the project's working tree", async () => {
    read.mockResolvedValue(statusWith(["a.rs"]));

    const { result } = renderHook(() => useGitStatus(7));

    await waitFor(() => expect(result.current.status?.changes).toHaveLength(1));
    expect(result.current.loading).toBe(false);
  });

  it("shows a project without version control as having none, not as an error", async () => {
    read.mockResolvedValue(null);

    const { result } = renderHook(() => useGitStatus(7));

    await waitFor(() => expect(result.current.loading).toBe(false));
    expect(result.current.status).toBeNull();
    expect(result.current.error).toBeNull();
  });

  it("re-reads the working tree when version control says it changed", async () => {
    read.mockResolvedValue(statusWith(["a.rs"]));
    const { result } = renderHook(() => useGitStatus(7));
    await waitFor(() => expect(result.current.status?.changes).toHaveLength(1));

    read.mockResolvedValue(statusWith(["a.rs", "b.rs"]));
    announce({ type: "GitStatusChanged", project: 7 });

    await waitFor(() => expect(result.current.status?.changes).toHaveLength(2));
  });

  it("costs one re-read per frame however many changes are announced in it", async () => {
    read.mockResolvedValue(statusWith(["a.rs"]));
    const { result } = renderHook(() => useGitStatus(7));
    await waitFor(() => expect(result.current.status?.changes).toHaveLength(1));
    read.mockClear();

    for (let i = 0; i < 20; i++) announce({ type: "GitStatusChanged", project: 7 });
    await nextFrame();

    expect(read).toHaveBeenCalledTimes(1);
  });

  it("ignores announcements that are not about version control", async () => {
    read.mockResolvedValue(statusWith(["a.rs"]));
    const { result } = renderHook(() => useGitStatus(7));
    await waitFor(() => expect(result.current.status?.changes).toHaveLength(1));
    read.mockClear();

    announce({ type: "ProcessRemoved", id: 1 });
    await nextFrame();

    expect(read).not.toHaveBeenCalled();
  });

  it("never shows one project's repository while another project's read is in flight", async () => {
    read.mockResolvedValue(statusWith(["a.rs"]));
    const { result, rerender } = renderHook(({ project }) => useGitStatus(project), {
      initialProps: { project: 7 },
    });
    await waitFor(() => expect(result.current.status?.changes).toHaveLength(1));

    let resolveSecond: (value: GitStatus | null) => void = () => {};
    read.mockReturnValue(
      new Promise<GitStatus | null>((resolve) => {
        resolveSecond = resolve;
      }),
    );
    rerender({ project: 8 });

    expect(result.current.status).toBeNull();
    expect(result.current.loading).toBe(true);

    await act(async () => {
      resolveSecond(statusWith(["b.rs", "c.rs"]));
    });
    await waitFor(() => expect(result.current.status?.changes).toHaveLength(2));
  });
});
