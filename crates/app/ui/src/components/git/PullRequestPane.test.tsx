// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";

// The three calls and the event subscription are the IPC boundary; mocking them leaves the view's
// own behaviour — what it offers in each state, what it seeds, and what it sends — as what the
// test exercises.
vi.mock("@/api", () => ({
  gitPullRequestSurface: vi.fn(),
  gitCreatePullRequest: vi.fn(() => Promise.resolve("")),
  gitDraftPullRequestBody: vi.fn(() => Promise.resolve("")),
  gitPullRequestReview: vi.fn(() => Promise.resolve(null)),
  gitMergePullRequest: vi.fn(() => Promise.resolve()),
  gitHandOff: vi.fn(),
  assistSettings: vi.fn(() => Promise.resolve({ tool: null })),
  onDomainEvent: vi.fn(() => Promise.resolve(() => {})),
  onResync: vi.fn(() => Promise.resolve(() => {})),
}));

vi.mock("@/lib/opener", () => ({ openExternal: vi.fn(() => Promise.resolve()) }));

import {
  assistSettings,
  gitCreatePullRequest,
  gitDraftPullRequestBody,
  gitPullRequestReview,
  gitPullRequestSurface,
} from "@/api";
import { PullRequestPane } from "@/components/git/PullRequestPane";
import { TooltipProvider } from "@/components/ui/tooltip";
import { openExternal } from "@/lib/opener";
import type { PullRequest, PullRequestSurface } from "@/domain";

const readSurface = vi.mocked(gitPullRequestSurface);
const readReview = vi.mocked(gitPullRequestReview);
const create = vi.mocked(gitCreatePullRequest);
const draftBody = vi.mocked(gitDraftPullRequestBody);
const readAssist = vi.mocked(assistSettings);
const openLink = vi.mocked(openExternal);

const PROJECT = 7;
const HEAD = "feature";
const BASE = "main";
const URL = "https://github.example/owner/repo/pull/12";

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
  localStorage.clear();
});

/** Every install starts with no tool picked to draft with. */
beforeEach(() => {
  readAssist.mockResolvedValue({ tool: null });
});

function surface(over: Partial<PullRequestSurface> = {}): PullRequestSurface {
  return {
    readiness: "ready",
    head: HEAD,
    base: BASE,
    existing: null,
    templates: [],
    merge_methods: [],
    ...over,
  };
}

function existing(over: Partial<PullRequest> = {}): PullRequest {
  return {
    number: 12,
    url: URL,
    title: "Propose the thing",
    state: "open",
    draft: false,
    base: BASE,
    head: HEAD,
    ...over,
  };
}

function renderPane() {
  return render(
    <TooltipProvider>
      <PullRequestPane project={PROJECT} agent={null} onClose={() => {}} />
    </TooltipProvider>,
  );
}

function pane(): HTMLElement {
  return screen.getByRole("region", { name: "Pull request" });
}

describe("PullRequestPane", () => {
  it("says how to install the tool instead of offering a form it could not send", async () => {
    readSurface.mockResolvedValue(surface({ readiness: "missing", head: null, base: null }));

    renderPane();

    await waitFor(() => expect(within(pane()).getByText(/not installed/i)).toBeTruthy());
    expect(within(pane()).queryByLabelText("Title")).toBeNull();
  });

  it("says how to sign in, which is a different thing from the tool being absent", async () => {
    readSurface.mockResolvedValue(surface({ readiness: "logged_out", head: null, base: null }));

    renderPane();

    await waitFor(() => expect(within(pane()).getByText(/gh auth login/i)).toBeTruthy());
    expect(within(pane()).queryByLabelText("Title")).toBeNull();
  });

  it("seeds the description from the shape on offer and the base from the repository", async () => {
    readSurface.mockResolvedValue(
      surface({ templates: [{ name: "house", body: "## What changed\n" }] }),
    );

    renderPane();

    const body = await screen.findByLabelText("Description");
    expect((body as HTMLTextAreaElement).value).toBe("## What changed\n");
    expect((screen.getByLabelText("Merge into") as HTMLInputElement).value).toBe(BASE);
    expect(
      screen.queryByLabelText("Starting shape"),
      "one shape is not a choice, so nothing is offered to choose between",
    ).toBeNull();
  });

  it("offers a choice only where more than one shape is on offer, and applies the one chosen", async () => {
    readSurface.mockResolvedValue(
      surface({
        templates: [
          { name: "bugfix", body: "## The bug\n" },
          { name: "feature", body: "## The feature\n" },
        ],
      }),
    );

    renderPane();

    const chooser = await screen.findByLabelText("Starting shape");
    fireEvent.keyDown(chooser, { key: "Enter" });
    fireEvent.click(await screen.findByRole("option", { name: "feature" }));

    await waitFor(() =>
      expect((screen.getByLabelText("Description") as HTMLTextAreaElement).value).toBe(
        "## The feature\n",
      ),
    );
  });

  it("proposes what was typed and opens what came back", async () => {
    readSurface.mockResolvedValue(surface());
    create.mockResolvedValue(URL);

    renderPane();

    fireEvent.change(await screen.findByLabelText("Title"), {
      target: { value: "Propose the thing" },
    });
    fireEvent.change(screen.getByLabelText("Description"), { target: { value: "Because." } });
    fireEvent.click(screen.getByRole("checkbox", { name: /draft/i }));
    fireEvent.click(screen.getByRole("button", { name: "Open pull request" }));

    await waitFor(() =>
      expect(create).toHaveBeenCalledWith(PROJECT, {
        title: "Propose the thing",
        body: "Because.",
        base: BASE,
        draft: true,
      }),
    );
    await waitFor(() => expect(openLink).toHaveBeenCalledWith(URL));
  });

  it("refuses to send a proposal with no title, so the core is never asked one it would refuse", async () => {
    readSurface.mockResolvedValue(surface());

    renderPane();

    const propose = await screen.findByRole("button", { name: "Open pull request" });
    expect((propose as HTMLButtonElement).disabled).toBe(true);
    fireEvent.click(propose);
    expect(create).not.toHaveBeenCalled();
  });

  it("shows the pull request this branch already has instead of offering a second", async () => {
    readSurface.mockResolvedValue(surface({ existing: existing() }));

    renderPane();

    await waitFor(() => expect(within(pane()).getByText("#12")).toBeTruthy());
    expect(within(pane()).getByText("Open")).toBeTruthy();
    expect(within(pane()).queryByLabelText("Title")).toBeNull();
  });

  it("reads and shows the review of the pull request the branch already has", async () => {
    readSurface.mockResolvedValue(surface({ existing: existing() }));
    readReview.mockResolvedValue({
      pull_request: existing(),
      checks: [{ name: "build", state: "failed", workflow: null, url: null }],
      threads: [],
    });

    renderPane();

    await waitFor(() => expect(within(pane()).getByText("build")).toBeTruthy());
    expect(readReview).toHaveBeenCalledWith(PROJECT);
    expect(
      within(pane()).queryByRole("button", { name: "Merge" }),
      "the repository permits no way of merging, so the pane offers none — it does not invent one",
    ).toBeNull();
  });

  it("offers a new one where the last was merged, because that branch may propose again", async () => {
    readSurface.mockResolvedValue(surface({ existing: existing({ state: "merged" }) }));

    renderPane();

    await waitFor(() => expect(within(pane()).getByText("Merged")).toBeTruthy());
    expect(within(pane()).getByLabelText("Title")).toBeTruthy();
  });

  it("offers no way to draft a description until a tool is picked to draft with", async () => {
    readSurface.mockResolvedValue(surface());

    renderPane();

    await screen.findByLabelText("Title");
    expect(screen.queryByRole("button", { name: "Draft a description" })).toBeNull();
    expect(draftBody).not.toHaveBeenCalled();
  });

  it("puts a drafted description in the box to edit, and proposes nothing by itself", async () => {
    readAssist.mockResolvedValue({ tool: "Claude" });
    readSurface.mockResolvedValue(
      surface({ templates: [{ name: "house", body: "## What changed\n" }] }),
    );
    draftBody.mockResolvedValue("## What changed\n\nIt changed the thing.\n");

    renderPane();

    fireEvent.click(await screen.findByRole("button", { name: "Draft a description" }));

    await waitFor(() =>
      expect((screen.getByLabelText("Description") as HTMLTextAreaElement).value).toBe(
        "## What changed\n\nIt changed the thing.\n",
      ),
    );
    expect(draftBody).toHaveBeenCalledWith(PROJECT, BASE, "## What changed\n");
    expect(create).not.toHaveBeenCalled();
  });

  it("states a refused proposal where it was asked for", async () => {
    readSurface.mockResolvedValue(surface());
    create.mockRejectedValue("a pull request for that branch already exists");

    renderPane();

    fireEvent.change(await screen.findByLabelText("Title"), { target: { value: "Whatever" } });
    fireEvent.click(screen.getByRole("button", { name: "Open pull request" }));

    await waitFor(() =>
      expect(within(pane()).getByRole("alert").textContent).toContain("already exists"),
    );
    expect(openLink).not.toHaveBeenCalled();
  });

  it("has nothing to propose from a detached head and says so", async () => {
    readSurface.mockResolvedValue(surface({ head: null }));

    renderPane();

    await waitFor(() => expect(within(pane()).getByText(/no branch to propose/i)).toBeTruthy());
    expect(within(pane()).queryByLabelText("Title")).toBeNull();
  });
});
