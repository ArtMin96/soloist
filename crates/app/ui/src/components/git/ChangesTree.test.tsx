// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { ChangesTree } from "@/components/git/ChangesTree";
import { TooltipProvider } from "@/components/ui/tooltip";
import { buildChangesTree } from "@/store/git/tree";
import { useTreeExpansion } from "@/store/git/useTreeExpansion";
import type { ChangeKind, FileChange } from "@/domain";

function change(path: string, unstaged: ChangeKind): FileChange {
  return { path, status: { staged: null, unstaged }, original_path: null };
}

/** The rows a screen reader would walk, in the order they are presented. */
function rows(): HTMLElement[] {
  return within(screen.getByRole("tree")).getAllByRole("treeitem");
}

/** The row whose name is `name`. */
function row(name: string): HTMLElement {
  const found = rows().find((item) => within(item).queryByText(name) !== null);
  if (!found) throw new Error(`no row named ${name}`);
  return found;
}

/** A whole key press. The release matters: the tree matches a chord against the keys still
 *  held, so a test that only ever presses would have every later key read as a chord. */
function press(target: HTMLElement, key: string): void {
  fireEvent.keyDown(target, { key });
  fireEvent.keyUp(target, { key });
}

/**
 * The tree as the rail composes it — the same builder and the same expansion owner — so what these
 * exercise is the whole of what a reader sees, including a changed-files list opening itself.
 */
function Changed({ changes }: { changes: FileChange[] }) {
  const tree = buildChangesTree(changes);
  const folders = useTreeExpansion(tree, true);
  return (
    <ChangesTree
      tree={tree}
      changes={changes}
      actions={null}
      expanded={folders.expanded}
      onExpandedChange={folders.setExpanded}
    />
  );
}

afterEach(cleanup);

describe("ChangesTree", () => {
  it("presents the changed paths as a tree a screen reader can walk", () => {
    render(<Changed changes={[change("src/main.rs", "modified")]} />);

    const tree = screen.getByRole("tree", { name: "Changed files" });
    const [folder, file] = within(tree).getAllByRole("treeitem");
    expect(folder.getAttribute("aria-level")).toBe("1");
    expect(folder.getAttribute("aria-expanded")).toBe("true");
    expect(file.getAttribute("aria-level")).toBe("2");
    expect(file.getAttribute("aria-expanded")).toBeNull();
  });

  it("names each row's change in words, so the letter is never the only thing carrying it", () => {
    render(
      <Changed
        changes={[
          change("a.rs", "modified"),
          change("b.rs", "deleted"),
          change("c.rs", "conflicted"),
        ]}
      />,
    );

    expect(screen.getByRole("img", { name: "Modified" }).textContent).toBe("M");
    expect(screen.getByRole("img", { name: "Deleted" }).textContent).toBe("D");
    expect(screen.getByRole("img", { name: "Conflicted" }).textContent).toBe("C");
  });

  it("shows a folder the strongest change beneath it, so a closed folder still reports", () => {
    render(
      <Changed changes={[change("src/a.rs", "untracked"), change("src/b.rs", "conflicted")]} />,
    );
    const folder = row("src");
    expect(within(folder).getByRole("img").getAttribute("aria-label")).toBe("Conflicted");

    press(screen.getByRole("tree"), "ArrowLeft");

    expect(rows()).toHaveLength(1);
    expect(within(rows()[0]).getByRole("img").getAttribute("aria-label")).toBe("Conflicted");
  });

  it("walks, closes and opens by keyboard alone", async () => {
    render(<Changed changes={[change("src/a.rs", "modified"), change("zeta.rs", "modified")]} />);
    const tree = screen.getByRole("tree");

    press(tree, "ArrowLeft");
    expect(row("src").getAttribute("aria-expanded")).toBe("false");
    expect(rows()).toHaveLength(2);

    press(tree, "ArrowRight");
    expect(row("src").getAttribute("aria-expanded")).toBe("true");
    expect(rows()).toHaveLength(3);

    // The tree moves the browser's own focus, which it does off the event loop.
    press(tree, "ArrowDown");
    await waitFor(() => expect(document.activeElement).toBe(row("a.rs")));
  });

  it("draws a deleted file as gone, and the folder holding it as still there", () => {
    render(<Changed changes={[change("src/gone.rs", "deleted")]} />);

    expect(within(row("gone.rs")).getByText("gone.rs").className).toContain("line-through");
    expect(within(row("src")).getByText("src").className).not.toContain("line-through");
  });

  it("offers discard only for paths the core says can be restored from the index", () => {
    const changes = [change("tracked.rs", "modified"), change("new.rs", "untracked")];

    render(
      <TooltipProvider>
        <ChangesTree
          tree={buildChangesTree(changes)}
          changes={changes}
          actions={{
            onStage: vi.fn(),
            onDiscard: vi.fn(),
            busy: () => false,
            discardable: new Set(["tracked.rs"]),
          }}
          expanded={[]}
          onExpandedChange={vi.fn()}
        />
      </TooltipProvider>,
    );

    expect(screen.getByRole("button", { name: "Discard the changes to tracked.rs" })).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Discard the changes to new.rs" })).toBeNull();
  });
});
