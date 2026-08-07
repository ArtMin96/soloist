// @vitest-environment jsdom
import { afterEach, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";

import { BranchMenu, type BranchActions } from "@/components/git/BranchMenu";
import { TooltipProvider } from "@/components/ui/tooltip";
import type { Branches } from "@/domain";

afterEach(cleanup);

function actions(): BranchActions {
  return {
    switchTo: vi.fn(),
    create: vi.fn(() => Promise.resolve(true)),
    remove: vi.fn(),
    stash: vi.fn(),
    popStash: vi.fn(),
  };
}

function open(branches: Branches | null, acts: BranchActions, onDelete = vi.fn()) {
  render(
    <TooltipProvider>
      <BranchMenu branches={branches} actions={acts} busy={false} onDelete={onDelete} />
    </TooltipProvider>,
  );
}

const TWO: Branches = {
  entries: [
    { name: "main", upstream: "origin/main", head: true },
    { name: "feature", upstream: null, head: false },
  ],
  stashed: false,
};

it("marks the branch that is checked out and offers no way to delete it", () => {
  open(TWO, actions());

  expect(screen.getByText("Checked out")).toBeTruthy();
  expect(screen.queryByRole("button", { name: "Delete branch main" })).toBeNull();
  expect(screen.getByRole("button", { name: "Delete branch feature" })).toBeTruthy();
});

it("does nothing when the branch already checked out is chosen again", () => {
  const acts = actions();
  open(TWO, acts);

  fireEvent.click(screen.getByText("main"));

  expect(acts.switchTo).not.toHaveBeenCalled();
});

it("makes a branch out of a name nothing matches, rather than reporting no results", () => {
  const acts = actions();
  open(TWO, acts);

  fireEvent.change(screen.getByPlaceholderText("Switch or create a branch"), {
    target: { value: "spike/one" },
  });
  fireEvent.click(screen.getByText("spike/one"));

  expect(acts.create).toHaveBeenCalledWith("spike/one");
  expect(
    acts.switchTo,
    "creating is not switching to something that already exists",
  ).not.toHaveBeenCalled();
});

it("offers to switch to a branch whose name was typed in full rather than to create it again", () => {
  const acts = actions();
  open(TWO, acts);

  fireEvent.change(screen.getByPlaceholderText("Switch or create a branch"), {
    target: { value: "feature" },
  });
  fireEvent.click(screen.getByText("feature"));

  expect(acts.switchTo).toHaveBeenCalledWith("feature");
  expect(acts.create).not.toHaveBeenCalled();
});

it("offers to take stashed changes back only when something is set aside", () => {
  open(TWO, actions());
  expect(
    screen.queryByText("Restore stashed changes"),
    "taking back what was never stashed is not an action anybody can take",
  ).toBeNull();
  cleanup();

  open({ ...TWO, stashed: true }, actions());
  expect(screen.getByText("Restore stashed changes")).toBeTruthy();
});

it("offers to set the working tree's changes aside whatever the branches say", () => {
  const acts = actions();
  open(null, acts);

  fireEvent.click(screen.getByText("Stash changes"));

  expect(acts.stash).toHaveBeenCalled();
});
