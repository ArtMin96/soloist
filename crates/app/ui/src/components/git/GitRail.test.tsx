// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";

// The two reads and the event subscription are the IPC boundary; mocking them leaves the rail's
// own behaviour — which tab reads what, what an absent repository looks like, what survives a
// relaunch — as what the test exercises.
vi.mock("@/api", () => ({
  gitStatus: vi.fn(),
  gitFiles: vi.fn(),
  onDomainEvent: vi.fn(() => Promise.resolve(() => {})),
  onResync: vi.fn(() => Promise.resolve(() => {})),
}));

import { gitFiles, gitStatus } from "@/api";
import { GitRail } from "@/components/git/GitRail";
import { TooltipProvider } from "@/components/ui/tooltip";
import type { GitStatus, ProjectFile } from "@/domain";

const readStatus = vi.mocked(gitStatus);
const readFiles = vi.mocked(gitFiles);

const PROJECT = 7;

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
  localStorage.clear();
});

function statusWith(paths: string[]): GitStatus {
  return {
    branch: { name: "main", upstream: "origin/main", sync: { state: "ahead", ahead: 2 } },
    changes: paths.map((path) => ({
      path,
      status: { staged: null, unstaged: "modified" },
      original_path: null,
    })),
  };
}

function renderRail() {
  return render(
    <TooltipProvider>
      <GitRail project={PROJECT} />
    </TooltipProvider>,
  );
}

function rail(): HTMLElement {
  return screen.getByRole("complementary", { name: "Version control" });
}

describe("GitRail", () => {
  it("shows what is checked out and how far it stands from its upstream", async () => {
    readStatus.mockResolvedValue(statusWith(["src/a.rs"]));

    renderRail();

    await waitFor(() => expect(within(rail()).getByText("main")).toBeTruthy());
    expect(within(rail()).getByLabelText("2 ahead")).toBeTruthy();
  });

  it("counts the changed files in the shared Git view switcher", async () => {
    readStatus.mockResolvedValue(statusWith(["a.rs", "b.rs", "c.rs"]));

    renderRail();

    const changes = await screen.findByRole("radio", { name: /changes/i });
    expect(changes.textContent).toContain("3");
  });

  it("states that a project is not a repository, rather than reporting a failure", async () => {
    readStatus.mockResolvedValue(null);

    renderRail();

    await waitFor(() => expect(within(rail()).getByText("Not a git repository")).toBeTruthy());
    expect(screen.queryByRole("alert")).toBeNull();
    expect(screen.queryByLabelText("Git views")).toBeNull();
  });

  it("says nothing about a repository until the first read has answered", () => {
    readStatus.mockReturnValue(new Promise<GitStatus | null>(() => {}));

    renderRail();

    expect(screen.queryByText("Not a git repository")).toBeNull();
  });

  it("says a clean working tree is clean instead of showing an empty tree", async () => {
    readStatus.mockResolvedValue(statusWith([]));

    renderRail();

    await waitFor(() => expect(within(rail()).getByText("No changes")).toBeTruthy());
    expect(screen.queryByRole("tree")).toBeNull();
  });

  it("reads the project's files only once the tab that shows them is open", async () => {
    readStatus.mockResolvedValue(statusWith(["a.rs"]));
    const files: ProjectFile[] = [{ path: "src/main.rs", ignored: false }];
    readFiles.mockResolvedValue(files);

    renderRail();
    await screen.findByRole("radio", { name: /changes/i });
    expect(readFiles).not.toHaveBeenCalled();

    fireEvent.click(screen.getByRole("radio", { name: "Files" }));

    await waitFor(() => expect(screen.getByRole("tree", { name: "Project files" })).toBeTruthy());
    expect(readFiles).toHaveBeenCalledTimes(1);
  });

  it("expands and collapses the Files tree without hiding version control", async () => {
    readStatus.mockResolvedValue(statusWith([]));
    readFiles.mockResolvedValue([{ path: "src/components/GitRail.tsx", ignored: false }]);

    renderRail();
    await screen.findByRole("radio", { name: /changes/i });
    fireEvent.click(screen.getByRole("radio", { name: "Files" }));

    const tree = await screen.findByRole("tree", { name: "Project files" });
    expect(within(tree).queryByText("GitRail.tsx")).toBeNull();

    fireEvent.click(screen.getByRole("button", { name: "Expand all folders" }));
    expect(within(tree).getByText("GitRail.tsx")).toBeTruthy();
    expect(screen.getByRole("button", { name: "Collapse all folders" })).toBeTruthy();

    fireEvent.click(screen.getByRole("button", { name: "Collapse all folders" }));
    expect(within(tree).queryByText("GitRail.tsx")).toBeNull();
    expect(screen.getByLabelText("Git views")).toBeTruthy();
  });

  it("keeps each tab's folder expansion state while switching views", async () => {
    readStatus.mockResolvedValue(statusWith(["changed/readme.md"]));
    readFiles.mockResolvedValue([{ path: "src/components/GitRail.tsx", ignored: false }]);

    renderRail();
    await screen.findByRole("radio", { name: /changes/i });
    const changesTree = await screen.findByRole("tree", { name: "Changed files" });
    expect(within(changesTree).getByText("readme.md")).toBeTruthy();
    fireEvent.click(within(changesTree).getByRole("treeitem", { name: "changed" }));
    expect(within(changesTree).queryByText("readme.md")).toBeNull();

    fireEvent.click(screen.getByRole("radio", { name: "Files" }));

    const filesTree = await screen.findByRole("tree", { name: "Project files" });
    expect(within(filesTree).queryByText("GitRail.tsx")).toBeNull();

    fireEvent.click(within(filesTree).getByRole("treeitem", { name: "src" }));
    expect(within(filesTree).getByRole("treeitem", { name: "components" })).toBeTruthy();

    fireEvent.click(screen.getByRole("radio", { name: /changes/i }));
    await screen.findByRole("tree", { name: "Changed files" });
    expect(within(changesTree).queryByText("readme.md")).toBeNull();
    fireEvent.click(screen.getByRole("radio", { name: "Files" }));

    expect(
      await within(screen.getByRole("tree", { name: "Project files" })).findByRole("treeitem", {
        name: "components",
      }),
    ).toBeTruthy();
  });

  it("collapses and expands the Changes tree without changing tabs", async () => {
    readStatus.mockResolvedValue(statusWith(["src/components/GitRail.tsx"]));

    renderRail();
    const tree = await screen.findByRole("tree", { name: "Changed files" });
    expect(within(tree).getByText("GitRail.tsx")).toBeTruthy();
    expect(screen.getByRole("button", { name: "Collapse all folders" })).toBeTruthy();

    fireEvent.click(screen.getByRole("button", { name: "Collapse all folders" }));
    expect(within(tree).queryByText("GitRail.tsx")).toBeNull();
    expect(screen.getByRole("button", { name: "Expand all folders" })).toBeTruthy();

    fireEvent.click(screen.getByRole("button", { name: "Expand all folders" }));
    expect(within(tree).getByText("GitRail.tsx")).toBeTruthy();
    expect(screen.getByRole("radio", { name: /changes/i })).toBeTruthy();
  });

  it("keeps a resized rail at the width it was left, across a relaunch", async () => {
    readStatus.mockResolvedValue(statusWith(["a.rs"]));
    const { container } = renderRail();
    await waitFor(() => expect(within(rail()).getByText("main")).toBeTruthy());
    const width = () => (container.firstElementChild as HTMLElement).style.width;
    const before = width();

    const divider = screen.getByRole("separator", { name: /resize/i });
    fireEvent.keyDown(divider, { key: "ArrowLeft" });
    const widened = width();
    expect(widened).not.toBe(before);

    cleanup();
    const relaunched = renderRail();
    await waitFor(() => expect(within(rail()).getByText("main")).toBeTruthy());

    expect((relaunched.container.firstElementChild as HTMLElement).style.width).toBe(widened);
  });
});
