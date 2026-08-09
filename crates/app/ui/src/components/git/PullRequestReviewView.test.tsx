// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";

// The reads and the two actions are the IPC boundary; mocking them leaves what the view offers in
// each state, and what it sends, as what the test exercises.
vi.mock("@/api", () => ({
  gitPullRequestReview: vi.fn(),
  gitMergePullRequest: vi.fn(() => Promise.resolve()),
  gitHandOff: vi.fn(),
  onDomainEvent: vi.fn(() => Promise.resolve(() => {})),
  onResync: vi.fn(() => Promise.resolve(() => {})),
}));

vi.mock("@/lib/opener", () => ({ openExternal: vi.fn(() => Promise.resolve()) }));
vi.mock("@/lib/clipboard", () => ({ writeClipboard: vi.fn(() => Promise.resolve()) }));

// The markdown renderer mounts a rich-text editor behind a lazy chunk; a comment's text is what
// this view is judged on, so it is rendered plainly here.
vi.mock("@/components/editor/MarkdownView", () => ({
  MarkdownView: ({ markdown }: { markdown: string }) => <p>{markdown}</p>,
}));

import { gitHandOff, gitMergePullRequest, gitPullRequestReview } from "@/api";
import { PullRequestReviewView } from "@/components/git/PullRequestReviewView";
import { TooltipProvider } from "@/components/ui/tooltip";
import { writeClipboard } from "@/lib/clipboard";
import { REVIEW_BACKOFF_MS, REVIEW_POLL_MS } from "@/store/git/usePullRequestReview";
import type { CheckRun, PullRequestReview, ReviewThread } from "@/domain";

const read = vi.mocked(gitPullRequestReview);
const merge = vi.mocked(gitMergePullRequest);
const handOff = vi.mocked(gitHandOff);
const copy = vi.mocked(writeClipboard);

const PROJECT = 7;
const AGENT = 3;

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
  vi.useRealTimers();
});

beforeEach(() => {
  handOff.mockResolvedValue({ delivery: "delivered", process: AGENT, text: "the context" });
});

function check(over: Partial<CheckRun> = {}): CheckRun {
  return {
    name: "build",
    state: "failed",
    workflow: "Tests",
    url: "https://forge.example/owner/repo/actions/runs/9/job/77",
    ...over,
  };
}

function thread(over: Partial<ReviewThread> = {}): ReviewThread {
  return {
    id: "PRRT_1",
    url: "https://forge.example/pull/12#discussion_r1",
    path: "src/main.rs",
    line: 42,
    resolved: false,
    outdated: false,
    comments: [
      { author: "octocat", body: "this leaks a file handle", url: "https://forge.example/c1" },
    ],
    ...over,
  };
}

function review(over: Partial<PullRequestReview> = {}): PullRequestReview {
  return {
    pull_request: {
      number: 12,
      url: "https://forge.example/pull/12",
      title: "Propose the thing",
      state: "open",
      draft: false,
      base: "main",
      head: "feature",
    },
    checks: [check()],
    threads: [thread()],
    ...over,
  };
}

function renderView(methods: ("merge" | "squash" | "rebase")[] = ["squash"]) {
  return render(
    <TooltipProvider>
      <PullRequestReviewView project={PROJECT} agent={AGENT} methods={methods} />
    </TooltipProvider>,
  );
}

describe("PullRequestReviewView", () => {
  it("shows each check with a word as well as a colour, so it survives a grayscale screen", async () => {
    read.mockResolvedValue(
      review({ checks: [check(), check({ name: "lint", state: "passed", url: null })] }),
    );

    renderView();

    await waitFor(() => expect(screen.getByText("build")).toBeTruthy());
    expect(screen.getByText("Failed")).toBeTruthy();
    expect(screen.getByText("Passed")).toBeTruthy();
  });

  it("offers a handoff on a check that objected and on no other", async () => {
    read.mockResolvedValue(
      review({ checks: [check(), check({ name: "lint", state: "passed", url: null })] }),
    );

    renderView();

    await waitFor(() => expect(screen.getByText("lint")).toBeTruthy());
    const rows = screen.getAllByRole("listitem");
    expect(within(rows[0]).queryByLabelText("Hand to an agent")).toBeTruthy();
    expect(
      within(rows[1]).queryByLabelText("Hand to an agent"),
      "a check that passed has nothing anybody needs to fix",
    ).toBeNull();
  });

  it("names the check the reader pointed at rather than any text of its own", async () => {
    read.mockResolvedValue(review());

    renderView();

    await waitFor(() => expect(screen.getByText("build")).toBeTruthy());
    fireEvent.click(screen.getAllByLabelText("Hand to an agent")[0]);

    await waitFor(() =>
      expect(handOff).toHaveBeenCalledWith(PROJECT, { kind: "check", name: "build" }, AGENT),
    );
  });

  it("says the context is in the session and that nothing was sent for the reader", async () => {
    read.mockResolvedValue(review());

    renderView();

    await waitFor(() => expect(screen.getByText("build")).toBeTruthy());
    fireEvent.click(screen.getAllByLabelText("Hand to an agent")[0]);

    await waitFor(() => expect(screen.getByText(/unsent/i)).toBeTruthy());
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  it("offers the context to copy when there was nowhere to deliver it, rather than doing nothing", async () => {
    read.mockResolvedValue(review());
    handOff.mockResolvedValue({ delivery: "copy", text: "the whole context" });

    renderView();

    await waitFor(() => expect(screen.getByText("build")).toBeTruthy());
    fireEvent.click(screen.getAllByLabelText("Hand to an agent")[0]);

    const dialog = await screen.findByRole("dialog");
    expect(within(dialog).getByText("the whole context")).toBeTruthy();
    fireEvent.click(within(dialog).getByRole("button", { name: "Copy the context" }));
    await waitFor(() => expect(copy).toHaveBeenCalledWith("the whole context"));
  });

  it("names the conversation the reader pointed at, as a conversation and not as a check", async () => {
    read.mockResolvedValue(review());

    renderView();

    await waitFor(() => expect(screen.getByText("src/main.rs:42")).toBeTruthy());
    const rows = screen.getAllByLabelText("Hand to an agent");
    fireEvent.click(rows[rows.length - 1]);

    await waitFor(() =>
      expect(handOff).toHaveBeenCalledWith(PROJECT, { kind: "thread", id: "PRRT_1" }, AGENT),
    );
  });

  it("says why a handoff was refused rather than looking like a button that does nothing", async () => {
    read.mockResolvedValue(review());
    handOff.mockRejectedValue("that is not a running agent in this project");

    renderView();

    await waitFor(() => expect(screen.getByText("build")).toBeTruthy());
    fireEvent.click(screen.getAllByLabelText("Hand to an agent")[0]);

    await waitFor(() => expect(screen.getByRole("alert").textContent).toContain("running agent"));
  });

  it("keeps a settled conversation out of the way until it is asked for", async () => {
    read.mockResolvedValue(
      review({
        threads: [
          thread(),
          thread({
            id: "PRRT_2",
            resolved: true,
            path: "git/client.go",
            comments: [{ author: "hubot", body: "already dealt with", url: null }],
          }),
        ],
      }),
    );

    renderView();

    await waitFor(() => expect(screen.getByText(/this leaks/)).toBeTruthy());
    expect(screen.queryByText(/already dealt with/)).toBeNull();

    fireEvent.click(screen.getByRole("button", { name: /1 settled/ }));
    expect(screen.getByText(/already dealt with/)).toBeTruthy();
  });

  it("shows where a comment hangs, which is what makes it a comment on something", async () => {
    read.mockResolvedValue(review());

    renderView();

    await waitFor(() => expect(screen.getByText("src/main.rs:42")).toBeTruthy());
  });

  it("offers only the ways of merging this repository permits", async () => {
    read.mockResolvedValue(review());

    renderView(["rebase"]);

    await waitFor(() => expect(screen.getByLabelText("How to merge")).toBeTruthy());
    expect(screen.getByLabelText("How to merge").textContent).toContain("Rebase");
  });

  it("offers no merge at all where the repository permits none", async () => {
    read.mockResolvedValue(review());

    renderView([]);

    await waitFor(() => expect(screen.getByText("build")).toBeTruthy());
    expect(
      screen.queryByRole("button", { name: "Merge" }),
      "an action nobody may take is not an action",
    ).toBeNull();
  });

  it("offers no merge on a pull request that is not open any more", async () => {
    read.mockResolvedValue(review({ pull_request: { ...review().pull_request, state: "merged" } }));

    renderView();

    await waitFor(() => expect(screen.getByText("build")).toBeTruthy());
    expect(screen.queryByRole("button", { name: "Merge" })).toBeNull();
  });

  it("merges nothing until it has been confirmed", async () => {
    read.mockResolvedValue(review());

    renderView(["squash"]);

    await waitFor(() => expect(screen.getByRole("button", { name: "Merge" })).toBeTruthy());
    fireEvent.click(screen.getByRole("button", { name: "Merge" }));
    expect(merge, "asking is not doing").not.toHaveBeenCalled();

    const dialog = await screen.findByRole("alertdialog");
    fireEvent.click(within(dialog).getByRole("button", { name: "Merge" }));
    await waitFor(() => expect(merge).toHaveBeenCalledWith(PROJECT, 12, "squash"));
  });

  it("leaves the pull request alone when the reader changes their mind", async () => {
    read.mockResolvedValue(review());

    renderView(["squash"]);

    await waitFor(() => expect(screen.getByRole("button", { name: "Merge" })).toBeTruthy());
    fireEvent.click(screen.getByRole("button", { name: "Merge" }));
    const dialog = await screen.findByRole("alertdialog");
    fireEvent.click(within(dialog).getByRole("button", { name: "Leave it open" }));

    expect(merge).not.toHaveBeenCalled();
  });

  it("says what a refused action said rather than swallowing it", async () => {
    read.mockResolvedValue(review());
    merge.mockRejectedValue("the base branch policy prohibits the merge");

    renderView(["squash"]);

    await waitFor(() => expect(screen.getByRole("button", { name: "Merge" })).toBeTruthy());
    fireEvent.click(screen.getByRole("button", { name: "Merge" }));
    const dialog = await screen.findByRole("alertdialog");
    fireEvent.click(within(dialog).getByRole("button", { name: "Merge" }));

    await waitFor(() => expect(screen.getByRole("alert").textContent).toContain("prohibits"));
  });

  it("keeps reading while it is open, because a check finishing announces nothing", async () => {
    vi.useFakeTimers();
    read.mockResolvedValue(review());

    renderView();

    await act(async () => {
      await vi.advanceTimersByTimeAsync(0);
    });
    const first = read.mock.calls.length;

    await act(async () => {
      await vi.advanceTimersByTimeAsync(REVIEW_POLL_MS + 1);
    });

    expect(read.mock.calls.length).toBeGreaterThan(first);
  });

  it("never stacks a second read behind one the service has not answered", async () => {
    vi.useFakeTimers();
    // A read that never comes back — a service holding the connection open, which is exactly when
    // a timer that keeps firing turns one slow request into a queue of them.
    read.mockReturnValue(new Promise(() => {}));

    renderView();
    await act(async () => {
      await vi.advanceTimersByTimeAsync(0);
    });

    await act(async () => {
      await vi.advanceTimersByTimeAsync(REVIEW_POLL_MS * 4);
    });

    expect(read.mock.calls.length, "one fact, one request in flight for it").toBe(1);
  });

  it("stops reading the moment it is closed", async () => {
    vi.useFakeTimers();
    read.mockResolvedValue(review());

    const view = renderView();
    await act(async () => {
      await vi.advanceTimersByTimeAsync(0);
    });
    view.unmount();
    const afterClose = read.mock.calls.length;

    await act(async () => {
      await vi.advanceTimersByTimeAsync(REVIEW_POLL_MS * 5);
    });

    expect(
      read.mock.calls.length,
      "a panel nobody is looking at must not keep spending a rate limit",
    ).toBe(afterClose);
  });

  it("slows down against a service that is refusing rather than keeping the rate up", async () => {
    vi.useFakeTimers();
    read.mockRejectedValue("rate limit exceeded");

    renderView();
    await act(async () => {
      await vi.advanceTimersByTimeAsync(0);
    });
    const afterFirst = read.mock.calls.length;

    await act(async () => {
      await vi.advanceTimersByTimeAsync(REVIEW_POLL_MS + 1);
    });
    expect(read.mock.calls.length).toBe(afterFirst);

    await act(async () => {
      await vi.advanceTimersByTimeAsync(REVIEW_BACKOFF_MS);
    });
    expect(read.mock.calls.length).toBeGreaterThan(afterFirst);
  });
});
