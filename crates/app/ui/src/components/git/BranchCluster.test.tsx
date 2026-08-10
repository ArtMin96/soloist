// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";

// The reads and the event subscription are the IPC boundary; mocking them leaves the real behaviour
// — what the chrome shows about the checked-out branch, what it lets a reader do to it, and how many
// times the repository is asked about itself — as what the test exercises. The event stream is
// captured rather than stubbed away, so a test can announce a change the way the core does.
const stream = vi.hoisted(() => ({
  listeners: [] as ((event: { type: string; project: number }) => void)[],
}));

vi.mock("@/api", () => ({
  gitStatus: vi.fn(),
  gitFiles: vi.fn(() => Promise.resolve(null)),
  gitTrusted: vi.fn(() => Promise.resolve(false)),
  gitTrustProject: vi.fn(() => Promise.resolve()),
  gitStage: vi.fn(() => Promise.resolve()),
  gitUnstage: vi.fn(() => Promise.resolve()),
  gitDiscard: vi.fn(() => Promise.resolve()),
  gitStageHunk: vi.fn(() => Promise.resolve()),
  gitUnstageHunk: vi.fn(() => Promise.resolve()),
  gitDiscardHunk: vi.fn(() => Promise.resolve()),
  gitCommit: vi.fn(() => Promise.resolve()),
  gitCommitTemplate: vi.fn(() => Promise.resolve(null)),
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
  onDomainEvent: vi.fn((handler: (event: { type: string; project: number }) => void) => {
    stream.listeners.push(handler);
    return Promise.resolve(() => {
      stream.listeners = stream.listeners.filter((candidate) => candidate !== handler);
    });
  }),
  onResync: vi.fn(() => Promise.resolve(() => {})),
}));

import {
  gitBranches,
  gitDeleteBranch,
  gitFetch,
  gitPush,
  gitStatus,
  gitStopExchange,
  gitSwitchBranch,
  gitTrusted,
} from "@/api";
import { BranchCluster } from "@/components/git/BranchCluster";
import { GitRail } from "@/components/git/GitRail";
import { Toaster } from "@/components/ui/sonner";
import { TooltipProvider } from "@/components/ui/tooltip";
import { requestBranchSwitcher } from "@/store/git/branchCluster";
import type { GitStatus } from "@/domain";

const readStatus = vi.mocked(gitStatus);
const readTrust = vi.mocked(gitTrusted);
const readBranches = vi.mocked(gitBranches);
const switchBranch = vi.mocked(gitSwitchBranch);
const deleteBranch = vi.mocked(gitDeleteBranch);
const fetch = vi.mocked(gitFetch);
const push = vi.mocked(gitPush);
const stopExchange = vi.mocked(gitStopExchange);

const PROJECT = 7;

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
  localStorage.clear();
  stream.listeners = [];
});

/** Every project starts untrusted, which is where the cluster starts too. */
beforeEach(() => {
  readTrust.mockResolvedValue(false);
});

function statusWith(branch: GitStatus["branch"]): GitStatus {
  return { branch, changes: [], merging: false };
}

const MAIN = statusWith({
  name: "main",
  upstream: "origin/main",
  sync: { state: "ahead", ahead: 2 },
});

/**
 * The window chrome and the rail beside the terminal, as the app mounts them: the rail is where the
 * repository is read and the cluster is what the title bar shows of it, so a test of the cluster is
 * a test of the two together. The alert stack comes along because a refused exchange is reported
 * there rather than in the strip, which has no room for a sentence.
 */
function renderChrome(onOpenPullRequest?: () => void) {
  return render(
    <TooltipProvider>
      <header>
        <BranchCluster />
      </header>
      <GitRail project={PROJECT} onOpenPullRequest={onOpenPullRequest} />
      <Toaster />
    </TooltipProvider>,
  );
}

function chrome(): HTMLElement {
  return screen.getByRole("banner");
}

function rail(): HTMLElement {
  return screen.getByRole("complementary", { name: "Version control" });
}

/** Announces a working-tree change the way the core does. */
function announceChange(): void {
  act(() => {
    for (const listener of [...stream.listeners]) {
      listener({ type: "GitStatusChanged", project: PROJECT });
    }
  });
}

describe("the checked-out branch in the window chrome", () => {
  it("shows what is checked out and how far it stands from its upstream", async () => {
    readStatus.mockResolvedValue(MAIN);

    renderChrome();

    await waitFor(() => expect(within(chrome()).getByText("main")).toBeTruthy());
    // Asked for by role and name rather than by the attribute: the arrows are hidden from assistive
    // technology, so the words are all a reader who is not looking at them gets, and a name on an
    // element whose role cannot carry one is a name nothing reads.
    expect(within(chrome()).getByRole("img", { name: "2 ahead" })).toBeTruthy();
  });

  it("names the standing of a branch nobody has published, which a hue alone would not", async () => {
    // The most common state a new branch is in, and the one where a tone was the whole difference
    // from a branch that matches its upstream: two greens apart in hue and barely in lightness, so
    // in grayscale or to a colour-blind reader they were the same chip.
    readStatus.mockResolvedValue(
      statusWith({ name: "spike", upstream: null, sync: { state: "unknown" } }),
    );

    renderChrome();

    await within(chrome()).findByText("spike");
    expect(within(chrome()).getByText("Local only")).toBeTruthy();
  });

  it("tells a branch tracking nothing from one whose upstream has not been fetched", async () => {
    // Different next actions — publish the one, fetch the other — so they may not read alike.
    readStatus.mockResolvedValue(
      statusWith({ name: "spike", upstream: "origin/spike", sync: { state: "unknown" } }),
    );

    renderChrome();

    await within(chrome()).findByText("spike");
    expect(within(chrome()).getByText("Not fetched")).toBeTruthy();
    expect(within(chrome()).queryByText("Local only")).toBeNull();
  });

  it("keeps the branch out of the rail, where its width had nowhere left to come from", async () => {
    // The cluster needs most of a 280px rail on its own, so in the rail the name was the only thing
    // left that could shrink and it shrank to nothing. Whatever the rail is resized to now, the name
    // is not inside it to be squeezed.
    readStatus.mockResolvedValue(
      statusWith({
        name: "feature/a-branch-name-nobody-would-shorten",
        upstream: "origin/feature/a-branch-name-nobody-would-shorten",
        sync: { state: "up_to_date" },
      }),
    );

    renderChrome();

    const name = await within(chrome()).findByText(
      "feature/a-branch-name-nobody-would-shorten",
      {},
      { timeout: 2000 },
    );
    expect(name.textContent, "the whole name is there to read, not an initial letter of it").toBe(
      "feature/a-branch-name-nobody-would-shorten",
    );
    expect(within(rail()).queryByText(/feature\/a-branch-name/)).toBeNull();
  });

  it("says nothing at all about a project that is not a repository", async () => {
    readStatus.mockResolvedValue(null);

    renderChrome();

    await within(rail()).findByText("Not a git repository");
    // Nothing at all, not an empty plate: the strip and the divider beside it stand down on the
    // strength of the strip having no content, so a box with nothing in it would leave a divider
    // dividing one side from nothing.
    expect(
      chrome().childElementCount,
      "a project with nothing to report puts nothing in the chrome",
    ).toBe(0);
    expect(chrome().textContent).toBe("");
  });

  it("offers neither a branch switcher nor a sync action until the project is trusted", async () => {
    readStatus.mockResolvedValue(MAIN);

    renderChrome();

    await within(chrome()).findByText("main");
    expect(
      within(chrome()).queryByRole("button", { name: "Switch branch" }),
      "an action nobody may take is absent, not disabled",
    ).toBeNull();
    expect(within(chrome()).queryByRole("button", { name: "Fetch" })).toBeNull();
    expect(
      readBranches,
      "and nothing is read for a switcher that is not offered",
    ).not.toHaveBeenCalled();
  });

  it("reads the branches only once the switcher is opened, and switches to the one chosen", async () => {
    readTrust.mockResolvedValue(true);
    readStatus.mockResolvedValue(MAIN);
    readBranches.mockResolvedValue({
      entries: [
        { name: "main", upstream: "origin/main", head: true },
        { name: "feature", upstream: null, head: false },
      ],
      stashed: false,
    });

    renderChrome();
    const switcher = await within(chrome()).findByRole("button", { name: "Switch branch" });
    expect(
      readBranches,
      "a list nobody is looking at is a subprocess nobody needed",
    ).not.toHaveBeenCalled();
    fireEvent.click(switcher);

    fireEvent.click(await screen.findByText("feature"));

    await waitFor(() => expect(switchBranch).toHaveBeenCalledWith(PROJECT, "feature"));
    expect(readBranches).toHaveBeenCalledWith(PROJECT);
  });

  it("opens its switcher when a surface with no room for one asks for it", async () => {
    // What the command palette's "Switch branch…" does: there is one switcher, in the chrome, and
    // asking for it reads the branches the same way pressing the badge does.
    readTrust.mockResolvedValue(true);
    readStatus.mockResolvedValue(MAIN);
    readBranches.mockResolvedValue({
      entries: [
        { name: "main", upstream: "origin/main", head: true },
        { name: "feature", upstream: null, head: false },
      ],
      stashed: false,
    });

    renderChrome();
    await within(chrome()).findByRole("button", { name: "Switch branch" });
    act(() => requestBranchSwitcher());

    fireEvent.click(await screen.findByText("feature"));

    await waitFor(() => expect(switchBranch).toHaveBeenCalledWith(PROJECT, "feature"));
  });

  it("confirms before a branch is deleted, and deletes nothing until it is confirmed", async () => {
    readTrust.mockResolvedValue(true);
    readStatus.mockResolvedValue(MAIN);
    readBranches.mockResolvedValue({
      entries: [
        { name: "main", upstream: "origin/main", head: true },
        { name: "spike", upstream: null, head: false },
      ],
      stashed: false,
    });

    renderChrome();
    fireEvent.click(await within(chrome()).findByRole("button", { name: "Switch branch" }));
    fireEvent.click(await screen.findByRole("button", { name: "Delete branch spike" }));

    expect(deleteBranch, "nothing goes before the question is answered").not.toHaveBeenCalled();
    fireEvent.click(await screen.findByRole("button", { name: "Delete" }));
    await waitFor(() => expect(deleteBranch).toHaveBeenCalledWith(PROJECT, "spike"));
  });

  it("offers to publish a branch that tracks nothing rather than to pull from it", async () => {
    readTrust.mockResolvedValue(true);
    readStatus.mockResolvedValue(
      statusWith({ name: "spike", upstream: null, sync: { state: "unknown" } }),
    );

    renderChrome();

    await within(chrome()).findByRole("button", { name: "Publish" });
    expect(
      within(chrome()).queryByRole("button", { name: "Pull" }),
      "there is nothing to pull from an upstream that does not exist",
    ).toBeNull();
  });

  it("offers to stop an exchange while it is under way, and reports nothing when it is stopped", async () => {
    readTrust.mockResolvedValue(true);
    readStatus.mockResolvedValue(MAIN);
    let refuse: (reason: Error) => void = () => {};
    push.mockImplementation(() => new Promise((_, reject) => (refuse = reject)));

    renderChrome();
    fireEvent.click(await within(chrome()).findByRole("button", { name: "Push" }));

    const stop = await within(chrome()).findByRole("button", { name: "Stop" });
    fireEvent.click(stop);
    expect(stopExchange).toHaveBeenCalledWith(PROJECT);
    // The core reports a stopped exchange as refused, because from its side it did not finish.
    await act(async () => {
      refuse(new Error("the git command was stopped"));
    });

    await within(chrome()).findByRole("button", { name: "Push" });
    expect(
      screen.queryByRole("alert"),
      "an exchange the reader stopped is what they asked for, not a failure to report back at them",
    ).toBeNull();
  });

  it("states the reason an exchange really failed, where the strip has no room to say it", async () => {
    // The controls that reach the remote live in a 44px strip that cannot grow a line, so the one
    // thing a reader has to be told goes to the alert stack instead of nowhere.
    readTrust.mockResolvedValue(true);
    readStatus.mockResolvedValue(MAIN);
    fetch.mockRejectedValue(new Error("no credential the remote would accept"));

    renderChrome();
    fireEvent.click(await within(chrome()).findByRole("button", { name: "Fetch" }));

    const refusal = await screen.findByRole("alert");
    expect(refusal.textContent).toContain("no credential the remote would accept");
  });

  it("says the same refusal again when the retry fails the same way, rather than once", async () => {
    // A reader who tries again and is told nothing concludes the retry worked. The message is
    // identical both times, which is exactly the case a surface that reports only what changed
    // would swallow.
    readTrust.mockResolvedValue(true);
    readStatus.mockResolvedValue(MAIN);
    fetch.mockRejectedValue(new Error("no credential the remote would accept"));

    renderChrome();
    // Asked for again each time: while the exchange is under way the control is the one that stops
    // it, so the button that comes back after a refusal is a new one.
    fireEvent.click(await within(chrome()).findByRole("button", { name: "Fetch" }));
    await screen.findByRole("alert");

    fireEvent.click(await within(chrome()).findByRole("button", { name: "Fetch" }));

    await waitFor(() => expect(screen.getAllByRole("alert")).toHaveLength(2));
    expect(fetch).toHaveBeenCalledTimes(2);
  });

  it("offers no way to reach a pull request until the project is trusted", async () => {
    readStatus.mockResolvedValue(MAIN);

    renderChrome(() => {});

    await within(chrome()).findByText("main");
    expect(
      within(chrome()).queryByRole("button", { name: "Pull request" }),
      "proposing one pushes the branch and runs the repository's own configuration",
    ).toBeNull();
  });

  it("shows the pull request view when it is asked for", async () => {
    readTrust.mockResolvedValue(true);
    readStatus.mockResolvedValue(MAIN);
    const open = vi.fn();

    renderChrome(open);

    fireEvent.click(await within(chrome()).findByRole("button", { name: "Pull request" }));
    expect(open).toHaveBeenCalled();
  });

  it("asks the repository about itself once for both surfaces, not once each", async () => {
    // The chrome and the rail show the same status. Reading it twice would double the git
    // subprocesses on every working-tree change, and a working tree under an agent changes often.
    readStatus.mockResolvedValue(MAIN);

    renderChrome();
    await within(chrome()).findByText("main");
    expect(readStatus).toHaveBeenCalledTimes(1);

    announceChange();

    await waitFor(() => expect(readStatus).toHaveBeenCalledTimes(2));
    // Settle any further frame the announcement might have scheduled before counting again.
    await act(async () => {
      await new Promise((resolve) => requestAnimationFrame(() => resolve(null)));
    });
    expect(readStatus, "one change announced, one status read").toHaveBeenCalledTimes(2);
  });
});
