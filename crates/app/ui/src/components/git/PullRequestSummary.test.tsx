// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { PullRequestSummary } from "@/components/git/PullRequestSummary";
import type { PullRequest } from "@/domain";

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

const URL = "https://github.example/owner/repo/pull/12";

function pullRequest(over: Partial<PullRequest> = {}): PullRequest {
  return {
    number: 12,
    url: URL,
    title: "Live changes rail",
    state: "open",
    draft: false,
    base: "main",
    head: "feature",
    ...over,
  };
}

function renderSummary(over: Partial<PullRequest> = {}) {
  const onOpen = vi.fn();
  render(<PullRequestSummary pullRequest={pullRequest(over)} onOpen={onOpen} />);
  return { onOpen };
}

describe("PullRequestSummary", () => {
  it("names the pull request, its number, and the two branches it joins", () => {
    renderSummary();

    expect(screen.getByText("#12")).toBeTruthy();
    expect(screen.getByText("Live changes rail")).toBeTruthy();
    expect(screen.getByText("feature → main")).toBeTruthy();
  });

  it("hands the address up rather than opening anything itself", () => {
    const { onOpen } = renderSummary();

    fireEvent.click(screen.getByRole("button", { name: /open on the forge/i }));

    expect(onOpen).toHaveBeenCalledWith(URL);
  });

  it("says where it stands in words rather than by colour alone", () => {
    renderSummary({ state: "merged" });

    expect(screen.getByText("Merged")).toBeTruthy();
  });

  it("marks a draft as well as its state, because those are two different facts", () => {
    renderSummary({ draft: true });

    expect(screen.getByText("Draft")).toBeTruthy();
    expect(screen.getByText("Open")).toBeTruthy();
  });

  it("never reads a wire value out loud", () => {
    renderSummary({ state: "closed" });

    expect(screen.getByText("Closed")).toBeTruthy();
    expect(screen.queryByText("closed")).toBeNull();
  });
});
