// @vitest-environment jsdom
import { afterEach, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";

import { SyncActions } from "@/components/git/SyncActions";
import { TooltipProvider } from "@/components/ui/tooltip";
import type { BranchInfo } from "@/domain";

afterEach(cleanup);

const TRACKING: BranchInfo = {
  name: "main",
  upstream: "origin/main",
  sync: { state: "up_to_date" },
};

const LOCAL_ONLY: BranchInfo = { name: "spike", upstream: null, sync: { state: "unknown" } };

function show(branch: BranchInfo, exchanging: boolean) {
  const handlers = {
    onFetch: vi.fn(),
    onPull: vi.fn(),
    onPush: vi.fn(),
    onStop: vi.fn(),
  };
  render(
    <TooltipProvider>
      <SyncActions branch={branch} exchanging={exchanging} {...handlers} />
    </TooltipProvider>,
  );
  return handlers;
}

it("offers to hand a tracking branch's commits to the upstream it has", () => {
  const handlers = show(TRACKING, false);

  fireEvent.click(screen.getByRole("button", { name: "Push" }));

  expect(handlers.onPush).toHaveBeenCalled();
  expect(screen.queryByRole("button", { name: "Publish" })).toBeNull();
});

it("offers to publish a branch that tracks nothing, and nothing to pull from", () => {
  const handlers = show(LOCAL_ONLY, false);

  fireEvent.click(screen.getByRole("button", { name: "Publish" }));

  expect(
    handlers.onPush,
    "publishing and pushing are one intent, and the core picks which",
  ).toHaveBeenCalled();
  expect(screen.queryByRole("button", { name: "Pull" })).toBeNull();
  expect(screen.queryByRole("button", { name: "Push" })).toBeNull();
});

it("replaces the three actions with the one that ends them while an exchange is under way", () => {
  const handlers = show(TRACKING, true);

  expect(screen.queryByRole("button", { name: "Fetch" })).toBeNull();
  expect(screen.queryByRole("button", { name: "Push" })).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "Stop" }));

  expect(
    handlers.onStop,
    "a bounded wait with no way out reads as a frozen window",
  ).toHaveBeenCalled();
});
