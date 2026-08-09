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

/** A template with the two headings a repository is asking to have filled in. */
const HOUSE_TEMPLATE = "## What changed\n\n## Checklist\n\n- [ ] Tested\n";

/** The same template with the computed account of the commits written into it — what the core
 *  suggests, and the only one of the two that says anything about this branch. */
const SUGGESTED_BODY =
  "## What changed\n\nThe rail follows the working tree.\n\n## Checklist\n\n- [ ] Tested\n";

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
    suggestion: null,
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

/** Opens the fields, which is where every by-hand proposal starts. */
async function editDetails(): Promise<void> {
  fireEvent.click(await screen.findByRole("button", { name: "Edit details…" }));
  await screen.findByLabelText("Title");
}

describe("PullRequestPane — what it can offer", () => {
  it("says how to install the tool instead of offering a form it could not send", async () => {
    readSurface.mockResolvedValue(surface({ readiness: "missing", head: null, base: null }));

    renderPane();

    await waitFor(() => expect(within(pane()).getByText(/not installed/i)).toBeTruthy());
    expect(within(pane()).queryByRole("button", { name: "Edit details…" })).toBeNull();
  });

  it("says how to sign in, which is a different thing from the tool being absent", async () => {
    readSurface.mockResolvedValue(surface({ readiness: "logged_out", head: null, base: null }));

    renderPane();

    await waitFor(() => expect(within(pane()).getByText(/gh auth login/i)).toBeTruthy());
    expect(within(pane()).queryByRole("button", { name: "Edit details…" })).toBeNull();
  });

  it("has nothing to propose from a detached head and says what to do about it", async () => {
    readSurface.mockResolvedValue(surface({ head: null }));

    renderPane();

    await waitFor(() =>
      expect(within(pane()).getByText(/nothing is checked out by name/i)).toBeTruthy(),
    );
    expect(within(pane()).getByText(/check a branch out/i)).toBeTruthy();
    expect(within(pane()).queryByRole("button", { name: "Edit details…" })).toBeNull();
  });

  it("shows the pull request this branch already has, and says why it offers no second", async () => {
    readSurface.mockResolvedValue(surface({ existing: existing() }));

    renderPane();

    await waitFor(() => expect(within(pane()).getByText("#12")).toBeTruthy());
    expect(within(pane()).getByText("Open")).toBeTruthy();
    expect(within(pane()).getByText(/already has a pull request open/i)).toBeTruthy();
    expect(within(pane()).queryByRole("button", { name: "Open pull request" })).toBeNull();
  });

  it("offers a new one where the last was merged, because that branch may propose again", async () => {
    readSurface.mockResolvedValue(
      surface({
        existing: existing({ state: "merged" }),
        suggestion: { title: "Live changes rail", body: "It changed." },
      }),
    );

    renderPane();

    await waitFor(() => expect(within(pane()).getByText("Merged")).toBeTruthy());
    expect(within(pane()).getByRole("button", { name: "Open pull request" })).toBeTruthy();
  });

  it("stops claiming to be reading once the read has come back refused", async () => {
    // The read resolved — with a failure. A surface that goes on saying "reading" is claiming
    // something is still coming that nothing will bring, and it says it beside the reason it won't.
    readSurface.mockRejectedValue(new Error("gh: could not reach github.com"));

    renderPane();

    await waitFor(() =>
      expect(within(pane()).getByRole("alert").textContent).toContain("could not reach github.com"),
    );
    expect(within(pane()).queryByText(/reading what this branch has/i)).toBeNull();
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
});

describe("PullRequestPane — proposing what the branch already says", () => {
  it("proposes the suggestion as it stands from one press, and opens what came back", async () => {
    readSurface.mockResolvedValue(
      surface({
        suggestion: { title: "Live changes rail", body: "The rail follows the working tree.\n" },
      }),
    );
    create.mockResolvedValue(URL);

    renderPane();
    fireEvent.click(await screen.findByRole("button", { name: "Open pull request" }));

    await waitFor(() =>
      expect(create).toHaveBeenCalledWith(PROJECT, {
        title: "Live changes rail",
        body: "The rail follows the working tree.\n",
        base: BASE,
        draft: false,
      }),
    );
    await waitFor(() => expect(openLink).toHaveBeenCalledWith(URL));
  });

  it("shows the title it would carry and the branches it would join before it is pressed", async () => {
    readSurface.mockResolvedValue(
      surface({ suggestion: { title: "Live changes rail", body: "Because." } }),
    );

    renderPane();

    await screen.findByRole("button", { name: "Open pull request" });
    expect(
      within(pane()).getByText("Live changes rail"),
      "the reader is about to publish a title they did not type; it is on screen first",
    ).toBeTruthy();
    expect(within(pane()).getByText(`${BASE} ← ${HEAD}`)).toBeTruthy();
  });

  it("proposes nothing until the press, neither on mount nor on the read landing", async () => {
    readSurface.mockResolvedValue(
      surface({ suggestion: { title: "Live changes rail", body: "Because." } }),
    );

    renderPane();

    await screen.findByRole("button", { name: "Open pull request" });
    expect(create).not.toHaveBeenCalled();
    expect(openLink).not.toHaveBeenCalled();
  });

  it("carries the repository's own headings into what one press proposes", async () => {
    // The shape is the repository's contract with whoever opens a pull request, so the computed
    // account is written into it rather than over it — and the press sends that, not a rewrite.
    readSurface.mockResolvedValue(
      surface({
        templates: [{ name: "pull_request_template", body: HOUSE_TEMPLATE }],
        suggestion: { title: "Live changes rail", body: SUGGESTED_BODY },
      }),
    );
    create.mockResolvedValue(URL);

    renderPane();
    fireEvent.click(await screen.findByRole("button", { name: "Open pull request" }));

    await waitFor(() => expect(create).toHaveBeenCalled());
    // The whole body, not the headings it shares with the bare template: those are in both, so
    // asserting them would pass just as well on an empty checklist with the commits dropped.
    expect(create.mock.calls[0][1].body).toBe(SUGGESTED_BODY);
  });

  it("says where the description came from, so a filled-in template is not a surprise", async () => {
    readSurface.mockResolvedValue(
      surface({
        templates: [{ name: "pull_request_template", body: HOUSE_TEMPLATE }],
        suggestion: { title: "Live changes rail", body: "## What changed\n\nIt changed.\n" },
      }),
    );

    renderPane();

    await screen.findByRole("button", { name: "Open pull request" });
    expect(within(pane()).getByText(/written into the pull-request template/i)).toBeTruthy();
  });

  it("offers no press at all where there is nothing to propose, and says why", async () => {
    readSurface.mockResolvedValue(surface({ suggestion: null }));

    renderPane();

    await waitFor(() =>
      expect(
        within(pane()).getByText(new RegExp(`holds nothing ${BASE} does not`, "i")),
      ).toBeTruthy(),
    );
    expect(
      within(pane()).queryByRole("button", { name: "Open pull request" }),
      "an action nobody can take is absent, not offered and refused",
    ).toBeNull();
    expect(create).not.toHaveBeenCalled();
  });

  it("says so where the repository names no branch to merge into", async () => {
    readSurface.mockResolvedValue(surface({ base: null, suggestion: null }));

    renderPane();

    await waitFor(() =>
      expect(within(pane()).getByText(/names no branch to merge into/i)).toBeTruthy(),
    );
    expect(within(pane()).queryByRole("button", { name: "Open pull request" })).toBeNull();
  });
});

describe("PullRequestPane — editing the details first", () => {
  it("opens the fields on the suggestion, title and description both", async () => {
    readSurface.mockResolvedValue(
      surface({
        suggestion: { title: "Live changes rail", body: "The rail follows the working tree.\n" },
      }),
    );

    renderPane();
    await editDetails();

    expect((screen.getByLabelText("Title") as HTMLInputElement).value).toBe("Live changes rail");
    expect((screen.getByLabelText("Description") as HTMLTextAreaElement).value).toBe(
      "The rail follows the working tree.\n",
    );
    expect((screen.getByLabelText("Merge into") as HTMLInputElement).value).toBe(BASE);
  });

  it("proposes what was edited rather than what was suggested", async () => {
    readSurface.mockResolvedValue(
      surface({ suggestion: { title: "Live changes rail", body: "Because." } }),
    );
    create.mockResolvedValue(URL);

    renderPane();
    await editDetails();
    fireEvent.change(screen.getByLabelText("Title"), { target: { value: "Say it better" } });
    fireEvent.change(screen.getByLabelText("Description"), { target: { value: "And why." } });
    fireEvent.click(screen.getByRole("checkbox", { name: /draft/i }));
    fireEvent.click(screen.getByRole("button", { name: "Open pull request" }));

    await waitFor(() =>
      expect(create).toHaveBeenCalledWith(PROJECT, {
        title: "Say it better",
        body: "And why.",
        base: BASE,
        draft: true,
      }),
    );
    await waitFor(() => expect(openLink).toHaveBeenCalledWith(URL));
  });

  it("carries the repository's own headings into the fields, so editing does not lose them", async () => {
    readSurface.mockResolvedValue(
      surface({
        templates: [{ name: "pull_request_template", body: HOUSE_TEMPLATE }],
        suggestion: { title: "Live changes rail", body: SUGGESTED_BODY },
      }),
    );
    create.mockResolvedValue(URL);

    renderPane();
    await editDetails();

    // The filled-in shape, not the bare one: both carry the headings, and only one carries the
    // account of the commits the reader is about to publish.
    expect((screen.getByLabelText("Description") as HTMLTextAreaElement).value).toBe(
      SUGGESTED_BODY,
    );

    fireEvent.click(screen.getByRole("button", { name: "Open pull request" }));
    await waitFor(() => expect(create).toHaveBeenCalled());
    expect(create.mock.calls[0][1].body).toBe(SUGGESTED_BODY);
  });

  it("seeds the description from the shape on offer where nothing could be suggested", async () => {
    readSurface.mockResolvedValue(
      surface({ templates: [{ name: "house", body: "## What changed\n" }], suggestion: null }),
    );

    renderPane();
    await editDetails();

    expect((screen.getByLabelText("Description") as HTMLTextAreaElement).value).toBe(
      "## What changed\n",
    );
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
        suggestion: { title: "Live changes rail", body: "## The bug\n\nIt changed.\n" },
      }),
    );

    renderPane();
    await editDetails();
    const chooser = screen.getByLabelText("Starting shape");
    fireEvent.keyDown(chooser, { key: "Enter" });
    fireEvent.click(await screen.findByRole("option", { name: "feature" }));

    await waitFor(() =>
      expect((screen.getByLabelText("Description") as HTMLTextAreaElement).value).toBe(
        "## The feature\n",
      ),
    );
  });

  it("refuses to send a proposal with no title, so the core is never asked one it would refuse", async () => {
    readSurface.mockResolvedValue(surface({ suggestion: null }));

    renderPane();
    await editDetails();

    const propose = screen.getByRole("button", { name: "Open pull request" });
    expect((propose as HTMLButtonElement).disabled).toBe(true);
    fireEvent.click(propose);
    expect(create).not.toHaveBeenCalled();
  });

  it("states a refused proposal where it was asked for", async () => {
    readSurface.mockResolvedValue(
      surface({ suggestion: { title: "Live changes rail", body: "Because." } }),
    );
    create.mockRejectedValue("a pull request for that branch already exists");

    renderPane();
    fireEvent.click(await screen.findByRole("button", { name: "Open pull request" }));

    await waitFor(() =>
      expect(within(pane()).getByRole("alert").textContent).toContain("already exists"),
    );
    expect(openLink).not.toHaveBeenCalled();
  });
});

describe("PullRequestPane — drafting a description", () => {
  it("offers no way to draft a description until a tool is picked to draft with", async () => {
    readSurface.mockResolvedValue(surface());

    renderPane();
    await editDetails();

    expect(screen.queryByRole("button", { name: "Draft a description" })).toBeNull();
    expect(draftBody).not.toHaveBeenCalled();
  });

  it("puts a drafted description in the box to edit, and proposes nothing by itself", async () => {
    readAssist.mockResolvedValue({ tool: "Claude" });
    readSurface.mockResolvedValue(
      surface({ templates: [{ name: "house", body: "## What changed\n" }], suggestion: null }),
    );
    draftBody.mockResolvedValue("## What changed\n\nIt changed the thing.\n");

    renderPane();
    await editDetails();
    fireEvent.click(screen.getByRole("button", { name: "Draft a description" }));

    await waitFor(() =>
      expect((screen.getByLabelText("Description") as HTMLTextAreaElement).value).toBe(
        "## What changed\n\nIt changed the thing.\n",
      ),
    );
    expect(draftBody).toHaveBeenCalledWith(PROJECT, BASE, "## What changed\n");
    expect(create).not.toHaveBeenCalled();
  });
});
