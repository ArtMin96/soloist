// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { renderHook, waitFor } from "@testing-library/react";

// The picker and the load command are the IPC boundary; mock them so the test exercises
// the hook's own logic — picking, the cancel guard, and where failures go.
vi.mock("@/api", () => ({
  openProjectDirectory: vi.fn(),
  projectLoad: vi.fn(),
}));

import { openProjectDirectory, projectLoad } from "@/api";
import { useProjects } from "@/store/useProjects";

const pickDirectory = vi.mocked(openProjectDirectory);
const load = vi.mocked(projectLoad);

afterEach(() => vi.clearAllMocks());

describe("useProjects", () => {
  it("loads the chosen folder's stack", async () => {
    pickDirectory.mockResolvedValue("/home/dev/app");
    load.mockResolvedValue(1);
    const { result } = renderHook(() => useProjects(() => {}));

    result.current.open();

    await waitFor(() => expect(load).toHaveBeenCalledWith("/home/dev/app"));
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
