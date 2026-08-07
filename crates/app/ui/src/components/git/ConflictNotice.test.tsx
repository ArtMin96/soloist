// @vitest-environment jsdom
import { afterEach, expect, it, vi } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";

import { ConflictNotice } from "@/components/git/ConflictNotice";
import type { ChangeKind, FileChange } from "@/domain";

afterEach(cleanup);

function change(path: string, unstaged: ChangeKind): FileChange {
  return { path, status: { staged: null, unstaged }, original_path: null };
}

it("says nothing at all about a working tree with nothing unresolved", () => {
  const { container } = render(
    <ConflictNotice
      changes={[change("src/a.rs", "modified")]}
      merging={false}
      busy={false}
      onAbandon={vi.fn()}
    />,
  );

  expect(container.textContent).toBe("");
});

it("counts what a merge left to resolve", () => {
  render(
    <ConflictNotice
      changes={[
        change("src/a.rs", "conflicted"),
        change("src/b.rs", "conflicted"),
        change("src/c.rs", "modified"),
      ]}
      merging
      busy={false}
      onAbandon={vi.fn()}
    />,
  );

  expect(screen.getByText("2 files need resolving")).toBeTruthy();
});

it("offers no way to abandon a merge where there is no merge to abandon", () => {
  // Putting stashed changes back conflicts the same way and leaves no merge behind it, so the
  // conflict is worth saying and abandoning is not on offer.
  render(
    <ConflictNotice
      changes={[change("src/a.rs", "conflicted")]}
      merging={false}
      busy={false}
      onAbandon={vi.fn()}
    />,
  );

  expect(screen.getByText("1 file needs resolving")).toBeTruthy();
  expect(screen.queryByRole("button", { name: "Abandon merge" })).toBeNull();
});
