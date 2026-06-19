// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { renderHook, waitFor } from "@testing-library/react";

// The api module is the IPC boundary; mock it so the test exercises the hook's own logic —
// picking, the cancel guard, the notice copy, and where failures go. The read-model seed
// (projectList) and event subscription (onDomainEvent) resolve to empty/no-op here.
vi.mock("@/api", () => ({
  openProjectDirectory: vi.fn(),
  projectLoad: vi.fn(),
  projectList: vi.fn(() => Promise.resolve([])),
  onDomainEvent: vi.fn(() => Promise.resolve(() => {})),
}));

import { openProjectDirectory, projectLoad } from "@/api";
import { mergeProject, useProjects } from "@/store/useProjects";

const pickDirectory = vi.mocked(openProjectDirectory);
const load = vi.mocked(projectLoad);

afterEach(() => vi.clearAllMocks());

describe("useProjects", () => {
  it("loads the chosen folder's stack without a notice", async () => {
    pickDirectory.mockResolvedValue("/home/dev/app");
    load.mockResolvedValue({ id: 1, processes: 2, created: false });
    const { result } = renderHook(() => useProjects(() => {}));

    result.current.open();

    await waitFor(() => expect(load).toHaveBeenCalledWith("/home/dev/app"));
    expect(result.current.notice).toBeNull();
  });

  it("announces an auto-created solo.yml with detected commands", async () => {
    pickDirectory.mockResolvedValue("/home/dev/app");
    load.mockResolvedValue({ id: 1, processes: 3, created: true });
    const { result } = renderHook(() => useProjects(() => {}));

    result.current.open();

    await waitFor(() => expect(result.current.notice).toMatch(/Created a solo\.yml/));
    expect(result.current.notice).toContain("app");
    expect(result.current.notice).toContain("3 commands");
  });

  it("announces a starter solo.yml when nothing was detected", async () => {
    pickDirectory.mockResolvedValue("/home/dev/blank");
    load.mockResolvedValue({ id: 1, processes: 0, created: true });
    const { result } = renderHook(() => useProjects(() => {}));

    result.current.open();

    await waitFor(() => expect(result.current.notice).toMatch(/starter solo\.yml/));
    expect(result.current.notice).toContain("blank");
  });

  it("notes an existing solo.yml that declares no commands", async () => {
    pickDirectory.mockResolvedValue("/home/dev/empty");
    load.mockResolvedValue({ id: 1, processes: 0, created: false });
    const { result } = renderHook(() => useProjects(() => {}));

    result.current.open();

    await waitFor(() => expect(result.current.notice).toMatch(/no commands yet/));
    expect(result.current.notice).toContain("empty");
  });

  it("does nothing when the picker is cancelled", async () => {
    pickDirectory.mockResolvedValue(null);
    const { result } = renderHook(() => useProjects(() => {}));

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
});

describe("mergeProject", () => {
  const opened = (id: number, name: string) =>
    ({ type: "ProjectOpened", id, name, root: `/p/${name}`, icon: null }) as const;

  it("prepends a newly opened project, newest first", () => {
    const next = mergeProject([{ id: 1, name: "a", root: "/p/a", icon: null }], opened(2, "b"));
    expect(next.map((project) => project.id)).toEqual([2, 1]);
  });

  it("replaces an already-open project in place (a re-open updates its identity)", () => {
    const next = mergeProject([{ id: 1, name: "old", root: "/p/a", icon: null }], opened(1, "new"));
    expect(next).toHaveLength(1);
    expect(next[0].name).toBe("new");
  });
});
