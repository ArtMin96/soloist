// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { act, renderHook, waitFor } from "@testing-library/react";

// The api module is the IPC boundary; mock it so the test exercises the hook's own logic —
// picking, the cancel guard, the notice copy, the refetch-on-open, and where failures go.
vi.mock("@/api", () => ({
  openProjectDirectory: vi.fn(),
  projectLoad: vi.fn(),
  projectList: vi.fn(() => Promise.resolve([])),
  onDomainEvent: vi.fn(() => Promise.resolve(() => {})),
}));

// The persisted cache is the disk boundary; mock it so the hook revalidates against `@/api`
// from a cold cache (a miss) without touching tauri-plugin-store under test.
vi.mock("@/store/cache/persistentCache", () => ({
  CacheKey: { projects: "projects", appInfo: "app-info", agents: "agents" },
  readSnapshot: vi.fn(() => Promise.resolve(null)),
  writeSnapshot: vi.fn(() => Promise.resolve()),
}));

import { onDomainEvent, openProjectDirectory, projectList, projectLoad } from "@/api";
import { useProjects } from "@/store/projects/useProjects";
import type { DomainEvent } from "@/domain";

const pickDirectory = vi.mocked(openProjectDirectory);
const load = vi.mocked(projectLoad);
const list = vi.mocked(projectList);
const subscribe = vi.mocked(onDomainEvent);

// A stable error sink, like the one the app passes (`store.reportError` is a useCallback):
// a fresh callback each render would churn the subscribe effect.
const noop = () => {};

afterEach(() => vi.clearAllMocks());

describe("useProjects", () => {
  it("loads the chosen folder's stack without a notice", async () => {
    pickDirectory.mockResolvedValue("/home/dev/app");
    load.mockResolvedValue({ id: 1, processes: 2, created: false });
    const { result } = renderHook(() => useProjects(noop));

    result.current.open();

    await waitFor(() => expect(load).toHaveBeenCalledWith("/home/dev/app"));
    expect(result.current.notice).toBeNull();
  });

  it("announces an auto-created solo.yml with detected commands", async () => {
    pickDirectory.mockResolvedValue("/home/dev/app");
    load.mockResolvedValue({ id: 1, processes: 3, created: true });
    const { result } = renderHook(() => useProjects(noop));

    result.current.open();

    await waitFor(() => expect(result.current.notice).toMatch(/Created a solo\.yml/));
    expect(result.current.notice).toContain("app");
    expect(result.current.notice).toContain("3 commands");
  });

  it("announces a starter solo.yml when nothing was detected", async () => {
    pickDirectory.mockResolvedValue("/home/dev/blank");
    load.mockResolvedValue({ id: 1, processes: 0, created: true });
    const { result } = renderHook(() => useProjects(noop));

    result.current.open();

    await waitFor(() => expect(result.current.notice).toMatch(/starter solo\.yml/));
    expect(result.current.notice).toContain("blank");
  });

  it("notes an existing solo.yml that declares no commands", async () => {
    pickDirectory.mockResolvedValue("/home/dev/empty");
    load.mockResolvedValue({ id: 1, processes: 0, created: false });
    const { result } = renderHook(() => useProjects(noop));

    result.current.open();

    await waitFor(() => expect(result.current.notice).toMatch(/no commands yet/));
    expect(result.current.notice).toContain("empty");
  });

  it("does nothing when the picker is cancelled", async () => {
    pickDirectory.mockResolvedValue(null);
    const { result } = renderHook(() => useProjects(noop));

    result.current.open();

    await waitFor(() => expect(pickDirectory).toHaveBeenCalled());
    expect(load).not.toHaveBeenCalled();
  });

  it("reports a load failure through the error sink", async () => {
    pickDirectory.mockResolvedValue("/home/dev/app");
    load.mockRejectedValue("solo.yml not found");
    const reportError = vi.fn();
    const { result } = renderHook(() => useProjects(reportError));

    result.current.open();

    await waitFor(() => expect(reportError).toHaveBeenCalledWith("solo.yml not found"));
  });

  it("re-reads the rendered project snapshot when a project opens", async () => {
    renderHook(() => useProjects(noop));
    // The hook seeds from the snapshot once on mount.
    await waitFor(() => expect(list).toHaveBeenCalledTimes(1));

    // A ProjectOpened event triggers a re-read (the snapshot carries the rendered icons), so
    // the icon never needs a separate request.
    const handler = subscribe.mock.calls[0][0];
    act(() => handler({ type: "ProjectOpened", id: 1 } as DomainEvent));
    await waitFor(() => expect(list).toHaveBeenCalledTimes(2));
  });
});
