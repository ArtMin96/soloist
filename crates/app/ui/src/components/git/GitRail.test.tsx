// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";

// The two reads and the event subscription are the IPC boundary; mocking them leaves the rail's
// own behaviour — which tab reads what, what an absent repository looks like, what survives a
// relaunch — as what the test exercises.
vi.mock("@/api", () => ({
  gitStatus: vi.fn(),
  gitFiles: vi.fn(),
  gitTrusted: vi.fn(() => Promise.resolve(false)),
  gitTrustProject: vi.fn(() => Promise.resolve()),
  gitStage: vi.fn(() => Promise.resolve()),
  gitUnstage: vi.fn(() => Promise.resolve()),
  gitDiscard: vi.fn(() => Promise.resolve()),
  gitStageHunk: vi.fn(() => Promise.resolve()),
  gitUnstageHunk: vi.fn(() => Promise.resolve()),
  gitDiscardHunk: vi.fn(() => Promise.resolve()),
  gitCommit: vi.fn(() => Promise.resolve()),
  gitDraftCommitMessage: vi.fn(() => Promise.resolve("")),
  gitBranches: vi.fn(() => Promise.resolve(null)),
  gitCreateBranch: vi.fn(() => Promise.resolve()),
  gitSwitchBranch: vi.fn(() => Promise.resolve()),
  gitDeleteBranch: vi.fn(() => Promise.resolve()),
  gitStash: vi.fn(() => Promise.resolve()),
  gitPopStash: vi.fn(() => Promise.resolve()),
  gitPush: vi.fn(() => Promise.resolve()),
  gitPull: vi.fn(() => Promise.resolve()),
  gitFetch: vi.fn(() => Promise.resolve()),
  gitStopExchange: vi.fn(() => Promise.resolve()),
  gitAbortMerge: vi.fn(() => Promise.resolve()),
  assistSettings: vi.fn(() => Promise.resolve({ tool: null })),
  onDomainEvent: vi.fn(() => Promise.resolve(() => {})),
  onResync: vi.fn(() => Promise.resolve(() => {})),
}));

import {
  assistSettings,
  gitAbortMerge,
  gitBranches,
  gitCommit,
  gitDeleteBranch,
  gitFetch,
  gitPush,
  gitStopExchange,
  gitSwitchBranch,
  gitDiscard,
  gitDraftCommitMessage,
  gitFiles,
  gitStage,
  gitStatus,
  gitTrusted,
  gitTrustProject,
} from "@/api";
import { GitRail } from "@/components/git/GitRail";
import { TooltipProvider } from "@/components/ui/tooltip";
import type { GitStatus, ProjectFile } from "@/domain";

const readStatus = vi.mocked(gitStatus);
const readFiles = vi.mocked(gitFiles);
const readTrust = vi.mocked(gitTrusted);
const trustProject = vi.mocked(gitTrustProject);
const stage = vi.mocked(gitStage);
const discard = vi.mocked(gitDiscard);
const commit = vi.mocked(gitCommit);
const draftMessage = vi.mocked(gitDraftCommitMessage);
const readAssist = vi.mocked(assistSettings);
const readBranches = vi.mocked(gitBranches);
const switchBranch = vi.mocked(gitSwitchBranch);
const deleteBranch = vi.mocked(gitDeleteBranch);
const fetch = vi.mocked(gitFetch);
const push = vi.mocked(gitPush);
const stopExchange = vi.mocked(gitStopExchange);
const abortMerge = vi.mocked(gitAbortMerge);

const PROJECT = 7;

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
  localStorage.clear();
});

/** The rail starts on a project nobody has trusted yet, which is where every project starts, and
 * with no tool picked to draft with, which is where every install starts. */
beforeEach(() => {
  readTrust.mockResolvedValue(false);
  readAssist.mockResolvedValue({ tool: null });
});

/** A trusted project with `path` staged, so a commit and a draft both have something to work from. */
function trustedWithStaged(path: string): void {
  readTrust.mockResolvedValue(true);
  const staged = statusWith([path]);
  staged.changes[0].status = { staged: "modified", unstaged: null };
  readStatus.mockResolvedValue(staged);
}

function statusWith(paths: string[]): GitStatus {
  return {
    branch: { name: "main", upstream: "origin/main", sync: { state: "ahead", ahead: 2 } },
    changes: paths.map((path) => ({
      path,
      status: { staged: null, unstaged: "modified" },
      original_path: null,
    })),
    merging: false,
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

  it("offers to trust a project before it offers to change one", async () => {
    readStatus.mockResolvedValue(statusWith(["src/a.rs"]));

    renderRail();
    await waitFor(() => expect(within(rail()).getByText("main")).toBeTruthy());

    expect(
      within(rail()).queryByLabelText(/^Stage /),
      "an action nobody may take is absent, not disabled",
    ).toBeNull();
    expect(within(rail()).queryByLabelText("Commit message")).toBeNull();

    fireEvent.click(within(rail()).getByRole("button", { name: /trust this project/i }));

    await waitFor(() => expect(trustProject).toHaveBeenCalledWith(PROJECT));
  });

  it("stages a path when its box is ticked, and takes it back when it is cleared", async () => {
    readTrust.mockResolvedValue(true);
    readStatus.mockResolvedValue(statusWith(["src/a.rs"]));

    renderRail();

    const box = await within(rail()).findByLabelText("Stage src/a.rs");
    fireEvent.click(box);

    await waitFor(() => expect(stage).toHaveBeenCalledWith(PROJECT, "src/a.rs"));
  });

  it("asks before throwing a change away, and throws nothing away until it is answered", async () => {
    readTrust.mockResolvedValue(true);
    readStatus.mockResolvedValue(statusWith(["src/a.rs"]));

    renderRail();
    const button = await within(rail()).findByLabelText("Discard the changes to src/a.rs");
    fireEvent.click(button);

    const question = await screen.findByRole("alertdialog");
    expect(discard, "nothing goes on the strength of one click").not.toHaveBeenCalled();

    fireEvent.click(within(question).getByRole("button", { name: /^discard$/i }));
    await waitFor(() => expect(discard).toHaveBeenCalledWith(PROJECT, "src/a.rs"));
  });

  it("will not commit until there is a message and something staged to record", async () => {
    readTrust.mockResolvedValue(true);
    readStatus.mockResolvedValue(statusWith(["src/a.rs"]));

    renderRail();
    const message = await within(rail()).findByLabelText("Commit message");
    const button = within(rail()).getByRole("button", { name: /^commit$/i });

    fireEvent.change(message, { target: { value: "Record it" } });
    expect(
      (button as HTMLButtonElement).disabled,
      "the only change is unstaged, so there is nothing for a commit to record",
    ).toBe(true);
  });

  it("commits the message it was given once something is staged", async () => {
    readTrust.mockResolvedValue(true);
    const staged = statusWith(["src/a.rs"]);
    staged.changes[0].status = { staged: "modified", unstaged: null };
    readStatus.mockResolvedValue(staged);

    renderRail();
    const message = await within(rail()).findByLabelText("Commit message");
    fireEvent.change(message, { target: { value: "Record it" } });
    fireEvent.click(within(rail()).getByRole("button", { name: /^commit$/i }));

    await waitFor(() => expect(commit).toHaveBeenCalledWith(PROJECT, "Record it", false));
    await waitFor(() => expect((message as HTMLTextAreaElement).value).toBe(""));
  });

  it("offers no way to draft a message until a tool is picked to draft with", async () => {
    // The opt-in, at the surface: an action nobody may take is absent rather than disabled — and
    // nothing is asked of an agent that was never configured.
    trustedWithStaged("src/a.rs");

    renderRail();

    await within(rail()).findByLabelText("Commit message");
    expect(within(rail()).queryByRole("button", { name: "Draft a message" })).toBeNull();
    expect(draftMessage).not.toHaveBeenCalled();
  });

  it("puts a drafted message in the box to edit, and commits nothing by itself", async () => {
    readAssist.mockResolvedValue({ tool: "Claude" });
    trustedWithStaged("src/a.rs");
    draftMessage.mockResolvedValue("Record the index");

    renderRail();
    const message = (await within(rail()).findByLabelText("Commit message")) as HTMLTextAreaElement;
    fireEvent.click(await within(rail()).findByRole("button", { name: "Draft a message" }));

    await waitFor(() => expect(message.value).toBe("Record the index"));
    expect(
      commit,
      "a draft is a draft; committing it stays the user's action",
    ).not.toHaveBeenCalled();

    fireEvent.change(message, { target: { value: "Record the index, corrected" } });
    expect(message.value, "what came back is editable like anything typed").toBe(
      "Record the index, corrected",
    );
  });

  it("says why a draft was refused and leaves the message as it was", async () => {
    readAssist.mockResolvedValue({ tool: "Claude" });
    trustedWithStaged("src/a.rs");
    draftMessage.mockRejectedValue("the agent tool did not answer within its time limit");

    renderRail();
    const message = (await within(rail()).findByLabelText("Commit message")) as HTMLTextAreaElement;
    fireEvent.change(message, { target: { value: "Half a message" } });
    fireEvent.click(await within(rail()).findByRole("button", { name: "Draft a message" }));

    const refusal = await within(rail()).findByRole("alert");
    expect(refusal.textContent).toContain("did not answer within its time limit");
    expect(message.value, "a refused draft overwrites nothing").toBe("Half a message");
  });

  it("will not ask for a draft of a change that is not staged", async () => {
    // There is nothing for a message to describe until something is staged, which the core would
    // refuse — so the button says so instead of spending an agent to be told.
    readAssist.mockResolvedValue({ tool: "Claude" });
    readTrust.mockResolvedValue(true);
    readStatus.mockResolvedValue(statusWith(["src/a.rs"]));

    renderRail();

    const button = (await within(rail()).findByRole("button", {
      name: "Draft a message",
    })) as HTMLButtonElement;
    expect(button.disabled).toBe(true);
  });
});

describe("moving between branches and exchanging commits with the remote", () => {
  it("offers neither a branch switcher nor a sync action until the project is trusted", async () => {
    readStatus.mockResolvedValue(statusWith(["src/a.rs"]));

    renderRail();

    await within(rail()).findByText("main");
    expect(
      within(rail()).queryByRole("button", { name: "Switch branch" }),
      "an action nobody may take is absent, not disabled",
    ).toBeNull();
    expect(within(rail()).queryByRole("button", { name: "Fetch" })).toBeNull();
    expect(
      readBranches,
      "and nothing is read for a switcher that is not offered",
    ).not.toHaveBeenCalled();
  });

  it("reads the branches only once the switcher is opened, and switches to the one chosen", async () => {
    readTrust.mockResolvedValue(true);
    readStatus.mockResolvedValue(statusWith(["src/a.rs"]));
    readBranches.mockResolvedValue({
      entries: [
        { name: "main", upstream: "origin/main", head: true },
        { name: "feature", upstream: null, head: false },
      ],
      stashed: false,
    });

    renderRail();
    const switcher = await within(rail()).findByRole("button", { name: "Switch branch" });
    expect(
      readBranches,
      "a list nobody is looking at is a subprocess nobody needed",
    ).not.toHaveBeenCalled();
    fireEvent.click(switcher);

    fireEvent.click(await screen.findByText("feature"));

    await waitFor(() => expect(switchBranch).toHaveBeenCalledWith(PROJECT, "feature"));
    expect(readBranches).toHaveBeenCalledWith(PROJECT);
  });

  it("confirms before a branch is deleted, and deletes nothing until it is confirmed", async () => {
    readTrust.mockResolvedValue(true);
    readStatus.mockResolvedValue(statusWith(["src/a.rs"]));
    readBranches.mockResolvedValue({
      entries: [
        { name: "main", upstream: null, head: true },
        { name: "spike", upstream: null, head: false },
      ],
      stashed: false,
    });

    renderRail();
    fireEvent.click(await within(rail()).findByRole("button", { name: "Switch branch" }));
    fireEvent.click(await screen.findByRole("button", { name: "Delete branch spike" }));

    expect(deleteBranch, "nothing goes before the question is answered").not.toHaveBeenCalled();
    fireEvent.click(await screen.findByRole("button", { name: "Delete" }));
    await waitFor(() => expect(deleteBranch).toHaveBeenCalledWith(PROJECT, "spike"));
  });

  it("offers to publish a branch that tracks nothing rather than to pull from it", async () => {
    readTrust.mockResolvedValue(true);
    const untracked = statusWith(["src/a.rs"]);
    untracked.branch = { name: "spike", upstream: null, sync: { state: "unknown" } };
    readStatus.mockResolvedValue(untracked);

    renderRail();

    await within(rail()).findByRole("button", { name: "Publish" });
    expect(
      within(rail()).queryByRole("button", { name: "Pull" }),
      "there is nothing to pull from an upstream that does not exist",
    ).toBeNull();
  });

  it("offers to stop an exchange while it is under way, and reports nothing when it is stopped", async () => {
    readTrust.mockResolvedValue(true);
    readStatus.mockResolvedValue(statusWith(["src/a.rs"]));
    let refuse: (reason: Error) => void = () => {};
    push.mockImplementation(() => new Promise((_, reject) => (refuse = reject)));

    renderRail();
    fireEvent.click(await within(rail()).findByRole("button", { name: "Push" }));

    const stop = await within(rail()).findByRole("button", { name: "Stop" });
    fireEvent.click(stop);
    expect(stopExchange).toHaveBeenCalledWith(PROJECT);
    // The core reports a stopped exchange as refused, because from its side it did not finish.
    refuse(new Error("the git command was stopped"));

    await within(rail()).findByRole("button", { name: "Push" });
    expect(
      within(rail()).queryByRole("alert"),
      "an exchange the reader stopped is what they asked for, not a failure to report back at them",
    ).toBeNull();
  });

  it("states the reason an exchange really failed", async () => {
    readTrust.mockResolvedValue(true);
    readStatus.mockResolvedValue(statusWith(["src/a.rs"]));
    fetch.mockRejectedValue(new Error("no credential the remote would accept"));

    renderRail();
    fireEvent.click(await within(rail()).findByRole("button", { name: "Fetch" }));

    const refusal = await within(rail()).findByRole("alert");
    expect(refusal.textContent).toContain("no credential the remote would accept");
  });

  it("says how many files a merge left to resolve, and abandons it only after asking", async () => {
    readTrust.mockResolvedValue(true);
    const conflicted = statusWith(["src/a.rs"]);
    conflicted.changes[0].status = { staged: null, unstaged: "conflicted" };
    conflicted.merging = true;
    readStatus.mockResolvedValue(conflicted);

    renderRail();

    await within(rail()).findByText("1 file needs resolving");
    fireEvent.click(await within(rail()).findByRole("button", { name: "Abandon merge" }));
    expect(
      abortMerge,
      "abandoning throws away resolved conflicts, so it asks first",
    ).not.toHaveBeenCalled();
    fireEvent.click(await screen.findByRole("button", { name: "Abandon" }));
    await waitFor(() => expect(abortMerge).toHaveBeenCalledWith(PROJECT));
  });

  it("says nothing about conflicts when a merge is under way with none left", async () => {
    readTrust.mockResolvedValue(true);
    const merging = statusWith(["src/a.rs"]);
    merging.merging = true;
    readStatus.mockResolvedValue(merging);

    renderRail();

    await within(rail()).findByText("Merge in progress");
    expect(within(rail()).queryByText(/needs resolving/)).toBeNull();
  });
});
